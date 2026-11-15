use std::str::FromStr;

use regex::{Captures, NoExpand, Regex};
use serde::{Deserialize, Serialize};

use crate::platform::Platform;
use crate::token::expand_tokens;
use crate::width::{self, WidthTargets};

/// ルール一覧の 1 行（有効/無効フラグ付き）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleEntry {
    pub enabled: bool,
    pub rule: RenameRule,
}

impl RuleEntry {
    pub fn new(rule: RenameRule) -> Self {
        Self {
            enabled: true,
            rule,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenameRule {
    Replace {
        from: String,
        to: String,
        case_sensitive: bool,
    },
    RegexReplace {
        pattern: String,
        replacement: String,
    },
    Prefix {
        value: String,
    },
    Suffix {
        value: String,
    },
    Insert {
        position: usize,
        value: String,
    },
    Remove {
        start: usize,
        count: usize,
    },
    Numbering {
        start: u64,
        step: u64,
        digits: usize,
        separator: String,
        position: NumberPosition,
    },
    Case {
        mode: CaseMode,
    },
    WidthConvert {
        direction: WidthDirection,
        alnum: bool,
        symbol: bool,
        space: bool,
        katakana: bool,
    },
    Extension {
        mode: ExtensionMode,
        extension: String,
    },
}

/// 選択肢を持つ列挙型の共通処理。
macro_rules! choice_enum {
    ($name:ident { $($variant:ident => $label:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }

            pub fn labels() -> Vec<&'static str> {
                Self::ALL.iter().map(|v| v.label()).collect()
            }

            pub fn index(self) -> usize {
                Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
            }

            pub fn from_index(i: usize) -> Option<Self> {
                Self::ALL.get(i).copied()
            }
        }
    };
}

choice_enum!(CaseMode {
    Lower => "小文字",
    Upper => "大文字",
    Title => "単語の先頭を大文字",
});

choice_enum!(WidthDirection {
    ToHalf => "全角 → 半角",
    ToFull => "半角 → 全角",
});

choice_enum!(NumberPosition {
    Prefix => "先頭",
    Suffix => "末尾",
});

choice_enum!(ExtensionMode {
    Replace => "置き換え",
    Lower => "小文字にする",
    Upper => "大文字にする",
    Remove => "削除",
});

/// GUI で編集する値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Text(String),
    Number(String),
    /// ドロップダウンで選ぶ
    Choice {
        options: Vec<&'static str>,
        selected: usize,
    },
    /// チェックボックス
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleField {
    pub label: &'static str,
    pub value: FieldValue,
    /// `\f0`〜`\f9`（親フォルダ名）を使える欄か
    pub accepts_tokens: bool,
}

fn text(label: &'static str, value: &str) -> RuleField {
    RuleField {
        label,
        value: FieldValue::Text(value.to_string()),
        accepts_tokens: false,
    }
}

/// `\f0`〜`\f9` を使えるテキスト欄。
fn token_text(label: &'static str, value: &str) -> RuleField {
    RuleField {
        accepts_tokens: true,
        ..text(label, value)
    }
}

fn number(label: &'static str, value: impl ToString) -> RuleField {
    RuleField {
        label,
        value: FieldValue::Number(value.to_string()),
        accepts_tokens: false,
    }
}

fn choice(label: &'static str, options: Vec<&'static str>, selected: usize) -> RuleField {
    RuleField {
        label,
        value: FieldValue::Choice { options, selected },
        accepts_tokens: false,
    }
}

fn flag(label: &'static str, on: bool) -> RuleField {
    RuleField {
        label,
        value: FieldValue::Bool(on),
        accepts_tokens: false,
    }
}

/// 空文字は 0（Default）として扱う。数値として解釈できなければ false。
fn set_num<T: FromStr + Default>(slot: &mut T, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        *slot = T::default();
        return true;
    }
    match value.parse::<T>() {
        Ok(n) => {
            *slot = n;
            true
        }
        Err(_) => false,
    }
}

fn set_choice<T: Copy>(slot: &mut T, value: Option<T>) -> bool {
    match value {
        Some(v) => {
            *slot = v;
            true
        }
        None => false,
    }
}

const MAX_DIGITS: usize = 20;

impl RenameRule {
    /// 「ルールを追加」欄の分類と並び。追加しただけでは名前が変わらない初期値にしている
    /// （連番・大文字小文字・全角半角を除く）。
    pub fn template_groups() -> Vec<(&'static str, Vec<RenameRule>)> {
        vec![
            (
                "置換",
                vec![
                    Self::Replace {
                        from: String::new(),
                        to: String::new(),
                        case_sensitive: true,
                    },
                    Self::RegexReplace {
                        pattern: String::new(),
                        replacement: String::new(),
                    },
                ],
            ),
            (
                "追加",
                vec![
                    Self::Prefix {
                        value: String::new(),
                    },
                    Self::Suffix {
                        value: String::new(),
                    },
                    Self::Insert {
                        position: 0,
                        value: String::new(),
                    },
                    Self::Numbering {
                        start: 1,
                        step: 1,
                        digits: 3,
                        separator: "_".to_string(),
                        position: NumberPosition::Suffix,
                    },
                ],
            ),
            ("削除", vec![Self::Remove { start: 0, count: 0 }]),
            (
                "文字種",
                vec![
                    Self::Case {
                        mode: CaseMode::Lower,
                    },
                    Self::WidthConvert {
                        direction: WidthDirection::ToHalf,
                        alnum: true,
                        symbol: true,
                        space: true,
                        katakana: false,
                    },
                ],
            ),
            (
                "拡張子",
                vec![Self::Extension {
                    mode: ExtensionMode::Lower,
                    extension: String::new(),
                }],
            ),
        ]
    }

