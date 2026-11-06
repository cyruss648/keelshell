use crate::{
    AppState, Error, KnowledgeCaptureType, KnowledgeSelection, KnowledgeService,
    KnowledgeSourceKind, SavedKnowledgeEntry, SelectedFact, SourceBinding, StateStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn entry(index: usize) -> Result<SavedKnowledgeEntry, crate::KnowledgeError> {
    let source = SourceBinding::new(
        KnowledgeSourceKind::Session,
        Uuid::new_v4(),
        7,
        &format!("Owned source {index}"),
    )?;
    let fact = SelectedFact::completed(
        source,
        100,
        &format!("Entry {index}"),
        &format!("Original text {index}"),
    )?;
    let document = KnowledgeService::build(&KnowledgeSelection::new(vec![fact])?)?;
    SavedKnowledgeEntry::from_document(
        Uuid::new_v4(),
        &document,
        "owned-historical-session",
        KnowledgeCaptureType::Selection,
    )
}

fn state(count: usize) -> Result<AppState, crate::KnowledgeError> {
    Ok(AppState {
        knowledge_entries: (0..count).map(entry).collect::<Result<_, _>>()?,
        ..AppState::default()
    })
}

fn unchanged(state: &AppState, original: &AppState) {
    assert_eq!(state, original);
    assert_eq!(state.snapshot, original.snapshot);
}

#[test]
fn knowledge_batch_delete_empty_duplicate_and_over_limit_are_atomic() -> TestResult {
    let mut state = state(33)?;
    let original = state.clone();
    for expected in [
        Vec::new(),
        vec![original.knowledge_entries[0].clone(); 2],
        original.knowledge_entries.clone(),
    ] {
        assert!(matches!(
            state.remove_knowledge_entries(&expected),
            Err(Error::Validation(_))
        ));
        unchanged(&state, &original);
    }
    Ok(())
}

#[test]
fn knowledge_batch_delete_missing_last_record_never_removes_valid_predecessor() -> TestResult {
    let mut state = state(3)?;
    let original = state.clone();
    let expected = vec![original.knowledge_entries[0].clone(), entry(99)?];
    assert!(matches!(
        state.remove_knowledge_entries(&expected),
        Err(Error::Conflict)
    ));
    unchanged(&state, &original);
    Ok(())
}

#[test]
fn knowledge_batch_delete_checks_every_original_field_before_mutation() -> TestResult {
    let mut state = state(3)?;
    let original = state.clone();
    for (pointer, value) in [
        ("/id", json!(Uuid::new_v4())),
        ("/title", json!("Different title")),
        ("/summary", json!("Different body")),
        ("/source/id", json!(Uuid::new_v4())),
        ("/source/revision", json!(8)),
        ("/source/label", json!("Different source")),
        ("/session_id", json!("other-session")),
        ("/capture_type", json!("visible_screen")),
        ("/occurred_at", json!(101)),
        ("/outcome", json!("failed")),
    ] {
        let mut wire = serde_json::to_value(&original.knowledge_entries[1])?;
        *wire.pointer_mut(pointer).ok_or("missing fixture field")? = value;
        let changed = serde_json::from_value(wire)?;
        assert!(
            matches!(
                state.remove_knowledge_entries(&[original.knowledge_entries[0].clone(), changed]),
                Err(Error::Conflict)
            ),
            "{pointer}"
        );
        unchanged(&state, &original);
    }
    Ok(())
}

#[test]
fn knowledge_batch_delete_ambiguous_identity_rejects_whole_batch() -> TestResult {
    let mut state = state(3)?;
    state
        .knowledge_entries
        .push(state.knowledge_entries[1].clone());
    let original = state.clone();
    assert!(matches!(
        state.remove_knowledge_entries(&original.knowledge_entries[..2]),
        Err(Error::Validation(_))
    ));
    unchanged(&state, &original);
    Ok(())
}

#[test]
fn knowledge_batch_delete_invalid_remaining_configuration_leaves_everything_unchanged() -> TestResult
{
    let mut state = state(3)?;
    state.settings.scrollback_lines = 0;
    let original = state.clone();
    assert!(matches!(
        state.remove_knowledge_entries(&original.knowledge_entries[..2]),
        Err(Error::Validation(_))
    ));
    unchanged(&state, &original);
    Ok(())
}

#[test]
fn knowledge_batch_delete_32_preserves_survivor_bytes_order_and_loaded_revision() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let original = store.save(&state(35)?)?;
    let before = std::fs::read(store.path())?;
    let mut candidate = original.clone();
    let mut expected = original.knowledge_entries[1..33].to_vec();
    expected.reverse();
    candidate.remove_knowledge_entries(&expected)?;
    assert_eq!(candidate.snapshot, original.snapshot);
    assert_eq!(
        std::fs::read(store.path())?,
        before,
        "candidate performs no I/O"
    );
    let remaining: Vec<_> = [0, 33, 34]
        .into_iter()
        .map(|index| original.knowledge_entries[index].clone())
        .collect();
    assert_eq!(
        serde_json::to_vec(&candidate.knowledge_entries)?,
        serde_json::to_vec(&remaining)?
    );
    let saved = store.save(&candidate)?;
    assert_ne!(saved.snapshot, original.snapshot);
    drop(store);
    let reloaded = StateStore::new(directory.path().join("state.json")).load()?;
    assert_eq!(
        serde_json::to_vec(&reloaded.knowledge_entries)?,
        serde_json::to_vec(&remaining)?
    );
    Ok(())
}

