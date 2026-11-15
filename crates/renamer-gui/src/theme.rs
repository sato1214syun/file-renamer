//! 配色。ライト / ダークの 2 パレットと、システム設定に従うモードを持つ。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    /// OS の外観設定に従う
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "システム",
            Self::Light => "ライト",
            Self::Dark => "ダーク",
        }
    }
}

pub struct Theme {
    pub bg: u32,
    pub panel: u32,
    pub input_bg: u32,
    pub border: u32,
    pub row_border: u32,
    pub text: u32,
    pub muted: u32,
    pub disabled: u32,
    pub hover: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub accent_soft: u32,
    pub on_accent: u32,
    pub selected: u32,
    pub error: u32,
    pub error_bg: u32,
    pub scrollbar: u32,
    pub scrollbar_hover: u32,
}

pub static LIGHT: Theme = Theme {
    bg: 0xf6f7f9,
    panel: 0xffffff,
    input_bg: 0xffffff,
    border: 0xd9dce1,
    row_border: 0xeef0f3,
    text: 0x1f2328,
    muted: 0x6b7280,
    disabled: 0xa3a9b3,
    hover: 0xf1f5f9,
    accent: 0x2563eb,
    accent_hover: 0x1d4ed8,
    accent_soft: 0xdbeafe,
    on_accent: 0xffffff,
    selected: 0xdbe6fe,
    error: 0xb91c1c,
    error_bg: 0xfef2f2,
    scrollbar: 0xc4c8cf,
    scrollbar_hover: 0x9aa0a9,
};

pub static DARK: Theme = Theme {
    bg: 0x1e1f22,
    panel: 0x2b2d30,
    input_bg: 0x1e1f22,
    border: 0x43454a,
    row_border: 0x35373b,
    text: 0xdfe1e5,
    muted: 0x9da0a8,
    disabled: 0x6b6f76,
    hover: 0x393b40,
    accent: 0x3b82f6,
    accent_hover: 0x2563eb,
    accent_soft: 0x1e3a5f,
    on_accent: 0xffffff,
    selected: 0x2c3e63,
    error: 0xf87171,
    error_bg: 0x3a2225,
    scrollbar: 0x4e5157,
    scrollbar_hover: 0x6b6f76,
};