    /// 全テンプレート（分類順）。
    pub fn templates() -> Vec<RenameRule> {
        Self::template_groups()
            .into_iter()
            .flat_map(|(_, rules)| rules)
            .collect()
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Replace { .. } => "文字列置換",
            Self::RegexReplace { .. } => "正規表現",
            Self::Prefix { .. } => "先頭に追加",
            Self::Suffix { .. } => "末尾に追加",
            Self::Insert { .. } => "位置を指定して挿入",
            Self::Remove { .. } => "文字数で削除",
            Self::Numbering { .. } => "連番",
            Self::Case { .. } => "大文字/小文字",
            Self::WidthConvert { .. } => "全角/半角",
            Self::Extension { .. } => "拡張子",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Replace { from, to, .. } => format!("置換 「{from}」→「{to}」"),
            Self::RegexReplace {
                pattern,
                replacement,
            } => format!("正規表現 /{pattern}/ →「{replacement}」"),
            Self::Prefix { value } => format!("先頭に「{value}」"),
            Self::Suffix { value } => format!("末尾に「{value}」"),
            Self::Insert { position, value } => format!("{position} 文字目に「{value}」"),
            Self::Remove { start, count } => format!("{start} 文字目から {count} 文字削除"),
            Self::Numbering {
                start,
                step,
                digits,
                position,
                ..
            } => format!("連番 {}{start} (+{step}, {digits} 桁)", position.label()),
            Self::Case { mode } => format!("{}に変換", mode.label()),
            Self::WidthConvert {
                direction,
                alnum,
                symbol,
                space,
                katakana,
            } => {
                let kinds: Vec<&str> = [
                    (*alnum, "英数字"),
                    (*symbol, "記号"),
                    (*space, "スペース"),
                    (*katakana, "カタカナ"),
                ]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect();
                let kinds = if kinds.is_empty() {
                    "対象なし".to_string()
                } else {
                    kinds.join("・")
                };
                format!("{}（{kinds}）", direction.label())
            }
            Self::Extension { mode, extension } => match mode {
                ExtensionMode::Replace => format!("拡張子を「{extension}」に"),
                _ => format!("拡張子を{}", mode.label()),
            },
        }
    }

