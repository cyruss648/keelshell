//! Controlled GPUI event integration retains the actual mounted transport entities.
use super::{attach_remote_panes, mount, writes};
use gpui_kit::{AppContext, TestAppContext, test::TestWindowExt};
use keelshell_core::{Connection, ProfileSyncChoice, ProfileSyncService};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
use zeroize::Zeroizing;
#[gpui_kit::test]
fn saved_sync_updates_do_not_replace_or_write_active_remote_panes(cx: &mut TestAppContext) {
    let c = Connection::new("Original", "fixture.invalid", "fixture");
    let id = c.id;
    let fixture = mount(cx, vec![c]);
    let panes = attach_remote_panes(&fixture, cx);
    let temporary = tempfile::tempdir().checked();
    let dir = temporary.path().join("shared");
    std::fs::create_dir(&dir).checked();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |w, cx| {
            w.show_connections = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("profile-sync-open", cx);
        window.render_frame(cx);
        assert!(window.try_find("profile-sync-panel").is_some());
    })
    .checked();
    let service = ProfileSyncService::new(fixture.store.clone());
    let review = service
        .inspect(
            dir,
            Zeroizing::new("workspace-sync-password".into()),
            &AtomicBool::new(false),
        )
        .checked();
    let choices = review
        .rows()
        .iter()
        .map(|r| (r.id, ProfileSyncChoice::Local))
        .collect::<BTreeMap<_, _>>();
    let mut outcome = service
        .apply(
            review,
            choices,
            Zeroizing::new("workspace-sync-password".into()),
            &AtomicBool::new(false),
        )
        .checked();
    outcome.state.connections[0].name = "Updated saved profile".into();
    let state = fixture.store.save(&outcome.state).checked();
    cx.update_window(fixture.window, |_, _window, cx| {
        let panel = fixture.workspace.read(cx).profile_sync.clone().checked();
        panel.update(cx, |_, cx| {
            cx.emit(crate::profile_sync::ProfileSyncEvent::Changed {
                state: Box::new(state),
                message: crate::i18n::Message::new("保存完成", "Saved"),
            })
        });
    })
    .checked();
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        assert_eq!(
            workspace
                .state
                .connections
                .iter()
                .find(|c| c.id == id)
                .checked()
                .name,
            "Updated saved profile"
        );
        assert_eq!(workspace.tabs.len(), 2);
        for (i, p) in panes.iter().enumerate() {
            assert_eq!(workspace.tabs[i].entity_id(), p.terminal.entity_id());
            assert!(workspace.remote_hosts.contains_key(&p.terminal.entity_id()));
        }
        window.render_frame(cx);
    })
    .checked();
    for pane in &panes {
        assert!(
            writes(pane).is_empty(),
            "sync must not write a command to mounted remote transports"
        );
    }
}

trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("sync fixture failed: {error:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("sync fixture missing expected value"),
        }
    }
}

#[gpui_kit::test]
async fn sync_lease_preserves_late_usage_and_batch_receipts_until_close(cx: &mut TestAppContext) {
    use gpui_kit::test::TestAppContextExt;
    let connection = Connection::new("Receipt profile", "fixture.invalid", "fixture");
    let id = connection.id;
    let fixture = mount(cx, vec![connection.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.open_profile_sync(window, cx);
            let route = workspace.state.connection_route(id).checked();
            workspace.pending_recents.push((connection, route, 10));
            let audit = keelshell_core::BatchAuditRecord::new(
                "printf 'isolated receipt'",
                10,
                vec![id],
                keelshell_core::BatchAuditSummary {
                    target_count: 1,
                    succeeded: 1,
                    failed: 0,
                    unknown: 0,
                    not_started: 0,
                    cancelled: false,
                    stopped_after_failure: false,
                },
            )
            .checked();
            workspace.pending_batch_audits.push(audit);
            workspace.flush_recent_connections(window, cx);
            workspace.flush_batch_audits(window, cx);
            assert_eq!(workspace.pending_recents.len(), 1);
            assert_eq!(workspace.pending_batch_audits.len(), 1);
            assert!(!workspace.saving);
        });
        window.render_frame(cx);
        window.click("profile-sync-close", cx);
    })
    .checked();
    cx.wait_for(
        fixture.window,
        std::time::Duration::from_secs(20),
        |_, cx| {
            let workspace = fixture.workspace.read(cx);
            workspace.profile_sync.is_none()
                && !workspace.saving
                && workspace.pending_recents.is_empty()
                && workspace.pending_batch_audits.is_empty()
        },
    )
    .await;
    let saved = fixture.store.load().checked();
    assert_eq!(saved.recent_connections.len(), 1);
    assert_eq!(saved.batch_audits.len(), 1);
}
