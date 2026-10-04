use super::*;
use keelshell_core::{AppState, AuthMethod, OpenSshImportReport};

/// Candidate state and parser report held until the user explicitly confirms an
/// OpenSSH clipboard import. The candidate is never persisted while this value
/// is only displayed in the review modal.
pub(super) struct OpenSshImportReview {
    pub(super) candidate: AppState,
    pub(super) report: OpenSshImportReport,
}

impl OpenSshImportReview {
    pub(super) fn new(candidate: AppState, report: OpenSshImportReport) -> Self {
        Self { candidate, report }
    }
}

fn auth_label(auth: &AuthMethod, cx: &App) -> &'static str {
    match auth {
        AuthMethod::Agent => t(cx, "Agent", "Agent"),
        AuthMethod::Password => t(cx, "密码（导入后询问）", "Password (prompt after import)"),
        AuthMethod::PrivateKey { .. } => t(cx, "私钥", "Private key"),
    }
}

fn warning_label(reason: &str, cx: &App) -> &'static str {
    match reason {
        "wildcard Host block was skipped" => {
            t(cx, "通配 Host 块已跳过", "Wildcard Host block was skipped")
        }
        "Host pattern block was skipped" => {
            t(cx, "Host 模式块已跳过", "Host pattern block was skipped")
        }
        "Match block was skipped" => t(cx, "Match 条件块已跳过", "Match block was skipped"),
        "Include directive was skipped because its exact content was not supplied" => t(
            cx,
            "Include 未导入：没有提供对应的精确内容",
            "Include skipped: exact content was not supplied",
        ),
        "Include path pattern was skipped" => t(
            cx,
            "Include 路径模式已跳过",
            "Include path pattern was skipped",
        ),
        "Include inside a Host or ignored block was skipped" => t(
            cx,
            "Host 块中的 Include 已跳过（仅支持全局 Include）",
            "Include inside a Host block was skipped (only global Include is supported)",
        ),
        "connection-semantic directive was skipped" => t(
            cx,
            "可能改变连接语义的指令已跳过",
            "Connection-semantic directive was skipped",
        ),
        "unsupported directive was skipped" => t(
            cx,
            "未支持的指令已跳过",
            "Unsupported directive was skipped",
        ),
        "duplicate directive was ignored after the first value" => t(
            cx,
            "重复指令已跳过（采用首次出现的值）",
            "Duplicate directive skipped (the first value is used)",
        ),
        _ => t(
            cx,
            "未支持的配置语义已跳过",
            "Unsupported configuration semantics were skipped",
        ),
    }
}