    pub fn fields(&self) -> Vec<RuleField> {
        match self {
            Self::Replace {
                from,
                to,
                case_sensitive,
            } => vec![
                text("検索文字列", from),
                token_text("置換後", to),
                flag("大文字小文字を区別", *case_sensitive),
            ],
            Self::RegexReplace {
                pattern,
                replacement,
            } => vec![
                text("正規表現", pattern),
                token_text("置換後", replacement),
            ],
            Self::Prefix { value } => vec![token_text("追加する文字", value)],
            Self::Suffix { value } => vec![token_text("追加する文字", value)],
            Self::Insert { position, value } => vec![
                number("位置 (0 = 先頭)", position),
                token_text("挿入する文字", value),
            ],
            Self::Remove { start, count } => vec![
                number("開始位置 (0 = 先頭)", start),
                number("削除する文字数", count),
            ],
            Self::Numbering {
                start,
                step,
                digits,
                separator,
                position,
            } => vec![
                number("開始番号", start),
                number("増分", step),
                number("桁数", digits),
                token_text("区切り文字", separator),
                choice("位置", NumberPosition::labels(), position.index()),
            ],
            Self::Case { mode } => vec![choice("変換", CaseMode::labels(), mode.index())],
            Self::WidthConvert {
                direction,
                alnum,
                symbol,
                space,
                katakana,
            } => vec![
                choice("変換", WidthDirection::labels(), direction.index()),
                flag("英数字", *alnum),
                flag("記号", *symbol),
                flag("スペース", *space),
                flag("カタカナ", *katakana),
            ],
            Self::Extension { mode, extension } => vec![
                choice("操作", ExtensionMode::labels(), mode.index()),
                text("新しい拡張子", extension),
            ],
        }
    }

    /// テキスト/数値フィールドを更新する。受け付けられない入力なら false。
    pub fn set_field(&mut self, index: usize, value: &str) -> bool {
        match (self, index) {
            (Self::Replace { from, .. }, 0) => {
                *from = value.to_string();
                true
            }
            (Self::Replace { to, .. }, 1) => {
                *to = value.to_string();
                true
            }
            (Self::RegexReplace { pattern, .. }, 0) => {
                *pattern = value.to_string();
                true
            }
            (Self::RegexReplace { replacement, .. }, 1) => {
                *replacement = value.to_string();
                true
            }
            (Self::Prefix { value: v }, 0) | (Self::Suffix { value: v }, 0) => {
                *v = value.to_string();
                true
            }
            (Self::Insert { position, .. }, 0) => set_num(position, value),
            (Self::Insert { value: v, .. }, 1) => {
                *v = value.to_string();
                true
            }
            (Self::Remove { start, .. }, 0) => set_num(start, value),
            (Self::Remove { count, .. }, 1) => set_num(count, value),
            (Self::Numbering { start, .. }, 0) => set_num(start, value),
            (Self::Numbering { step, .. }, 1) => set_num(step, value),
            (Self::Numbering { digits, .. }, 2) => {
                let mut d = *digits;
                if set_num(&mut d, value) && d <= MAX_DIGITS {
                    *digits = d;
                    true
                } else {
                    false
                }
            }
            (Self::Numbering { separator, .. }, 3) => {
                *separator = value.to_string();
                true
            }
            (Self::Extension { extension, .. }, 1) => {
                *extension = value.to_string();
                true
            }
            _ => false,
        }
    }

    /// ドロップダウンの選択を反映する。
    pub fn set_choice(&mut self, index: usize, selected: usize) -> bool {
        match (self, index) {
            (Self::Numbering { position, .. }, 4) => {
                set_choice(position, NumberPosition::from_index(selected))
            }
            (Self::Case { mode }, 0) => set_choice(mode, CaseMode::from_index(selected)),
            (Self::WidthConvert { direction, .. }, 0) => {
                set_choice(direction, WidthDirection::from_index(selected))
            }
            (Self::Extension { mode, .. }, 0) => {
                set_choice(mode, ExtensionMode::from_index(selected))
            }
            _ => false,
        }
    }

    /// チェックボックスの状態を反映する。
    pub fn set_bool(&mut self, index: usize, on: bool) -> bool {
        let slot = match (self, index) {
            (Self::Replace { case_sensitive, .. }, 2) => case_sensitive,
            (Self::WidthConvert { alnum, .. }, 1) => alnum,
            (Self::WidthConvert { symbol, .. }, 2) => symbol,
            (Self::WidthConvert { space, .. }, 3) => space,
            (Self::WidthConvert { katakana, .. }, 4) => katakana,
            _ => return false,
        };
        *slot = on;
        true
    }

