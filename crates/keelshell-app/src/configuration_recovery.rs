//! Explicit metadata backup/recovery; every filesystem operation uses a worker.
use crate::i18n::{Message, t};
use gpui_kit::{
    base::Selectable,
    component::{
        Disableable,
        button::{Button, ButtonVariants},
    },
    *,
};
use keelshell_core::{
    AppState, ConfigBackup, ConfigBackupId, ConfigBackupStatus, ConfigRecoveryPreview,
    ConfigRecoverySummary, ConfigSourceStatus, Error, StateStore,
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub(crate) enum ConfigRecoveryEvent {
    RequestRestore { generation: u64 },
    Restored(Box<AppState>),
    Close,
}

pub(crate) struct ConfigRecoveryPanel {
    store: Arc<StateStore>,
    backups: Vec<ConfigBackup>,
    preview: Option<ConfigRecoveryPreview>,
    generation: u64,
    acknowledged: bool,
    busy: bool,
    close_after_work: bool,
    failure: Option<Error>,
    status: Message,
    _job: Option<Task<()>>,
}

impl EventEmitter<ConfigRecoveryEvent> for ConfigRecoveryPanel {}

enum Work {
    List,
    Snapshot,
    Preview(ConfigBackupId),
    Restore(ConfigRecoveryPreview),
}

enum Report {
    Listed(Vec<ConfigBackup>),
    Previewed(ConfigRecoveryPreview),
    Restored(Box<AppState>),
}

impl ConfigRecoveryPanel {
    #[cfg(test)]
    pub(crate) fn pending_for_test(&self) -> bool {
        self.busy
    }

    #[cfg(test)]
    pub(crate) fn status_for_test(&self, cx: &App) -> String {
        self.status.render(cx)
    }

    pub(crate) fn new(store: Arc<StateStore>, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            store,
            backups: Vec::new(),
            preview: None,
            generation: 0,
            acknowledged: false,
            busy: false,
            close_after_work: false,
            failure: None,
            status: Message::new(
                "正在读取本机配置备份…",
                "Reading local configuration backups…",
            ),
            _job: None,
        };
        panel.submit(Work::List, cx);
        panel
    }

    pub(crate) fn close(&mut self, cx: &mut Context<Self>) {
        self.acknowledged = false;
        if self.busy {
            // A dispatched atomic write is not cancellable. Keep the owner mounted
            // until its real result is delivered, then close after state propagation.
            self.close_after_work = true;
            self.status = Message::new(
                "正在等待本次后台操作结束，随后关闭。",
                "Waiting for this background operation to finish before closing.",
            );
            cx.notify();
        } else {
            cx.emit(ConfigRecoveryEvent::Close);
        }
    }

    pub(crate) fn restore_approved(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.busy || !self.acknowledged || self.generation != generation {
            return;
        }
        let Some(preview) = self.preview.clone() else {
            return;
        };
        self.submit(Work::Restore(preview), cx);
    }

    pub(crate) fn refuse_restore(&mut self, cx: &mut Context<Self>) {
        self.acknowledged = false;
        self.status = Message::new(
            "工作区状态已变化。请先关闭 SSH 标签、停止 AI 和外部 MCP 操作，再重新审核。",
            "The workspace changed. Close SSH tabs and stop AI and external MCP work, then review again.",
        );
        cx.notify();
    }

    fn submit(&mut self, work: Work, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.generation = self.generation.wrapping_add(1);
        self.acknowledged = false;
        self.preview = None;
        self.failure = None;
        self.status = Message::new(
            "正在后台验证配置…",
            "Validating configuration in the background…",
        );
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            match work {
                Work::List => store.config_backups().map(Report::Listed),
                Work::Snapshot => {
                    store.create_config_backup()?;
                    store.config_backups().map(Report::Listed)
                }
                Work::Preview(id) => store.preview_config_backup(id).map(Report::Previewed),
                Work::Restore(preview) => store
                    .restore_config_backup(&preview)
                    .map(|state| Report::Restored(Box::new(state))),
            }
        });
        self._job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| {
                panel.finish_work(result, cx);
            });
        }));
        cx.notify();
    }

    fn finish_work(&mut self, result: Result<Report, Error>, cx: &mut Context<Self>) {
        self.busy = false;
        self.failure = None;
        match result {
            Ok(Report::Listed(backups)) => {
                self.backups = backups;
                self.status = Message::new(
                    "请选择一份备份并查看影响；读取和选择都不会恢复。",
                    "Select a backup and review its impact. Reading and selection do not restore it.",
                );
            }
            Ok(Report::Previewed(preview)) => {
                self.preview = Some(preview);
                self.status = Message::new(
                    "已验证所选备份。确认替换影响后，才可明确恢复。",
                    "The selected backup is validated. Acknowledge replacement before restoring explicitly.",
                );
            }
            Ok(Report::Restored(state)) => {
                self.status = Message::new(
                    "配置已恢复；原配置文件存在时已保留完整副本。没有连接主机或重放操作。",
                    "Configuration restored; the exact original was preserved if a configuration file existed. No host was connected and no operation replayed.",
                );
                cx.emit(ConfigRecoveryEvent::Restored(state));
            }
            Err(error) => {
                // A previous close request is not acknowledgement of an
                // outcome that did not exist yet. Retain the typed cause
                // and its mounted UI, including uncertain rollback state.
                self.close_after_work = false;
                self.status = failure_message(&error);
                self.failure = Some(error);
            }
        }
        if self.close_after_work {
            cx.emit(ConfigRecoveryEvent::Close);
        }
        cx.notify();
    }
}

