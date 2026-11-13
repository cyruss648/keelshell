//! Layout metadata is exercised through real isolated state files, never a session.
use std::{fs, io::Write, path::Path};

use keelshell_core::{
    AppState, Connection, DeletedConnection, Error, LayoutPane, LayoutTab, LayoutTabAvailability,
    LayoutWindow, MAX_LAYOUT_TABS, MAX_LAYOUT_TABS_PER_PANE, MAX_LAYOUT_WINDOWS, PaneLayout,
    PaneNode, PaneNodeId, SplitAxis, StateStore, WORKSPACE_LAYOUT_VERSION, WorkspaceLayout,
};
use serde_json::{Value, json};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn window(n: u128, connection_id: Option<Uuid>) -> LayoutWindow {
    let pane = PaneNodeId(id(n + 1));
    let tab = LayoutTab {
        id: id(n + 2),
        connection_id,
    };
    LayoutWindow {
        id: id(n),
        layout: PaneLayout {
            root: pane,
            active: pane,
            nodes: vec![PaneNode::Leaf { id: pane }],
        },
        panes: vec![LayoutPane {
            pane,
            active_tab: Some(tab.id),
            tabs: vec![tab],
        }],
    }
}

fn snapshot() -> WorkspaceLayout {
    WorkspaceLayout {
        version: WORKSPACE_LAYOUT_VERSION,
        windows: vec![window(100, Some(id(1))), window(200, None)],
        active_window: id(200),
    }
}

fn state_with_layout() -> AppState {
    let mut profile = Connection::new("synthetic profile", "layout.invalid", "test");
    profile.id = id(1);
    AppState {
        connections: vec![profile],
        workspace_layout: Some(snapshot()),
        ..AppState::default()
    }
}

fn write_private(path: &Path, value: &Value) -> TestResult {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&serde_json::to_vec(value)?)?;
    Ok(())
}

#[test]
fn legacy_state_load_does_not_add_layout_or_write_disk() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let expected = AppState::default();
    let document = serde_json::to_value(&expected)?;
    assert!(document.get("workspace_layout").is_none());
    write_private(&path, &document)?;
    let original = fs::read(&path)?;

    let store = StateStore::new(&path);
    let loaded = store.load()?;
    assert_eq!(loaded, expected);
    assert!(loaded.workspace_layout.is_none());
    assert_eq!(fs::read(&path)?, original);
    let saved = store.save(&loaded)?;
    assert!(
        serde_json::to_value(saved)?
            .get("workspace_layout")
            .is_none()
    );
    Ok(())
}

#[test]
fn nested_layout_tabs_and_selections_round_trip_through_disk() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let mut state = state_with_layout();
    let saved_layout = state.workspace_layout.as_mut().ok_or("layout missing")?;
    let window = &mut saved_layout.windows[0];
    for (old, new, split, axis, ratio) in [
        (101, 103, 104, SplitAxis::Horizontal, 3500),
        (103, 105, 106, SplitAxis::Vertical, 6500),
    ] {
        window.layout.split(
            PaneNodeId(id(old)),
            PaneNodeId(id(new)),
            PaneNodeId(id(split)),
            axis,
            ratio,
        )?;
        window.panes.push(LayoutPane {
            pane: PaneNodeId(id(new)),
            tabs: Vec::new(),
            active_tab: None,
        });
    }
    state.validate()?;
    let saved = store.save(&state)?;
    let reloaded = StateStore::new(store.path()).load()?;
    assert_eq!(reloaded, saved);
    assert_eq!(reloaded.workspace_layout, state.workspace_layout);
    assert_eq!(
        reloaded
            .workspace_layout
            .ok_or("reloaded layout missing")?
            .windows[0]
            .layout
            .leaf_ids()?,
        vec![
            PaneNodeId(id(101)),
            PaneNodeId(id(103)),
            PaneNodeId(id(105))
        ]
    );
    Ok(())
}

#[test]
fn missing_trashed_and_unsaved_profiles_are_inactive_placeholders() -> TestResult {
    let mut state = state_with_layout();
    let layout = state.workspace_layout.as_ref().ok_or("layout missing")?;
    let available = layout.windows[0].panes[0].tabs[0].clone();
    let unbound = layout.windows[1].panes[0].tabs[0].clone();
    assert_eq!(
        available.availability(&state),
        LayoutTabAvailability::Available
    );
    assert_eq!(unbound.availability(&state), LayoutTabAvailability::Unbound);

    let removed = state.connections.remove(0);
    assert_eq!(
        available.availability(&state),
        LayoutTabAvailability::Missing
    );
    state.validate()?;
    state.deleted_connections.push(DeletedConnection {
        connection: removed,
        deleted_at: 1,
    });
    state.validate()?;
    assert_eq!(
        available.availability(&state),
        LayoutTabAvailability::Missing
    );
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, state);
    Ok(())
}

