//! Test-only bridge mounting the real workspace around an actual SFTP panel.
use super::*;
use keelshell_core::{
    ProfileSyncChoice, ProfileSyncService, WorkflowAuditOutcome, WorkflowAuditRecord,
    WorkflowAuditTrigger, WorkflowTaskAudit,
};
use keelshell_session::SshSession;
use uuid::Uuid;

pub(crate) struct Scene {
    pub(crate) window: AnyWindowHandle,
    pub(crate) files: Entity<crate::files::FilesPanel>,
    fixture: Fixture,
    panes: Vec<RemotePane>,
    lifecycle: tokio::sync::watch::Sender<crate::terminal::TransportState>,
}

pub(crate) fn mount_scene(
    cx: &mut TestAppContext,
    session: SshSession,
    runtime: Arc<tokio::runtime::Runtime>,
) -> Scene {
    let connection = Connection::new(
        "Captured original SSH",
        "original.fixture.invalid",
        "fixture",
    );
    let profile_id = connection.id;
    let fixture = mount(cx, vec![connection]);
    let mut state = fixture.store.load().checked("combination baseline");
    state
        .record_workflow_audit(WorkflowAuditRecord {
            id: Uuid::new_v4(),
            recorded_at: 1_725_000_000,
            trigger: WorkflowAuditTrigger::Manual,
            tasks: vec![WorkflowTaskAudit {
                id: Uuid::new_v4(),
                target_id: profile_id,
                outcome: WorkflowAuditOutcome::Unknown,
            }],
            cancelled: false,
            stopped_after_failure: false,
        })
        .checked("metadata-only unknown history");
    fixture.store.save(&state).checked("persist local history");
    let shared = fixture
        .store
        .path()
        .parent()
        .checked_option("fixture parent")
        .join("shared");
    std::fs::create_dir(&shared).checked("owned shared directory");
    let service = ProfileSyncService::new(fixture.store.clone());
    let review = service
        .inspect(
            shared,
            zeroize::Zeroizing::new("owned-mirror-sync-password".into()),
            &AtomicBool::new(false),
        )
        .checked("real encrypted channel inspection");
    let choices = review
        .rows()
        .iter()
        .map(|row| (row.id, ProfileSyncChoice::Local))
        .collect();
    let outcome = service
        .apply(
            review,
            choices,
            zeroize::Zeroizing::new("owned-mirror-sync-password".into()),
            &AtomicBool::new(false),
        )
        .checked("real encrypted channel publication");
    assert!(outcome.published);
    let panes = attach_remote_panes(&fixture, cx);
    let (lifecycle, source) = tokio::sync::watch::channel(crate::terminal::TransportState::Ready);
    panes[0]
        .terminal
        .update(cx, |terminal, _| terminal.attach_lifecycle(source));
    let files = cx
        .update_window(fixture.window, |_, window, cx| {
            let files = cx.new(|cx| {
                crate::files::FilesPanel::new(
                    session.clone(),
                    "captured SFTP".into(),
                    runtime.clone(),
                    window,
                    cx,
                )
            });
            fixture.workspace.update(cx, |view, cx| {
                view.state = outcome.state;
                view.runtime = runtime;
                view.show_connections = false;
                let route = view
                    .state
                    .connection_route(profile_id)
                    .checked("captured profile route");
                view.bind_remote_tab(panes[0].terminal.entity_id(), route);
                view.remote_sessions
                    .insert(panes[0].terminal.entity_id(), session);
                view.panels.insert(
                    panes[0].terminal.entity_id(),
                    super::super::RemotePanels {
                        files: Some(files.clone()),
                        ..Default::default()
                    },
                );
                view.visible_panel = Some(super::super::ToolPanel::Files);
                cx.notify();
            });
            window.render_frame(cx);
            files
        })
        .checked("mount same real file entity in production workspace");
    Scene {
        window: fixture.window,
        files,
        fixture,
        panes,
        lifecycle,
    }
}

impl Scene {
    pub(crate) fn open_sync(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            self.fixture
                .workspace
                .update(cx, |view, cx| view.open_profile_sync(window, cx));
            window.render_frame(cx);
            assert!(window.find("profile-sync-panel").visible());
        })
        .checked("production sync lease beside existing file entity");
        cx.run_until_parked();
    }

    pub(crate) fn external_saved_host_then_disable(&self, cx: &mut TestAppContext) {
        let other = StateStore::new(self.fixture.store.path());
        let mut changed = other.load().checked("independent legitimate store read");
        changed.connections[0].host = "updated-saved-only.fixture.invalid".into();
        other
            .save(&changed)
            .checked("independent saved endpoint change");
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("profile-sync-disable", cx);
        })
        .checked("real Disable worker refreshes authoritative saved state");
    }

    pub(crate) async fn disabled(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(8), |_, cx| {
            let view = self.fixture.workspace.read(cx);
            view.state.connections[0].host == "updated-saved-only.fixture.invalid"
                && view
                    .state
                    .profile_sync
                    .as_ref()
                    .is_some_and(|sync| !sync.enabled())
        })
        .await;
        self.fixture.workspace.read_with(cx, |view, _| {
            assert_eq!(
                view.panels[&self.panes[0].terminal.entity_id()]
                    .files
                    .as_ref()
                    .checked_option("retained files")
                    .entity_id(),
                self.files.entity_id()
            );
            assert_eq!(view.state.workflow_audits.len(), 1);
        });
    }

    pub(crate) fn close_sync(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("profile-sync-close", cx);
        })
        .checked("actual Close releases configuration lease");
        cx.run_until_parked();
        self.fixture
            .workspace
            .read_with(cx, |view, _| assert!(view.profile_sync.is_none()));
    }

    pub(crate) async fn retire_files(&self, cx: &mut TestAppContext) {
        // The terminal lifecycle is controlled here; the separate TCP SFTP peer
        // remains available so exact target bytes can be observed after retirement.
        self.lifecycle
            .send_replace(crate::terminal::TransportState::Ended(
                keelshell_session::ShellEnd::Exited { code: 0 },
            ));
        cx.wait_for(self.window, Duration::from_secs(5), |_, cx| {
            self.panes[0].terminal.read(cx).end_reason().is_some()
        })
        .await;
        cx.update_window(self.window, |_, window, cx| {
            self.fixture
                .workspace
                .update(cx, |view, cx| view.poll_reconnect(window, cx));
        })
        .checked("production lifecycle dispatcher retires existing panels while modal is open");
        cx.run_until_parked();
    }

    pub(crate) fn inspect_history_without_replay(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            self.fixture
                .workspace
                .update(cx, |view, cx| view.open_workflow(false, window, cx));
            window.render_frame(cx);
            window.click("workflow-audit-history", cx);
            window.render_frame(cx);
            assert!(window.find("workflow-audit-list").visible());
            window.click(("workflow-audit-select", 0_usize), cx);
            window.render_frame(cx);
            assert!(window.find(("workflow-audit-task", 0_usize)).visible());
            assert!(window.try_find("workflow-confirm").is_none());
            assert!(window.try_find("confirm-file-operation").is_none());
            window.click("workflow-audit-hide", cx);
        })
        .checked("read-only Unknown result selected through actual controls");
        cx.run_until_parked();
        assert!(self.panes.iter().all(|pane| writes(pane).is_empty()));
    }
}
