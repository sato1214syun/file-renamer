//! 文字列中の特殊記号の展開。
//!
//! | 記号 | 意味 |
//! |---|---|
//! | `\f0` | 対象が入っているフォルダの名前 |
//! | `\f1` | その 1 つ上のフォルダの名前 |
//! | … | … |
//! | `\f9` | 9 つ上のフォルダの名前 |
//! | `\\` | `\` 1 文字 |
//!
//! 存在しない階層（ドライブやルートより上）は空文字になる。

use std::path::Path;

pub const MAX_PARENT_LEVEL: usize = 9;

/// `path` の親フォルダ名を近い順に最大 10 階層分返す（`[0]` = 直接の親）。
pub fn parent_names(path: &Path) -> Vec<String> {
    path.ancestors()
        .skip(1)
        .map_while(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .take(MAX_PARENT_LEVEL + 1)
        .collect()
}

/// `\f0`〜`\f9` と `\\` を展開する。それ以外の `\` はそのまま残す。
pub fn expand_tokens(s: &str, parents: &[String]) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('\\') => {
                chars.next();
                out.push('\\');
            }
            Some('f') => {
                let mut ahead = chars.clone();
                ahead.next();
                match ahead.next() {
                    Some(d @ '0'..='9') => {
                        chars.next();
                        chars.next();
                        let level = d as usize - '0' as usize;
                        if let Some(name) = parents.get(level) {
                            out.push_str(name);
                        }
                    }
                    _ => out.push('\\'),
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn parents() -> Vec<String> {
        parent_names(&PathBuf::from("/data/2024/旅行/a.jpg"))
    }

    #[test]
    fn collects_parents() {
        assert_eq!(parents(), vec!["旅行", "2024", "data"]);
    }

    #[test]
    fn expands() {
        let p = parents();
        assert_eq!(expand_tokens(r"\f0_", &p), "旅行_");
        assert_eq!(expand_tokens(r"\f1-\f0", &p), "2024-旅行");
        assert_eq!(expand_tokens(r"\f9x", &p), "x");
        assert_eq!(expand_tokens(r"\f01", &p), "旅行1");
    }

    #[test]
    fn escapes_and_literals() {
        let p = parents();
        assert_eq!(expand_tokens(r"\\f0", &p), r"\f0");
        assert_eq!(expand_tokens(r"a\b\f", &p), r"a\b\f");
        assert_eq!(expand_tokens("plain", &p), "plain");
    }
}
