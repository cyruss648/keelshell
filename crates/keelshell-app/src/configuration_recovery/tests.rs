//! Typed completion controls test rendered feedback, not real disk-sync failure.
use super::{ConfigRecoveryEvent, ConfigRecoveryPanel, Error};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{AppState, Connection, Language, StateStore, Theme};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}

impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, operation: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{operation}: {error:?}"),
        }
    }
}

async fn mount(
    cx: &mut TestAppContext,
) -> (
    tempfile::TempDir,
    AnyWindowHandle,
    Entity<ConfigRecoveryPanel>,
) {
    let temporary = tempfile::tempdir().checked("isolated metadata directory");
    let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
    store
        .save(&AppState::default())
        .checked("seed valid metadata");
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|cx| ConfigRecoveryPanel::new(store, cx)),
        )
        .checked("mount production recovery panel")
    });
    cx.wait_for(window, Duration::from_secs(10), |_, cx| {
        !panel.read(cx).busy
    })
    .await;
    (temporary, window, panel)
}

async fn typed_failure_stays_visible(error: Error, cx: &mut TestAppContext, required: bool) {
    let (_temporary, window, panel) = mount(cx).await;
    let closed = Arc::new(AtomicUsize::new(0));
    let count = closed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&panel, move |_, event: &ConfigRecoveryEvent, _| {
            if matches!(event, ConfigRecoveryEvent::Close) {
                count.fetch_add(1, Ordering::SeqCst);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            // Only the UI completion boundary is controlled. The production
            // core rollback controls exercise filesystem failures separately.
            panel.busy = true;
            panel.close(cx);
            assert!(panel.close_after_work);
            panel.finish_work(Err(error), cx);
            assert!(!panel.close_after_work);
            assert!(matches!(
                &panel.failure,
                Some(Error::ConfigRecoveryRequired) | Some(Error::ConfigRecoveryRolledBack(_))
            ));
        });
        window.render_frame(cx);
        assert!(window.find("configuration-recovery-failure").visible());
    })
    .checked("deliver exact typed error through the real completion handler");
    cx.run_until_parked();
    assert_eq!(
        closed.load(Ordering::SeqCst),
        0,
        "the actual failure must not emit deferred Close"
    );
    for (language, theme, fragment) in [
        (
            Language::ZhCn,
            Theme::Light,
            if required {
                "需要人工修复"
            } else {
                "已确认回滚"
            },
        ),
        (
            Language::En,
            Theme::Dark,
            if required {
                "manual repair is required"
            } else {
                "rollback to the prior state was confirmed"
            },
        ),
    ] {
        cx.update_window(window, |_, window, cx| {
            crate::i18n::set_language(language, cx);
            crate::design::apply(theme, Some(window), cx);
            assert!(panel.read(cx).status_for_test(cx).contains(fragment));
            window.render_frame(cx);
            assert!(window.find("configuration-recovery-failure").visible());
        })
        .checked("typed rollback status remains visible in both locales and themes");
    }
    cx.update_window(window, |_, window, cx| {
        window.click("configuration-recovery-close", cx);
    })
    .checked("only a new explicit close acknowledges the rendered failure");
    cx.run_until_parked();
    assert_eq!(closed.load(Ordering::SeqCst), 1);
    cx.update(|cx| crate::i18n::set_language(Language::ZhCn, cx));
}

#[gpui_kit::test]
async fn uncertain_rollback_keeps_typed_manual_repair_status_after_deferred_close(
    cx: &mut TestAppContext,
) {
    typed_failure_stays_visible(Error::ConfigRecoveryRequired, cx, true).await;
}

#[gpui_kit::test]
async fn confirmed_rollback_keeps_distinct_typed_failure_after_deferred_close(
    cx: &mut TestAppContext,
) {
    typed_failure_stays_visible(
        Error::ConfigRecoveryRolledBack(std::io::Error::other(
            "controlled post-commit sync failure",
        )),
        cx,
        false,
    )
    .await;
}

