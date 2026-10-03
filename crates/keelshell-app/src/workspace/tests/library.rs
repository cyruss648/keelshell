use super::*;
use crate::workspace::library::{DestinationTarget, LibraryFilter};

async fn saved(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
}

fn create_folder(fixture: &Fixture, name: &str, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-folder", cx);
        let input = fixture
            .workspace
            .read(cx)
            .folder_form
            .as_ref()
            .checked_option("folder editor")
            .name
            .entity_id();
        window.click(("input", input), cx);
        window.input(name, cx);
        window.click("save-folder", cx);
    })
    .checked("create folder using real controls");
}

#[gpui_kit::test]
async fn folders_move_trash_restore_and_rename_persist_through_real_controls(
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("生产主机", "node.invalid", "operator");
    let id = profile.id;
    let fixture = mount(cx, vec![profile]);
    create_folder(&fixture, "研发", cx);
    saved(&fixture, cx).await;
    let root = fixture
        .workspace
        .read_with(cx, |view, _| view.state.folders[0].id);
    create_folder(&fixture, "应用", cx);
    saved(&fixture, cx).await;
    let child = fixture.workspace.read_with(cx, |view, _| {
        view.state
            .folders
            .iter()
            .find(|folder| folder.parent_id == Some(root))
            .checked_option("child folder")
            .id
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("all-connections", cx);
        window.render_frame(cx);
        window.click(("move", 0_usize), cx);
        window.render_frame(cx);
        window.click(("destination-folder", 1_usize), cx);
        window.click("save-destination", cx);
    })
    .checked("move profile to nested destination");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load saved membership");
    assert_eq!(state.folder_id_of(id), Some(child));
    assert_eq!(state.connections[0].group, "研发/应用");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("delete", 0_usize), cx);
    })
    .checked("move profile to trash");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load persistent trash");
    assert!(state.connections.is_empty());
    assert_eq!(state.deleted_connections[0].connection.id, id);
    assert_eq!(state.folder_id_of(id), Some(child));
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("trashed-connections", cx);
        window.render_frame(cx);
        assert!(window.try_find(("connect", 0_usize)).is_none());
        window.click(("restore", 0_usize), cx);
    })
    .checked("restore without a connect action in trash");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load restored profile");
    assert_eq!(state.connections[0].id, id);
    assert!(state.deleted_connections.is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("manage-folder", 0_usize), cx);
        window.render_frame(cx);
        fixture.workspace.update(cx, |view, cx| {
            view.folder_form
                .as_ref()
                .checked_option("folder editor")
                .name
                .update(cx, |input, cx| input.set_value("运维", window, cx));
        });
        window.click("save-folder", cx);
    })
    .checked("rename ancestor folder");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load renamed tree");
    assert_eq!(state.folder_path(child).as_deref(), Some("运维/应用"));
    assert_eq!(state.connections[0].group, "运维/应用");
}

#[gpui_kit::test]
async fn folder_cycle_and_nonempty_delete_keep_draft_and_existing_tree(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    create_folder(&fixture, "父目录", cx);
    saved(&fixture, cx).await;
    create_folder(&fixture, "子目录", cx);
    saved(&fixture, cx).await;
    let before = fixture.store.load().checked("load original tree");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("manage-folder", 0_usize), cx);
        window.render_frame(cx);
        window.click("remove-empty-folder", cx);
        window.render_frame(cx);
        let view = fixture.workspace.read(cx);
        assert!(!view.saving);
        assert!(
            view.folder_form
                .as_ref()
                .checked_option("retained folder editor")
                .message
                .is_some()
        );
        window.click(("folder-parent", 1_usize), cx);
        window.click("save-folder", cx);
        window.render_frame(cx);
        let view = fixture.workspace.read(cx);
        assert!(!view.saving);
        assert!(
            view.folder_form
                .as_ref()
                .checked_option("cycle rejected")
                .message
                .is_some()
        );
    })
    .checked("refuse nonempty delete and ancestor cycle");
    assert_eq!(
        fixture
            .store
            .load()
            .checked("compare tree after failed mutations"),
        before
    );
}

