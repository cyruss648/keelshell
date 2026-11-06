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