#[test]
fn layout_edit_invalidates_equality_and_cannot_overwrite_a_newer_save() -> TestResult {
    let directory = tempfile::tempdir()?;
    let first = StateStore::new(directory.path().join("state.json"));
    let old = first.save(&state_with_layout())?;
    let second = StateStore::new(first.path());
    let stale = second.load()?;
    let mut edited = old.clone();
    edited
        .workspace_layout
        .as_mut()
        .ok_or("layout missing")?
        .active_window = id(100);
    assert_ne!(edited, old, "review snapshots must notice layout changes");
    let saved = first.save(&edited)?;
    let bytes = fs::read(first.path())?;
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    assert!(matches!(first.save(&old), Err(Error::Conflict)));
    assert_eq!(fs::read(first.path())?, bytes);
    assert_eq!(StateStore::new(first.path()).load()?, saved);
    Ok(())
}

#[test]
fn explicit_backup_restore_preserves_exact_layout_and_issues_new_revision() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let saved = store.save(&state_with_layout())?;
    let backup = store.create_config_backup()?;
    let mut edited = saved.clone();
    edited.workspace_layout = None;
    let edited = store.save(&edited)?;
    let preview = store.preview_config_backup(backup)?;
    assert_eq!(store.load()?, edited, "preview must not restore");
    let restored = store.restore_config_backup(&preview)?;
    assert_eq!(restored.workspace_layout, saved.workspace_layout);
    assert_eq!(StateStore::new(store.path()).load()?, restored);
    assert!(matches!(store.save(&saved), Err(Error::Conflict)));
    Ok(())
}

#[test]
fn connection_export_omits_layout_and_import_preserves_local_layout() -> TestResult {
    let source = state_with_layout();
    let exported = source.export_connections()?;
    assert!(
        serde_json::from_str::<Value>(&exported)?
            .get("workspace_layout")
            .is_none()
    );
    let mut target = AppState {
        workspace_layout: Some(snapshot()),
        ..AppState::default()
    };
    let expected = target.workspace_layout.clone();
    assert_eq!(target.import_connections(&exported)?.added, 1);
    assert_eq!(target.workspace_layout, expected);
    Ok(())
}

#[test]
fn unsupported_null_and_malformed_layout_documents_cannot_be_overwritten() -> TestResult {
    let original = serde_json::to_value(state_with_layout())?;
    let mut cases = Vec::new();
    for value in [json!(null), json!({}), json!([]), json!("layout")] {
        let mut candidate = original.clone();
        candidate["workspace_layout"] = value;
        cases.push(candidate);
    }
    for value in [json!(0), json!(2), json!(-1), json!(true)] {
        let mut candidate = original.clone();
        candidate["workspace_layout"]["version"] = value;
        cases.push(candidate);
    }
    let mut invalid_selection = original.clone();
    invalid_selection["workspace_layout"]["active_window"] = json!(Uuid::nil());
    cases.push(invalid_selection);
    for candidate in cases {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        write_private(&path, &candidate)?;
        let bytes = fs::read(&path)?;
        let store = StateStore::new(&path);
        assert!(store.load().is_err());
        assert!(store.save(&AppState::default()).is_err());
        assert_eq!(fs::read(path)?, bytes);
    }
    Ok(())
}

