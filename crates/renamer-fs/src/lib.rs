//! ファイルシステムに触れる処理はすべてこのクレートに集約する。

pub mod execute;
pub mod preset;
pub mod scan;
pub mod undo;

use std::path::PathBuf;

pub use execute::{Operation, execute_plan};
pub use preset::{Preset, delete_preset, list_presets, load_preset, save_preset};
pub use scan::{ScanOptions, scan_inputs};
pub use undo::{UndoJournal, undo};

/// アプリのデータ保存先（`.../flex-renamer`）。
///
/// - Windows: `%APPDATA%\flex-renamer`
/// - macOS: `~/Library/Application Support/flex-renamer`
/// - Linux: `$XDG_DATA_HOME/flex-renamer` または `~/.local/share/flex-renamer`
pub fn app_data_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join("flex-renamer"))
}

#[cfg(test)]
pub(crate) mod test_util {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// テスト用の一時ディレクトリ（Drop で削除）。
    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "renamer-fs-test-{}-{nanos}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }

        pub fn touch(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, rel.as_bytes()).unwrap();
            p
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