#[gpui_kit::test]
async fn recent_success_queues_behind_save_without_losing_metadata_and_filters(
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("最近主机", "recent.invalid", "operator");
    let id = profile.id;
    let fixture = mount(cx, vec![profile]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.toggle_favorite(id, window, cx);
            assert!(view.saving);
            view.remember_successful_connection(view.state.connections[0].clone(), window, cx);
            assert_eq!(view.pending_recents.len(), 1);
            assert!(view.state.recent_connections.is_empty());
        });
    })
    .checked("queue successful usage behind favorite write");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load both serialized updates");
    assert!(state.connections[0].favorite);
    assert_eq!(state.recent_connections.len(), 1);
    assert_eq!(state.recent_connections[0].connection_id, id);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("recent-connections", cx);
        window.render_frame(cx);
        assert!(window.try_find(("connect", 0_usize)).is_some());
        window.click(("delete", 0_usize), cx);
    })
    .checked("trash recently connected profile");
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("recent removed on trash")
            .recent_connections
            .is_empty()
    );
}

#[gpui_kit::test]
async fn tags_and_folder_selection_save_with_profile_and_language_keeps_drafts(
    cx: &mut TestAppContext,
) {
    let profile = Connection::new("标签主机", "tags.invalid", "operator");
    let id = profile.id;
    let fixture = mount(cx, vec![profile]);
    create_folder(&fixture, "服务", cx);
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("all-connections", cx);
        window.render_frame(cx);
        window.click(("edit", 0_usize), cx);
        fixture.workspace.update(cx, |view, cx| {
            let form = view.form.as_ref().checked_option("profile editor");
            form.tags
                .update(cx, |input, cx| input.set_value("生产, API", window, cx));
            view.open_destination(DestinationTarget::Draft, window, cx);
        });
        window.render_frame(cx);
        window.click(("destination-folder", 0_usize), cx);
        window.click("save-destination", cx);
        fixture
            .workspace
            .update(cx, |view, cx| view.switch_language(window, cx));
    })
    .checked("select draft folder and change locale");
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        let form = fixture
            .workspace
            .read(cx)
            .form
            .as_ref()
            .checked_option("draft survived language switch");
        assert_eq!(form.tags.read(cx).value(), "生产, API");
        assert!(form.folder_id.is_some());
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("save bilingual profile editor");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load folder and tags");
    assert_eq!(state.connections[0].tags, ["生产", "API"]);
    assert_eq!(state.folder_id_of(id), Some(state.folders[0].id));
}

#[gpui_kit::test]
fn destination_picker_focus_never_sends_keys_to_ssh(cx: &mut TestAppContext) {
    let profile = Connection::new("选择目标", "focus.invalid", "operator");
    let id = profile.id;
    let fixture = mount(cx, vec![profile]);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.open_destination(DestinationTarget::Connection(id), window, cx)
        });
        window.render_frame(cx);
        window.input("must-not-reach-ssh", cx);
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .checked("type while destination picker owns focus");
    cx.run_until_parked();
    for pane in &panes {
        assert!(writes(pane).is_empty());
    }
    fixture.workspace.read_with(cx, |view, _| {
        assert!(view.destination_prompt.is_some());
        assert_eq!(view.library_filter, LibraryFilter::All);
    });
}

#[gpui_kit::test]
async fn empty_folder_import_is_persisted_even_without_new_connections(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let mut incoming = AppState::default();
    let root = incoming
        .create_folder("导入的空目录", None)
        .checked("create imported root");
    incoming
        .create_folder("空子目录", Some(root))
        .checked("create imported child");
    let document = incoming
        .export_connections()
        .checked("export empty directory tree");
    cx.update_window(fixture.window, |_, window, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(document));
        window.render_frame(cx);
        window.click("import-connections", cx);
    })
    .checked("import empty folders with production button");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load imported empty folders");
    assert!(state.connections.is_empty());
    assert_eq!(state.folders.len(), 2);
    assert!(
        state
            .folders
            .iter()
            .any(|folder| state.folder_path(folder.id).as_deref() == Some("导入的空目录/空子目录"))
    );
}

