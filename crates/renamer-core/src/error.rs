use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenamerError {
    #[error("ファイルが見つかりません: {0}")]
    NotFound(PathBuf),

    #[error("変更先が既に存在します: {0}")]
    DestinationExists(PathBuf),

    #[error("無効な名前: {0}")]
    InvalidName(String),

    #[error("ルールエラー: {0}")]
    Rule(String),

    #[error("エラーのある項目が {0} 件あるため実行できません")]
    PlanHasErrors(usize),

    #[error("I/O エラー ({path}): {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl RenamerError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, RenamerError>;
