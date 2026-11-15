use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 走査で得られた 1 エントリ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// リネーム対象の種類。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetType {
    #[default]
    Files,
    Directories,
    Both,
}

impl TargetType {
    pub const ALL: [Self; 3] = [Self::Files, Self::Directories, Self::Both];

    pub fn label(self) -> &'static str {
        match self {
            Self::Files => "ファイル",
            Self::Directories => "フォルダ",
            Self::Both => "ファイルとフォルダ",
        }
    }

    pub fn accepts(self, is_dir: bool) -> bool {
        match self {
            Self::Files => !is_dir,
            Self::Directories => is_dir,
            Self::Both => true,
        }
    }
}

/// プラン生成時のオプション。名前の検査は実行中の OS に合わせて自動で行う。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanOptions {
    /// true のとき拡張子を含めた名前全体にルールを適用する。
    pub include_extension: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateStatus {
    /// リネーム可能。
    Ready,
    /// 名前が変わらない。
    Unchanged,
    /// ユーザーが対象外にした。
    Excluded,
    /// 変更後の名前が無効。
    Invalid(String),
    /// 重複・既存ファイルと衝突。
    Conflict(String),
    /// ルール自体のエラー（詳細は `RenamePlan::rule_error`）。
    RuleError,
}

impl CandidateStatus {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Invalid(_) | Self::Conflict(_) | Self::RuleError)
    }

    /// 行ごとのエラー理由（ルールエラーは行には表示しない）。
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Invalid(msg) | Self::Conflict(msg) => Some(msg),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameCandidate {
    pub source: PathBuf,
    pub destination: PathBuf,
    /// 変更後の名前（無効な名前でも表示できるよう文字列で保持）。
    pub new_name: String,
    pub is_dir: bool,
    pub status: CandidateStatus,
}

impl RenameCandidate {
    pub fn source_name(&self) -> String {
        self.source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn will_rename(&self) -> bool {
        self.status == CandidateStatus::Ready
    }

    /// 実行後にこの項目が占有するパス。
    pub fn final_path(&self) -> &PathBuf {
        if self.will_rename() {
            &self.destination
        } else {
            &self.source
        }
    }
}
