use super::*;
use gpui_kit::component::Selectable;

fn action_label(state: ActionState, cx: &App) -> &'static str {
    match state {
        ActionState::PendingReview => t(cx, "待人工审阅", "Awaiting human review"),
        ActionState::Rejected => t(cx, "已拒绝", "Rejected"),
        ActionState::Expired => t(cx, "已到期", "Expired"),
        ActionState::Cancelled => t(cx, "授权已撤销", "Grant revoked"),
        ActionState::Running => t(cx, "执行中", "Running"),
        ActionState::Succeeded => t(cx, "已成功", "Succeeded"),
        ActionState::Failed => t(cx, "失败", "Failed"),
        ActionState::OutcomeUnknown => t(cx, "远端结果未知", "Remote outcome unknown"),
    }
}

impl Workspace {
    pub(in crate::workspace) fn mcp_file_review_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(action) = self.mcp.reviewing.and_then(|id| {
            self.mcp
                .actions
                .iter()
                .find(|action| action.proposal.id() == id)
        }) else {
            return div().into_any_element();
        };
        let ReviewedProposal::File { proposal, diff, .. } = &action.proposal else {
            return div().into_any_element();
        };
        let id = proposal.id;
        let visual = crate::design::palette(cx);
        let can_approve = matches!(action.state, ActionState::PendingReview)
            && !self.mcp.busy
            && Instant::now() < action.deadline
            && action.lease.check().is_ok()
            && self.mcp_target_current(action.entity, cx)
            && !self
                .mcp
                .actions
                .iter()
                .any(|action| matches!(action.state, ActionState::Running));
        div().absolute().inset_0().occlude().bg(rgba(0x00000088)).flex().items_center().justify_center()
            .child(div().id("mcp-file-review-dialog").test_support().track_focus(&self.overlay_focus)
                .w(px(850.)).h(px(660.)).max_w(relative(0.96)).max_h(relative(0.94)).min_w_0().min_h_0()
                .bg(rgb(visual.surface)).text_color(rgb(visual.text)).border_1().border_color(rgb(visual.border)).rounded_lg().shadow_lg().flex().flex_col().overflow_hidden()
                .child(div().p_3().flex_shrink_0().font_weight(FontWeight::SEMIBOLD).child(t(cx, "审阅远程文件完整替换", "Review complete remote file replacement")))
                .child(div().id("mcp-file-review-scroll").test_support().flex_1().min_h_0().overflow_y_scroll().p_3().flex().flex_col().gap_2()
                    .child(div().child(format!("{} · {}", action.label, action_label(action.state, cx))))
                    .child(div().id("mcp-file-review-path").test_support().role(accesskit::Role::Label).aria_label(review::visible_command(&proposal.path)).font_family("monospace").overflow_x_scroll().child(review::visible_command(&proposal.path)))
                    .child(div().text_xs().child(review::visible_command(&action.route)))
                    .child(div().text_xs().child(format!("connection: {}\nsession: {}\nroute: {}\naction: {}\ndigest: {}", proposal.target.connection_id, proposal.target.session_id, proposal.target.route_revision, id, proposal.digest)))
                    .child(div().text_xs().child(format!("SHA-256: {}\n→ {}", proposal.expected_sha256, proposal.replacement_sha256)))
                    .child(div().text_sm().text_color(rgb(visual.muted)).child(t(cx,
                        "下面显示原文全部删除、替换正文全部加入。写入前再次核对内容、大小、权限及时间；并发远端修改仍无法获得 SFTP 原子比较锁。关闭审阅保留待审提案；拒绝、到期或撤权不会写入。",
                        "The diff shows every original line removed and every replacement line added. Content, size, permissions and time are checked again before writing; SFTP cannot atomically lock out concurrent remote changes. Closing keeps the pending proposal. Rejection, expiry or revocation does not write.")))
                    .child(div().id("mcp-file-review-diff").test_support().flex_shrink_0().overflow_x_scroll().font_family("monospace").text_sm().whitespace_normal().bg(rgb(visual.canvas)).p_2().child(diff.clone())))
                .child(div().id("mcp-file-review-footer").test_support().flex_shrink_0().p_3().border_t_1().border_color(rgb(visual.border)).flex().flex_wrap().justify_end().gap_2()
                    .child(Button::new("mcp-file-review-close").ghost().label(t(cx, "暂不处理", "Keep pending"))
                        .on_click(cx.listener(|view, _, _, cx| { view.mcp.reviewing = None; cx.notify(); })))
                    .child(Button::new("mcp-file-review-reject").ghost().label(t(cx, "拒绝修改", "Reject change"))
                        .on_click(cx.listener(move |view, _, _, cx| { view.review_mcp_action(id, false, cx); view.mcp.reviewing = None; cx.notify(); })))
                    .child(Button::new("mcp-file-review-approve").primary().label(t(cx, "确认完整目标并替换文件", "Confirm exact target and replace file")).disabled(!can_approve)
                        .on_click(cx.listener(move |view, _, _, cx| view.review_mcp_action(id, true, cx)))))
            ).into_any_element()
    }

    pub(in crate::workspace) fn mcp_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.mcp.show {
            return div().into_any_element();
        }
        let visual = crate::design::palette(cx);
        let target = self
            .mcp
            .draft_entity
            .and_then(|id| self.tabs.iter().find(|tab| tab.entity_id() == id));
        let target_name = target
            .map(|tab| tab.read(cx).title.clone())
            .unwrap_or_else(|| t(cx, "无活动 SSH 会话", "No active SSH session").into());
        let endpoint = self
            .mcp
            .draft_entity
            .and_then(|id| {
                self.batch_route_description(id)
                    .map(|(_, route)| route)
                    .or_else(|| self.remote_hosts.get(&id).cloned())
            })
            .unwrap_or_default();
        let pending = self
            .mcp
            .actions
            .iter()
            .filter(|action| matches!(action.state, ActionState::PendingReview))
            .count();
        let mut tools = div().flex().flex_wrap().gap_2();
        for (tool, zh, en) in [
            (ToolKind::ListSessions, "枚举此会话", "List this session"),
            (
                ToolKind::ReadSelection,
                "读取明确选区",
                "Read selected fragment",
            ),
            (ToolKind::SftpList, "列出授权目录", "List granted directory"),
            (
                ToolKind::SftpRead,
                "读取常规 UTF-8 文件",
                "Read regular UTF-8 files",
            ),
            (
                ToolKind::MonitorSnapshot,
                "读取监控缓存",
                "Read monitor cache",
            ),
            (ToolKind::ProposeCommand, "提交待审命令", "Propose commands"),
            (
                ToolKind::ProposeFileChange,
                "提交待审文件修改",
                "Propose file changes",
            ),
            (
                ToolKind::GetActionStatus,
                "查询提案状态",
                "Read proposal status",
            ),
        ] {
            tools = tools.child(
                Button::new(tool.name())
                    .ghost()
                    .label(t(cx, zh, en))
                    .selected(self.mcp.draft_tools.contains(&tool))
                    .disabled(self.mcp.busy || target.is_none())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.mcp.draft_tools.remove(&tool) {
                            view.mcp.draft_tools.insert(tool);
                        }
                        cx.notify();
                    })),
            );
        }
        let mut grants = div().flex().flex_col().gap_2();
        for target in &self.mcp.targets {
            let entity = target.entity;
            grants = grants.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(target.label.clone()).child(
                        div().text_xs().text_color(rgb(visual.muted)).child(format!(
                            "{} · {}",
                            target.identity.session_id,
                            target.roots.join(", ")
                        )),
                    ))
                    .child(
                        Button::new(("mcp-remove-grant", entity))
                            .ghost()
                            .compact()
                            .label(t(cx, "撤销此会话", "Revoke session"))
                            .disabled(self.mcp.busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.mcp.targets.retain(|target| target.entity != entity);
                                if view.mcp.targets.is_empty() {
                                    view.mcp.stop();
                                } else {
                                    view.apply_mcp_grants(cx);
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        let mut actions = div().flex().flex_col().gap_3();
        for action in &self.mcp.actions {
            let id = action.proposal.id();
            let pending = matches!(action.state, ActionState::PendingReview);
            let can_approve = pending
                && !self.mcp.busy
                && !self
                    .mcp
                    .actions
                    .iter()
                    .any(|action| matches!(action.state, ActionState::Running))
                && action.lease.check().is_ok()
                && self.mcp_target_current(action.entity, cx)
                && Instant::now() < action.deadline;
            actions = actions.child(
                div()
                    .border_1()
                    .border_color(rgb(visual.border))
                    .rounded_md()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{} · {}",
                        action.label,
                        action_label(action.state, cx)
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(visual.muted))
                            .child(action.route.clone()),
                    )
                    .child(div().text_xs().text_color(rgb(visual.muted)).child(format!(
                        "{} · {}",
                        action.proposal.target().session_id,
                        action.proposal.digest()
                    )))
                    .child(
                        div()
                            .id(format!("mcp-command-preview-{id}"))
                            .test_support()
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .bg(rgb(visual.canvas))
                            .p_2()
                            .font_family("monospace")
                            .whitespace_normal()
                            .child(match &action.proposal {
                                ReviewedProposal::Command(proposal) => {
                                    review::visible_command(&proposal.command)
                                }
                                ReviewedProposal::File { proposal, .. } => format!(
                                    "{}\n{} → {}",
                                    review::visible_command(&proposal.path),
                                    proposal.expected_sha256,
                                    proposal.replacement_sha256
                                ),
                            }),
                    )
                    .when(!action.output.is_empty(), |card| {
                        card.child(
                            div()
                                .id(format!("mcp-output-{id}"))
                                .max_h(px(100.))
                                .overflow_y_scroll()
                                .font_family("monospace")
                                .text_xs()
                                .child(action.output.clone()),
                        )
                    })
                    .when(pending, |card| {
                        card.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new(format!("mcp-reject-{id}"))
                                        .ghost()
                                        .label(t(cx, "拒绝", "Reject"))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.review_mcp_action(id, false, cx)
                                        })),
                                )
                                .child(
                                    Button::new(format!("mcp-execute-{id}"))
                                        .primary()
                                        .label(t(
                                            cx,
                                            if action.proposal.kind() == ActionKind::FileChange {
                                                "打开完整文件审阅"
                                            } else {
                                                "确认目标并执行此命令"
                                            },
                                            if action.proposal.kind() == ActionKind::FileChange {
                                                "Open full file review"
                                            } else {
                                                "Confirm target and run this command"
                                            },
                                        ))
                                        .disabled(!can_approve)
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            if view.mcp.actions.iter().any(|action| {
                                                action.proposal.id() == id
                                                    && action.proposal.kind()
                                                        == ActionKind::FileChange
                                            }) {
                                                view.mcp.reviewing = Some(id);
                                                cx.notify();
                                            } else {
                                                view.review_mcp_action(id, true, cx);
                                            }
                                        })),
                                ),
                        )
                    }),
            );
        }
        div().absolute().inset_0().occlude().bg(rgba(0x00000066)).flex().items_center().justify_center()
            .child(div().id("mcp-dialog").test_support().track_focus(&self.overlay_focus)
                .w(px(850.)).h(px(660.)).max_w(relative(0.96)).max_h(relative(0.94)).min_w_0().min_h_0()
                .bg(rgb(visual.surface)).text_color(rgb(visual.text)).border_1().border_color(rgb(visual.border)).rounded_lg().shadow_lg().flex().flex_col().overflow_hidden()
                .child(div().p_3().flex().items_center().justify_between()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(t(cx, "对外 MCP 授权与审阅", "External MCP grants and review")))
                    .child(Button::new("mcp-close").ghost().compact().label("×").accessibility_label(t(cx, "关闭 MCP 设置", "Close MCP settings"))
                        .on_click(cx.listener(|view, _, window, cx| { view.mcp.show = false; view.focus_current_surface(window, cx); cx.notify(); }))))
                .child(div().id("mcp-scroll").test_support().flex_1().min_h_0().overflow_y_scroll().px_4().pb_4().flex().flex_col().gap_3()
                    .child(div().text_sm().text_color(rgb(visual.muted)).child(t(cx,
                        "外部智能体只能读取勾选能力、所选文本和授权路径，或提交待审提案。不能直接执行、代登录或解锁凭据。临时配置可供多个客户端使用，直到授权变更、撤销或应用退出；授权不保存，变更后需重新复制。",
                        "External agents may read only selected capabilities, fragments and paths, or propose commands for review. They cannot execute, log in or unlock credentials. Temporary settings may serve multiple clients until grants change, access is disabled or the app exits. Grants are not saved; changes require fresh settings.")))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(target_name))
                    .child(div().text_xs().text_color(rgb(visual.muted)).child(endpoint))
                    .child(tools)
                    .child(Input::new(&self.mcp.root).id("mcp-remote-root").aria_label(t(cx, "授权的 canonical 远程目录", "Granted canonical remote directory")).disabled(self.mcp.busy))
                    .child(div().flex().items_center().gap_2()
                        .child(Button::new("mcp-capture-selection").ghost().label(t(cx, "捕获当前所选文本", "Capture selected terminal text"))
                            .disabled(self.mcp.busy || target.is_none())
                            .on_click(cx.listener(|view, _, _, cx| view.mcp_capture_selection(cx))))
                        .child(div().text_xs().child(self.mcp.draft_selection.as_ref().map(|(_, text)| format!("{} {}", text.len(), t(cx, "字节", "bytes"))).unwrap_or_else(|| t(cx, "未选择片段", "No selected fragment").into()))))
                    .when_some(self.mcp.draft_selection.as_ref(), |body, (id, text)| body.child(div().flex().flex_col()
                        .child(div().text_xs().text_color(rgb(visual.muted)).child(id.to_string()))
                        .child(div().id("mcp-selection-preview").max_h(px(110.)).overflow_y_scroll().bg(rgb(visual.canvas)).p_2().text_xs().font_family("monospace").child(text.clone()))))
                    .child(div().text_sm().child(self.mcp.status.render(cx)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(cx, "已授权会话", "Granted sessions")))
                    .child(grants)
                    .child(div().flex().items_center().justify_between()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} ({pending})", t(cx, "操作提案", "Action proposals"))))
                        .child(Button::new("mcp-clear-actions").ghost().compact().label(t(cx, "清除已处理记录", "Clear finished records"))
                            .on_click(cx.listener(|view, _, _, cx| { view.mcp.actions.retain(|action| matches!(action.state, ActionState::PendingReview | ActionState::Running)); cx.notify(); }))))
                    .child(actions))
                .child(div().p_3().flex_shrink_0().border_t_1().border_color(rgb(visual.border)).flex().flex_wrap().justify_end().gap_2()
                    .child(Button::new("mcp-disable").ghost().label(t(cx, "关闭并撤销全部授权", "Disable and revoke all"))
                        .on_click(cx.listener(|view, _, _, cx| { view.mcp.stop(); view.mcp.status = Message::new("MCP 已关闭，全部临时授权已撤销。", "MCP is off; all temporary grants are revoked."); cx.notify(); })))
                    .child(Button::new("mcp-copy-launch").ghost().label(t(cx, "复制临时启动配置", "Copy temporary launch settings"))
                        .disabled(self.mcp.busy || self.mcp.host.is_none() || self.mcp.executable.is_none())
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let (Some(host), Some(executable)) = (&view.mcp.host, &view.mcp.executable) {
                                let Ok(configuration) = host.launch_environment_for_command(executable) else { return; };
                                cx.write_to_clipboard(ClipboardItem::new_string(configuration));
                                view.mcp.status = Message::new("临时凭据已复制；仅交给你信任的智能体，关闭授权立即失效。", "Temporary credentials copied; share only with trusted agents. Disabling access invalidates them.");
                            }
                            cx.notify();
                        })))
                    .child(Button::new("mcp-grant-session").primary().label(t(cx, "授权此会话", "Grant this session"))
                        .disabled(self.mcp.busy || target.is_none() || self.mcp.draft_tools.is_empty())
                        .on_click(cx.listener(|view, _, _, cx| view.grant_mcp(cx))))))
            .into_any_element()
    }
}
