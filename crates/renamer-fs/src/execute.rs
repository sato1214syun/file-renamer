use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use renamer_core::{RenamePlan, RenamerError, Result};
use serde::{Deserialize, Serialize};

use crate::undo::UndoJournal;

/// 1 件のリネーム操作。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// プランを実行し、Undo 用のジャーナルを返す。
///
/// - エラーのある項目が 1 件でもあれば何もせずにエラーを返す
/// - 途中で失敗した場合は、それまでの変更をすべて巻き戻してからエラーを返す
pub fn execute_plan(plan: &RenamePlan) -> Result<UndoJournal> {
    if let Some(err) = &plan.rule_error {
        return Err(RenamerError::Rule(err.message.clone()));
    }
    let summary = plan.summary();
    if summary.errors > 0 {
        return Err(RenamerError::PlanHasErrors(summary.errors));
    }
    let ops: Vec<Operation> = plan
        .executable_items()
        .map(|c| Operation {
            from: c.source.clone(),
            to: c.destination.clone(),
        })
        .collect();

    let done = run_operations(ops, Order::DeepestFirst)?;
    Ok(UndoJournal::new(done))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
    /// 実行時: 子の項目を先に処理し、親フォルダのリネームで子のパスが変わらないようにする。
    DeepestFirst,
    /// Undo 時: 親フォルダを先に元の名前へ戻す。
    ShallowestFirst,
}

/// 同じフォルダ内の操作をまとめ、2 段階（元名 → 一時名 → 新名）で実行する。
///
/// 2 段階にすることで次のケースを安全に扱える。
/// - 名前の入れ替え（A→B, B→A）や循環
/// - 大文字小文字のみの変更（大文字小文字を区別しないファイルシステム）
pub(crate) fn run_operations(ops: Vec<Operation>, order: Order) -> Result<Vec<Operation>> {
    let mut groups: BTreeMap<PathBuf, Vec<Operation>> = BTreeMap::new();
    for op in ops {
        let parent = op.from.parent().map(Path::to_path_buf).unwrap_or_default();
        groups.entry(parent).or_default().push(op);
    }
    let mut groups: Vec<(PathBuf, Vec<Operation>)> = groups.into_iter().collect();
    groups.sort_by_key(|(parent, _)| parent.components().count());
    if order == Order::DeepestFirst {
        groups.reverse();
    }

    let mut done: Vec<Operation> = Vec::new();
    for (parent, group) in groups {
        if let Err(err) = run_group(&parent, &group, &mut done) {
            rollback(&done);
            return Err(err);
        }
    }
    Ok(done)
}

fn run_group(parent: &Path, group: &[Operation], done: &mut Vec<Operation>) -> Result<()> {
    // フェーズ 1: すべて一時名へ退避
    let mut temps: Vec<PathBuf> = Vec::with_capacity(group.len());
    for (i, op) in group.iter().enumerate() {
        if fs::symlink_metadata(&op.from).is_err() {
            restore_from_temps(group, &temps);
            return Err(RenamerError::NotFound(op.from.clone()));
        }
        let temp = unique_temp_path(parent, i);
        if let Err(e) = fs::rename(&op.from, &temp) {
            restore_from_temps(group, &temps);
            return Err(RenamerError::io(op.from.clone(), e));
        }
        temps.push(temp);
    }

    // フェーズ 2: 一時名から新しい名前へ
    for (i, (op, temp)) in group.iter().zip(&temps).enumerate() {
        // Windows の rename は既存ファイルを上書きするため、必ず事前に確認する。
        if fs::symlink_metadata(&op.to).is_ok() {
            restore_partial(group, &temps, i);
            return Err(RenamerError::DestinationExists(op.to.clone()));
        }
        if let Err(e) = fs::rename(temp, &op.to) {
            restore_partial(group, &temps, i);
            return Err(RenamerError::io(op.to.clone(), e));
        }
    }

    done.extend(group.iter().cloned());
    tracing::info!(count = group.len(), dir = %parent.display(), "renamed");
    Ok(())
}

