//! Production GPUI handlers with isolated encrypted vaults and saved references.

use super::{VaultSettings, VaultSettingsEvent};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{Connection, CredentialKind, Language, StateStore, VaultStore};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

const MASTER: &str = "maintenance-fixture-master";
const NEW_MASTER: &str = "rotated-fixture-master";
const LINKED: Uuid = Uuid::from_u128(401);
const ORPHAN: Uuid = Uuid::from_u128(402);
const OWNER: Uuid = Uuid::from_u128(403);

trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}
struct Fixture {
    path: PathBuf,
    state_path: PathBuf,
    window: AnyWindowHandle,
    panel: Entity<VaultSettings>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

fn mount(cx: &mut TestAppContext, in_use: BTreeSet<Uuid>) -> Fixture {
    let directory = std::env::temp_dir().join(format!("keelshell-maintenance-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).checked("temporary directory");
    let path = directory.join("vault.json");
    let state_path = directory.join("state.json");
    let store = VaultStore::new(&path);
    let mut vault = store.load(MASTER).checked("create vault");
    vault
        .set(
            LINKED,
            OWNER,
            CredentialKind::Password,
            "never-display-linked-secret",
        )
        .checked("linked credential");
    vault
        .set(
            ORPHAN,
            OWNER,
            CredentialKind::AiApiKey,
            "never-display-api-key",
        )
        .checked("orphan credential");
    store.save(&mut vault).checked("save fixture");
    let states = StateStore::new(&state_path);
    let state = states.load().checked("load empty state");
    states.save(&state).checked("save state");
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .checked("runtime"),
    );
    let panel_path = path.clone();
    let panel_state = state_path.clone();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1100.), px(820.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    VaultSettings::new(
                        panel_path,
                        panel_state,
                        runtime,
                        in_use,
                        BTreeMap::from([(OWNER, "测试配置".to_owned())]),
                        window,
                        cx,
                    )
                })
            },
        )
        .checked("mount vault maintenance")
    });
    Fixture {
        path,
        state_path,
        window,
        panel,
    }
}

fn set_master(fixture: &Fixture, value: &str, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.panel.update(cx, |panel, cx| {
            panel.master.update(cx, |input, cx| {
                input.set_value(value.to_owned(), window, cx)
            })
        })
    })
    .checked("enter master");
}
async fn wait(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        !fixture.panel.read(cx).is_busy()
    })
    .await;
}
async fn inspect(fixture: &Fixture, cx: &mut TestAppContext) {
    set_master(fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("vault-inspect", cx);
        assert!(fixture.panel.read(cx).master.read(cx).value().is_empty());
    })
    .checked("click inspect");
    wait(fixture, cx).await;
}

#[gpui_kit::test]
async fn inspection_protects_linked_rows_confirms_orphan_deletion_and_locks(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, BTreeSet::from([LINKED]));
    inspect(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(fixture.panel.read(cx).inspected);
        assert_eq!(fixture.panel.read(cx).entries.len(), 2);
        window.render_frame(cx);
        window.click(("vault-delete-entry", 0_usize), cx);
        assert!(fixture.panel.read(cx).deletion.is_none());
        window.click(("vault-delete-entry", 1_usize), cx);
        assert_eq!(fixture.panel.read(cx).deletion, Some(ORPHAN));
        assert!(!fixture.panel.read(cx).is_busy());
    })
    .checked("protected row and destructive confirmation");
    assert_eq!(
        VaultStore::new(&fixture.path)
            .load(MASTER)
            .checked("before confirmation")
            .len(),
        2
    );
    set_master(&fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("vault-confirm-delete", cx);
    })
    .checked("confirm deletion");
    wait(&fixture, cx).await;
    let vault = VaultStore::new(&fixture.path)
        .load(MASTER)
        .checked("after deletion");
    assert_eq!(vault.len(), 1);
    assert!(vault.get(ORPHAN, OWNER, CredentialKind::AiApiKey).is_err());
    assert_eq!(
        vault
            .get(LINKED, OWNER, CredentialKind::Password)
            .checked("linked retained")
            .as_str(),
        "never-display-linked-secret"
    );
    cx.update_window(fixture.window, |_, window, cx| {
        assert!(fixture.panel.read(cx).status.render(cx).contains("已删除"));
        crate::i18n::set_language(Language::En, cx);
        fixture
            .panel
            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
        assert!(fixture.panel.read(cx).status.render(cx).contains("deleted"));
        window.render_frame(cx);
        window.click("vault-lock", cx);
        assert!(!fixture.panel.read(cx).inspected);
        assert!(fixture.panel.read(cx).entries.is_empty());
    })
    .checked("language and lock");
}

