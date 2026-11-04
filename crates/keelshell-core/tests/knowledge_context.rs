#![allow(clippy::unwrap_used, clippy::expect_used)]

use keelshell_core::{
    KnowledgeCaptureType, KnowledgeSelection, KnowledgeService, KnowledgeSourceKind,
    MAX_SAVED_CONTEXT_ENTRIES, SavedContextError, SavedKnowledgeContext, SavedKnowledgeEntry,
    SelectedFact, SourceBinding,
};
use uuid::Uuid;

fn entry(summary: &str) -> SavedKnowledgeEntry {
    let source = SourceBinding::new(
        KnowledgeSourceKind::Session,
        Uuid::new_v4(),
        7,
        "retired host",
    )
    .unwrap();
    let fact = SelectedFact::failed(source, 42, "Historical observation", summary).unwrap();
    let document = KnowledgeService::build(&KnowledgeSelection::new(vec![fact]).unwrap()).unwrap();
    SavedKnowledgeEntry::from_document(
        Uuid::new_v4(),
        &document,
        "retired-session",
        KnowledgeCaptureType::VisibleScreen,
    )
    .unwrap()
}

#[test]
fn saved_context_is_explicit_ordered_and_preserves_complete_historical_identity() {
    let library = vec![entry("Remote refused"), entry("Earlier connection failed")];
    let mut context = SavedKnowledgeContext::default();
    assert!(context.compose(&library).unwrap().is_empty());
    assert!(context.select(&library[1], &library).unwrap());
    assert!(context.select(&library[0], &library).unwrap());
    assert!(!context.select(&library[1], &library).unwrap());
    let composed = context.compose(&library).unwrap();
    for (record, expected) in composed.iter().zip([&library[1], &library[0]]) {
        assert_eq!(
            serde_json::from_str::<SavedKnowledgeEntry>(record).unwrap(),
            *expected
        );
    }
    assert_eq!(
        context.bytes(),
        composed.iter().map(String::len).sum::<usize>()
    );
    assert!(!format!("{context:?}").contains("retired"));
    assert!(context.remove(library[1].id()));
    assert_eq!(context.entries(), &library[..1]);
    assert!(context.clear());
    assert!(!context.clear());
    assert_eq!(context.bytes(), 0);
}

#[test]
fn saved_context_rejects_edited_deleted_and_ambiguous_versions_without_retargeting() {
    let original = entry("Original facts");
    let library = vec![original.clone()];
    let mut context = SavedKnowledgeContext::default();
    context.select(&original, &library).unwrap();
    let edited = original
        .with_reviewed_content("Updated title", "Updated summary")
        .unwrap();
    for changed in [
        vec![edited],
        Vec::new(),
        vec![original.clone(), original.clone()],
    ] {
        assert_eq!(
            context.compose(&changed),
            Err(SavedContextError::StaleEntry)
        );
        assert_eq!(
            context.select(&original, &changed),
            Err(SavedContextError::StaleEntry)
        );
        assert_eq!(context.entries(), &library);
        let mut refreshed = context.clone();
        assert!(refreshed.retain_current(&changed));
        assert!(refreshed.entries().is_empty());
    }
    assert!(!context.retain_current(&library));
}

#[test]
fn saved_context_limits_are_atomic_and_count_serialized_unicode_metadata() {
    let library: Vec<_> = (0..=MAX_SAVED_CONTEXT_ENTRIES)
        .map(|_| entry("bounded fact"))
        .collect();
    let mut context = SavedKnowledgeContext::default();
    for item in &library[..MAX_SAVED_CONTEXT_ENTRIES] {
        context.select(item, &library).unwrap();
    }
    let before = context.clone();
    assert_eq!(
        context.select(&library[MAX_SAVED_CONTEXT_ENTRIES], &library),
        Err(SavedContextError::TooManyEntries)
    );
    assert_eq!(context, before);
    let library = vec![entry(&"中".repeat(5_400)), entry(&"文".repeat(5_400))];
    let mut context = SavedKnowledgeContext::default();
    context.select(&library[0], &library).unwrap();
    let before = context.clone();
    assert_eq!(
        context.select(&library[1], &library),
        Err(SavedContextError::TooLarge)
    );
    assert_eq!(context, before);
}

#[test]
fn saved_context_ingress_cannot_bypass_the_existing_sensitive_content_gate() {
    let record = entry("Reviewed harmless output");
    for field in ["summary", "title", "session_id"] {
        let mut wire = serde_json::to_value(&record).unwrap();
        wire[field] = "password = synthetic-test-value".into();
        assert!(serde_json::from_value::<SavedKnowledgeEntry>(wire).is_err());
    }
    assert!(
        record
            .with_reviewed_content("Credential output", "api key = synthetic-test-value")
            .is_err()
    );
}