/// フェーズ 1 の途中で失敗: 退避済みのものを元に戻す。
fn restore_from_temps(group: &[Operation], temps: &[PathBuf]) {
    for (op, temp) in group.iter().zip(temps).rev() {
        if let Err(e) = fs::rename(temp, &op.from) {
            tracing::error!("復元失敗 {} -> {}: {e}", temp.display(), op.from.display());
        }
    }
}

/// フェーズ 2 の途中で失敗: 完了済み（0..i）は新名から、残りは一時名から元に戻す。
fn restore_partial(group: &[Operation], temps: &[PathBuf], completed: usize) {
    for op in group[..completed].iter().rev() {
        if let Err(e) = fs::rename(&op.to, &op.from) {
            tracing::error!("復元失敗 {} -> {}: {e}", op.to.display(), op.from.display());
        }
    }
    restore_from_temps(&group[completed..], &temps[completed..]);
}

/// 完了済みのグループを逆順に巻き戻す。
fn rollback(done: &[Operation]) {
    for op in done.iter().rev() {
        if let Err(e) = fs::rename(&op.to, &op.from) {
            tracing::error!(
                "巻き戻し失敗 {} -> {}: {e}",
                op.to.display(),
                op.from.display()
            );
        }
    }
}

fn unique_temp_path(parent: &Path, i: usize) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut n = 0u32;
    loop {
        let candidate = parent.join(format!(".~flexren-{pid}-{nanos}-{i}-{n}.tmp"));
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use renamer_core::{FileEntry, PlanOptions, RenameRule, RuleEntry, build_plan};

    use super::*;
    use crate::test_util::TempDir;
    use crate::undo::undo;

    fn plan_for(paths: &[PathBuf], dirs: &[bool], rules: Vec<RenameRule>) -> RenamePlan {
        let entries: Vec<FileEntry> = paths
            .iter()
            .zip(dirs)
            .map(|(p, d)| FileEntry {
                path: p.clone(),
                is_dir: *d,
            })
            .collect();
        let rules: Vec<RuleEntry> = rules.into_iter().map(RuleEntry::new).collect();
        build_plan(
            &entries,
            &rules,
            &PlanOptions::default(),
            &HashSet::new(),
            |p| p.symlink_metadata().is_ok(),
        )
    }

    fn replace(from: &str, to: &str) -> RenameRule {
        RenameRule::Replace {
            from: from.into(),
            to: to.into(),
            case_sensitive: true,
        }
    }

    #[test]
    fn swap_and_undo() {
        let dir = TempDir::new();
        let a = dir.touch("a.txt");
        let b = dir.touch("b.txt");
        let plan = plan_for(
            &[a.clone(), b.clone()],
            &[false, false],
            vec![replace("a", "x"), replace("b", "a"), replace("x", "b")],
        );
        assert!(plan.can_execute());
        let journal = execute_plan(&plan).unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "b.txt");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "a.txt");

        undo(&journal).unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "a.txt");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "b.txt");
    }

    #[test]
    fn case_only_change() {
        let dir = TempDir::new();
        let a = dir.touch("photo.jpg");
        let plan = plan_for(
            &[a],
            &[false],
            vec![RenameRule::Case {
                mode: renamer_core::CaseMode::Upper,
            }],
        );
        execute_plan(&plan).unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["PHOTO.jpg"]);
    }

    #[test]
    fn nested_directory_and_child() {
        let dir = TempDir::new();
        let f = dir.touch("sub/file.txt");
        let sub = dir.path().join("sub");
        let plan = plan_for(
            &[sub, f],
            &[true, false],
            vec![RenameRule::Prefix {
                value: "new_".into(),
            }],
        );
        let journal = execute_plan(&plan).unwrap();
        assert!(dir.path().join("new_sub/new_file.txt").exists());

        undo(&journal).unwrap();
        assert!(dir.path().join("sub/file.txt").exists());
    }

    #[test]
    fn rollback_when_destination_appears() {
        let dir = TempDir::new();
        let a = dir.touch("a.txt");
        let plan = plan_for(
            &[a.clone()],
            &[false],
            vec![RenameRule::Prefix { value: "x".into() }],
        );
        // プラン作成後に変更先が作られた場合
        dir.touch("xa.txt");
        assert!(execute_plan(&plan).is_err());
        assert!(a.exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("xa.txt")).unwrap(),
            "xa.txt"
        );
    }
}