#[gpui_kit::test]
async fn rotation_rejects_mismatch_then_preserves_references_and_uses_new_password(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, BTreeSet::new());
    inspect(&fixture, cx).await;
    let original = std::fs::read(&fixture.path).checked("original bytes");
    set_master(&fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.panel.update(cx, |panel, cx| {
            panel
                .replacement
                .update(cx, |input, cx| input.set_value(NEW_MASTER, window, cx));
            panel
                .confirmation
                .update(cx, |input, cx| input.set_value("mismatch", window, cx));
        });
        window.render_frame(cx);
        window.click("vault-rotate", cx);
        assert!(!fixture.panel.read(cx).is_busy());
        assert!(fixture.panel.read(cx).status.render(cx).contains("一致"));
    })
    .checked("reject mismatch");
    assert_eq!(
        std::fs::read(&fixture.path).checked("unchanged bytes"),
        original
    );
    set_master(&fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.panel.update(cx, |panel, cx| {
            for input in [&panel.replacement, &panel.confirmation] {
                input.update(cx, |input, cx| input.set_value(NEW_MASTER, window, cx));
            }
        });
        window.render_frame(cx);
        window.click("vault-rotate", cx);
    })
    .checked("rotate");
    wait(&fixture, cx).await;
    assert!(VaultStore::new(&fixture.path).load(MASTER).is_err());
    let vault = VaultStore::new(&fixture.path)
        .load(NEW_MASTER)
        .checked("new master works");
    assert_eq!(
        vault
            .entries()
            .map(|entry| entry.reference)
            .collect::<Vec<_>>(),
        vec![LINKED, ORPHAN]
    );
    cx.update(|cx| {
        assert!(fixture.panel.read(cx).status.render(cx).contains("已更改"));
        assert!(fixture.panel.read(cx).master.read(cx).value().is_empty());
        assert!(
            fixture
                .panel
                .read(cx)
                .replacement
                .read(cx)
                .value()
                .is_empty()
        );
    });
}

#[gpui_kit::test]
async fn disk_reference_refresh_refuses_deletion_and_wrong_master_cannot_inspect(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, BTreeSet::new());
    set_master(&fixture, "wrong", cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("vault-inspect", cx);
    })
    .checked("wrong master");
    wait(&fixture, cx).await;
    cx.update(|cx| {
        assert!(!fixture.panel.read(cx).inspected);
        assert!(fixture.panel.read(cx).entries.is_empty());
    });
    inspect(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("vault-delete-entry", 1_usize), cx);
    })
    .checked("request orphan deletion");
    // Another saved state now references the previously unlinked entry.
    let store = StateStore::new(&fixture.state_path);
    let mut state = store.load().checked("state");
    let mut connection = Connection::new("protected", "127.0.0.1", "fixture");
    connection.credential_ref = Some(ORPHAN);
    state.connections.push(connection);
    store.save(&state).checked("external state update");
    set_master(&fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("vault-confirm-delete", cx);
    })
    .checked("confirm stale unlinked view");
    wait(&fixture, cx).await;
    assert_eq!(
        VaultStore::new(&fixture.path)
            .load(MASTER)
            .checked("preserved vault")
            .len(),
        2
    );
    cx.update(|cx| {
        assert!(
            fixture
                .panel
                .read(cx)
                .status
                .render(cx)
                .contains("不能删除")
        )
    });
}

