//! Human command/output review exposes invisible and directional characters.

/// Display escapes only; execution always retains the original immutable bytes.
pub(crate) fn visible_command(command: &str) -> String {
    let mut visible = String::new();
    for ch in command.chars() {
        if (ch.is_control() && ch != '\n')
            || matches!(ch, '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            visible.push_str(&format!("\\u{{{:04x}}}", u32::from(ch)));
        } else {
            visible.push(ch);
        }
    }
    visible
}
