use super::*;
use gpui_kit::{TestAppContext, px, rgb};

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

fn alpha_blend(foreground: u32, background: u32, alpha: f64) -> u32 {
    let channel = |shift: u32| {
        let foreground = f64::from((foreground >> shift) & 255u32);
        let background = f64::from((background >> shift) & 255u32);
        (foreground * alpha + background * (1. - alpha)).floor() as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn alpha(color: u32) -> u8 {
    (color & 0xff) as u8
}

#[test]
fn every_registered_variant_has_readable_text_actions_and_input_boundaries() {
    for theme in registry() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let p = theme.palette(mode);
            for (role, foreground) in [("text", p.text), ("muted", p.muted), ("accent", p.accent)] {
                for background in [
                    p.canvas,
                    p.surface,
                    p.surface_hover,
                    p.surface_active,
                    p.selected,
                    p.chrome,
                    p.toolbar,
                    p.sidebar,
                ] {
                    assert!(
                        contrast(foreground, background) >= 4.5,
                        "{}/{mode:?}/{role}: {foreground:x} on {background:x} = {}",
                        theme.id,
                        contrast(foreground, background)
                    );
                }
            }
            for background in [p.primary, p.primary_hover, p.primary_active] {
                assert!(
                    contrast(p.primary_text, background) >= 4.5,
                    "{}/{mode:?} action",
                    theme.id
                );
            }
            assert!(contrast(p.danger, p.danger_surface) >= 4.5);
            for color in [p.danger, p.warning, p.success] {
                assert!(
                    contrast(color, p.surface) >= 4.5,
                    "{}/{mode:?} status",
                    theme.id
                );
                assert!(
                    contrast(p.status_text, color) >= 4.5,
                    "{}/{mode:?} status button",
                    theme.id
                );
            }
            assert!(
                contrast(p.field_border, p.surface) >= 3.,
                "{}/{mode:?} input",
                theme.id
            );
            assert!(
                contrast(p.focus, p.surface) >= 3.,
                "{}/{mode:?} focus ring",
                theme.id
            );
            for background in [p.surface, p.chrome, p.sidebar] {
                let composited = alpha_blend(p.focus, background, 0.5);
                assert!(
                    contrast(composited, background) >= 3.,
                    "{}/{mode:?} composited focus ring: {composited:x} on {background:x} = {}",
                    theme.id,
                    contrast(composited, background)
                );
            }
            assert_ne!(p.resource, 0, "{}/{mode:?} resource token", theme.id);
            assert!(
                contrast(p.resource, p.border) >= 3.,
                "{}/{mode:?} resource meter: {} on {} = {}",
                theme.id,
                p.resource,
                p.border,
                contrast(p.resource, p.border)
            );
            assert!(
                alpha(p.scrim) > 0 && alpha(p.scrim) < u8::MAX,
                "{}/{mode:?} scrim must be translucent: {:#010x}",
                theme.id,
                p.scrim
            );
            assert!(
                alpha(p.overlay) > alpha(p.scrim) && alpha(p.overlay) < u8::MAX,
                "{}/{mode:?} overlay must be stronger than scrim: {:#010x} vs {:#010x}",
                theme.id,
                p.scrim,
                p.overlay
            );
            assert_ne!(
                p.scrim, p.overlay,
                "{}/{mode:?} scrim and overlay need separate semantic tokens",
                theme.id
            );
            assert_ne!(
                p.surface_hover, p.surface_active,
                "{}/{mode:?} hover and active need separate surfaces",
                theme.id
            );
        }
        assert!(contrast(theme.terminal.foreground, theme.terminal.background) >= 7.);
        assert!(contrast(theme.terminal.foreground, theme.terminal.selection) >= 4.5);
        assert!(contrast(theme.terminal.cursor, theme.terminal.background) >= 3.);
    }
}

#[test]
fn registry_identifiers_are_valid_unique_and_unknown_families_fall_back() {
    let mut ids = std::collections::HashSet::new();
    for theme in registry() {
        assert!(keelshell_core::ThemeId::new(theme.id).is_ok());
        assert!(ids.insert(theme.id));
        assert_eq!(definition(theme.id).id, theme.id);
        assert!(!theme.name_zh.is_empty() && !theme.name_en.is_empty());
        assert!(!theme.description_zh.is_empty() && !theme.description_en.is_empty());
        assert!(matches!(
            theme.preferred_appearance,
            Preference::Light | Preference::Dark
        ));
        assert!(theme.chrome.toolbar_height >= 36.);
        assert!(theme.chrome.session_strip_height >= theme.chrome.session_tab_height);
        assert!(theme.chrome.command_input_height >= 56.);
        assert!(theme.chrome.status_bar_height >= 24.);
        assert!(theme.chrome.pane_header_height >= 24.);
    }
    assert_eq!(definition("future-theme-v2").id, "frost");
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

#[gpui_kit::test]
fn kit_controls_and_custom_views_receive_the_same_family_snapshot(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        for family in registry() {
            for mode in [Preference::Light, Preference::Dark] {
                let settings = Settings {
                    theme_family: keelshell_core::ThemeId::new(family.id)
                        .unwrap_or_else(|error| panic!("registered ID: {error}")),
                    theme: mode,
                    ..Default::default()
                };
                apply_settings(&settings, None, cx);
                let p = palette(cx);
                let kit = cx.global::<Theme>();
                assert_eq!(current_definition(cx).id, family.id);
                assert_eq!(kit.colors.background, rgb(p.surface).into());
                assert_eq!(kit.colors.accent, rgb(p.surface_hover).into());
                assert_eq!(kit.colors.button_active, rgb(p.surface_active).into());
                assert_eq!(kit.colors.ring, rgb(p.focus).into());
                assert_eq!(
                    kit.colors.button_primary_foreground,
                    rgb(p.primary_text).into()
                );
                assert_eq!(kit.colors.input, rgb(p.field_border).into());
                assert_eq!(kit.colors.list_active, rgb(p.surface_active).into());
                assert_eq!(kit.colors.table_head, rgb(p.toolbar).into());
                assert_eq!(kit.colors.tab_bar, rgb(p.toolbar).into());
                assert_eq!(kit.colors.popover, rgb(p.surface).into());
                assert_eq!(kit.colors.sidebar, rgb(p.sidebar).into());
                assert_eq!(kit.radius, px(family.components.control_radius));
                assert_eq!(kit.radius_lg, px(family.components.panel_radius));
                assert_eq!(chrome(cx), family.chrome);
            }
        }
        let settings = Settings {
            theme_family: keelshell_core::ThemeId::new("unavailable-theme")
                .unwrap_or_else(|error| panic!("valid unknown ID: {error}")),
            theme: Preference::Dark,
            ..Default::default()
        };
        apply_settings(&settings, None, cx);
        assert_eq!(palette(cx), Palette::DARK);
        assert_eq!(settings.theme_family.as_str(), "unavailable-theme");
    });
}