#[gpui_kit::test]
async fn delayed_recent_write_rechecks_destination_after_profile_edit(cx: &mut TestAppContext) {
    let profile = Connection::new("旧目标", "old.invalid", "operator");
    let fixture = mount(cx, vec![profile.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            let mut changed = view.state.clone();
            changed.connections[0].host = "new.invalid".into();
            view.persist(changed, crate::workspace::AfterSave::None, window, cx);
            view.remember_successful_connection(profile.clone(), window, cx);
            assert_eq!(view.pending_recents.len(), 1);
        });
    })
    .checked("success of old destination arrives during changed-target save");
    saved(&fixture, cx).await;
    let state = fixture.store.load().checked("load changed destination");
    assert_eq!(state.connections[0].host, "new.invalid");
    assert!(state.recent_connections.is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.remember_successful_connection(profile, window, cx)
        });
    })
    .checked("reject stale success before queue admission");
    fixture.workspace.read_with(cx, |view, _| {
        assert!(!view.saving);
        assert!(view.pending_recents.is_empty());
    });
}

#[gpui_kit::test]
async fn saving_profile_with_manager_open_restores_manager_focus(cx: &mut TestAppContext) {
    let profile = Connection::new("管理器", "manager.invalid", "operator");
    let fixture = mount(cx, vec![profile.clone()]);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = true;
            view.edit_connection(profile, window, cx);
        });
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("save profile while manager occludes the SSH terminal");
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("manager-search", cx);
    })
    .checked("subsequent keys belong to manager search");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
}

#[gpui_kit::test]
async fn profile_metadata_keeps_recent_but_destination_change_clears_it(cx: &mut TestAppContext) {
    let profile = Connection::new("已连接", "old.invalid", "operator");
    let fixture = mount(cx, vec![profile.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.remember_successful_connection(profile.clone(), window, cx);
        });
    })
    .checked("record prior successful destination");
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.edit_connection(profile, window, cx);
            view.form
                .as_ref()
                .checked_option("profile editor")
                .name
                .update(cx, |input, cx| input.set_value("新名称", window, cx));
        });
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("rename profile without changing destination");
    saved(&fixture, cx).await;
    assert_eq!(
        fixture
            .store
            .load()
            .checked("renamed profile")
            .recent_connections
            .len(),
        1
    );
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.edit_connection(view.state.connections[0].clone(), window, cx);
            view.form
                .as_ref()
                .checked_option("profile editor")
                .host
                .update(cx, |input, cx| input.set_value("new.invalid", window, cx));
        });
        window.render_frame(cx);
        window.click("save-connection", cx);
    })
    .checked("change profile destination");
    saved(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("changed profile")
            .recent_connections
            .is_empty()
    );
}

#[gpui_kit::test]
fn closing_profile_and_manager_restore_visible_keyboard_target(cx: &mut TestAppContext) {
    let profile = Connection::new("管理器焦点", "focus.invalid", "operator");
    let fixture = mount(cx, vec![profile.clone()]);
    let panes = attach_remote_panes(&fixture, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.show_connections = true;
            view.edit_connection(profile, window, cx);
            view.close_tab(&crate::workspace::CloseTab, window, cx);
        });
        window.render_frame(cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("search", cx);
        window.click("close-manager", cx);
        assert!(
            panes[0]
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("ssh", cx);
    })
    .checked("close layers and restore the newly exposed input");
    cx.run_until_parked();
    assert_eq!(writes(&panes[0]), b"ssh");
    assert!(writes(&panes[1]).is_empty());
}

