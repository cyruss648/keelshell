use super::*;
use gpui_kit::base::Selectable;
use keelshell_mcp::{McpClientKind, client_configuration_template};

impl Workspace {
    pub(super) fn mcp_client_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        div()
            .id("mcp-client-settings")
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t(cx, "连接外部客户端", "Connect an external client")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("mcp-client-codex")
                            .ghost()
                            .label("Codex CLI")
                            .selected(self.mcp.client_kind == McpClientKind::CodexCli)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.mcp.client_kind = McpClientKind::CodexCli;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mcp-client-claude")
                            .ghost()
                            .label("Claude Code")
                            .selected(self.mcp.client_kind == McpClientKind::ClaudeCode)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.mcp.client_kind = McpClientKind::ClaudeCode;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mcp-copy-client-template")
                            .ghost()
                            .label(t(cx, "复制配置模板", "Copy configuration template"))
                            .disabled(self.mcp.busy || self.mcp.executable.is_none())
                            .on_click(cx.listener(|view, _, _, cx| {
                                let template = view.mcp.executable.as_ref().and_then(|command| {
                                    client_configuration_template(view.mcp.client_kind, command).ok()
                                });
                                if let Some(template) = template {
                                    cx.write_to_clipboard(ClipboardItem::new_string(template));
                                    view.mcp.status = Message::new(
                                        "已复制客户端模板，不含临时凭据。由启动器向客户端提供两项环境值后再连接。",
                                        "Client template copied without temporary credentials. Supply both environment values through the client launcher before connecting.",
                                    );
                                } else {
                                    view.mcp.status = Message::new(
                                        "此程序路径无法用于所选客户端模板；请检查完整应用路径及客户端的变量插值规则。",
                                        "This program path cannot be used in the selected client template. Check the full application path and the client's variable expansion rules.",
                                    );
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(visual.muted))
                    .child(t(cx,
                        "模板只引用临时环境变量。下方的临时启动配置包含当前授权凭据，需单独复制并交给可信启动器；客户端连接状态由客户端确认。",
                        "Templates reference temporary environment variables. The separate temporary launch settings below contain the current grant credentials for a trusted launcher. Check connection status in the client.")),
            )
            .into_any_element()
    }
}
