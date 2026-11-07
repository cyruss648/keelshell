//! Built-in families. Adding a theme requires a definition and one registry entry.

use super::{ChromeTokens, ComponentTokens, Palette, TerminalPalette, TypographyTokens};
use gpui_kit::component::ThemeMode;
use keelshell_core::Theme;

/// One catalog entry owns its metadata, variants, component shapes and defaults.
#[derive(Clone, Copy)]
pub struct ThemeDefinition {
    pub id: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub description_zh: &'static str,
    pub description_en: &'static str,
    /// Applied together with the family by its explicitly labeled preset button.
    pub preferred_appearance: Theme,
    pub light: Palette,
    pub dark: Palette,
    pub components: ComponentTokens,
    pub typography: TypographyTokens,
    pub chrome: ChromeTokens,
    pub terminal: TerminalPalette,
}

impl ThemeDefinition {
    /// Resolve a complete variant; business views do not branch on family IDs.
    pub fn palette(&self, mode: ThemeMode) -> Palette {
        if mode.is_dark() {
            self.dark
        } else {
            self.light
        }
    }
}

pub(super) const FROST: ThemeDefinition = ThemeDefinition {
    id: "frost",
    name_zh: "霜白",
    name_en: "Frost",
    description_zh: "清透灰蓝 · 轻盈边框 · 清晰层次",
    description_en: "Cool neutrals, fine borders, clear layers",
    preferred_appearance: Theme::Light,
    light: Palette::LIGHT,
    dark: Palette::DARK,
    components: ComponentTokens {
        control_radius: 6.,
        panel_radius: 8.,
    },
    typography: TypographyTokens {
        code_font_family: "monospace",
        code_font_weight: gpui_kit::FontWeight::NORMAL,
    },
    chrome: ChromeTokens {
        toolbar_height: 42.,
        session_strip_height: 40.,
        session_tab_height: 34.,
        tool_tabs_height: 32.,
        command_input_height: 64.,
        status_bar_height: 30.,
        pane_header_height: 28.,
    },
    terminal: TerminalPalette {
        foreground: 0xd9e3ed,
        background: 0x0c121b,
        cursor: 0x7bdec5,
        selection: 0x304563,
    },
};

const GRAPHITE: ThemeDefinition = ThemeDefinition {
    id: "graphite",
    name_zh: "曜石",
    name_en: "Graphite",
    description_zh: "温暖石墨 · 琥珀点缀 · 利落轮廓",
    description_en: "Warm graphite, amber accents, precise edges",
    preferred_appearance: Theme::Dark,
    light: Palette {
        canvas: 0xf3f1ed,
        surface: 0xfffefa,
        surface_hover: 0xf8f2e9,
        surface_active: 0xf0e5d3,
        chrome: 0xf8f6f1,
        toolbar: 0xfffbf5,
        sidebar: 0xefede7,
        scrim: 0x251e1455,
        overlay: 0x251e1466,
        border: 0xd8d2c8,
        field_border: 0x8b8173,
        text: 0x2c2822,
        muted: 0x64594b,
        accent: 0x84500d,
        resource: 0x0f6f78,
        // Kit renders the ordinary focus ring at 50% alpha; keep the
        // composited ring visible against the light chrome and sidebar layers.
        focus: 0x3f2000,
        selected: 0xf0e5d3,
        primary: 0x89540f,
        primary_hover: 0x75470a,
        primary_active: 0x613b08,
        primary_text: 0xffffff,
        danger: 0xa52b35,
        danger_surface: 0xfdeeee,
        danger_border: 0xe7bcc0,
        warning: 0x805000,
        success: 0x28663d,
        status_text: 0xffffff,
    },
    dark: Palette {
        canvas: 0x141311,
        surface: 0x211f1c,
        surface_hover: 0x292621,
        surface_active: 0x33291f,
        chrome: 0x1c1a18,
        toolbar: 0x211e1a,
        sidebar: 0x1a1916,
        scrim: 0x00000077,
        overlay: 0x0a080699,
        border: 0x3e3932,
        field_border: 0x837b70,
        text: 0xf0ece4,
        muted: 0xbab1a4,
        accent: 0xf1b36c,
        resource: 0x69c8c2,
        focus: 0xffc98c,
        selected: 0x3a3024,
        primary: 0xefa95c,
        primary_hover: 0xf6bc7e,
        primary_active: 0xdc9850,
        primary_text: 0x24190d,
        danger: 0xf69d9f,
        danger_surface: 0x3b2426,
        danger_border: 0x795054,
        warning: 0xe8c082,
        success: 0x9dcfab,
        status_text: 0x141311,
    },
    components: ComponentTokens {
        control_radius: 4.,
        panel_radius: 6.,
    },
    typography: TypographyTokens {
        code_font_family: "monospace",
        code_font_weight: gpui_kit::FontWeight::MEDIUM,
    },
    chrome: ChromeTokens {
        toolbar_height: 42.,
        session_strip_height: 40.,
        session_tab_height: 34.,
        tool_tabs_height: 32.,
        command_input_height: 64.,
        status_bar_height: 30.,
        pane_header_height: 28.,
    },
    terminal: TerminalPalette {
        foreground: 0xe5dfd5,
        background: 0x11100e,
        cursor: 0xefb46f,
        selection: 0x4b3d2d,
    },
};

