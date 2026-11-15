//! 前回終了時の状態（ルール・オプション・テーマ・各領域のサイズ）を自動保存・復元する。

use std::path::PathBuf;

use renamer_fs::Preset;
use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

/// 境界のドラッグで変えられる各領域のサイズ（px）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// 左パネル（ルール）の幅
    pub left_width: f32,
    pub preset_height: f32,
    pub rule_list_height: f32,
    pub editor_height: f32,
    /// 「現在の名前」「新しい名前」列の幅（「場所」は残り幅）
    pub column_widths: [f32; 2],
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            left_width: 420.0,
            preset_height: 90.0,
            rule_list_height: 180.0,
            editor_height: 260.0,
            column_widths: [320.0, 320.0],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub preset: Preset,
    pub theme: ThemeMode,
    pub layout: Layout,
}

impl Session {
    fn path() -> Option<PathBuf> {
        renamer_fs::app_data_dir().map(|d| d.join("session.json"))
    }

    pub fn load() -> Option<Self> {
        let bytes = std::fs::read(Self::path()?).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self).ok()
    }

    pub fn write_json(json: &str) -> std::io::Result<()> {
        let path = Self::path().ok_or_else(|| std::io::Error::other("保存先を特定できません"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, json)
    }
}
