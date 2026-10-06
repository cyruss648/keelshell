use super::*;
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::{base::Selectable, prelude::*};
use keelshell_core::{ProxyAuthentication, ProxyKind, ReconnectPolicy, SyncProfile};
fn profile(value: Option<&SyncProfile>, heading: &'static str, cx: &App) -> AnyElement {
    let visual = crate::design::palette(cx);
    let text = match value {
        None => t(
            cx,
            "删除 / 不存在（保留墓碑）",
            "Deleted / absent (tombstone retained)",
        )
        .to_owned(),
        Some(p) => format!(
            "{}\n{}@{}:{}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}",
            p.name,
            p.username,
            p.host,
            p.port,
            t(cx, "ID", "ID"),
            p.id,
            t(cx, "同步旧式分组标签", "Shared legacy group label"),
            p.group,
            t(cx, "标签", "Tags"),
            p.tags.join(", "),
            t(cx, "收藏", "Favorite"),
            if p.favorite {
                t(cx, "是", "Yes")
            } else {
                t(cx, "否", "No")
            },
            t(cx, "跳板连接", "Jump profile"),
            p.jump_host
                .map(|i| i.to_string())
                .unwrap_or_else(|| t(cx, "未设置", "None").into()),
            t(cx, "代理", "Proxy"),
            p.proxy
                .as_ref()
                .map(|proxy| {
                    let protocol = match proxy.kind {
                        ProxyKind::Socks5 => "SOCKS5",
                        ProxyKind::HttpConnect => "HTTP CONNECT",
                    };
                    let account = match &proxy.auth {
                        ProxyAuthentication::None => {
                            t(cx, "无需身份验证", "No authentication").to_owned()
                        }
                        ProxyAuthentication::UsernamePassword { username } => {
                            format!("{}: {username}", t(cx, "账户", "Account"))
                        }
                    };
                    format!("{protocol} {}:{} · {account}", proxy.host, proxy.port)
                })
                .unwrap_or_else(|| t(cx, "未设置", "None").into()),
            t(cx, "重连", "Reconnect"),
            match p.reconnect {
                ReconnectPolicy::Manual => t(cx, "手动重连", "Manual reconnect").to_owned(),
                ReconnectPolicy::Automatic {
                    max_attempts,
                    initial_delay_seconds,
                    max_delay_seconds,
                } =>
                    if crate::i18n::language(cx) == keelshell_core::Language::ZhCn {
                        format!(
                            "自动重连，最多 {max_attempts} 次；首次等待 {initial_delay_seconds} 秒，最长等待 {max_delay_seconds} 秒"
                        )
                    } else {
                        format!(
                            "Automatic, up to {max_attempts} attempts; first wait {initial_delay_seconds}s, maximum wait {max_delay_seconds}s"
                        )
                    },
            }
        ),
    };
    div()
        .min_w(px(220.))
        .max_w_full()
        .flex_1()
        .p_2()
        .rounded_md()
        .bg(rgb(visual.canvas))
        .text_xs()
        .whitespace_normal()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(heading))
        .child(text)
        .into_any_element()
}
impl Render for ProfileSyncPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visual = crate::design::palette(cx);
        let busy = self.busy();
        let mut body=div().id("profile-sync-body").test_support().flex_1().min_h_0().overflow_y_scroll().p_4().flex().flex_col().gap_3()
            .child(div().text_color(rgb(visual.muted)).text_sm().child(t(cx,
                "仅同步连接元数据。不同步密码、私钥路径、凭据库、主机信任、AI 密钥、历史或终端内容。现有会话不会因同步而重新连接。目录须由你自行共享或挂载，并支持跨设备文件锁与原子替换；应用不会接入云账户。",
                "Only connection metadata is synchronized. Passwords, private-key paths, vaults, host trust, AI keys, history and terminal contents are excluded. Active sessions are not reconnected. You must share or mount a directory supporting cross-device file locks and atomic replacement; the app does not connect cloud accounts.")))
            .child(div().id("profile-sync-directory-field").test_support().child(Input::new(&self.directory).disabled(busy||self.configured||self.review.is_some())))
            .child(div().id("profile-sync-password-field").test_support().child(Input::new(&self.password).disabled(busy)))
            .child(div().text_xs().text_color(rgb(visual.accent)).child(self.status.render(cx)));
        if let Some(review) = &self.review {
            body = body.child(div().text_xs().child(format!(
                "{} · {} · {} {}",
                if review.creates_channel() {
                    t(cx, "创建同步空间", "Create sync channel")
                } else {
                    t(cx, "已认证同步空间", "Authenticated sync channel")
                },
                review.channel(),
                t(cx, "版本", "Generation"),
                review.generation()
            )));
            let rows = review.rows();
            if rows.is_empty() {
                body=body.child(t(cx,"没有配置差异。批准会启用同步并记录当前版本。","No profile differences. Approval enables sync and records the current version."));
            }
            for (i, row) in rows.iter().enumerate().skip(self.page * 20).take(20) {
                let id = row.id;
                let choice = self.choices.get(&id).copied();
                let placement = row.local_placement.as_ref().map(|path| {
                    let label = if path.is_empty() { t(cx, "根目录", "Root") } else { path };
                    let selected = match choice {
                        Some(ProfileSyncChoice::Local) => row.local.as_ref(),
                        Some(ProfileSyncChoice::Remote) => row.remote.as_ref(),
                        None => None,
                    };
                    let applied = match (choice, selected) {
                        (None, _) => t(cx, "待逐项选择", "Choose a resolution"),
                        (_, None) => t(cx, "回收站（保留本机位置）", "Trash (local placement retained)"),
                        (_, Some(profile)) => if path.is_empty() { &profile.group } else { path },
                    };
                    if crate::i18n::language(cx) == keelshell_core::Language::ZhCn {
                        format!("本机文件夹位置保留：{label}。共享旧式分组标签单独记录于同步基线；批准后本机分组显示：{applied}。文件夹树不会上传或移动。")
                    } else {
                        format!("Retained local folder: {label}. The shared legacy label is stored separately in the sync baseline; resulting local group display: {applied}. The local folder tree is neither uploaded nor moved.")
                    }
                });
                body = body.child(
                    div()
                        .id(("profile-sync-row", i))
                        .test_support()
                        .flex_shrink_0()
                        .p_3()
                        .border_1()
                        .border_color(rgb(if row.conflict {
                            visual.warning
                        } else {
                            visual.border
                        }))
                        .rounded_md()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                            "{} · {}",
                            row.id,
                            if row.conflict {
                                t(
                                    cx,
                                    "离线并发冲突：必须选择",
                                    "Offline conflict: choose explicitly",
                                )
                            } else {
                                t(cx, "配置差异", "Profile difference")
                            }
                        )))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .child(profile(row.local.as_ref(), t(cx, "本地", "Local"), cx))
                                .child(profile(row.remote.as_ref(), t(cx, "共享", "Shared"), cx)),
                        )
                        .when_some(placement, |row, text| {
                            row.child(div().text_xs().text_color(rgb(visual.accent)).child(text))
                        })
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    Button::new(("profile-sync-local", i))
                                        .ghost()
                                        .label(t(cx, "保留本地版本", "Keep local version"))
                                        .selected(choice == Some(ProfileSyncChoice::Local))
                                        .disabled(busy)
                                        .on_click(cx.listener(move |p, _, _, cx| {
                                            p.choose(id, ProfileSyncChoice::Local, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("profile-sync-remote", i))
                                        .ghost()
                                        .label(t(cx, "接受共享版本", "Accept shared version"))
                                        .selected(choice == Some(ProfileSyncChoice::Remote))
                                        .disabled(busy)
                                        .on_click(cx.listener(move |p, _, _, cx| {
                                            p.choose(id, ProfileSyncChoice::Remote, cx)
                                        })),
                                ),
                        ),
                );
            }
            let pages = rows.len().div_ceil(20).max(1);
            body = body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("profile-sync-page-prev")
                            .ghost()
                            .label(t(cx, "上一页", "Previous"))
                            .disabled(busy || self.page == 0)
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.page = p.page.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(format!(
                        "{}/{} · {}/{}",
                        self.page + 1,
                        pages,
                        self.choices.len(),
                        rows.len()
                    ))
                    .child(
                        Button::new("profile-sync-page-next")
                            .ghost()
                            .label(t(cx, "下一页", "Next"))
                            .disabled(busy || self.page + 1 >= pages)
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.page += 1;
                                cx.notify();
                            })),
                    ),
            );
        }
        if let Some(preview) = &self.preview
            && !preview.route_changes.is_empty()
        {
            body = body.child(div().id("profile-sync-route-effects").test_support().font_weight(FontWeight::SEMIBOLD).child(t(cx,
                    "完整路线变更：下列保存的连接将使用本机 Agent 默认。认证引用会清除，但 OS/vault 中的凭据不删除；当前 SSH 实例保持。",
                    "Complete route changes: the saved profiles below will use local Agent defaults. References are cleared; OS/vault entries are retained. Current SSH instances remain.")));
            for (index, change) in preview
                .route_changes
                .iter()
                .enumerate()
                .skip(self.impact_page * 20)
                .take(20)
            {
                body = body.child(
                    div()
                        .id(("profile-sync-route-effect", index))
                        .test_support()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(visual.warning))
                        .text_xs()
                        .whitespace_normal()
                        .child(format!(
                            "{} · {}",
                            change.id,
                            if change.resets_authentication {
                                t(
                                    cx,
                                    "撤销本机认证引用",
                                    "Clear local authentication references",
                                )
                            } else {
                                t(cx, "完整路线改变", "Complete route changes")
                            }
                        ))
                        .child(format!(
                            "{}: {}",
                            t(cx, "当前", "Current"),
                            route(&change.before, cx)
                        ))
                        .child(format!(
                            "{}: {}",
                            t(cx, "批准后", "After approval"),
                            route(&change.after, cx)
                        )),
                );
            }
            let pages = preview.route_changes.len().div_ceil(20).max(1);
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("profile-sync-impact-prev")
                            .ghost()
                            .label(t(cx, "上一组路线", "Previous routes"))
                            .disabled(busy || self.impact_page == 0)
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.impact_page = p.impact_page.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(format!(
                        "{}/{} · {} {}",
                        self.impact_page + 1,
                        pages,
                        preview.route_changes.len(),
                        t(cx, "个保存的连接受影响", "saved profiles affected")
                    ))
                    .child(
                        Button::new("profile-sync-impact-next")
                            .ghost()
                            .label(t(cx, "下一组路线", "Next routes"))
                            .disabled(busy || self.impact_page + 1 >= pages)
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.impact_page += 1;
                                cx.notify();
                            })),
                    ),
            );
        }
        let reviewed = self.review.is_some();
        let ready = self
            .review
            .as_ref()
            .is_some_and(|r| r.rows().len() == self.choices.len())
            && self.preview.is_some()
            && self.effects_acknowledged;
        let footer = div()
            .id("profile-sync-footer")
            .test_support()
            .flex_shrink_0()
            .p_3()
            .border_t_1()
            .border_color(rgb(visual.border))
            .bg(rgb(visual.canvas))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                Button::new("profile-sync-close")
                    .ghost()
                    .label(t(cx, "关闭", "Close"))
                    .on_click(cx.listener(|p, _, w, cx| p.close(w, cx))),
            )
            .when(busy, |d| {
                d.child(
                    Button::new("profile-sync-cancel")
                        .ghost()
                        .label(t(cx, "取消任务", "Cancel operation"))
                        .on_click(cx.listener(|p, _, _, cx| p.cancel(cx))),
                )
            })
            .when(!busy && reviewed, |d| {
                d.child(
                    Button::new("profile-sync-discard-review")
                        .ghost()
                        .label(t(cx, "取消审核", "Cancel review"))
                        .on_click(cx.listener(|p, _, w, cx| p.discard_review(w, cx))),
                )
            })
            .when(!self.pending, |d| {
                d.child(
                    Button::new("profile-sync-inspect")
                        .ghost()
                        .label(t(cx, "读取并审核差异", "Pull and review"))
                        .disabled(busy)
                        .on_click(cx.listener(|p, _, w, cx| p.submit(Action::Inspect, w, cx))),
                )
            })
            .when(self.pending, |d| {
                d.child(
                    Button::new("profile-sync-resume")
                        .primary()
                        .label(t(cx, "继续已批准发布", "Resume approved publication"))
                        .disabled(busy)
                        .on_click(cx.listener(|p, _, w, cx| p.submit(Action::Resume, w, cx))),
                )
                .child(
                    Button::new("profile-sync-discard-pending")
                        .ghost()
                        .label(t(cx, "放弃未发布记录", "Discard unpublished receipt"))
                        .disabled(busy)
                        .on_click(
                            cx.listener(|p, _, w, cx| p.submit(Action::DiscardPending, w, cx)),
                        ),
                )
            })
            .when(self.enabled, |d| {
                d.child(
                    Button::new("profile-sync-disable")
                        .ghost()
                        .label(t(cx, "关闭同步", "Disable sync"))
                        .disabled(busy || reviewed)
                        .on_click(cx.listener(|p, _, w, cx| p.submit(Action::Disable, w, cx))),
                )
            })
            .when(self.configured && !self.pending && !reviewed, |d| {
                d.child(
                    Button::new("profile-sync-forget")
                        .ghost()
                        .label(t(cx, "解除本机同步关联", "Forget this device pairing"))
                        .disabled(busy)
                        .on_click(cx.listener(|p, _, _, cx| p.request_forget(cx))),
                )
            })
            .when(self.forget_confirmation, |d| {
                d.child(
                    Button::new("profile-sync-forget-confirm")
                        .ghost()
                        .label(t(
                            cx,
                            "确认解除并保留连接",
                            "Confirm forget; retain profiles",
                        ))
                        .disabled(busy)
                        .on_click(cx.listener(|p, _, w, cx| p.submit(Action::Forget, w, cx))),
                )
            })
            .when(
                reviewed && self.preview.is_some() && !self.effects_acknowledged,
                |d| {
                    d.child(
                        Button::new("profile-sync-acknowledge-effects")
                            .ghost()
                            .label(t(
                                cx,
                                "已核对路线与本机位置",
                                "Reviewed routes and local placement",
                            ))
                            .disabled(busy)
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.effects_acknowledged = true;
                                cx.notify();
                            })),
                    )
                },
            )
            .when(reviewed, |d| {
                d.child(
                    Button::new("profile-sync-approve")
                        .primary()
                        .label(t(cx, "批准、启用并发布", "Approve, enable and publish"))
                        .disabled(busy || !ready)
                        .on_click(cx.listener(|p, _, w, cx| p.submit(Action::Apply, w, cx))),
                )
            });
        div()
            .id("profile-sync-panel")
            .test_support()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(visual.surface))
            .child(
                div()
                    .flex_shrink_0()
                    .p_4()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(
                        cx,
                        "端到端加密连接同步",
                        "End-to-end encrypted profile sync",
                    )),
            )
            .child(body)
            .child(footer)
    }
}

fn route(identity: &keelshell_core::RouteIdentity, cx: &App) -> String {
    identity
        .endpoints()
        .iter()
        .map(|endpoint| {
            let proxy = endpoint
                .proxy
                .as_ref()
                .map(|proxy| {
                    let protocol = match proxy.kind {
                        ProxyKind::Socks5 => "SOCKS5",
                        ProxyKind::HttpConnect => "HTTP CONNECT",
                    };
                    let account = match &proxy.auth {
                        ProxyAuthentication::None => {
                            t(cx, "无需身份验证", "No authentication").to_owned()
                        }
                        ProxyAuthentication::UsernamePassword { username } => {
                            format!("{}: {username}", t(cx, "账户", "Account"))
                        }
                    };
                    format!(" [{protocol} {}:{} · {account}]", proxy.host, proxy.port)
                })
                .unwrap_or_default();
            format!(
                "{}@{}:{}{}",
                endpoint.username, endpoint.host, endpoint.port, proxy
            )
        })
        .collect::<Vec<_>>()
        .join(" → ")
}
