//! Semantic application colors. Terminal ANSI/OSC content has an independent palette.

use gpui_kit::{
    App, Window, WindowAppearance,
    component::{Theme, ThemeMode},
    px, rgb,
};
use keelshell_core::Theme as Preference;

/// A copied render snapshot: closures do not retain a borrow of the UI context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub canvas: u32,
    pub surface: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub selected: u32,
    pub primary: u32,
    pub primary_hover: u32,
    pub primary_active: u32,
    pub danger: u32,
    pub danger_surface: u32,
    pub danger_border: u32,
    pub warning: u32,
    pub success: u32,
}

impl Palette {
    pub const LIGHT: Self = Self {
        canvas: 0xf6f8fb,
        surface: 0xffffff,
        border: 0xdce3ec,
        text: 0x1d2939,
        muted: 0x5d6c80,
        accent: 0x2166c5,
        selected: 0xe8f1ff,
        primary: 0x2166c5,
        primary_hover: 0x1c58ad,
        primary_active: 0x16478d,
        danger: 0xb32638,
        danger_surface: 0xfff0f2,
        danger_border: 0xf2bdc6,
        warning: 0x805500,
        success: 0x187445,
    };
    pub const DARK: Self = Self {
        canvas: 0x101927,
        surface: 0x172231,
        border: 0x34465b,
        text: 0xe5edf6,
        muted: 0xa0afc0,
        accent: 0x83b6ff,
        selected: 0x223e5b,
        primary: 0x316fca,
        primary_hover: 0x2862b7,
        primary_active: 0x2255a0,
        danger: 0xff97a6,
        danger_surface: 0x3a202a,
        danger_border: 0x774050,
        warning: 0xefc176,
        success: 0x76d5a1,
    };
}

pub fn palette(cx: &App) -> Palette {
    if cx
        .try_global::<Theme>()
        .is_some_and(|theme| theme.mode.is_dark())
    {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

pub fn resolve(preference: Preference, appearance: WindowAppearance) -> ThemeMode {
    match preference {
        Preference::Light => ThemeMode::Light,
        Preference::Dark => ThemeMode::Dark,
        Preference::System => appearance.into(),
    }
}

/// Clear the native override before reading System: the macOS window cache may
/// still hold the previous explicit value until its appearance notification.
pub fn apply(preference: Preference, window: Option<&mut Window>, cx: &mut App) {
    cx.set_window_appearance(match preference {
        Preference::System => None,
        Preference::Light => Some(WindowAppearance::Light),
        Preference::Dark => Some(WindowAppearance::Dark),
    });
    #[cfg(target_os = "macos")]
    let appearance = cx.window_appearance();
    #[cfg(not(target_os = "macos"))]
    let appearance = window
        .as_ref()
        .map_or_else(|| cx.window_appearance(), |window| window.appearance());
    install(resolve(preference, appearance), window, cx);
}

/// System notifications change only visuals, without configuration writes.
pub fn sync_system(appearance: WindowAppearance, window: &mut Window, cx: &mut App) {
    let mode = resolve(Preference::System, appearance);
    if cx
        .try_global::<Theme>()
        .is_none_or(|theme| theme.mode != mode)
    {
        install(mode, Some(window), cx);
    }
}

fn install(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window, cx);
    let p = palette(cx);
    Theme::update(cx, |theme| {
        theme.radius = px(6.);
        theme.radius_lg = px(12.);
        theme.font_size = px(14.);
        theme.colors.background = rgb(p.surface).into();
        theme.colors.foreground = rgb(p.text).into();
        theme.colors.border = rgb(p.border).into();
        theme.colors.input = rgb(p.border).into();
        theme.colors.muted = rgb(p.canvas).into();
        theme.colors.muted_foreground = rgb(p.muted).into();
        theme.colors.accent = rgb(p.selected).into();
        theme.colors.accent_foreground = rgb(p.accent).into();
        theme.colors.ring = rgb(p.accent).into();
        theme.colors.primary = rgb(p.primary).into();
        theme.colors.primary_hover = rgb(p.primary_hover).into();
        theme.colors.primary_active = rgb(p.primary_active).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary = rgb(p.primary).into();
        theme.colors.button_primary_hover = rgb(p.primary_hover).into();
        theme.colors.button_primary_active = rgb(p.primary_active).into();
        theme.colors.button_primary_foreground = rgb(0xffffff).into();
        theme.colors.button_hover = rgb(p.selected).into();
        theme.colors.button_active = rgb(p.selected).into();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(rgb: u32) -> f64 {
        let channel = |shift: u32| {
            let value = f64::from((rgb >> shift) & 255u32) / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }
    fn contrast(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_and_action_labels_have_sufficient_contrast_in_both_modes() {
        for p in [Palette::LIGHT, Palette::DARK] {
            for background in [p.canvas, p.surface, p.selected] {
                assert!(contrast(p.text, background) >= 4.5);
                assert!(contrast(p.muted, background) >= 4.5);
                assert!(contrast(p.accent, background) >= 4.5);
            }
            assert!(contrast(0xffffff, p.primary) >= 4.5);
            assert!(contrast(p.danger, p.danger_surface) >= 4.5);
            assert!(contrast(p.warning, p.surface) >= 4.5);
            assert!(contrast(p.success, p.surface) >= 4.5);
        }
    }

    #[test]
    fn explicit_preferences_ignore_system_and_system_accepts_vibrant_modes() {
        for appearance in [
            WindowAppearance::Light,
            WindowAppearance::Dark,
            WindowAppearance::VibrantLight,
            WindowAppearance::VibrantDark,
        ] {
            assert_eq!(resolve(Preference::Light, appearance), ThemeMode::Light);
            assert_eq!(resolve(Preference::Dark, appearance), ThemeMode::Dark);
        }
        assert_eq!(
            resolve(Preference::System, WindowAppearance::VibrantDark),
            ThemeMode::Dark
        );
        assert_eq!(
            resolve(Preference::System, WindowAppearance::VibrantLight),
            ThemeMode::Light
        );
    }
}
