use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{CandidateStatus, FileEntry, PlanOptions, RenameCandidate};
use crate::rule::{ApplyContext, CompiledRule, NameParts, RuleEntry};
use crate::token::parent_names;
use crate::util::path_key;
use crate::validate::validate_name;

/// ルール自体のエラー（正規表現の誤りなど）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleError {
    /// ルール一覧での位置（0 始まり、無効化されたルールも数える）
    pub index: usize,
    pub message: String,
}

/// プレビューと実行の両方に使う、確定前のリネーム計画。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenamePlan {
    pub items: Vec<RenameCandidate>,
    pub rule_error: Option<RuleError>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanSummary {
    pub total: usize,
    pub ready: usize,
    pub unchanged: usize,
    pub excluded: usize,
    pub errors: usize,
}

impl RenamePlan {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn executable_items(&self) -> impl Iterator<Item = &RenameCandidate> + '_ {
        self.items.iter().filter(|c| c.will_rename())
    }

    pub fn summary(&self) -> PlanSummary {
        let mut s = PlanSummary {
            total: self.items.len(),
            ..Default::default()
        };
        for c in &self.items {
            match &c.status {
                CandidateStatus::Ready => s.ready += 1,
                CandidateStatus::Unchanged => s.unchanged += 1,
                CandidateStatus::Excluded => s.excluded += 1,
                _ => s.errors += 1,
            }
        }
        s
    }

    /// 実行してよい状態か（ルールエラーなし・エラー 0 件・変更 1 件以上）。
    pub fn can_execute(&self) -> bool {
        let s = self.summary();
        self.rule_error.is_none() && s.errors == 0 && s.ready > 0
    }
}

/// エントリにルールを順番に適用し、検証・衝突検出まで行ったプランを返す。
///
/// `exists` はディスク上にパスが存在するかを返す関数（テストではモックに差し替え可能）。
pub fn build_plan<F>(
    entries: &[FileEntry],
    rules: &[RuleEntry],
    options: &PlanOptions,
    excluded: &HashSet<PathBuf>,
    exists: F,
) -> RenamePlan
where
    F: Fn(&Path) -> bool,
{
    let mut compiled: Vec<CompiledRule<'_>> = Vec::new();
    let mut rule_error = None;
    for (index, entry) in rules.iter().enumerate().filter(|(_, r)| r.enabled) {
        match entry.rule.compile() {
            Ok(c) => compiled.push(c),
            Err(message) => {
                rule_error = Some(RuleError { index, message });
                break;
            }
        }
    }

    let mut items = Vec::with_capacity(entries.len());
    let mut index = 0usize;

    for entry in entries {
        let source = entry.path.clone();
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        if excluded.contains(&source) {
            items.push(RenameCandidate {
                destination: source.clone(),
                source,
                new_name: name,
                is_dir: entry.is_dir,
                status: CandidateStatus::Excluded,
            });
            continue;
        }

        let (new_name, status) = if rule_error.is_some() {
            (name.clone(), CandidateStatus::RuleError)
        } else {
            let parents = parent_names(&source);
            let ctx = ApplyContext {
                index,
                parents: &parents,
            };
            let mut parts = NameParts::new(&name, entry.is_dir, options.include_extension);
            for rule in &compiled {
                rule.apply(&mut parts, &ctx);
            }
            let new_name = parts.into_name();
            let status = if new_name == name {
                CandidateStatus::Unchanged
            } else if let Err(msg) = validate_name(&new_name) {
                CandidateStatus::Invalid(msg)
            } else {
                CandidateStatus::Ready
            };
            (new_name, status)
        };
        index += 1;

        let destination = if status == CandidateStatus::Ready {
            source.with_file_name(&new_name)
        } else {
            source.clone()
        };
        items.push(RenameCandidate {
            source,
            destination,
            new_name,
            is_dir: entry.is_dir,
            status,
        });
    }

    detect_conflicts(&mut items, &exists);
    RenamePlan { items, rule_error }
}

