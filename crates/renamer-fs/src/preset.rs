//! ルールとオプションの保存・読み込み（プリセット）。

use std::fs;
use std::path::{Path, PathBuf};

use renamer_core::{
    PlanOptions, Platform, RenamerError, Result, RuleEntry, natural_cmp, validate_name,
    validate_name_for,
};
use serde::{Deserialize, Serialize};

use crate::scan::ScanOptions;

pub const PRESET_VERSION: u32 = 1;
const EXTENSION: &str = "json";

/// 変換の状態一式（ルール + オプション）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub version: u32,
    pub rules: Vec<RuleEntry>,
    pub plan_options: PlanOptions,
    pub scan_options: ScanOptions,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            version: PRESET_VERSION,
            rules: Vec::new(),
            plan_options: PlanOptions::default(),
            scan_options: ScanOptions::default(),
        }
    }
}

impl Preset {
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| RenamerError::io(dir, e))?;
        }
        let json =
            serde_json::to_vec_pretty(self).map_err(|e| RenamerError::Other(e.to_string()))?;
        fs::write(path, json).map_err(|e| RenamerError::io(path, e))
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).map_err(|e| RenamerError::io(path, e))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| RenamerError::Other(format!("プリセットを読み込めません: {e}")))
    }
}

/// プリセットの保存フォルダ。
pub fn presets_dir() -> Result<PathBuf> {
    crate::app_data_dir()
        .map(|d| d.join("presets"))
        .ok_or_else(|| RenamerError::Other("保存先を特定できません".into()))
}

fn preset_path(name: &str) -> Result<PathBuf> {
    let name = name.trim();
    // 名前は OS をまたいで共有できるよう、最も厳しい Windows の規則でも検査する
    validate_name_for(name, Platform::Windows)
        .and_then(|_| validate_name(name))
        .map_err(|e| RenamerError::InvalidName(format!("プリセット名: {e}")))?;
    Ok(presets_dir()?.join(format!("{name}.{EXTENSION}")))
}

/// 保存済みプリセット名の一覧（自然順）。
pub fn list_presets() -> Vec<String> {
    let Ok(dir) = presets_dir() else {
        return Vec::new();
    };
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == EXTENSION))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    names
}

pub fn save_preset(name: &str, preset: &Preset) -> Result<()> {
    preset.save_to(&preset_path(name)?)
}

pub fn load_preset(name: &str) -> Result<Preset> {
    Preset::load_from(&preset_path(name)?)
}

pub fn delete_preset(name: &str) -> Result<()> {
    let path = preset_path(name)?;
    fs::remove_file(&path).map_err(|e| RenamerError::io(path, e))
}

#[cfg(test)]
mod tests {
    use renamer_core::{RenameRule, TargetType};

    use super::*;
    use crate::test_util::TempDir;

    #[test]
    fn roundtrip() {
        let dir = TempDir::new();
        let path = dir.path().join("p.json");
        let preset = Preset {
            rules: RenameRule::templates()
                .into_iter()
                .map(RuleEntry::new)
                .collect(),
            scan_options: ScanOptions {
                recursive: true,
                target: TargetType::Both,
                include_hidden: true,
            },
            ..Default::default()
        };
        preset.save_to(&path).unwrap();
        assert_eq!(Preset::load_from(&path).unwrap(), preset);
    }

    #[test]
    fn old_and_partial_json_load() {
        let p: Preset = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Preset::default());
        // v0.2 の portable_names は無視される
        let p: Preset = serde_json::from_str(
            r#"{"plan_options":{"include_extension":true,"portable_names":true}}"#,
        )
        .unwrap();
        assert!(p.plan_options.include_extension);
    }

    #[test]
    fn rejects_bad_names() {
        assert!(preset_path("a/b").is_err());
        assert!(preset_path("a:b").is_err());
        assert!(preset_path("").is_err());
        assert!(preset_path("  ").is_err());
    }
}
