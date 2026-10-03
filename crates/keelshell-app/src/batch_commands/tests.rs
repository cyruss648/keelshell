use super::output_preview;
use gpui_kit::TestAppContext;

#[gpui_kit::test]
fn output_preview_escapes_terminal_and_direction_controls(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let preview = output_preview("中文\n\u{1b}[31m\u{202e}text\0".as_bytes(), cx);
        assert!(preview.starts_with("中文\n"));
        assert!(!preview.contains(['\u{1b}', '\u{202e}', '\0']));
        assert!(preview.contains("\\u{202e}"));
        assert!(preview.contains("\\u{1b}"));
        let large = output_preview(&vec![b'a'; 65_537], cx);
        assert!(large.contains("64 KiB"));
        assert!(large.len() < 66_000);
    });
}
