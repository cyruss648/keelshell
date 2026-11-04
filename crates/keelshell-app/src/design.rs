//! Application-owned theme registry and the single GPUI Kit adapter.
//!
//! Views consume semantic tokens, never theme identifiers. Appearance selects a
//! light/dark variant; it does not own density, pane geometry, or terminal state.

mod kit;
mod palette;
mod registry;
#[cfg(test)]
mod tests;

pub use palette::Palette;
pub use registry::{ThemeDefinition, registry};

use gpui_kit::{
    App, Global, Window, WindowAppearance,
    component::{Theme, ThemeMode},
};
use keelshell_core::{Settings, Theme as Preference};

/// Defaults only: remote ANSI, truecolor and OSC overrides retain precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPalette {
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
    pub selection: u32,
}

/// Geometry of controls and panels, independent of workspace layout and density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentTokens {
    pub control_radius: f32,
    pub panel_radius: f32,
}

struct InstalledTheme(&'static ThemeDefinition);
impl Global for InstalledTheme {}

/// Resolve an unavailable family without overwriting its persisted identifier.
pub fn definition(id: &str) -> &'static ThemeDefinition {
    registry()
        .iter()
        .find(|theme| theme.id == id)
        .unwrap_or(&registry::FROST)
}

/// The effective registered theme, including a safe fallback for legacy settings.
pub fn current_definition(cx: &App) -> &'static ThemeDefinition {
    cx.try_global::<InstalledTheme>()
        .map_or(&registry::FROST, |theme| theme.0)
}

/// A copied render snapshot: closures never retain a borrow of the UI context.
pub fn palette(cx: &App) -> Palette {
    current_definition(cx).palette(
        cx.try_global::<Theme>()
            .map_or(ThemeMode::Light, |theme| theme.mode),
    )
}

/// Theme defaults for terminal rendering; no terminal instance is recreated.
pub fn terminal_palette(cx: &App) -> TerminalPalette {
    current_definition(cx).terminal
}

/// Shared shape tokens for custom application surfaces.
pub fn components(cx: &App) -> ComponentTokens {
    current_definition(cx).components
}

pub fn resolve(preference: Preference, appearance: WindowAppearance) -> ThemeMode {
    match preference {
        Preference::Light => ThemeMode::Light,
        Preference::Dark => ThemeMode::Dark,
        Preference::System => appearance.into(),
    }
}

/// Install both persisted axes after startup, successful saving or state reload.
pub fn apply_settings(settings: &Settings, window: Option<&mut Window>, cx: &mut App) {
    apply_definition(
        definition(settings.theme_family.as_str()),
        settings.theme,
        window,
        cx,
    );
}

/// Change appearance while retaining the selected family.
#[cfg(test)]
pub fn apply(preference: Preference, window: Option<&mut Window>, cx: &mut App) {
    apply_definition(current_definition(cx), preference, window, cx);
}

fn apply_definition(
    definition: &'static ThemeDefinition,
    preference: Preference,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    // Clear the native override before reading System. The macOS window cache
    // can still contain an earlier explicit value until its notification.
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
    install(definition, resolve(preference, appearance), window, cx);
}

/// System notifications update only visuals, without configuration writes.
pub fn sync_system(appearance: WindowAppearance, window: &mut Window, cx: &mut App) {
    let mode = resolve(Preference::System, appearance);
    if cx
        .try_global::<Theme>()
        .is_none_or(|theme| theme.mode != mode)
    {
        install(current_definition(cx), mode, Some(window), cx);
    }
}

fn install(
    definition: &'static ThemeDefinition,
    mode: ThemeMode,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    cx.set_global(InstalledTheme(definition));
    Theme::change(mode, window, cx);
    kit::install(definition.palette(mode), definition.components, cx);
    cx.refresh_windows();
}