fn failure_message(error: &Error) -> Message {
    let (zh, en) = match error {
        Error::ConfigRecoveryRequired => (
            "恢复后无法确认回滚，需要人工修复。原配置文件存在时已保留完整副本；请检查配置位置，不要把当前状态视为完整恢复。",
            "Rollback could not be confirmed after replacement; manual repair is required. If the reviewed configuration file existed, its exact bytes were preserved. Inspect the configuration location; do not treat current state as a completed recovery.",
        ),
        Error::ConfigRecoveryRolledBack(_) => (
            "本次恢复未完成，已确认回滚到操作前状态。原配置文件存在时已保留完整副本；请检查配置位置并重新审核。",
            "Recovery did not complete; rollback to the prior state was confirmed. If the reviewed configuration file existed, its exact bytes were preserved. Inspect the configuration location and review again.",
        ),
        Error::ConfigRecoveryConflict => (
            "恢复审核已失效，当前配置未被替换。请重新读取并重新审核。",
            "The recovery review is stale; current configuration was not replaced. Refresh and review again.",
        ),
        _ => (
            "配置操作未完成；请保留文件并重新检查。",
            "Configuration operation did not complete; preserve the files and inspect again.",
        ),
    };
    Message::detail(zh, en, error)
}

fn current_status(status: ConfigSourceStatus, cx: &App) -> String {
    match status {
        ConfigSourceStatus::Missing => {
            t(cx, "当前配置文件缺失", "Current configuration is missing").into()
        }
        ConfigSourceStatus::Valid => t(
            cx,
            "当前配置有效，将保存其完整原件",
            "Current configuration is valid; its exact original will be preserved",
        )
        .into(),
        ConfigSourceStatus::Corrupt => t(
            cx,
            "当前配置损坏，将保存其完整原件",
            "Current configuration is damaged; its exact original will be preserved",
        )
        .into(),
        ConfigSourceStatus::UnsupportedSchema(version) => format!(
            "{} {version} · {}",
            t(cx, "当前配置版本", "Current schema"),
            t(
                cx,
                "此程序无法读取，恢复会替换它并保留完整原件",
                "This build cannot read it; recovery replaces it and preserves the exact original"
            )
        ),
    }
}