#[test]
fn unknown_action_and_secret_fields_are_rejected_at_every_layout_level() -> TestResult {
    let source = serde_json::to_value(state_with_layout())?;
    for pointer in [
        "/workspace_layout",
        "/workspace_layout/windows/0",
        "/workspace_layout/windows/0/layout",
        "/workspace_layout/windows/0/layout/nodes/0",
        "/workspace_layout/windows/0/panes/0",
        "/workspace_layout/windows/0/panes/0/tabs/0",
    ] {
        for field in [
            "command",
            "password",
            "endpoint",
            "transfer",
            "approved",
            "runtime_session",
        ] {
            let mut candidate = source.clone();
            candidate
                .pointer_mut(pointer)
                .ok_or("fixture pointer missing")?
                .as_object_mut()
                .ok_or("fixture object missing")?
                .insert(field.into(), json!("synthetic-rejected-value"));
            assert!(
                serde_json::from_value::<AppState>(candidate).is_err(),
                "{pointer}/{field}"
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_in_memory_layout_save_keeps_last_valid_disk_and_revision() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let saved = store.save(&state_with_layout())?;
    let bytes = fs::read(store.path())?;
    let mut candidate = saved.clone();
    candidate
        .workspace_layout
        .as_mut()
        .ok_or("layout missing")?
        .windows[0]
        .panes[0]
        .active_tab = None;
    assert!(matches!(store.save(&candidate), Err(Error::Validation(_))));
    assert_eq!(fs::read(store.path())?, bytes);
    assert_eq!(
        store.save(&saved)?,
        saved,
        "invalid candidate must not rotate revision"
    );
    Ok(())
}

#[test]
fn window_pane_and_tab_identities_and_selections_are_checked() {
    let source = snapshot();
    let mut invalid = Vec::new();
    let mut changed = source.clone();
    changed.windows[1].id = changed.windows[0].id;
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].id = Uuid::nil();
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[1].panes[0].tabs[0].id = changed.windows[0].panes[0].tabs[0].id;
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes[0].tabs[0].id = Uuid::nil();
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes[0].tabs[0].connection_id = Some(Uuid::nil());
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes[0].pane = PaneNodeId(id(999));
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes.clear();
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes[0].active_tab = Some(id(202));
    invalid.push(changed);
    let mut changed = source.clone();
    changed.windows[0].panes[0].tabs.clear();
    invalid.push(changed);
    let mut changed = source.clone();
    changed.active_window = id(999);
    invalid.push(changed);
    for changed in invalid {
        assert!(changed.validate().is_err());
    }
}

#[test]
fn every_leaf_must_have_one_tab_collection_and_splits_cannot_have_tabs() -> TestResult {
    let mut candidate = snapshot();
    let first = &mut candidate.windows[0];
    first.layout.split(
        PaneNodeId(id(101)),
        PaneNodeId(id(103)),
        PaneNodeId(id(104)),
        SplitAxis::Vertical,
        5000,
    )?;
    assert!(candidate.validate().is_err());
    candidate.windows[0].panes.push(LayoutPane {
        pane: PaneNodeId(id(103)),
        tabs: Vec::new(),
        active_tab: None,
    });
    candidate.validate()?;
    candidate.windows[0].panes[1].pane = PaneNodeId(id(101));
    assert!(candidate.validate().is_err());
    candidate.windows[0].panes[1].pane = PaneNodeId(id(104));
    assert!(candidate.validate().is_err());
    Ok(())
}

#[test]
fn aggregate_window_and_tab_limits_apply_across_individually_valid_panes() -> TestResult {
    let mut candidate = snapshot();
    candidate.windows.clear();
    assert!(candidate.validate().is_err());
    for n in 1..=MAX_LAYOUT_WINDOWS {
        candidate
            .windows
            .push(window((n * 100) as u128, Some(id(1))));
    }
    candidate.validate()?;
    candidate.windows.push(window(900, None));
    assert!(candidate.validate().is_err());
    candidate.windows.pop();

    let mut next_id = 10_000;
    for window in candidate
        .windows
        .iter_mut()
        .take(MAX_LAYOUT_TABS / MAX_LAYOUT_TABS_PER_PANE)
    {
        let pane = &mut window.panes[0];
        pane.tabs.clear();
        for _ in 0..MAX_LAYOUT_TABS_PER_PANE {
            pane.tabs.push(LayoutTab {
                id: id(next_id),
                connection_id: Some(id(1)),
            });
            next_id += 1;
        }
        pane.active_tab = Some(pane.tabs[0].id);
    }
    for window in &mut candidate.windows[MAX_LAYOUT_TABS / MAX_LAYOUT_TABS_PER_PANE..] {
        window.panes[0].tabs.clear();
        window.panes[0].active_tab = None;
    }
    candidate.validate()?;
    candidate.windows[4].panes[0].tabs.push(LayoutTab {
        id: id(next_id),
        connection_id: None,
    });
    candidate.windows[4].panes[0].active_tab = Some(id(next_id));
    assert!(
        candidate.validate().is_err(),
        "aggregate bound must span windows"
    );
    candidate.windows[4].panes[0].tabs.clear();
    candidate.windows[4].panes[0].active_tab = None;
    candidate.windows[0].panes[0].tabs.push(LayoutTab {
        id: id(next_id),
        connection_id: None,
    });
    assert!(
        candidate.validate().is_err(),
        "per-pane bound must still apply"
    );
    Ok(())
}