#[gpui_kit::test]
async fn recovery_review_distinguishes_current_and_replacement_counts_without_defaulting_damage(
    cx: &mut TestAppContext,
) {
    let (temporary, window, panel) = mount(cx).await;
    let store = panel.read_with(cx, |panel, _| panel.store.clone());
    let mut selected = store.load().checked("load selected configuration");
    selected.snippets.clear();
    selected
        .connections
        .push(Connection::new("selected", "selected.test", "fixture"));
    let selected = store.save(&selected).checked("save selected configuration");
    let backup = store
        .create_config_backup()
        .checked("create selected backup");
    let mut current = selected;
    current
        .connections
        .push(Connection::new("current only", "current.test", "fixture"));
    let current = store
        .save(&current)
        .checked("save distinct current configuration");
    let valid = std::fs::read(store.path()).checked("read complete valid current bytes");
    let backup_path = temporary
        .path()
        .join(format!("state.json.backups/{}.json", backup.uuid()));
    let backup_bytes = std::fs::read(&backup_path).checked("read complete selected backup");
    assert_eq!(current.connections.len(), 2);
    for (bytes, readable) in [
        (Some(valid.as_slice()), true),
        (Some(b"broken".as_slice()), false),
        (Some(b"{\"schema_version\":999}".as_slice()), false),
        (None, false),
    ] {
        match bytes {
            Some(bytes) => std::fs::write(store.path(), bytes).checked("controlled current bytes"),
            None => std::fs::remove_file(store.path()).checked("controlled missing current file"),
        }
        cx.update(|cx| {
            panel.update(cx, |panel, cx| {
                panel.submit(super::Work::Preview(backup), cx)
            })
        });
        cx.wait_for(window, Duration::from_secs(10), |_, cx| {
            !panel.read(cx).busy
        })
        .await;
        for (language, theme) in [(Language::ZhCn, Theme::Light), (Language::En, Theme::Dark)] {
            cx.update_window(window, |_, window, cx| {
                crate::i18n::set_language(language, cx);
                crate::design::apply(theme, Some(window), cx);
                window.render_frame(cx);
                let expected_current = match (language, readable) {
                    (Language::ZhCn, true) => "当前配置 · 连接 2 · 回收站 0 · 文件夹 0 · 片段 0 · AI 0 · 信任 0",
                    (Language::En, true) => "Current configuration · Profiles 2 · Trash 0 · Folders 0 · Snippets 0 · AI 0 · Trust 0",
                    (Language::ZhCn, false) => "当前配置 · 无法读取数量",
                    (Language::En, false) => "Current configuration · Counts unavailable",
                };
                let expected_replacement = match language {
                    Language::ZhCn => "恢复后配置（所选备份） · 连接 1 · 回收站 0 · 文件夹 0 · 片段 0 · AI 0 · 信任 0",
                    Language::En => "After recovery (selected backup) · Profiles 1 · Trash 0 · Folders 0 · Snippets 0 · AI 0 · Trust 0",
                };
                for (id, expected) in [
                    ("configuration-recovery-current-summary", expected_current),
                    ("configuration-recovery-replacement-summary", expected_replacement),
                ] {
                    let row = window.find(id);
                    assert_eq!(row.label(), Some(expected));
                    assert!(row.visible());
                }
                assert!(!panel.read(cx).acknowledged, "preview never approves recovery");
            }).checked("render separate current and replacement counts in both locales");
        }
        assert_eq!(std::fs::read(store.path()).ok().as_deref(), bytes);
        assert_eq!(
            std::fs::read(&backup_path).checked("selected backup preserved"),
            backup_bytes
        );
        assert!(!temporary.path().join("state.json.originals").exists());
    }
    cx.update(|cx| crate::i18n::set_language(Language::ZhCn, cx));
}