#[test]
fn knowledge_batch_delete_competing_store_conflict_requires_fresh_review() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let first = StateStore::new(path.clone());
    first.save(&state(4)?)?;
    let mut stale = first.load()?;
    let expected = stale.knowledge_entries[..2].to_vec();
    let second = StateStore::new(path);
    let mut winner = second.load()?;
    winner.knowledge_entries[1] =
        winner.knowledge_entries[1].with_reviewed_content("Changed externally", "New facts")?;
    second.save(&winner)?;
    let before = std::fs::read(first.path())?;
    stale.remove_knowledge_entries(&expected)?;
    assert!(matches!(first.save(&stale), Err(Error::Conflict)));
    assert_eq!(std::fs::read(first.path())?, before);
    let mut fresh = first.load()?;
    let loaded = fresh.clone();
    assert!(matches!(
        fresh.remove_knowledge_entries(&expected),
        Err(Error::Conflict)
    ));
    unchanged(&fresh, &loaded);
    let renewed = fresh.knowledge_entries[..2].to_vec();
    fresh.remove_knowledge_entries(&renewed)?;
    first.save(&fresh)?;
    assert_eq!(
        first.load()?.knowledge_entries,
        winner.knowledge_entries[2..]
    );
    Ok(())
}

#[test]
fn knowledge_batch_delete_keeps_sensitive_content_validation_at_deserialization() -> TestResult {
    let original = entry(0)?;
    let mut wire = serde_json::to_value(original)?;
    wire["summary"] = Value::String("password = owned-fixture-only".to_owned());
    assert!(serde_json::from_value::<SavedKnowledgeEntry>(wire).is_err());
    Ok(())
}

#[test]
fn knowledge_category_old_state_defaults_to_uncategorized_and_content_edits_preserve_category()
-> TestResult {
    let original = entry(0)?;
    let mut legacy = serde_json::to_value(&original)?;
    legacy
        .as_object_mut()
        .ok_or("record object")?
        .remove("category");
    let loaded: SavedKnowledgeEntry = serde_json::from_value(legacy)?;
    assert_eq!(loaded, original);
    assert_eq!(loaded.category(), None);
    let categorized = loaded.with_reviewed_category(Some("  运维 Ops  "))?;
    assert_eq!(categorized.category(), Some("运维 Ops"));
    let edited = categorized.with_reviewed_content("Reviewed title", "Reviewed text")?;
    assert_eq!(edited.category(), categorized.category());
    let mut expected = serde_json::to_value(&original)?;
    expected["category"] = json!("运维 Ops");
    assert_eq!(serde_json::to_value(&categorized)?, expected);
    assert_eq!(
        serde_json::from_value::<SavedKnowledgeEntry>(expected)?,
        categorized
    );
    assert_eq!(categorized.with_reviewed_category(None)?, original);
    assert_eq!(
        categorized.with_reviewed_category(Some(" \u{3000} "))?,
        original
    );
    Ok(())
}

