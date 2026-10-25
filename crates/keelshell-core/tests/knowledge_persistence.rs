use std::fs;

use keelshell_core::{
    AppState, KnowledgeCaptureType, KnowledgeSelection, KnowledgeService, KnowledgeSourceKind,
    SavedKnowledgeEntry, SelectedFact, SourceBinding, StateStore,
};
use tempfile::tempdir;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn require<T, E: std::fmt::Display>(result: Result<T, E>, operation: &str) -> T {
    result.unwrap_or_else(|error| panic!("{operation}: {error}"))
}

fn saved_entry(
    summary: &str,
    capture_type: KnowledgeCaptureType,
    failed: bool,
) -> SavedKnowledgeEntry {
    let source = require(
        SourceBinding::new(
            KnowledgeSourceKind::Session,
            Uuid::new_v4(),
            1,
            "fixture host · session-a",
        ),
        "source",
    );
    let fact = if failed {
        require(
            SelectedFact::failed(source, 42, "Health check", summary),
            "fact",
        )
    } else {
        require(
            SelectedFact::completed(source, 42, "Health check", summary),
            "fact",
        )
    };
    let selection = require(KnowledgeSelection::new(vec![fact]), "selection");
    let document = require(KnowledgeService::build(&selection), "document");
    require(
        SavedKnowledgeEntry::from_document(Uuid::new_v4(), &document, "session-a", capture_type),
        "saved entry",
    )
}

#[test]
fn completed_and_failed_knowledge_entries_round_trip_through_state_store() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    state.record_knowledge_entry(saved_entry(
        "Service reports ready",
        KnowledgeCaptureType::Selection,
        false,
    ))?;
    state.record_knowledge_entry(saved_entry(
        "Service refused the request",
        KnowledgeCaptureType::VisibleScreen,
        true,
    ))?;

    let saved = store.save(&state)?;
    let loaded = StateStore::new(&path).load()?;

    assert_eq!(loaded.knowledge_entries, saved.knowledge_entries);
    assert_eq!(loaded.knowledge_entries.len(), 2);
    assert_eq!(
        loaded.knowledge_entries[0].entry().outcome(),
        keelshell_core::KnowledgeOutcome::Completed
    );
    assert_eq!(
        loaded.knowledge_entries[1].entry().outcome(),
        keelshell_core::KnowledgeOutcome::Failed
    );
    assert!(fs::read_to_string(path)?.contains("knowledge_entries"));
    Ok(())
}

#[test]
fn stale_state_revision_rejects_a_knowledge_save_without_replacing_the_file() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let first_store = StateStore::new(&path);
    let mut first = first_store.load()?;
    let second_store = StateStore::new(&path);
    let mut second = second_store.load()?;

    first.record_knowledge_entry(saved_entry(
        "First reviewed result",
        KnowledgeCaptureType::Selection,
        false,
    ))?;
    first_store.save(&first)?;

    second.record_knowledge_entry(saved_entry(
        "Second reviewed result",
        KnowledgeCaptureType::VisibleScreen,
        true,
    ))?;
    assert!(matches!(
        second_store.save(&second),
        Err(keelshell_core::Error::Conflict)
    ));
    let loaded = StateStore::new(&path).load()?;
    assert_eq!(loaded.knowledge_entries.len(), 1);
    assert_eq!(
        loaded.knowledge_entries[0].entry().summary(),
        "First reviewed result"
    );
    Ok(())
}

#[test]
fn state_load_rejects_sensitive_saved_knowledge_wire() -> TestResult {
    let directory = tempdir()?;
    let path = directory.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    state.record_knowledge_entry(saved_entry(
        "Safe result",
        KnowledgeCaptureType::Selection,
        false,
    ))?;
    let saved = store.save(&state)?;
    let mut wire = serde_json::to_value(&saved)?;
    wire["knowledge_entries"][0]["summary"] = serde_json::Value::String("api key = leaked".into());
    fs::write(&path, serde_json::to_vec_pretty(&wire)?)?;

    assert!(matches!(
        StateStore::new(&path).load(),
        Err(keelshell_core::Error::Json(_))
    ));
    Ok(())
}

#[test]
fn identical_knowledge_receipt_is_idempotent_in_app_state() {
    let entry = saved_entry("same review", KnowledgeCaptureType::Selection, false);
    let mut state = AppState::default();
    require(state.record_knowledge_entry(entry.clone()), "first receipt");
    require(
        state.record_knowledge_entry(entry),
        "identical receipt is a no-op",
    );
    assert_eq!(state.knowledge_entries.len(), 1);
}

#[test]
fn mixed_review_document_cannot_save_only_its_safe_subset() {
    let safe_source = require(
        SourceBinding::new(
            KnowledgeSourceKind::Session,
            Uuid::new_v4(),
            1,
            "fixture host · session-a",
        ),
        "safe source",
    );
    let pending_source = require(
        SourceBinding::new(
            KnowledgeSourceKind::Session,
            Uuid::new_v4(),
            1,
            "fixture host · session-a",
        ),
        "pending source",
    );
    let safe_fact = require(
        SelectedFact::completed(safe_source, 42, "Health check", "Service reports ready"),
        "safe fact",
    );
    let pending_fact = require(
        SelectedFact::unknown(pending_source, 43, "Pending check"),
        "unknown fact",
    );
    let selection = require(
        KnowledgeSelection::new(vec![safe_fact, pending_fact]),
        "selection",
    );
    let document = require(KnowledgeService::build(&selection), "document");

    assert!(matches!(
        SavedKnowledgeEntry::from_document(
            Uuid::new_v4(),
            &document,
            "session-a",
            KnowledgeCaptureType::Selection,
        ),
        Err(keelshell_core::KnowledgeError::NotPersistable)
    ));
}

#[test]
fn non_session_capture_cannot_cross_the_saved_entry_boundary() {
    let source = require(
        SourceBinding::new(KnowledgeSourceKind::File, Uuid::new_v4(), 1, "local note"),
        "source",
    );
    let fact = require(
        SelectedFact::completed(source, 42, "Local note", "Reviewed locally"),
        "fact",
    );
    let selection = require(KnowledgeSelection::new(vec![fact]), "selection");
    let document = require(KnowledgeService::build(&selection), "document");

    assert!(matches!(
        SavedKnowledgeEntry::from_document(
            Uuid::new_v4(),
            &document,
            "session-a",
            KnowledgeCaptureType::Selection,
        ),
        Err(keelshell_core::KnowledgeError::NotPersistable)
    ));
}
