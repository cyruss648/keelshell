use std::fs;

use keelshell_core::{AppState, Connection, Error, Snippet, StateStore};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn empty_state() -> AppState {
    AppState {
        snippets: Vec::new(),
        ..AppState::default()
    }
}

fn assert_unchanged(actual: &AppState, before: &AppState) {
    assert_eq!(actual, before);
    assert_eq!(actual.snapshot, before.snapshot);
}

fn assert_validation<T: std::fmt::Debug>(result: Result<T, Error>, field: &str) {
    assert!(
        matches!(result, Err(Error::Validation(ref validation)) if validation.field == field),
        "unexpected result: {result:?}"
    );
}

#[test]
fn crud_preserves_identity_display_order_and_unrelated_state() -> TestResult {
    let mut state = empty_state();
    state
        .connections
        .push(Connection::new("测试节点", "node.example.test", "tester"));
    let first = Snippet::new("系统信息", "uname -a");
    let second = Snippet::new("磁盘空间", "df -h");
    let third = Snippet::new("监听端口", "ss -lntup");
    for snippet in [&first, &second, &third] {
        state.insert_snippet(snippet.clone())?;
    }
    let mut updated = second.clone();
    updated.name = "磁盘与目录".into();
    updated.command = "df -h\n\tdu -sh /tmp".into();
    updated.description = "先审核，再填入命令栏。".into();
    updated.tags = vec!["磁盘".into(), "常用".into()];
    let before = state.clone();
    state.update_snippet(updated.clone())?;
    assert_eq!(
        state.snippets,
        vec![first.clone(), updated.clone(), third.clone()]
    );
    assert_eq!(state.connections, before.connections);
    assert_eq!(state.snapshot, before.snapshot);
    assert_eq!(state.remove_snippet(updated.id)?, updated);
    assert_eq!(state.snippets, vec![first, third]);
    assert_eq!(state.connections, before.connections);
    assert_eq!(state.snapshot, before.snapshot);
    Ok(())
}

#[test]
fn duplicate_insert_cannot_overwrite_an_existing_template() -> TestResult {
    let mut state = empty_state();
    let original = Snippet::new("Original", "printf 'original'");
    state.insert_snippet(original.clone())?;
    let mut replacement = original;
    replacement.name = "Unexpected replacement".into();
    replacement.command = "printf 'replacement'".into();
    let before = state.clone();
    assert_validation(state.insert_snippet(replacement), "snippet.id");
    assert_unchanged(&state, &before);
    Ok(())
}

#[test]
fn missing_update_is_not_an_upsert_and_missing_remove_is_atomic() {
    let mut state = AppState::default();
    let missing = Snippet::new("Missing", "printf 'missing'");
    let before = state.clone();
    assert_validation(state.update_snippet(missing.clone()), "snippet.id");
    assert_unchanged(&state, &before);
    assert_validation(state.remove_snippet(missing.id), "snippet.id");
    assert_unchanged(&state, &before);
}

#[test]
fn nil_identity_and_ambiguous_existing_id_are_rejected_atomically() {
    let mut state = empty_state();
    let mut snippet = Snippet::new("Invalid", "true");
    snippet.id = Uuid::nil();
    let before = state.clone();
    assert_validation(state.insert_snippet(snippet), "snippet.id");
    assert_unchanged(&state, &before);

    let duplicate = Snippet::new("Ambiguous", "true");
    state.snippets = vec![duplicate.clone(), duplicate.clone()];
    let before = state.clone();
    assert_validation(state.update_snippet(duplicate.clone()), "snippet.id");
    assert_unchanged(&state, &before);
    assert_validation(state.remove_snippet(duplicate.id), "snippet.id");
    assert_unchanged(&state, &before);
}

#[test]
fn duplicate_display_names_have_distinct_identities() -> TestResult {
    let mut state = empty_state();
    let first = Snippet::new("同名", "printf 'first'");
    let second = Snippet::new("同名", "printf 'second'");
    state.insert_snippet(first.clone())?;
    state.insert_snippet(second.clone())?;
    assert_eq!(state.remove_snippet(first.id)?, first);
    assert_eq!(state.snippets, vec![second]);
    Ok(())
}

#[test]
fn unicode_metadata_and_multiline_command_reach_their_existing_limits() -> TestResult {
    let mut state = empty_state();
    let mut snippet = Snippet::new("中".repeat(120), "界".repeat(21_845) + "\n");
    snippet.description = "文".repeat(2048);
    snippet.tags = (0..32).map(|_| "签".repeat(64)).collect();
    assert_eq!(snippet.command.len(), 65_536);
    state.insert_snippet(snippet.clone())?;
    assert_eq!(state.snippets, vec![snippet]);
    Ok(())
}

#[test]
fn invalid_metadata_and_command_limits_leave_both_insert_and_update_unchanged() -> TestResult {
    let mut state = empty_state();
    let original = Snippet::new("Original", "printf 'hello'\n\tuname -a");
    state.insert_snippet(original.clone())?;
    let mut invalid = Vec::new();
    for name in [
        "".into(),
        " leading".into(),
        "中".repeat(121),
        "bad\nname".into(),
    ] {
        let mut entry = original.clone();
        entry.name = name;
        invalid.push((entry, "snippet.name"));
    }
    for description in ["文".repeat(2049), "bad\tdescription".into()] {
        let mut entry = original.clone();
        entry.description = description;
        invalid.push((entry, "snippet.description"));
    }
    for command in ["\n\t ".into(), "x".repeat(65_537), "中".repeat(21_846)] {
        let mut entry = original.clone();
        entry.command = command;
        invalid.push((entry, "snippet.command"));
    }
    for tags in [
        vec!["tag".into(); 33],
        vec!["签".repeat(65)],
        vec!["bad\ntag".into()],
    ] {
        let field = if tags.len() > 32 { "tags" } else { "tag" };
        let mut entry = original.clone();
        entry.tags = tags;
        invalid.push((entry, field));
    }
    let before = state.clone();
    for (mut entry, field) in invalid {
        assert_validation(state.update_snippet(entry.clone()), field);
        assert_unchanged(&state, &before);
        entry.id = Uuid::new_v4();
        assert_validation(state.insert_snippet(entry), field);
        assert_unchanged(&state, &before);
    }
    Ok(())
}

