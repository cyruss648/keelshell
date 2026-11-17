//! Client templates are explicit clipboard actions without temporary secrets.
use super::*;

fn clipboard(cx: &mut TestAppContext) -> String {
    cx.update(|cx| {
        cx.read_from_clipboard()
            .checked_option("explicit copied configuration")
            .text()
            .checked_option("text configuration")
    })
}

fn click_template(h: &Harness, client: &'static str, cx: &mut TestAppContext) {
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("mcp-client-settings").bounds();
        let viewport = window.find("mcp-scroll").bounds();
        window.scroll(
            "mcp-scroll",
            ScrollDelta::Pixels(point(px(0.), viewport.origin.y + px(12.) - bounds.origin.y)),
            cx,
        );
        window.render_frame(cx);
        window.click(client, cx);
        window.render_frame(cx);
        assert!(window.find("mcp-copy-client-template").visible());
        window.click("mcp-copy-client-template", cx);
    })
    .checked("choose the external client and explicitly copy its template");
}

#[gpui_kit::test]
async fn mcp_client_templates_reference_environment_without_copying_live_grant_values(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.grant(0, &[ToolKind::ListSessions], "", cx).await;
    let command = if cfg!(windows) {
        r#"C:\Program Files\KeelShell 中文\keelshell-mcp.exe"#
    } else {
        "/opt/KeelShell 中文.app/Contents/MacOS/keelshell-mcp"
    };
    h.fixture.workspace.update(cx, |view, _| {
        view.mcp_test_set_companion(command.into());
    });
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("mcp-copy-launch", cx);
    })
    .checked("copy the distinct current sensitive launch settings");
    let launch: Value = serde_json::from_str(&clipboard(cx)).checked("launch object");
    let secret = launch["env"][keelshell_mcp::MCP_SECRET_ENV]
        .as_str()
        .checked_option("current private capability");
    let address = launch["env"][keelshell_mcp::MCP_ADDRESS_ENV]
        .as_str()
        .checked_option("current listener address");
    for client in ["mcp-client-codex", "mcp-client-claude"] {
        click_template(&h, client, cx);
        let template = clipboard(cx);
        assert!(!template.contains(secret));
        assert!(!template.contains(address));
        if client == "mcp-client-claude" {
            let document: Value = serde_json::from_str(&template).checked("Claude client template");
            let service = &document["mcpServers"]["keelshell"];
            assert_eq!(service["command"], command);
            assert_eq!(service["args"], json!([]));
            assert_eq!(service["type"], "stdio");
            assert_eq!(
                service["env"][keelshell_mcp::MCP_ADDRESS_ENV],
                "${KEELSHELL_MCP_ADDRESS}"
            );
            assert_eq!(
                service["env"][keelshell_mcp::MCP_SECRET_ENV],
                "${KEELSHELL_MCP_SECRET}"
            );
        } else {
            assert!(template.contains("[mcp_servers.keelshell]"));
            assert!(template.contains("env_vars = ["));
            assert!(template.contains("\"KEELSHELL_MCP_ADDRESS\""));
            assert!(template.contains("\"KEELSHELL_MCP_SECRET\""));
            assert!(!template.contains("[mcp_servers.keelshell.env]"));
        }
    }
    h.fixture
        .workspace
        .update(cx, |view, _| view.mcp_test_disable());
    cx.update_window(h.fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("mcp-copy-client-template").disabled(),
            Some(true)
        );
        assert_eq!(window.find("mcp-copy-launch").disabled(), Some(true));
    })
    .checked("revocation disables both configuration exports");
    assert!(h.command.requests().is_empty());
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
    let state = std::fs::read_to_string(h.fixture.store.path()).checked("persisted fixture state");
    assert!(!state.contains(secret));
    assert!(!state.contains(address));
}

#[gpui_kit::test]
async fn mcp_client_template_controls_keep_themes_languages_and_fixed_grant_footer(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    h.grant(0, &[ToolKind::ListSessions], "", cx).await;
    h.fixture.workspace.update(cx, |view, _| {
        view.mcp_test_set_companion(std::env::temp_dir().join("KeelShell/keelshell-mcp"));
    });
    for family in crate::design::registry() {
        for theme in [keelshell_core::Theme::Light, keelshell_core::Theme::Dark] {
            for language in [Language::ZhCn, Language::En] {
                for (width, height) in [(900., 580.), (1440., 900.)] {
                    cx.simulate_window_resize(h.fixture.window, size(px(width), px(height)));
                    cx.update_window(h.fixture.window, |_, window, cx| {
                        i18n::set_language(language, cx);
                        let mut settings = h.fixture.workspace.read(cx).state.settings.clone();
                        settings.theme = theme;
                        settings.theme_family = keelshell_core::ThemeId::new(family.id)
                            .checked("registered theme family");
                        crate::design::apply_settings(&settings, Some(window), cx);
                        window.render_frame(cx);
                    })
                    .checked("render native client controls with the shared theme");
                    for client in ["mcp-client-codex", "mcp-client-claude"] {
                        click_template(&h, client, cx);
                        cx.update_window(h.fixture.window, |_, window, cx| {
                            window.render_frame(cx);
                            let viewport = window.find("mcp-scroll").bounds();
                            let button = window.find("mcp-copy-client-template").bounds();
                            assert!(button.origin.y >= viewport.origin.y);
                            assert!(button.bottom() <= viewport.bottom());
                            assert!(button.right() <= window.bounds().right());
                            let grant = window.find("mcp-grant-session").bounds();
                            let disable = window.find("mcp-disable").bounds();
                            assert!(grant.bottom() <= window.bounds().bottom());
                            assert!(disable.bottom() <= window.bounds().bottom());
                            assert!(grant.right() <= window.bounds().right());
                            assert!(disable.right() <= window.bounds().right());
                        })
                        .checked(
                            "client action and existing grant footer stay inside the viewport",
                        );
                    }
                }
            }
        }
    }
    assert!(h.command.requests().is_empty());
    assert_eq!(
        h.files.filesystem.transfer_writes_started(),
        h.initial_writes
    );
}