    /// 正規表現などを事前コンパイルする。エラーは 1 行の日本語メッセージ。
    pub fn compile(&self) -> Result<CompiledRule<'_>, String> {
        let mut pieces = Vec::new();
        let regex = match self {
            Self::Replace {
                from,
                case_sensitive: false,
                ..
            } if !from.is_empty() => Some(
                Regex::new(&format!("(?i){}", regex::escape(from)))
                    .map_err(|e| regex_error_message(&e))?,
            ),
            Self::RegexReplace {
                pattern,
                replacement,
            } if !pattern.is_empty() => {
                let re = Regex::new(pattern).map_err(|e| regex_error_message(&e))?;
                pieces = parse_replacement(replacement);
                // captures_len はグループ 0（全体）を含む
                let groups = re.captures_len() - 1;
                if let Some(n) = pieces.iter().find_map(|p| match p {
                    Piece::Group(n) if *n > groups => Some(*n),
                    _ => None,
                }) {
                    return Err(if groups == 0 {
                        format!("${n} を使うには正規表現に ( ) のグループが必要です")
                    } else {
                        format!("${n} に対応するグループがありません（グループは {groups} 個）")
                    });
                }
                Some(re)
            }
            _ => None,
        };
        Ok(CompiledRule {
            rule: self,
            regex,
            pieces,
        })
    }
}

/// 置換文字列の構成要素。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Literal(String),
    /// `$1`〜`$9`
    Group(usize),
}

/// `$1`〜`$9` を 1 桁のグループ参照として解釈する（`$1a` は「グループ 1」+「a」）。
/// `$$` は `$` 1 文字。それ以外の `$` は文字としてそのまま扱う。
fn parse_replacement(s: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            literal.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                literal.push('$');
            }
            Some(d @ '1'..='9') => {
                chars.next();
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(Piece::Group(d as usize - '0' as usize));
            }
            _ => literal.push('$'),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    pieces
}

/// regex クレートの複数行のエラーを 1 行の日本語にする。
fn regex_error_message(e: &regex::Error) -> String {
    match e {
        regex::Error::Syntax(detail) => {
            let lines: Vec<&str> = detail.lines().collect();
            let error_line = lines.iter().position(|l| l.starts_with("error:"));
            let message = error_line
                .map(|i| lines[i].trim_start_matches("error:").trim())
                .unwrap_or("構文エラー");
            // エラー行の直前は「    ^^^」の位置表示（先頭 4 文字はインデント）
            let column = error_line
                .and_then(|i| i.checked_sub(1))
                .map(|i| lines[i])
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && t.chars().all(|c| c == '^')
                })
                .and_then(|l| l.find('^'))
                .map(caret_column);
            let position = column
                .map(|c| format!("（{c} 文字目）"))
                .unwrap_or_default();
            format!("正規表現の誤り: {}{position}", translate_regex_error(message))
        }
        regex::Error::CompiledTooBig(_) => "正規表現が大きすぎます".to_string(),
        other => format!("正規表現の誤り: {}", other.to_string().replace('\n', " ")),
    }
}

fn caret_column(caret_byte: usize) -> usize {
    caret_byte.saturating_sub(4) + 1
}

fn translate_regex_error(msg: &str) -> String {
    const TABLE: &[(&str, &str)] = &[
        ("unclosed group", "「)」が足りません"),
        ("unopened group", "対応する「(」がありません"),
        ("unclosed character class", "「]」が足りません"),
        ("unclosed counted repetition", "「}」が足りません"),
        (
            "repetition operator missing expression",
            "* + ? { } の前に対象の文字がありません",
        ),
        (
            "incomplete escape sequence",
            "末尾の「\\」の後に文字がありません",
        ),
        ("unrecognized escape sequence", "使えないエスケープ（\\）です"),
        ("invalid character class range", "[ ] 内の範囲の順序が逆です"),
        ("look-around", "先読み・後読みは使えません"),
        ("backreferences are not supported", "後方参照（\\1 など）は使えません"),
        ("empty character class", "空の [ ] は使えません"),
        ("invalid repetition count range", "{ } 内の回数の範囲が不正です"),
        ("duplicate capture group name", "グループ名が重複しています"),
    ];
    TABLE
        .iter()
        .find(|(en, _)| msg.starts_with(en))
        .map(|(_, ja)| ja.to_string())
        .unwrap_or_else(|| msg.to_string())
}

/// 名前を「本体」と「拡張子」に分けて保持する作業用構造体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameParts {
    pub base: String,
    pub ext: Option<String>,
    pub is_dir: bool,
}

impl NameParts {
    pub fn new(name: &str, is_dir: bool, include_extension: bool) -> Self {
        if is_dir || include_extension {
            Self {
                base: name.to_string(),
                ext: None,
                is_dir,
            }
        } else {
            let (base, ext) = split_extension(name);
            Self { base, ext, is_dir }
        }
    }

