//! Semantic application colors, shared by custom views and the Kit adapter.

/// A complete family variant. Foreground roles are tested against their surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub canvas: u32,
    pub surface: u32,
    pub chrome: u32,
    pub sidebar: u32,
    pub border: u32,
    pub field_border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub selected: u32,
    pub primary: u32,
    pub primary_hover: u32,
    pub primary_active: u32,
    pub primary_text: u32,
    pub danger: u32,
    pub danger_surface: u32,
    pub danger_border: u32,
    pub warning: u32,
    pub success: u32,
    pub status_text: u32,
}

impl Palette {
    /// Frost light, also the fallback for legacy preferences.
    pub const LIGHT: Self = Self {
        canvas: 0xf1f4f8,
        surface: 0xffffff,
        chrome: 0xf8fafc,
        sidebar: 0xf4f6f9,
        border: 0xdce3ec,
        field_border: 0x7d8999,
        text: 0x1d2939,
        muted: 0x526175,
        accent: 0x205eb7,
        selected: 0xe8f0fc,
        primary: 0x2166c5,
        primary_hover: 0x1c58ad,
        primary_active: 0x16478d,
        primary_text: 0xffffff,
        danger: 0xb32638,
        danger_surface: 0xfff0f2,
        danger_border: 0xf2bdc6,
        warning: 0x805500,
        success: 0x187445,
        status_text: 0xffffff,
    };
    /// Frost dark; used when System appearance becomes dark.
    pub const DARK: Self = Self {
        canvas: 0x101927,
        surface: 0x172231,
        chrome: 0x141e2c,
        sidebar: 0x121c29,
        border: 0x34465b,
        field_border: 0x718299,
        text: 0xe5edf6,
        muted: 0xaab8ca,
        accent: 0x91bdff,
        selected: 0x223e5b,
        primary: 0x316fca,
        primary_hover: 0x2862b7,
        primary_active: 0x2255a0,
        primary_text: 0xffffff,
        danger: 0xff97a6,
        danger_surface: 0x3a202a,
        danger_border: 0x774050,
        warning: 0xefc176,
        success: 0x76d5a1,
        status_text: 0x101927,
    };
}