impl Workspace {
    pub(super) fn openssh_import_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(review) = &self.openssh_review else {
            return div().into_any_element();
        };
        let report = &review.report;
        let entries = div()
            .id("openssh-import-entries")
            .test_support()
            .flex()
            .flex_col()
            .gap_1()
            .children(report.entries.iter().enumerate().map(|(index, entry)| {
                let connection = &entry.connection;
                let route = entry
                    .proxy_jump
                    .as_deref()
                    .map(|jump| {
                        format!(
                            "{} · {} {}",
                            t(cx, "跳板", "Jump"),
                            jump,
                            t(cx, "→ 目标", "→ target")
                        )
                    })
                    .unwrap_or_else(|| t(cx, "直连", "Direct").to_owned());
                div()
                    .id(("openssh-import-entry", index))
                    .test_support()
                    .p_2()
                    .rounded(px(6.))
                    .bg(rgb(visual.canvas))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .w(px(24.))
                            .text_color(rgb(visual.accent))
                            .child((index + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(entry.alias.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(rgb(visual.muted))
                                    .child(format!(
                                        "{}@{}:{}",
                                        connection.username, connection.host, connection.port
                                    )),
                            )
                            .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                                        "{} {}",
                                        t(cx, "来源行", "Source line"),
                                        entry
                                            .source
                                            .as_deref()
                                            .map(|source| format!("{source}:{}", entry.source_line))
                                            .unwrap_or_else(|| entry.source_line.to_string()),
                                    )))
                            .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                                "{} · {}",
                                auth_label(&connection.auth, cx),
                                route
                            ))),
                    )
            }));
        let warnings = div()
            .id("openssh-import-warnings")
            .test_support()
            .flex()
            .flex_col()
            .gap_1()
            .children(report.warnings.iter().enumerate().map(|(index, warning)| {
                let location = warning
                    .source
                    .as_deref()
                    .map(|source| format!("{source}:{}", warning.line))
                    .unwrap_or_else(|| warning.line.to_string());
                div()
                    .id(("openssh-import-warning", index))
                    .test_support()
                    .p_2()
                    .rounded(px(6.))
                    .bg(rgb(visual.danger_surface))
                    .text_color(rgb(visual.warning))
                    .child(format!(
                        "{}{}{}",
                        if warning.line == 0 {
                            String::new()
                        } else {
                            format!("{} {}：", t(cx, "第", "line "), location)
                        },
                        warning
                            .directive
                            .as_deref()
                            .map(|directive| format!("{directive} · "))
                            .unwrap_or_default(),
                        warning_label(warning.reason, cx),
                    ))
            }));
        let footer = div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel-openssh-import")
                    .ghost()
                    .label(t(cx, "取消", "Cancel"))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.cancel_openssh_import(window, cx)),
                    ),
            )
            .child(
                Button::new("confirm-openssh-import")
                    .primary()
                    .disabled(self.saving || report.imported.added == 0)
                    .label(t(cx, "确认导入", "Confirm import"))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.confirm_openssh_import(window, cx)),
                    ),
            );
        let summary = format!(
            "{} {} · {} {} · {} {}",
            t(cx, "候选连接", "Candidate connections"),
            report.entries.len(),
            t(cx, "将新增", "To add"),
            report.imported.added,
            t(cx, "重复", "Duplicates"),
            report.imported.skipped,
        );
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("openssh-import-review")
                    .test_support()
                    .w(px(720.))
                    .h(px(620.))
                    .max_w_full()
                    .max_h_full()
                    .min_w_0()
                    .min_h_0()
                    .rounded_lg()
                    .shadow_lg()
                    .bg(rgb(visual.surface))
                    .border_1()
                    .border_color(rgb(visual.border))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("openssh-import-review-header")
                            .p_4()
                            .flex_shrink_0()
                            .border_b_1()
                            .border_color(rgb(visual.border))
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(t(cx, "审阅 SSH 配置导入", "Review SSH import")))
                            .child(div().mt_1().text_sm().text_color(rgb(visual.muted)).child(summary)),
                    )
                    .child(
                        div()
                            .id("openssh-import-review-body")
                            .test_support()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(cx, "将写入连接库", "Profiles to add")))
                            .child(entries)
                            .when(!report.warnings.is_empty(), |body| {
                                body.child(div().mt_2().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{} ({})", t(cx, "需要审阅的配置语义", "Configuration items to review"), report.warnings.len())))
                                    .child(warnings)
                            })
                            .child(div().mt_2().text_xs().text_color(rgb(visual.muted)).child(t(
                                cx,
                                "确认后才会保存到连接库；密码和凭据不会从配置文件导入。",
                                "Nothing is saved until you confirm. Passwords and credentials are never imported from the configuration.",
                            ))),
                    )
                    .child(
                        div()
                            .id("openssh-import-review-footer")
                            .test_support()
                            .flex_shrink_0()
                            .p_3()
                            .bg(rgb(visual.canvas))
                            .border_t_1()
                            .border_color(rgb(visual.border))
                            .child(footer),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn cancel_openssh_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.openssh_review = None;
        self.status = Message::new("已取消 SSH 配置导入", "SSH import cancelled");
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    pub(super) fn confirm_openssh_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(review) = self.openssh_review.take() else {
            return;
        };
        let report = review.report;
        self.persist(
            review.candidate,
            AfterSave::ConnectionsImported {
                added: report.imported.added,
                skipped: report.imported.skipped,
                folders_added: 0,
                warnings: report.warnings.len(),
            },
            window,
            cx,
        );
    }
}
