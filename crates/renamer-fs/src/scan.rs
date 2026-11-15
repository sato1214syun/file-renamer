use std::collections::HashSet;
use std::path::{Path, PathBuf};

use renamer_core::{FileEntry, Result, TargetType, natural_cmp};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    pub recursive: bool,
    pub target: TargetType,
    pub include_hidden: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: false,
            target: TargetType::Files,
            include_hidden: false,
        }
    }
}

/// 入力（フォルダ・ファイルの混在可）からリネーム対象を列挙する。
///
/// - フォルダはその中身を走査する（フォルダ自体は対象にしない）
/// - ファイルはそのまま対象にする
/// - 結果は自然順にソートされ、重複は除かれる
pub fn scan_inputs(inputs: &[PathBuf], options: &ScanOptions) -> Result<Vec<FileEntry>> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for input in inputs {
        let meta = std::fs::metadata(input)
            .map_err(|e| renamer_core::RenamerError::io(input.clone(), e))?;

        if meta.is_dir() {
            let max_depth = if options.recursive { usize::MAX } else { 1 };
            let walker = WalkDir::new(input)
                .min_depth(1)
                .max_depth(max_depth)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| options.include_hidden || !is_hidden(e.path()));

            for entry in walker {
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        tracing::warn!("走査をスキップ: {err}");
                        continue;
                    }
                };
                let is_dir = entry.file_type().is_dir();
                if options.target.accepts(is_dir) && seen.insert(entry.path().to_path_buf()) {
                    entries.push(FileEntry {
                        path: entry.into_path(),
                        is_dir,
                    });
                }
            }
        } else if options.target.accepts(false) && seen.insert(input.clone()) {
            entries.push(FileEntry {
                path: input.clone(),
                is_dir: false,
            });
        }
    }

    entries.sort_by(|a, b| natural_cmp(&a.path.to_string_lossy(), &b.path.to_string_lossy()));
    Ok(entries)
}

fn is_hidden(path: &Path) -> bool {
    let dot = path
        .file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false);
    dot || has_hidden_attribute(path)
}

#[cfg(windows)]
fn has_hidden_attribute(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    std::fs::symlink_metadata(path)
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn has_hidden_attribute(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::TempDir;

    #[test]
    fn scan_flat_and_recursive() {
        let dir = TempDir::new();
        dir.touch("b10.txt");
        dir.touch("b2.txt");
        dir.touch(".hidden");
        dir.touch("sub/c.txt");

        let flat = scan_inputs(&[dir.path().to_path_buf()], &ScanOptions::default()).unwrap();
        let names: Vec<_> = flat
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["b2.txt", "b10.txt"]);

        let rec = scan_inputs(
            &[dir.path().to_path_buf()],
            &ScanOptions {
                recursive: true,
                target: TargetType::Both,
                include_hidden: false,
            },
        )
        .unwrap();
        assert_eq!(rec.len(), 4); // b2, b10, sub, sub/c
    }
}