const LAVENDER: ThemeDefinition = ThemeDefinition {
    id: "lavender",
    name_zh: "雾紫",
    name_en: "Lavender",
    description_zh: "柔和紫灰 · 圆润组件 · 安静工作",
    description_en: "Soft violet, rounded controls, calm surfaces",
    preferred_appearance: Theme::Light,
    light: Palette {
        canvas: 0xece6f2,
        surface: 0xfdfaff,
        surface_hover: 0xf7f0fb,
        surface_active: 0xeee3f8,
        chrome: 0xf4edf9,
        toolbar: 0xfffaff,
        sidebar: 0xeee6f7,
        scrim: 0x2a214155,
        overlay: 0x2a214166,
        border: 0xd5c9e3,
        field_border: 0x9681ab,
        text: 0x332840,
        muted: 0x665272,
        accent: 0x6939ab,
        resource: 0x4e6f9d,
        // Kit renders the ordinary focus ring at 50% alpha; keep the
        // composited ring visible against the light chrome and sidebar layers.
        focus: 0x37105f,
        selected: 0xe4d8f2,
        primary: 0x7143b5,
        primary_hover: 0x60379d,
        primary_active: 0x502b87,
        primary_text: 0xffffff,
        danger: 0xa62e4b,
        danger_surface: 0xffedf2,
        danger_border: 0xe6b4c3,
        warning: 0x795014,
        success: 0x266840,
        status_text: 0xffffff,
    },
    dark: Palette {
        canvas: 0x19141f,
        surface: 0x241d2e,
        surface_hover: 0x2d2439,
        surface_active: 0x382b49,
        chrome: 0x211a2a,
        toolbar: 0x261f31,
        sidebar: 0x1e1826,
        scrim: 0x00000077,
        overlay: 0x0d091299,
        border: 0x493955,
        field_border: 0x927aa5,
        text: 0xf0e8f7,
        muted: 0xbdb0ca,
        accent: 0xd4b5ff,
        resource: 0x9bd2d7,
        focus: 0xe0c5ff,
        selected: 0x3c2e50,
        primary: 0xc9a3f4,
        primary_hover: 0xd9b9ff,
        primary_active: 0xb68bdf,
        primary_text: 0x291b38,
        danger: 0xffa2bb,
        danger_surface: 0x432535,
        danger_border: 0x81506a,
        warning: 0xe9c28c,
        success: 0xa4d6ba,
        status_text: 0x19141f,
    },
    components: ComponentTokens {
        control_radius: 12.,
        panel_radius: 14.,
    },
    typography: TypographyTokens {
        code_font_family: "monospace",
        code_font_weight: gpui_kit::FontWeight::SEMIBOLD,
    },
    chrome: ChromeTokens {
        toolbar_height: 42.,
        session_strip_height: 40.,
        session_tab_height: 34.,
        tool_tabs_height: 32.,
        command_input_height: 64.,
        status_bar_height: 30.,
        pane_header_height: 28.,
    },
    terminal: TerminalPalette {
        foreground: 0xeee4f7,
        background: 0x1d1629,
        cursor: 0xd2b4f2,
        selection: 0x4b3866,
    },
};

/// Stable picker ordering. Neither persistence nor views enumerate family IDs.
pub fn registry() -> &'static [ThemeDefinition] {
    static THEMES: [ThemeDefinition; 3] = [FROST, GRAPHITE, LAVENDER];
    &THEMES
}
