//! Socket rows and cached pagination, isolated from the full monitor builder.
use super::*;

impl MonitorPanel {
    // Keep the socket builder in its own stack frame. The complete monitor is
    // also rendered inside a workspace; accumulating its debug builder
    // temporaries there exceeded a Linux test thread's stack budget.
    #[inline(never)]
    pub(super) fn sockets_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let components = crate::design::components(cx);
        let mut sockets = section(
            cx,
            t(cx, "端口列表", "Socket listing"),
            IconName::Network,
        )
        .id("monitor-sockets")
        .test_support()
        .role(accesskit::Role::Group)
        .aria_label(t(cx, "端口列表", "Socket listing"))
        .child(
            div()
                .id("monitor-sockets-note")
                .test_support()
                .role(accesskit::Role::Label)
                .aria_label(t(
                    cx,
                    "只读采集当前账户可见的 TCP 监听与 UDP 端口。macOS 的 UDP 仅表示已绑定或已指定对端，不代表连通或服务健康。",
                    "Read-only TCP listeners and UDP sockets visible to this account. macOS UDP labels show a bound address or specified peer, not reachability or service health.",
                ))
                .text_xs()
                .whitespace_normal()
                .text_color(rgb(visual.muted))
                .child(t(
                    cx,
                    "只读采集当前账户可见的 TCP 监听与 UDP 端口。macOS 的 UDP 仅表示已绑定或已指定对端，不代表连通或服务健康。",
                    "Read-only TCP listeners and UDP sockets visible to this account. macOS UDP labels show a bound address or specified peer, not reachability or service health.",
                )),
        );
        if self.sockets.is_empty() {
            sockets = sockets.child(div().text_xs().text_color(rgb(visual.muted)).child(t(
                cx,
                if self.capabilities.sockets.state == CapabilityState::Available {
                    "当前账户未观察到 TCP 监听或已绑定的 UDP 端口"
                } else {
                    "暂无可用端口采样；可点击端口或重新检测"
                },
                if self.capabilities.sockets.state == CapabilityState::Available {
                    "No TCP listeners or bound UDP sockets visible to this account"
                } else {
                    "No socket sample; click Sockets or Recheck"
                },
            )));
        } else {
            for (socket_index, socket) in self
                .sockets
                .iter()
                .enumerate()
                .skip(self.socket_page * SOCKET_PAGE_SIZE)
                .take(SOCKET_PAGE_SIZE)
            {
                let probe_key = socket_key(socket);
                let state = match (socket.protocol.as_str(), socket.state.as_str()) {
                    ("udp", "BOUND") => t(cx, "已绑定", "Bound"),
                    ("udp", "PEER") => t(cx, "已指定对端", "Peer set"),
                    _ => socket.state.as_str(),
                };
                let endpoint = if socket.peer == "*:*" {
                    socket.local.clone()
                } else {
                    format!("{} → {}", socket.local, socket.peer)
                };
                let mut row = div()
                    .id(("monitor-socket", socket_index))
                    .test_support()
                    .role(accesskit::Role::Group)
                    .aria_label(format!(
                        "{}: {state}, {endpoint}{}",
                        socket.protocol,
                        socket
                            .process
                            .as_deref()
                            .map(|process| format!(", {process}"))
                            .unwrap_or_default(),
                    ))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_1()
                    .text_xs()
                    .border_b_1()
                    .border_color(rgb(visual.border))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .px_1()
                                    .rounded_full()
                                    .bg(rgb(visual.surface_active))
                                    .text_color(rgb(visual.muted))
                                    .child(socket.protocol.clone()),
                            )
                            .child(
                                div()
                                    .id(("monitor-socket-state", socket_index))
                                    .test_support()
                                    .role(accesskit::Role::Label)
                                    .aria_label(state.to_owned())
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(rgb(visual.muted))
                                    .child(state.to_owned()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(visual.text))
                            .child(endpoint),
                    );
                if let Some(process) = &socket.process {
                    row = row.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(visual.muted))
                            .child(process.clone()),
                    );
                }
                if socket.protocol == "tcp" && self.linux_monitor_actions_available() {
                    let selected = socket.clone();
                    row = row.child(
                        Button::new(("probe-socket", socket_index))
                            .disabled(self.busy || self.suspended || self.pending.is_some())
                            .ghost()
                            .compact()
                            .rounded(px(components.control_radius))
                            .label(t(cx, "探测", "Probe"))
                            .accessibility_label(t(cx, "探测 TCP 端口", "Probe TCP socket"))
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                if !panel.busy && panel.pending.is_none() {
                                    panel.run(Job::Probe(selected.clone()), cx);
                                }
                            })),
                    );
                }
                if let Some(probe) = self.probes.get(&probe_key) {
                    row = row.child(
                        div()
                            .id(("probe-result", socket_index))
                            .test_support()
                            .role(accesskit::Role::Label)
                            .aria_label(if probe.reachable {
                                t(cx, "探测可达", "Probe reachable")
                            } else {
                                t(cx, "探测不可达", "Probe unreachable")
                            })
                            .text_color(rgb(if probe.reachable {
                                visual.success
                            } else {
                                visual.danger
                            }))
                            .child(if probe.reachable { "✓" } else { "×" }),
                    );
                }
                sockets = sockets.child(row);
            }
            let pages = self.sockets.len().div_ceil(SOCKET_PAGE_SIZE);
            if pages > 1 {
                let page_label = Message::new(
                    format!(
                        "第 {} / {pages} 页，共 {} 个端口条目",
                        self.socket_page + 1,
                        self.sockets.len()
                    ),
                    format!(
                        "Page {} of {pages}, {} socket entries",
                        self.socket_page + 1,
                        self.sockets.len()
                    ),
                )
                .render(cx);
                sockets = sockets.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("socket-page-previous")
                                .icon(IconName::ChevronLeft)
                                .disabled(self.socket_page == 0)
                                .ghost()
                                .compact()
                                .rounded(px(components.control_radius))
                                .accessibility_label(t(
                                    cx,
                                    "查看上一页端口",
                                    "Previous socket page",
                                ))
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    panel.socket_page = panel.socket_page.saturating_sub(1);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("monitor-socket-page")
                                .test_support()
                                .role(accesskit::Role::Label)
                                .aria_label(page_label.clone())
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .whitespace_normal()
                                .text_color(rgb(visual.muted))
                                .child(page_label),
                        )
                        .child(
                            Button::new("socket-page-next")
                                .icon(IconName::ChevronRight)
                                .disabled(self.socket_page + 1 >= pages)
                                .ghost()
                                .compact()
                                .rounded(px(components.control_radius))
                                .accessibility_label(t(cx, "查看下一页端口", "Next socket page"))
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    if (panel.socket_page + 1) * SOCKET_PAGE_SIZE
                                        < panel.sockets.len()
                                    {
                                        panel.socket_page += 1;
                                    }
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        sockets.into_any_element()
    }
}