#[gpui_kit::test]
async fn busy_close_cancels_queued_rotation_and_keeps_modal_until_worker_returns(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, BTreeSet::new());
    inspect(&fixture, cx).await;
    let before = std::fs::read(&fixture.path).checked("original bytes");
    // Hold the only blocking worker so cancellation deterministically precedes
    // save admission even on slow or heavily loaded target-native CI machines.
    let (release, held) = std::sync::mpsc::sync_channel(1);
    let (started, ready) = std::sync::mpsc::sync_channel(1);
    let runtime = cx.update(|cx| fixture.panel.read(cx).runtime.clone());
    let blocker = runtime.spawn_blocking(move || {
        started.send(()).checked("worker started");
        held.recv_timeout(Duration::from_secs(10))
            .checked("release worker");
    });
    ready
        .recv_timeout(Duration::from_secs(5))
        .checked("blocking worker is occupied");
    let closed = Arc::new(AtomicBool::new(false));
    let observed = closed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&fixture.panel, move |_, event: &VaultSettingsEvent, _| {
            if matches!(event, VaultSettingsEvent::Close { .. }) {
                observed.store(true, Ordering::Release);
            }
        })
    });
    set_master(&fixture, MASTER, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        fixture.panel.update(cx, |panel, cx| {
            for field in [&panel.replacement, &panel.confirmation] {
                field.update(cx, |input, cx| input.set_value(NEW_MASTER, window, cx));
            }
        });
        window.click("vault-rotate", cx);
        window.render_frame(cx);
        window.click("vault-close", cx);
        assert!(fixture.panel.read(cx).is_busy());
        assert!(fixture.panel.read(cx).close_after_work);
        assert!(!closed.load(Ordering::Acquire));
    })
    .checked("close waits for worker");
    release.send(()).checked("release queued rotation");
    wait(&fixture, cx).await;
    assert!(blocker.is_finished());
    assert!(closed.load(Ordering::Acquire));
    assert_eq!(
        std::fs::read(&fixture.path).checked("unchanged vault"),
        before
    );
    cx.update(|cx| assert!(fixture.panel.read(cx).master.read(cx).value().is_empty()));
}

#[gpui_kit::test]
async fn repeated_inspection_refreshes_disk_links_without_forgetting_workspace_protection(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, BTreeSet::from([LINKED]));
    let store = StateStore::new(&fixture.state_path);
    let mut state = store.load().checked("load state");
    let mut connection = Connection::new("external", "127.0.0.1", "fixture");
    connection.credential_ref = Some(ORPHAN);
    state.connections.push(connection);
    store.save(&state).checked("link external credential");
    inspect(&fixture, cx).await;
    cx.update(|cx| {
        assert_eq!(
            fixture.panel.read(cx).in_use,
            BTreeSet::from([LINKED, ORPHAN])
        )
    });
    state = store.load().checked("reload fresh state snapshot");
    state.connections[0].credential_ref = None;
    store.save(&state).checked("unlink external credential");
    inspect(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        assert_eq!(fixture.panel.read(cx).in_use, BTreeSet::from([LINKED]));
        window.render_frame(cx);
        window.click(("vault-delete-entry", 1_usize), cx);
        assert_eq!(fixture.panel.read(cx).deletion, Some(ORPHAN));
    })
    .checked("freshly unlinked entry can be reviewed for deletion");
}

#[gpui_kit::test]
async fn busy_close_delivers_completed_mutation_and_uncertain_durability_status(
    cx: &mut TestAppContext,
) {
    use super::{Action, Failure, Report};
    use keelshell_core::Error;
    use std::sync::Mutex;
    let fixture = mount(cx, BTreeSet::new());
    let messages = Arc::new(Mutex::new(Vec::new()));
    let observed = messages.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&fixture.panel, move |_, event: &VaultSettingsEvent, _| {
            let VaultSettingsEvent::Close { message } = event;
            observed.lock().checked("messages").push(message.clone());
        })
    });
    for (action, outcome, text) in [
        (
            Action::Rotate,
            Ok(Report::Ready {
                entries: Vec::new(),
                in_use: BTreeSet::new(),
            }),
            "主密码已更改",
        ),
        (
            Action::Delete(ORPHAN),
            Ok(Report::Ready {
                entries: Vec::new(),
                in_use: BTreeSet::new(),
            }),
            "凭据已删除",
        ),
        (
            Action::Rotate,
            Err(Failure::Core(Error::Durability(std::io::Error::other(
                "fixture sync failure",
            )))),
            "新主密码可能已生效",
        ),
        (Action::Rotate, Ok(Report::Cancelled), "未开始保存"),
    ] {
        cx.update_window(fixture.window, |_, window, cx| {
            fixture.panel.update(cx, |panel, cx| {
                panel.cancellation = Some(Arc::new(AtomicBool::new(false)));
                panel.close_after_work = true;
                panel.complete(outcome, action, window, cx);
            });
        })
        .checked("deliver real completion callback");
        cx.update(|cx| {
            let messages = messages.lock().checked("messages");
            assert!(
                messages
                    .last()
                    .and_then(Option::as_ref)
                    .is_some_and(|message| message.render(cx).contains(text))
            );
            assert!(!fixture.panel.read(cx).is_busy());
        });
    }
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .panel
            .update(cx, |panel, cx| panel.close(window, cx))
    })
    .checked("ordinary close");
    assert!(
        messages
            .lock()
            .checked("messages")
            .last()
            .is_some_and(Option::is_none)
    );
}