struct DelayedShell {
    gate: Arc<tokio::sync::Notify>,
    authenticating: Arc<AtomicBool>,
    received: Arc<std::sync::atomic::AtomicUsize>,
    channels: std::collections::HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>,
    jobs: tokio::task::JoinSet<()>,
}
impl russh::server::Handler for DelayedShell {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.authenticating
            .store(true, std::sync::atomic::Ordering::Release);
        self.gate.notified().await;
        Ok(
            if user == "fixture-user" && password == "delayed-test-password" {
                russh::server::Auth::Accept
            } else {
                russh::server::Auth::reject()
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<russh::server::Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        id: russh::ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        id: russh::ChannelId,
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        let Some(mut channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        let received = self.received.clone();
        self.jobs.spawn(async move {
            if channel.data(&b"READY"[..]).await.is_err() {
                return;
            }
            while let Some(message) = channel.wait().await {
                match message {
                    russh::ChannelMsg::Data { data } => {
                        received.fetch_add(data.len(), std::sync::atomic::Ordering::AcqRel);
                        let _ = channel.data(&data[..]).await;
                    }
                    russh::ChannelMsg::Close | russh::ChannelMsg::Eof => break,
                    _ => {}
                }
            }
        });
        Ok(())
    }
}

#[gpui_kit::test]
async fn asynchronous_ssh_success_preserves_folder_editor_focus(cx: &mut TestAppContext) {
    use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
    let fixture = mount(cx, vec![password_profile()]);
    let runtime = fixture
        .workspace
        .read_with(cx, |view, _| view.runtime.clone());
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .checked("bind delayed SSH fixture");
    let port = listener.local_addr().checked("fixture address").port();
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x75; 32]));
    let pin = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let gate = Arc::new(tokio::sync::Notify::new());
    let authenticating = Arc::new(AtomicBool::new(false));
    let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handler = DelayedShell {
        gate: gate.clone(),
        authenticating: authenticating.clone(),
        received: received.clone(),
        channels: Default::default(),
        jobs: tokio::task::JoinSet::new(),
    };
    let server = runtime.spawn(async move {
        if let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(20), listener.accept()).await
            && let Ok(session) = russh::server::run_stream(config, socket, handler).await
        {
            let _ = tokio::time::timeout(Duration::from_secs(20), session).await;
        }
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.state.connections[0].port = port;
            view.connect(
                view.state.connections[0].clone(),
                zeroize::Zeroizing::new("delayed-test-password".into()),
                Some(pin),
                window,
                cx,
            );
        });
    })
    .checked("start SSH while authentication is deliberately delayed");
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, _| {
        authenticating.load(std::sync::atomic::Ordering::Acquire)
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-folder", cx);
        window.render_frame(cx);
        window.input("before-", cx);
    })
    .checked("open and type in folder editor during connection");
    gate.notify_one();
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        !fixture.workspace.read(cx).connecting
    })
    .await;
    fixture.workspace.read_with(cx, |view, _| {
        assert!(
            !view.tabs.is_empty(),
            "SSH fixture did not connect: {:?}",
            view.status
        );
    });
    cx.wait_for(fixture.window, Duration::from_secs(10), |_, cx| {
        fixture
            .workspace
            .read(cx)
            .tabs
            .first()
            .is_some_and(|tab| tab.read(cx).visible_text().contains("READY"))
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let form = fixture
            .workspace
            .read(cx)
            .folder_form
            .as_ref()
            .checked_option("editor remains visible");
        assert!(form.name.read(cx).focus_handle(cx).is_focused(window));
        window.input("after", cx);
        assert_eq!(
            fixture
                .workspace
                .read(cx)
                .folder_form
                .as_ref()
                .checked_option("retained editor")
                .name
                .read(cx)
                .value(),
            "before-after"
        );
    })
    .checked("SSH completion must not steal editor keys");
    cx.run_until_parked();
    assert_eq!(received.load(std::sync::atomic::Ordering::Acquire), 0);
    saved(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("cancel-folder", cx);
        window.input("ok", cx);
    })
    .checked("closing editor intentionally restores terminal focus");
    cx.wait_for(fixture.window, Duration::from_secs(5), |_, _| {
        received.load(std::sync::atomic::Ordering::Acquire) == 2
    })
    .await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |view, cx| {
            view.close_tab(&crate::workspace::CloseTab, window, cx)
        });
    })
    .checked("close test session");
    server.abort();
}
