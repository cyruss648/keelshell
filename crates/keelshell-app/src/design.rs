//! Shared visual tokens. Native terminal content uses its independent ANSI palette.
/// Main application canvas and quiet toolbar background.
pub const CANVAS: u32 = 0xf6f8fb;
/// Raised and content surfaces.
pub const SURFACE: u32 = 0xffffff;
/// Low-contrast separators; do not use to convey interactive state alone.
pub const BORDER: u32 = 0xdce3ec;
/// Primary readable text.
pub const TEXT: u32 = 0x1d2939;
/// Secondary labels and metadata.
pub const MUTED: u32 = 0x66758b;
/// Primary action and focus accent.
pub const ACCENT: u32 = 0x2878e3;
/// Selected rows and navigation tint.
pub const SELECTED: u32 = 0xe8f1ff;

/// Apply the same palette to native GPUI Kit controls, including focus and hover.
pub fn install(cx: &mut gpui_kit::App) {
    use gpui_kit::{component::Theme, px, rgb};
    Theme::update(cx, |theme| {
        theme.radius = px(6.);
        theme.radius_lg = px(12.);
        theme.font_size = px(14.);
        theme.colors.background = rgb(SURFACE).into();
        theme.colors.foreground = rgb(TEXT).into();
        theme.colors.border = rgb(BORDER).into();
        theme.colors.input = rgb(BORDER).into();
        theme.colors.muted = rgb(CANVAS).into();
        theme.colors.muted_foreground = rgb(MUTED).into();
        theme.colors.accent = rgb(SELECTED).into();
        theme.colors.accent_foreground = rgb(ACCENT).into();
        theme.colors.ring = rgb(ACCENT).into();
        theme.colors.primary = rgb(ACCENT).into();
        theme.colors.primary_hover = rgb(0x2064c0).into();
        theme.colors.primary_active = rgb(0x194e99).into();
        theme.colors.primary_foreground = rgb(SURFACE).into();
        theme.colors.button_primary = rgb(ACCENT).into();
        theme.colors.button_primary_hover = rgb(0x2064c0).into();
        theme.colors.button_primary_active = rgb(0x194e99).into();
        theme.colors.button_primary_foreground = rgb(SURFACE).into();
        theme.colors.button_hover = rgb(SELECTED).into();
        theme.colors.button_active = rgb(SELECTED).into();
    });
}
