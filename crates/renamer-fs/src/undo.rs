use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use renamer_core::{RenamerError, Result};
use serde::{Deserialize, Serialize};

use crate::execute::{Operation, Order, run_operations};

/// 実行済み操作の記録。JSON で保存し、アプリ再起動後も Undo できる。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoJournal {
    pub created_at_unix: u64,
    pub operations: Vec<Operation>,
}

impl UndoJournal {
    pub fn new(operations: Vec<Operation>) -> Self {
        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            created_at_unix,
            operations,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

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
        serde_json::from_slice(&bytes).map_err(|e| RenamerError::Other(e.to_string()))
    }

    /// 既定の保存先（直近 1 回分）。
    pub fn default_path() -> Option<PathBuf> {
        crate::app_data_dir().map(|d| d.join("last-undo.json"))
    }

    pub fn save_default(&self) -> Result<()> {
        let path = Self::default_path()
            .ok_or_else(|| RenamerError::Other("保存先を特定できません".into()))?;
        self.save_to(&path)
    }

    pub fn load_default() -> Option<Self> {
        Self::load_from(&Self::default_path()?).ok()
    }

    pub fn clear_default() {
        if let Some(path) = Self::default_path() {
            let _ = fs::remove_file(path);
        }
    }
}

/// ジャーナルの操作を逆順に戻す。戻した操作のジャーナルを返す。
///
/// 存在確認はフォルダ単位の実行時に行う（親フォルダを先に戻すまで子のパスは存在しないため）。
/// 途中で失敗した場合は、戻した分を再適用して Undo 実行前の状態に戻す。
pub fn undo(journal: &UndoJournal) -> Result<UndoJournal> {
    let reversed: Vec<Operation> = journal
        .operations
        .iter()
        .rev()
        .map(|op| Operation {
            from: op.to.clone(),
            to: op.from.clone(),
        })
        .collect();
    let done = run_operations(reversed, Order::ShallowestFirst)?;
    Ok(UndoJournal::new(done))
}