#[test]
fn knowledge_category_bounds_use_raw_utf8_bytes_before_trimming() -> TestResult {
    let original = entry(0)?;
    let full = format!("{}ab", "中".repeat(42));
    assert_eq!(full.len(), SavedKnowledgeEntry::MAX_CATEGORY_BYTES);
    assert_eq!(
        original.with_reviewed_category(Some(&full))?.category(),
        Some(full.as_str())
    );
    for value in ["中".repeat(43), " ".repeat(129), format!(" {full}")] {
        assert_eq!(
            original.with_reviewed_category(Some(&value)),
            Err(crate::KnowledgeError::CategoryTooLarge),
        );
        let mut wire = serde_json::to_value(&original)?;
        wire["category"] = json!(value);
        assert!(serde_json::from_value::<SavedKnowledgeEntry>(wire).is_err());
    }
    Ok(())
}

#[test]
fn knowledge_category_controls_sensitive_markers_and_diagnostics_never_echo_labels() -> TestResult {
    let original = entry(0)?;
    for control in ['\0', '\n', '\r', '\t', '\u{7f}', '\u{85}'] {
        let value = format!("Personal fixture{control}label");
        let error = original
            .with_reviewed_category(Some(&value))
            .err()
            .ok_or("category rejected")?;
        assert_eq!(error, crate::KnowledgeError::InvalidCategory);
        assert!(!error.to_string().contains("Personal fixture"));
        let mut wire = serde_json::to_value(&original)?;
        wire["category"] = json!(value);
        let error = serde_json::from_value::<SavedKnowledgeEntry>(wire)
            .err()
            .ok_or("wire rejected")?;
        assert!(!error.to_string().contains("Personal fixture"));
    }
    assert_eq!(
        original.with_reviewed_category(Some("password = synthetic-private-data")),
        Err(crate::KnowledgeError::SensitiveGeneratedContent),
    );
    let categorized = original.with_reviewed_category(Some("Personal fixture label"))?;
    assert!(!format!("{categorized:?}").contains("Personal fixture label"));
    Ok(())
}

#[test]
fn knowledge_category_set_change_clear_persist_once_preserving_facts_and_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut current = store.save(&state(3)?)?;
    let original = current.clone();
    for category in [Some("Ops"), Some("ops"), None] {
        let before = std::fs::read(store.path())?;
        let backups = store.config_backups()?.len();
        let expected = current.knowledge_entries[1].clone();
        let replacement = expected.with_reviewed_category(category)?;
        current.replace_knowledge_entry(&expected, replacement.clone())?;
        assert_eq!(std::fs::read(store.path())?, before);
        current = store.save(&current)?;
        assert_eq!(store.config_backups()?.len(), backups + 1);
        assert_eq!(store.load()?, current);
        assert_eq!(current.knowledge_entries[0], original.knowledge_entries[0]);
        assert_eq!(current.knowledge_entries[2], original.knowledge_entries[2]);
        assert_eq!(current.knowledge_entries[1].category(), category);
        assert_eq!(
            current.knowledge_entries[1].with_reviewed_category(None)?,
            original.knowledge_entries[1],
        );
    }
    Ok(())
}

#[test]
fn knowledge_category_change_revokes_old_edit_single_delete_and_batch_delete_atomically()
-> TestResult {
    let mut current = state(3)?;
    let old = current.knowledge_entries[1].clone();
    current.replace_knowledge_entry(&old, old.with_reviewed_category(Some("Ops"))?)?;
    let original = current.clone();
    assert!(matches!(
        current.replace_knowledge_entry(&old, old.with_reviewed_content("New", "Text")?),
        Err(Error::Conflict),
    ));
    unchanged(&current, &original);
    assert!(matches!(
        current.remove_knowledge_entry(&old),
        Err(Error::Conflict)
    ));
    unchanged(&current, &original);
    assert!(matches!(
        current.remove_knowledge_entries(&[current.knowledge_entries[0].clone(), old]),
        Err(Error::Conflict),
    ));
    unchanged(&current, &original);
    Ok(())
}