#[test]
fn terminal_controls_are_rejected_but_tabs_and_line_feeds_are_preserved() -> TestResult {
    let mut state = empty_state();
    let original = Snippet::new("Text", "  printf '中文'\n\tuname -a\n");
    state.insert_snippet(original.clone())?;
    let before = state.clone();
    for control in ['\0', '\r', '\u{1b}', '\u{7f}', '\u{85}'] {
        let mut entry = original.clone();
        entry.command = format!("printf 'before'{control}printf 'after'");
        assert_validation(state.update_snippet(entry), "snippet.command");
        assert_unchanged(&state, &before);
    }
    assert_eq!(state.snippets[0].command, original.command);
    Ok(())
}

#[test]
fn collection_limit_allows_update_and_delete_but_refuses_one_more_insert() -> TestResult {
    let mut state = AppState {
        snippets: (0..2000)
            .map(|index| Snippet::new(format!("Snippet {index}"), "true"))
            .collect(),
        ..AppState::default()
    };
    state.validate()?;
    let before = state.clone();
    assert_validation(
        state.insert_snippet(Snippet::new("Overflow", "true")),
        "snippets",
    );
    assert_unchanged(&state, &before);
    let mut edited = state.snippets[999].clone();
    edited.command = "printf 'updated'".into();
    state.update_snippet(edited.clone())?;
    assert_eq!(state.snippets[999], edited);
    state.remove_snippet(edited.id)?;
    state.insert_snippet(Snippet::new("Available slot", "true"))?;
    assert_eq!(state.snippets.len(), 2000);
    Ok(())
}

#[test]
fn unrelated_validation_failures_do_not_commit_any_template_mutation() {
    let mut state = AppState::default();
    state.settings.font_size = 100.0;
    let before = state.clone();
    let mut updated = state.snippets[0].clone();
    updated.command = "printf 'not committed'".into();
    assert_validation(state.update_snippet(updated), "settings.font_size");
    assert_unchanged(&state, &before);
    assert_validation(
        state.remove_snippet(state.snippets[0].id),
        "settings.font_size",
    );
    assert_unchanged(&state, &before);
    assert_validation(
        state.insert_snippet(Snippet::new("Not committed", "true")),
        "settings.font_size",
    );
    assert_unchanged(&state, &before);
}

#[test]
fn explicit_crud_round_trips_through_real_storage_and_keeps_revision() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let store = StateStore::new(temporary.path().join("state.json"));
    let mut state = store.load()?;
    let before = state.snapshot;
    let mut snippet = Snippet::new("持久化中文", "printf '你好'\n\tuname -a\n");
    snippet.description = "手动保存的命令片段".into();
    snippet.tags = vec!["验收".into()];
    state.insert_snippet(snippet.clone())?;
    assert_eq!(state.snapshot, before);
    assert!(!store.path().exists());
    state = store.save(&state)?;
    assert_ne!(state.snapshot, before);
    assert_eq!(StateStore::new(store.path()).load()?, state);
    snippet.name = "已编辑".into();
    snippet.command.push_str("printf '完成'");
    state.update_snippet(snippet.clone())?;
    state = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, state);
    assert_eq!(state.remove_snippet(snippet.id)?, snippet);
    let saved = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    assert!(!fs::read_to_string(store.path())?.contains("已编辑"));
    Ok(())
}

#[test]
fn stale_template_update_and_delete_cannot_overwrite_another_process() -> TestResult {
    for remove in [false, true] {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("state.json");
        let first = StateStore::new(&path);
        let second = StateStore::new(&path);
        let mut initial = empty_state();
        let snippet = Snippet::new("共享片段", "printf 'initial'");
        initial.insert_snippet(snippet.clone())?;
        first.save(&initial)?;
        let mut winner = first.load()?;
        let mut stale = second.load()?;
        let mut winner_snippet = snippet.clone();
        winner_snippet.command = "printf 'saved by first process'".into();
        winner.update_snippet(winner_snippet)?;
        let winner = first.save(&winner)?;
        let disk_before = fs::read(&path)?;
        if remove {
            stale.remove_snippet(snippet.id)?;
        } else {
            let mut losing_snippet = snippet;
            losing_snippet.command = "printf 'unsaved second process draft'".into();
            stale.update_snippet(losing_snippet)?;
        }
        let draft = stale.clone();
        assert!(matches!(second.save(&stale), Err(Error::Conflict)));
        assert_eq!(fs::read(&path)?, disk_before);
        assert_eq!(StateStore::new(&path).load()?, winner);
        assert_unchanged(&stale, &draft);
    }
    Ok(())
}

#[test]
fn failed_oversized_document_save_preserves_disk_and_editable_draft() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let store = StateStore::new(temporary.path().join("state.json"));
    let mut state = store.save(&empty_state())?;
    let disk_before = fs::read(store.path())?;
    for index in 0..64 {
        state.insert_snippet(Snippet::new(format!("Large {index}"), "x".repeat(65_536)))?;
    }
    let draft = state.clone();
    assert!(matches!(store.save(&state), Err(Error::TooLarge)));
    assert_eq!(fs::read(store.path())?, disk_before);
    assert_unchanged(&state, &draft);
    Ok(())
}
