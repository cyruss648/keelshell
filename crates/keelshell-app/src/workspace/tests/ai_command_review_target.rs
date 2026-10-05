//! Production Workspace rejects a manual AI proposal for a different active terminal.
use super::*;

#[gpui_kit::test]
fn captured_ai_command_review_target_uses_live_active_terminal_without_rebinding(
    cx: &mut TestAppContext,
) {
    const COMMAND: &str = "  printf 'review target 中文'";
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let first = panes[0].terminal.entity_id();
    let second = panes[1].terminal.entity_id();
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.show_assistant = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("context-screen", cx);
    })
    .checked("explicitly capture the first terminal through the real button");
    cx.run_until_parked();
    let panel = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.assistant.clone());
    panel.update(cx, |panel, cx| {
        panel.receive_review_response_for_test(format!("```sh\n{COMMAND}\n```"), cx)
    });
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(
            workspace.command.read(cx).value().is_empty(),
            "reply must not fill the command field automatically"
        );
    });
    for language in [Language::ZhCn, Language::En] {
        cx.update_window(fixture.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            window.click(("session-tab", 1_usize), cx);
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.set_reviewed_command("keep manual draft".into(), Some(second), window, cx)
            });
            window.render_frame(cx);
            window.scroll(
                "assistant-scroll",
                gpui_kit::ScrollDelta::Lines(point(0., -1000.)),
                cx,
            );
            window.render_frame(cx);
            let target = window.find(("suggestion-review-target", 0_usize));
            let label = target.label().unwrap_or_default();
            assert!(
                label.contains("fixture-0@example.invalid:22")
                    && label.contains(&format!("{first:?}"))
                    && !label.contains(&format!("{second:?}"))
            );
            window.click(("review-suggestion", 0_usize), cx);
        })
        .checked("manual click rejects the captured proposal on another active terminal");
        cx.run_until_parked();
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.tabs[workspace.active].entity_id(), second);
            assert_eq!(
                workspace.command.read(cx).value().as_str(),
                "keep manual draft"
            );
            assert!(workspace.status.render(cx).contains(match language {
                Language::ZhCn => "另一会话",
                Language::En => "another session",
            }));
        });
        assert!(writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty());
        cx.update_window(fixture.window, |_, window, cx| {
            window.click(("session-tab", 0_usize), cx);
            window.render_frame(cx);
            window.click(("review-suggestion", 0_usize), cx);
        })
        .checked("explicitly select the original terminal and manually insert the exact proposal");
        cx.run_until_parked();
        fixture.workspace.read_with(cx, |workspace, cx| {
            assert_eq!(workspace.tabs[workspace.active].entity_id(), first);
            assert_eq!(workspace.command.read(cx).value().as_str(), COMMAND);
        });
        assert!(
            writes(&panes[0]).is_empty() && writes(&panes[1]).is_empty(),
            "insertion must never execute a command"
        );
    }
}