    pub fn into_name(self) -> String {
        join_extension(self.base, self.ext)
    }
}

/// `photo.tar.gz` → (`photo.tar`, `gz`)。`.bashrc` のような先頭ドットのみの名前は拡張子なし。
pub fn split_extension(name: &str) -> (String, Option<String>) {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => {
            (name[..i].to_string(), Some(name[i + 1..].to_string()))
        }
        _ => (name.to_string(), None),
    }
}

fn join_extension(base: String, ext: Option<String>) -> String {
    match ext {
        Some(ext) => format!("{base}.{ext}"),
        None => base,
    }
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut word_start = true;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if word_start {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            word_start = false;
        } else {
            out.push(ch);
            word_start = true;
        }
    }
    out
}

/// ルール適用時の項目ごとの情報。
#[derive(Debug, Clone, Copy)]
pub struct ApplyContext<'a> {
    /// 対象項目の通し番号（0 始まり、除外項目は数えない）
    pub index: usize,
    /// 親フォルダ名（`[0]` = 直接の親）。`\f0`〜`\f9` の展開に使う
    pub parents: &'a [String],
}

pub struct CompiledRule<'a> {
    rule: &'a RenameRule,
    regex: Option<Regex>,
    pieces: Vec<Piece>,
}

impl CompiledRule<'_> {
    pub fn apply(&self, parts: &mut NameParts, ctx: &ApplyContext<'_>) {
        let expand = |s: &str| expand_tokens(s, ctx.parents);
        match self.rule {
            RenameRule::Replace { from, to, .. } => {
                if from.is_empty() {
                    return;
                }
                let to = expand(to);
                parts.base = match &self.regex {
                    Some(re) => re.replace_all(&parts.base, NoExpand(&to)).into_owned(),
                    None => parts.base.replace(from.as_str(), &to),
                };
            }
            RenameRule::RegexReplace { .. } => {
                let Some(re) = &self.regex else {
                    return;
                };
                // 展開はファイルごとに 1 回だけ行う（グループの中身は展開しない）
                let literals: Vec<String> = self
                    .pieces
                    .iter()
                    .map(|p| match p {
                        Piece::Literal(s) => expand(s),
                        Piece::Group(_) => String::new(),
                    })
                    .collect();
                parts.base = re
                    .replace_all(&parts.base, |caps: &Captures<'_>| {
                        let mut out = String::new();
                        for (piece, literal) in self.pieces.iter().zip(&literals) {
                            match piece {
                                Piece::Literal(_) => out.push_str(literal),
                                Piece::Group(n) => {
                                    if let Some(m) = caps.get(*n) {
                                        out.push_str(m.as_str());
                                    }
                                }
                            }
                        }
                        out
                    })
                    .into_owned();
            }
            RenameRule::Prefix { value } => parts.base.insert_str(0, &expand(value)),
            RenameRule::Suffix { value } => parts.base.push_str(&expand(value)),
            RenameRule::Insert { position, value } => {
                let byte = parts
                    .base
                    .char_indices()
                    .nth(*position)
                    .map(|(i, _)| i)
                    .unwrap_or(parts.base.len());
                parts.base.insert_str(byte, &expand(value));
            }
            RenameRule::Remove { start, count } => {
                parts.base = parts
                    .base
                    .chars()
                    .enumerate()
                    .filter(|(i, _)| *i < *start || *i >= start.saturating_add(*count))
                    .map(|(_, c)| c)
                    .collect();
            }
            RenameRule::Numbering {
                start,
                step,
                digits,
                separator,
                position,
            } => {
                let n = start.saturating_add((ctx.index as u64).saturating_mul(*step));
                let num = format!("{n:0width$}", width = *digits);
                let separator = expand(separator);
                parts.base = match position {
                    NumberPosition::Prefix => format!("{num}{separator}{}", parts.base),
                    NumberPosition::Suffix => format!("{}{separator}{num}", parts.base),
                };
            }
            RenameRule::Case { mode } => {
                parts.base = match mode {
                    CaseMode::Lower => parts.base.to_lowercase(),
                    CaseMode::Upper => parts.base.to_uppercase(),
                    CaseMode::Title => title_case(&parts.base),
                };
            }
            RenameRule::WidthConvert {
                direction,
                alnum,
                symbol,
                space,
                katakana,
            } => {
                let targets = WidthTargets {
                    alnum: *alnum,
                    symbol: *symbol,
                    space: *space,
                    katakana: *katakana,
                };
                parts.base = match direction {
                    WidthDirection::ToHalf => {
                        width::to_half(&parts.base, &targets, Platform::current())
                    }
                    WidthDirection::ToFull => width::to_full(&parts.base, &targets),
                };
            }
            RenameRule::Extension { mode, extension } => {
                if parts.is_dir {
                    return;
                }
                // 「拡張子もルール対象」モードでは ext が None なので一時的に分割する。
                let merged = parts.ext.is_none();
                let (base, mut ext) = if merged {
                    split_extension(&parts.base)
                } else {
                    (parts.base.clone(), parts.ext.clone())
                };
                match mode {
                    ExtensionMode::Replace => {
                        let new_ext = extension.trim_start_matches('.');
                        if !new_ext.is_empty() {
                            ext = Some(new_ext.to_string());
                        }
                    }
                    ExtensionMode::Lower => ext = ext.map(|e| e.to_lowercase()),
                    ExtensionMode::Upper => ext = ext.map(|e| e.to_uppercase()),
                    ExtensionMode::Remove => ext = None,
                }
                if merged {
                    parts.base = join_extension(base, ext);
                } else {
                    parts.base = base;
                    parts.ext = ext;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with(rule: RenameRule, name: &str, index: usize, parents: &[&str]) -> String {
        let parents: Vec<String> = parents.iter().map(|s| s.to_string()).collect();
        let compiled = rule.compile().unwrap();
        let mut parts = NameParts::new(name, false, false);
        compiled.apply(
            &mut parts,
            &ApplyContext {
                index,
                parents: &parents,
            },
        );
        parts.into_name()
    }

    fn run(rule: RenameRule, name: &str, index: usize) -> String {
        run_with(rule, name, index, &[])
    }

    fn regex(pattern: &str, replacement: &str) -> RenameRule {
        RenameRule::RegexReplace {
            pattern: pattern.into(),
            replacement: replacement.into(),
        }
    }

    #[test]
    fn replace_case_insensitive() {
        let rule = RenameRule::Replace {
            from: "img".into(),
            to: "Photo".into(),
            case_sensitive: false,
        };
        assert_eq!(run(rule, "IMG_0001.JPG", 0), "Photo_0001.JPG");
    }

    #[test]
    fn empty_replace_is_noop() {
        let rule = RenameRule::Replace {
            from: String::new(),
            to: "x".into(),
            case_sensitive: true,
        };
        assert_eq!(run(rule, "a.txt", 0), "a.txt");
    }

    #[test]
    fn regex_groups_followed_by_text() {
        assert_eq!(run(regex(r"(\d+)_(\w+)", "$2_$1"), "001_abc.txt", 0), "abc_001.txt");
        assert_eq!(run(regex(r"(\d+)", "$1a"), "x12.txt", 0), "x12a.txt");
        assert_eq!(run(regex(r"(\d)(\d)", "$2$1"), "12.txt", 0), "21.txt");
        assert_eq!(run(regex(r"(a)", "$10"), "a.txt", 0), "a0.txt");
    }

    #[test]
    fn regex_dollar_literals() {
        assert_eq!(run(regex("a", "$$"), "a.txt", 0), "$.txt");
        assert_eq!(run(regex("a", "$x$"), "a.txt", 0), "$x$.txt");
    }

    #[test]
    fn regex_missing_group_is_error() {
        assert!(regex("a", "$1").compile().is_err());
        assert!(regex("(a)", "$2").compile().is_err());
        assert!(regex("(a)", "$1").compile().is_ok());
    }

    #[test]
    fn regex_error_is_single_line_japanese() {
        let err = match regex("ab(", "").compile() {
            Err(e) => e,
            Ok(_) => panic!("should fail"),
        };
        assert!(!err.contains('\n'), "{err}");
        assert!(err.contains("「)」が足りません"), "{err}");
        assert!(err.contains("3 文字目"), "{err}");
    }

    #[test]
    fn parent_folder_tokens() {
        let parents = ["旅行", "2024"];
        let rule = RenameRule::Prefix {
            value: r"\f1_\f0_".into(),
        };
        assert_eq!(run_with(rule, "a.jpg", 0, &parents), "2024_旅行_a.jpg");

        let rule = regex(r"^IMG", r"\f0$$");
        assert_eq!(run_with(rule, "IMG1.jpg", 0, &parents), "旅行$1.jpg");

        // グループの中身に \f0 が含まれていても展開しない
        let rule = regex(r"(.+)", r"$1-\f0");
        assert_eq!(run_with(rule, r"x\f0.txt", 0, &parents), r"x\f0-旅行.txt");

        let rule = RenameRule::Numbering {
            start: 1,
            step: 1,
            digits: 2,
            separator: r"_\f0_".into(),
            position: NumberPosition::Prefix,
        };
        assert_eq!(run_with(rule, "a.jpg", 0, &parents), "01_旅行_a.jpg");
    }

    #[test]
    fn numbering_suffix() {
        let rule = RenameRule::Numbering {
            start: 1,
            step: 2,
            digits: 3,
            separator: "-".into(),
            position: NumberPosition::Suffix,
        };
        assert_eq!(run(rule, "a.jpg", 2), "a-005.jpg");
    }

    #[test]
    fn insert_and_remove_by_char() {
        let ins = RenameRule::Insert {
            position: 2,
            value: "X".into(),
        };
        assert_eq!(run(ins, "あいう.txt", 0), "あいXう.txt");
        let rem = RenameRule::Remove { start: 1, count: 1 };
        assert_eq!(run(rem, "あいう.txt", 0), "あう.txt");
    }

    #[test]
    fn width_convert_rule() {
        let rule = RenameRule::WidthConvert {
            direction: WidthDirection::ToFull,
            alnum: true,
            symbol: false,
            space: false,
            katakana: true,
        };
        assert_eq!(run(rule, "ab ｶﾞ.txt", 0), "ａｂ ガ.txt");
    }

    #[test]
    fn choice_and_bool_fields() {
        let mut rule = RenameRule::Case {
            mode: CaseMode::Lower,
        };
        assert!(rule.set_choice(0, 2));
        assert_eq!(rule, RenameRule::Case { mode: CaseMode::Title });
        assert!(!rule.set_choice(0, 9));

        let mut rule = RenameRule::templates()
            .into_iter()
            .find(|r| matches!(r, RenameRule::WidthConvert { .. }))
            .unwrap();
        assert!(rule.set_bool(4, true));
        assert!(matches!(rule, RenameRule::WidthConvert { katakana: true, .. }));
        match &rule.fields()[0].value {
            FieldValue::Choice { options, selected } => {
                assert_eq!(options.len(), 2);
                assert_eq!(*selected, 0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn extension_modes() {
        let rule = RenameRule::Extension {
            mode: ExtensionMode::Lower,
            extension: String::new(),
        };
        assert_eq!(run(rule, "A.JPG", 0), "A.jpg");
        let rule = RenameRule::Extension {
            mode: ExtensionMode::Replace,
            extension: ".jpeg".into(),
        };
        assert_eq!(run(rule, "a.jpg", 0), "a.jpeg");
    }

    #[test]
    fn title_case_works() {
        let rule = RenameRule::Case {
            mode: CaseMode::Title,
        };
        assert_eq!(run(rule, "hello WORLD-foo.txt", 0), "Hello World-Foo.txt");
    }

    #[test]
    fn split_extension_edge_cases() {
        assert_eq!(split_extension(".bashrc"), (".bashrc".into(), None));
        assert_eq!(split_extension("a."), ("a.".into(), None));
        assert_eq!(
            split_extension("a.tar.gz"),
            ("a.tar".into(), Some("gz".into()))
        );
    }

    #[test]
    fn set_field_numbers() {
        let mut rule = RenameRule::Remove { start: 0, count: 0 };
        assert!(rule.set_field(1, "12"));
        assert!(!rule.set_field(1, "x"));
        assert!(rule.set_field(1, ""));
        assert_eq!(rule, RenameRule::Remove { start: 0, count: 0 });
    }

    #[test]
    fn rules_serialize() {
        for rule in RenameRule::templates() {
            let json = serde_json::to_string(&rule).unwrap();
            let back: RenameRule = serde_json::from_str(&json).unwrap();
            assert_eq!(rule, back);
        }
    }
}