fn summary_label(title: &str, summary: Option<ConfigRecoverySummary>, cx: &App) -> SharedString {
    let counts = match summary {
        Some(summary) => format!(
            "{} {} · {} {} · {} {} · {} {} · AI {} · {} {}",
            t(cx, "连接", "Profiles"),
            summary.connections,
            t(cx, "回收站", "Trash"),
            summary.deleted_connections,
            t(cx, "文件夹", "Folders"),
            summary.folders,
            t(cx, "片段", "Snippets"),
            summary.snippets,
            summary.ai_profiles,
            t(cx, "信任", "Trust"),
            summary.trusted_hosts
        ),
        None => t(cx, "无法读取数量", "Counts unavailable").to_owned(),
    };
    format!("{title} · {counts}").into()
}

impl Render for ConfigRecoveryPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let busy = self.busy;
        let generation = self.generation;
        let mut body = div().id("configuration-recovery-body").test_support()
            .flex_1().min_h_0().overflow_y_scroll().p_4().flex().flex_col().gap_3()
            .child(div().text_sm().text_color(rgb(visual.muted)).child(t(cx,
                "本机保存前保留最多 8 份有效配置，包含连接、文件夹、片段、信任和偏好。凭据只有引用：不复制密码、私钥、OS 凭据或加密凭据库。请避免在片段或名称中填写秘密。",
                "Up to eight valid local metadata snapshots are retained before saves, including profiles, folders, snippets, trust and preferences. Credentials are references only: passwords, private keys, OS credentials and encrypted vaults are not copied. Avoid secrets in snippets or names.")))
            .child(div().text_xs().child(self.store.path().to_string_lossy().to_string()))
            .child(div().id("configuration-recovery-status").test_support().text_sm().text_color(rgb(visual.accent)).child(self.status.render(cx)));
        if self.failure.is_some() {
            body = body.child(div().id("configuration-recovery-failure").test_support()
                .p_3().rounded_lg().border_1().border_color(rgb(visual.danger_border))
                .bg(rgb(visual.danger_surface)).text_color(rgb(visual.danger))
                .child(t(cx,
                    "本次操作失败，已取消此前的自动关闭请求。请阅读上方实际结果，再明确关闭或重新审核。",
                    "This operation failed, so the earlier deferred close was cancelled. Read the actual result above before closing explicitly or reviewing again.")));
        }
        if self.backups.is_empty() {
            body = body.child(t(cx, "尚无备份；首次保存没有旧版本，可明确创建当前配置备份。", "No backups yet. The first save has no previous version; you can explicitly snapshot the current configuration."));
        }
        for (index, backup) in self.backups.iter().enumerate() {
            let id = backup.id;
            let available = backup.status == ConfigBackupStatus::Available;
            let timestamp = backup
                .modified_unix_millis
                .and_then(|millis| i64::try_from(millis).ok())
                .and_then(chrono::DateTime::from_timestamp_millis)
                .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| t(cx, "时间未知", "Time unavailable").into());
            let label = format!(
                "{} · #{} · {} {} · {}",
                timestamp,
                backup
                    .sequence
                    .map(|sequence| sequence.to_string())
                    .unwrap_or_else(|| "?".into()),
                backup.bytes,
                t(cx, "字节", "bytes"),
                match backup.status {
                    ConfigBackupStatus::Available => t(cx, "可恢复", "Available").to_owned(),
                    ConfigBackupStatus::Invalid =>
                        t(cx, "损坏，不能恢复", "Invalid; cannot restore").to_owned(),
                    ConfigBackupStatus::UnsupportedSchema(version) =>
                        format!("{} {version}", t(cx, "不支持的版本", "Unsupported schema")),
                }
            );
            body = body.child(
                Button::new(("configuration-backup", index))
                    .ghost()
                    .label(label)
                    .selected(
                        self.preview
                            .as_ref()
                            .is_some_and(|preview| preview.backup_id() == id),
                    )
                    .disabled(busy || !available)
                    .on_click(
                        cx.listener(move |panel, _, _, cx| panel.submit(Work::Preview(id), cx)),
                    ),
            );
        }
        if let Some(preview) = &self.preview {
            let current = summary_label(
                t(cx, "当前配置", "Current configuration"),
                preview.current_summary(),
                cx,
            );
            let replacement = summary_label(
                t(
                    cx,
                    "恢复后配置（所选备份）",
                    "After recovery (selected backup)",
                ),
                Some(preview.summary()),
                cx,
            );
            body = body.child(div().id("configuration-recovery-preview").test_support()
                .rounded_lg().p_3().bg(rgb(visual.canvas)).flex().flex_col().gap_2()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(current_status(preview.current_status(), cx)))
                .child(div().id("configuration-recovery-current-summary").test_support().text_sm()
                    .role(accesskit::Role::Label).aria_label(current.clone()).child(current))
                .child(div().id("configuration-recovery-replacement-summary").test_support().text_sm()
                    .role(accesskit::Role::Label).aria_label(replacement.clone()).child(replacement))
                .child(div().text_sm().text_color(rgb(visual.muted)).child(t(cx,
                    "恢复将替换本机配置和偏好。旧 AI 上下文与临时凭据、外部 MCP 授权会失效；旧命令草稿不执行。OS 凭据库和加密 vault 保持独立，缺少的凭据须重新提供。原配置文件存在时，会在 state.json.originals 中保留独立副本；原文件不存在时不产生副本。最多保留 8 份且不会自动删除；达到上限须先手动归档。",
                    "Recovery replaces local metadata and preferences. Old AI context, temporary credentials and external MCP grants are invalidated; command drafts are not executed. OS credentials and the encrypted vault remain independent; missing credentials must be supplied again. If a configuration file exists, its exact original is preserved separately in state.json.originals; a missing file produces no copy. Up to eight originals are kept and never deleted automatically. Archive them manually when full.")))
                .child(Button::new("configuration-recovery-acknowledge").ghost()
                    .label(t(cx, "我已审核替换影响并同意恢复所选备份", "I reviewed the replacement and approve this selected backup"))
                    .selected(self.acknowledged).disabled(busy)
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if !panel.busy && panel.generation == generation && panel.preview.is_some() {
                            panel.acknowledged = !panel.acknowledged;
                            cx.notify();
                        }
                    }))));
        }
        div()
            .id("configuration-recovery-panel")
            .test_support()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_4()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(t(
                        cx,
                        "配置备份与恢复",
                        "Configuration backup and recovery",
                    ))),
            )
            .child(body)
            .child(
                div()
                    .id("configuration-recovery-footer")
                    .test_support()
                    .p_3()
                    .flex_shrink_0()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(visual.border))
                    .child(
                        Button::new("configuration-recovery-refresh")
                            .ghost()
                            .label(t(cx, "重新读取", "Refresh"))
                            .disabled(busy)
                            .on_click(cx.listener(|panel, _, _, cx| panel.submit(Work::List, cx))),
                    )
                    .child(
                        Button::new("configuration-recovery-snapshot")
                            .ghost()
                            .label(t(
                                cx,
                                "备份当前有效配置",
                                "Back up current valid configuration",
                            ))
                            .disabled(busy)
                            .on_click(
                                cx.listener(|panel, _, _, cx| panel.submit(Work::Snapshot, cx)),
                            ),
                    )
                    .child(
                        Button::new("configuration-recovery-restore")
                            .primary()
                            .label(t(cx, "明确恢复所选备份", "Restore selected backup"))
                            .disabled(busy || !self.acknowledged || self.preview.is_none())
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                if !panel.busy
                                    && panel.acknowledged
                                    && panel.generation == generation
                                    && panel.preview.is_some()
                                {
                                    cx.emit(ConfigRecoveryEvent::RequestRestore { generation });
                                }
                            })),
                    )
                    .child(
                        Button::new("configuration-recovery-location")
                            .ghost()
                            .label(t(cx, "打开配置位置", "Show configuration location"))
                            .on_click(
                                cx.listener(|panel, _, _, cx| cx.reveal_path(panel.store.path())),
                            ),
                    )
                    .child(
                        Button::new("configuration-recovery-close")
                            .ghost()
                            .label(t(cx, "关闭", "Close"))
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    ),
            )
    }
}
