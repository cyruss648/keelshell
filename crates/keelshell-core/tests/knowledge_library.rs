use std::fs;

use keelshell_core::{
    AppState, Connection, Error, KnowledgeCaptureType, KnowledgeError, KnowledgeOutcome,
    KnowledgeSelection, KnowledgeService, KnowledgeSourceKind, MAX_KNOWLEDGE_ENTRIES,
    MAX_KNOWLEDGE_TEXT_BYTES, SavedKnowledgeEntry, SelectedFact, SourceBinding, StateStore,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn sensitive_titles_with_alternative_whitespace_never_reach_state_store() -> TestResult {
    let directory = tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let original = completed_entry()?;
    let mut state = store.load()?;
    state.record_knowledge_entry(original.clone())?;
    store.save(&state)?;
    let before = fs::read(store.path())?;
    for title in [
        "api   key=fixture-value",
        "API\u{00a0}KEY=fixture-value",
        "api\u{2003}key=fixture-value",
    ] {
        let error = original
            .with_reviewed_content(title, "Reviewed result")
            .err();
        assert_eq!(error, Some(KnowledgeError::SensitiveGeneratedContent));
        assert!(!format!("{error:?}").contains("fixture-value"));
        let mut wire = serde_json::to_value(&original)?;
        wire["title"] = Value::String(title.to_owned());
        assert!(serde_json::from_value::<SavedKnowledgeEntry>(wire).is_err());
        assert_eq!(fs::read(store.path())?, before);
    }
    Ok(())
}

fn saved_entry(
    capture_type: KnowledgeCaptureType,
    outcome: KnowledgeOutcome,
) -> Result<SavedKnowledgeEntry, KnowledgeError> {
    let source = SourceBinding::new(
        KnowledgeSourceKind::Session,
        Uuid::new_v4(),
        7,
        "fixture.example.test · terminal-a",
    )?;
    let fact = match outcome {
        KnowledgeOutcome::Completed => SelectedFact::completed(
            source,
            42,
            "Fixture health check",
            "Fixture service is ready",
        )?,
        KnowledgeOutcome::Failed => SelectedFact::failed(
            source,
            42,
            "Fixture health check",
            "Fixture service refused the request",
        )?,
    };
    let document = KnowledgeService::build(&KnowledgeSelection::new(vec![fact])?)?;
    SavedKnowledgeEntry::from_document(Uuid::new_v4(), &document, "fixture-session-a", capture_type)
}

fn completed_entry() -> Result<SavedKnowledgeEntry, KnowledgeError> {
    saved_entry(KnowledgeCaptureType::Selection, KnowledgeOutcome::Completed)
}

fn state_with_entries(entries: Vec<SavedKnowledgeEntry>) -> AppState {
    AppState {
        knowledge_entries: entries,
        connections: vec![Connection::new(
            "Fixture host",
            "node.example.test",
            "fixture-user",
        )],
        ..AppState::default()
    }
}

fn changed_metadata() -> [(&'static str, Value); 8] {
    [
        ("/id", json!(Uuid::new_v4())),
        ("/source/id", json!(Uuid::new_v4())),
        ("/source/revision", json!(8)),
        ("/source/label", json!("another fixture")),
        ("/session_id", json!("fixture-session-b")),
        ("/capture_type", json!("visible_screen")),
        ("/occurred_at", json!(43)),
        ("/outcome", json!("failed")),
    ]
}

fn changed_wire(
    entry: &SavedKnowledgeEntry,
    pointer: &str,
    value: Value,
) -> Result<SavedKnowledgeEntry, Box<dyn std::error::Error>> {
    let mut wire = serde_json::to_value(entry)?;
    *wire.pointer_mut(pointer).ok_or("missing fixture field")? = value;
    Ok(serde_json::from_value(wire)?)
}

fn assert_unchanged(actual: &AppState, before: &AppState) {
    assert_eq!(actual, before);
    // AppState's PartialEq deliberately excludes the persistence revision.
    assert_eq!(actual.snapshot, before.snapshot);
}

fn assert_validation(result: Result<(), Error>, field: &str) {
    assert!(
        matches!(result, Err(Error::Validation(ref error)) if error.field == field),
        "unexpected result: {result:?}"
    );
}

#[test]
fn reviewed_content_normalizes_text_and_preserves_every_other_field() -> TestResult {
    for capture in [
        KnowledgeCaptureType::Selection,
        KnowledgeCaptureType::VisibleScreen,
    ] {
        for outcome in [KnowledgeOutcome::Completed, KnowledgeOutcome::Failed] {
            let original = saved_entry(capture, outcome)?;
            let before = serde_json::to_value(&original)?;
            let edited = original.with_reviewed_content(
                "  已审核的检查  ",
                "  Fixture\u{2003}service   reviewed successfully  ",
            )?;
            let mut expected = before.clone();
            expected["title"] = json!("已审核的检查");
            expected["summary"] = json!("Fixture service reviewed successfully");
            assert_eq!(serde_json::to_value(&edited)?, expected);
            assert_eq!(serde_json::to_value(&original)?, before);
            assert_eq!(
                serde_json::from_value::<SavedKnowledgeEntry>(expected)?,
                edited
            );
        }
    }
    Ok(())
}

#[test]
fn reviewed_content_preserves_loaded_session_metadata_byte_for_byte() -> TestResult {
    let original = changed_wire(
        &completed_entry()?,
        "/session_id",
        json!("  fixture-session-a  "),
    )?;
    let edited = original.with_reviewed_content("Reviewed title", "Reviewed fixture result")?;
    assert_eq!(edited.session_id(), "  fixture-session-a  ");
    assert_eq!(edited.id(), original.id());
    Ok(())
}

#[test]
fn reviewed_content_rejects_empty_and_control_data_in_either_field() -> TestResult {
    let original = completed_entry()?;
    for invalid in [
        "",
        " \u{2003} ",
        "before\0after",
        "before\nafter",
        "before\rafter",
        "before\tafter",
        "before\u{7f}after",
    ] {
        assert_eq!(
            original.with_reviewed_content(invalid, "Valid fixture summary"),
            Err(KnowledgeError::EmptyText)
        );
        assert_eq!(
            original.with_reviewed_content("Valid fixture title", invalid),
            Err(KnowledgeError::EmptyText)
        );
    }
    Ok(())
}

#[test]
fn reviewed_content_limits_raw_utf8_bytes_before_normalization() -> TestResult {
    let original = completed_entry()?;
    let at_limit =
        "测".repeat(MAX_KNOWLEDGE_TEXT_BYTES / 3) + &"x".repeat(MAX_KNOWLEDGE_TEXT_BYTES % 3);
    let edited = original.with_reviewed_content(&at_limit, &at_limit)?;
    assert_eq!(edited.entry().title().len(), MAX_KNOWLEDGE_TEXT_BYTES);
    assert_eq!(edited.entry().summary().len(), MAX_KNOWLEDGE_TEXT_BYTES);
    for oversized in [
        format!("{at_limit}界"),
        format!("x{}", " ".repeat(MAX_KNOWLEDGE_TEXT_BYTES)),
    ] {
        assert_eq!(
            original.with_reviewed_content(&oversized, "Fixture result"),
            Err(KnowledgeError::TextTooLarge)
        );
        assert_eq!(
            original.with_reviewed_content("Fixture title", &oversized),
            Err(KnowledgeError::TextTooLarge)
        );
    }
    Ok(())
}

#[test]
fn reviewed_content_rejects_synthetic_sensitive_markers_without_echoing_input() -> TestResult {
    let original = completed_entry()?;
    for input in [
        "Authorization: fixture-placeholder",
        "BEARER fixture-placeholder",
        "API_KEY=fixture-placeholder",
        "Password=fixture-placeholder",
        "private key fixture-placeholder",
        "token=fixture-placeholder",
    ] {
        for result in [
            original.with_reviewed_content(input, "Fixture result"),
            original.with_reviewed_content("Fixture title", input),
        ] {
            assert_eq!(result, Err(KnowledgeError::SensitiveGeneratedContent));
            assert!(!format!("{result:?}").contains("fixture-placeholder"));
        }
    }
    Ok(())
}

#[test]
fn reviewed_content_rechecks_sensitive_markers_after_summary_normalization() -> TestResult {
    let original = completed_entry()?;
    for input in [
        "api   key=fixture-placeholder",
        "API\u{a0}KEY=fixture-placeholder",
        "private\u{2003}key fixture-placeholder",
        "bearer\u{2003}fixture-placeholder",
    ] {
        assert_eq!(
            original.with_reviewed_content("Fixture title", input),
            Err(KnowledgeError::SensitiveGeneratedContent)
        );
    }
    Ok(())
}

#[test]
fn reviewed_edits_preserve_order_unrelated_state_and_revision_without_io() -> TestResult {
    let directory = tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let first = completed_entry()?;
    let middle = completed_entry()?;
    let last = completed_entry()?;
    let mut state = store.load()?;
    state.connections = state_with_entries(Vec::new()).connections;
    state.knowledge_entries = vec![first.clone(), middle.clone(), last.clone()];
    let mut expected = state.clone();
    let replacement = middle.with_reviewed_content("Reviewed fixture", "Edited fixture result")?;

    state.replace_knowledge_entry(&middle, replacement.clone())?;
    expected.knowledge_entries[1] = replacement.clone();
    assert_unchanged(&state, &expected);

    state.remove_knowledge_entry(&replacement)?;
    expected.knowledge_entries = vec![first, last];
    assert_unchanged(&state, &expected);
    assert!(!store.path().exists());
    Ok(())
}

#[test]
fn replacement_with_identical_reviewed_content_is_a_no_op() -> TestResult {
    let original = completed_entry()?;
    let mut state = state_with_entries(vec![original.clone()]);
    let before = state.clone();
    state.replace_knowledge_entry(&original, original.clone())?;
    assert_unchanged(&state, &before);
    Ok(())
}

#[test]
fn replacement_rejects_each_changed_immutable_metadata_field_atomically() -> TestResult {
    let original = completed_entry()?;
    let mut state = state_with_entries(vec![original.clone()]);
    let before = state.clone();
    for (pointer, value) in changed_metadata() {
        let replacement = changed_wire(&original, pointer, value)?;
        assert_validation(
            state.replace_knowledge_entry(&original, replacement),
            "knowledge_entries",
        );
        assert_unchanged(&state, &before);
    }
    Ok(())
}

#[test]
fn both_operations_require_the_complete_expected_entry() -> TestResult {
    let original = completed_entry()?;
    let mut state = state_with_entries(vec![original.clone()]);
    let before = state.clone();
    let mut changes = changed_metadata().to_vec();
    changes.extend([
        ("/title", json!("Unreviewed fixture title")),
        ("/summary", json!("Unreviewed fixture summary")),
    ]);
    for (pointer, value) in changes {
        let stale = changed_wire(&original, pointer, value)?;
        assert!(matches!(
            state.replace_knowledge_entry(&stale, original.clone()),
            Err(Error::Conflict)
        ));
        assert_unchanged(&state, &before);
        assert!(matches!(
            state.remove_knowledge_entry(&stale),
            Err(Error::Conflict)
        ));
        assert_unchanged(&state, &before);
    }
    Ok(())
}

#[test]
fn editing_invalidates_old_review_for_replacement_and_deletion() -> TestResult {
    let original = completed_entry()?;
    let replacement = original.with_reviewed_content("New title", "New fixture summary")?;
    let mut state = state_with_entries(vec![original.clone()]);
    state.replace_knowledge_entry(&original, replacement.clone())?;
    let before = state.clone();
    // Even submitting the current content again cannot refresh an old review.
    assert!(matches!(
        state.replace_knowledge_entry(&original, replacement),
        Err(Error::Conflict)
    ));
    assert_unchanged(&state, &before);
    assert!(matches!(
        state.remove_knowledge_entry(&original),
        Err(Error::Conflict)
    ));
    assert_unchanged(&state, &before);
    Ok(())
}

#[test]
fn missing_and_already_removed_entries_cannot_be_removed_or_reinserted() -> TestResult {
    let original = completed_entry()?;
    let neighbor = completed_entry()?;
    let absent = completed_entry()?;
    let mut state = state_with_entries(vec![original.clone(), neighbor]);
    for expected in [&absent, &original] {
        if expected == &original {
            state.remove_knowledge_entry(expected)?;
        }
        let before = state.clone();
        assert!(matches!(
            state.remove_knowledge_entry(expected),
            Err(Error::Conflict)
        ));
        assert_unchanged(&state, &before);
        assert!(matches!(
            state.replace_knowledge_entry(expected, expected.clone()),
            Err(Error::Conflict)
        ));
        assert_unchanged(&state, &before);
    }
    Ok(())
}

#[test]
fn ambiguous_ids_are_rejected_even_if_only_one_entry_matches_the_review() -> TestResult {
    let original = completed_entry()?;
    let changed = original.with_reviewed_content("Other title", "Other fixture result")?;
    for duplicate in [&original, &changed] {
        for entries in [
            vec![original.clone(), duplicate.clone()],
            vec![duplicate.clone(), original.clone()],
        ] {
            let mut state = state_with_entries(entries);
            let before = state.clone();
            assert_validation(
                state.replace_knowledge_entry(&original, changed.clone()),
                "knowledge_entries.id",
            );
            assert_unchanged(&state, &before);
            assert_validation(
                state.remove_knowledge_entry(&original),
                "knowledge_entries.id",
            );
            assert_unchanged(&state, &before);
        }
    }
    Ok(())
}

#[test]
fn invalid_candidate_state_preserves_data_and_loaded_revision_on_failure() -> TestResult {
    let directory = tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let original = completed_entry()?;
    let replacement =
        original.with_reviewed_content("Reviewed title", "Reviewed fixture result")?;
    let mut state = store.load()?;
    state.knowledge_entries.push(original.clone());
    state.snippets[0].name.clear();
    let before = state.clone();

    assert!(matches!(
        state.replace_knowledge_entry(&original, replacement),
        Err(Error::Validation(_))
    ));
    assert_unchanged(&state, &before);
    assert!(matches!(
        state.remove_knowledge_entry(&original),
        Err(Error::Validation(_))
    ));
    assert_unchanged(&state, &before);
    assert!(!store.path().exists());
    Ok(())
}

#[test]
fn a_full_library_allows_replacement_and_removal_frees_one_slot() -> TestResult {
    let entries = (0..MAX_KNOWLEDGE_ENTRIES)
        .map(|_| completed_entry())
        .collect::<Result<Vec<_>, _>>()?;
    let original = entries[0].clone();
    let mut state = state_with_entries(entries);
    let replacement =
        original.with_reviewed_content("Reviewed title", "Reviewed fixture result")?;
    state.replace_knowledge_entry(&original, replacement.clone())?;
    assert_eq!(state.knowledge_entries.len(), MAX_KNOWLEDGE_ENTRIES);
    assert_eq!(state.knowledge_entries[0], replacement);
    state.remove_knowledge_entry(&replacement)?;
    state.record_knowledge_entry(completed_entry()?)?;
    state.validate()?;
    assert_eq!(state.knowledge_entries.len(), MAX_KNOWLEDGE_ENTRIES);
    Ok(())
}

#[test]
fn edited_and_deleted_knowledge_round_trips_through_real_state_store() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let mut entries = Vec::new();
    for capture in [
        KnowledgeCaptureType::Selection,
        KnowledgeCaptureType::VisibleScreen,
    ] {
        for outcome in [KnowledgeOutcome::Completed, KnowledgeOutcome::Failed] {
            entries.push(saved_entry(capture, outcome)?);
        }
    }
    let initial = store.save(&state_with_entries(entries.clone()))?;
    let reopened = StateStore::new(&path);
    let mut state = reopened.load()?;
    for original in &entries {
        state.replace_knowledge_entry(
            original,
            original.with_reviewed_content("Reviewed fixture", "Reviewed fixture result")?,
        )?;
    }
    let edited = reopened.save(&state)?;
    assert_ne!(edited.snapshot, state.snapshot);
    assert_eq!(StateStore::new(&path).load()?, edited);
    assert_eq!(edited.schema_version, initial.schema_version);

    let entries = edited.knowledge_entries.clone();
    let mut state = reopened.load()?;
    state.remove_knowledge_entry(&entries[1])?;
    let partially_deleted = reopened.save(&state)?;
    assert_eq!(
        StateStore::new(&path).load()?.knowledge_entries,
        vec![entries[0].clone(), entries[2].clone(), entries[3].clone()]
    );
    let mut state = partially_deleted;
    for entry in [&entries[0], &entries[2], &entries[3]] {
        state.remove_knowledge_entry(entry)?;
    }
    let saved = reopened.save(&state)?;
    assert_eq!(StateStore::new(&path).load()?, saved);
    assert!(saved.knowledge_entries.is_empty());
    assert_eq!(saved.connections, initial.connections);
    assert_eq!(saved.snippets, initial.snippets);
    assert!(!fs::read_to_string(&path)?.contains("knowledge_entries"));
    Ok(())
}

#[test]
fn stale_external_edits_and_deletions_cannot_overwrite_a_winning_edit_or_delete() -> TestResult {
    for winner_removes in [false, true] {
        for stale_removes in [false, true] {
            let directory = tempdir()?;
            let path = directory.path().join("state.json");
            let first_store = StateStore::new(&path);
            let second_store = StateStore::new(&path);
            let original = completed_entry()?;
            first_store.save(&state_with_entries(vec![
                original.clone(),
                completed_entry()?,
            ]))?;
            let mut winner = first_store.load()?;
            let mut stale = second_store.load()?;
            if winner_removes {
                winner.remove_knowledge_entry(&original)?;
            } else {
                winner.replace_knowledge_entry(
                    &original,
                    original.with_reviewed_content("Winner title", "Winner fixture result")?,
                )?;
            }
            let winner = first_store.save(&winner)?;
            let disk_before = fs::read(&path)?;
            if stale_removes {
                stale.remove_knowledge_entry(&original)?;
            } else {
                stale.replace_knowledge_entry(
                    &original,
                    original.with_reviewed_content("Stale title", "Stale fixture result")?,
                )?;
            }
            let draft = stale.clone();
            assert!(matches!(second_store.save(&stale), Err(Error::Conflict)));
            assert_unchanged(&stale, &draft);
            assert_eq!(fs::read(&path)?, disk_before);
            assert_eq!(StateStore::new(&path).load()?, winner);

            let mut reloaded = second_store.load()?;
            let before = reloaded.clone();
            assert!(matches!(
                reloaded.replace_knowledge_entry(&original, original.clone()),
                Err(Error::Conflict)
            ));
            assert_unchanged(&reloaded, &before);
            assert!(matches!(
                reloaded.remove_knowledge_entry(&original),
                Err(Error::Conflict)
            ));
            assert_unchanged(&reloaded, &before);
        }
    }
    Ok(())
}

#[test]
fn store_revision_rejects_stale_writer_even_when_reviewed_entry_is_unchanged() -> TestResult {
    for remove in [false, true] {
        let directory = tempdir()?;
        let store = StateStore::new(directory.path().join("state.json"));
        let original = completed_entry()?;
        let initial = store.save(&state_with_entries(vec![original.clone()]))?;
        let mut winner = initial.clone();
        winner.connections[0].name = "Renamed fixture host".into();
        let winner = store.save(&winner)?;
        let disk_before = fs::read(store.path())?;
        let mut stale = initial;
        if remove {
            stale.remove_knowledge_entry(&original)?;
        } else {
            stale.replace_knowledge_entry(
                &original,
                original.with_reviewed_content("Stale title", "Stale fixture result")?,
            )?;
        }
        let draft = stale.clone();
        assert!(matches!(store.save(&stale), Err(Error::Conflict)));
        assert_unchanged(&stale, &draft);
        assert_eq!(fs::read(store.path())?, disk_before);
        assert_eq!(StateStore::new(store.path()).load()?, winner);
    }
    Ok(())
}