fn detect_conflicts<F: Fn(&Path) -> bool>(items: &mut [RenameCandidate], exists: &F) {
    // 1. 一覧に含まれない既存ファイルとの衝突。
    //    （大文字小文字のみの変更や名前の入れ替えは、変更元が一覧内にあるため除外される）
    let source_keys: HashSet<String> = items.iter().map(|c| path_key(&c.source)).collect();
    for c in items.iter_mut().filter(|c| c.will_rename()) {
        if !source_keys.contains(&path_key(&c.destination)) && exists(&c.destination) {
            c.status = CandidateStatus::Conflict("同名のファイルが既に存在します".into());
        }
    }

    // 2. 実行後の最終パスの重複。衝突扱いにした項目は元の場所に留まるため、収束するまで繰り返す。
    loop {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for c in items.iter() {
            *counts.entry(path_key(c.final_path())).or_default() += 1;
        }
        let mut changed = false;
        for c in items.iter_mut().filter(|c| c.will_rename()) {
            if counts.get(&path_key(&c.destination)).copied().unwrap_or(0) > 1 {
                c.status = CandidateStatus::Conflict("変更後の名前が重複しています".into());
                c.destination = c.source.clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{NumberPosition, RenameRule};

    fn files(names: &[&str]) -> Vec<FileEntry> {
        names
            .iter()
            .map(|n| FileEntry {
                path: PathBuf::from("/data").join(n),
                is_dir: false,
            })
            .collect()
    }

    fn plan(names: &[&str], rules: Vec<RenameRule>, on_disk: &[&str]) -> RenamePlan {
        let disk: HashSet<PathBuf> = on_disk
            .iter()
            .map(|n| PathBuf::from("/data").join(n))
            .collect();
        let rules: Vec<RuleEntry> = rules.into_iter().map(RuleEntry::new).collect();
        build_plan(
            &files(names),
            &rules,
            &PlanOptions::default(),
            &HashSet::new(),
            |p| disk.contains(p),
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
    fn numbering_skips_excluded() {
        let rules = vec![RuleEntry::new(RenameRule::Numbering {
            start: 1,
            step: 1,
            digits: 2,
            separator: "_".into(),
            position: NumberPosition::Suffix,
        })];
        let entries = files(&["a.txt", "b.txt", "c.txt"]);
        let excluded: HashSet<PathBuf> = [PathBuf::from("/data/b.txt")].into();
        let p = build_plan(&entries, &rules, &PlanOptions::default(), &excluded, |_| {
            false
        });
        assert_eq!(p.items[0].new_name, "a_01.txt");
        assert_eq!(p.items[1].status, CandidateStatus::Excluded);
        assert_eq!(p.items[2].new_name, "c_02.txt");
    }

    #[test]
    fn parent_folder_from_path() {
        let p = plan(
            &["sub/a.txt"],
            vec![RenameRule::Prefix {
                value: r"\f1-\f0-".into(),
            }],
            &[],
        );
        assert_eq!(p.items[0].new_name, "data-sub-a.txt");
    }

    #[test]
    fn duplicate_destinations_conflict() {
        let p = plan(
            &["a1.txt", "a2.txt"],
            vec![RenameRule::RegexReplace {
                pattern: r"\d".into(),
                replacement: String::new(),
            }],
            &[],
        );
        assert!(
            p.items
                .iter()
                .all(|c| matches!(c.status, CandidateStatus::Conflict(_)))
        );
        assert!(!p.can_execute());
    }

    #[test]
    fn existing_file_conflict() {
        let p = plan(
            &["a.txt"],
            vec![RenameRule::Prefix { value: "x".into() }],
            &["a.txt", "xa.txt"],
        );
        assert!(matches!(p.items[0].status, CandidateStatus::Conflict(_)));
    }

    #[test]
    fn swap_names_is_allowed() {
        let p = plan(
            &["a.txt", "b.txt"],
            vec![replace("a", "x"), replace("b", "a"), replace("x", "b")],
            &["a.txt", "b.txt"],
        );
        assert_eq!(p.summary().ready, 2);
        assert!(p.can_execute());
    }

    #[test]
    fn invalid_name_reported() {
        let p = plan(
            &["a.txt"],
            vec![RenameRule::Suffix { value: "/".into() }],
            &[],
        );
        assert!(matches!(p.items[0].status, CandidateStatus::Invalid(_)));
    }

    #[test]
    fn rule_error_reports_index() {
        let mut rules: Vec<RuleEntry> = vec![
            RuleEntry::new(RenameRule::RegexReplace {
                pattern: "(".into(),
                replacement: String::new(),
            }),
            RuleEntry::new(replace("a", "b")),
            RuleEntry::new(RenameRule::RegexReplace {
                pattern: "[".into(),
                replacement: String::new(),
            }),
        ];
        rules[0].enabled = false;
        let p = build_plan(
            &files(&["a.txt", "b.txt"]),
            &rules,
            &PlanOptions::default(),
            &HashSet::new(),
            |_| false,
        );
        assert_eq!(p.rule_error.as_ref().map(|e| e.index), Some(2));
        assert_eq!(p.summary().errors, 2);
        assert!(!p.can_execute());
    }
}
