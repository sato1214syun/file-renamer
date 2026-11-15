//! 全角 ⇔ 半角の変換。
//!
//! 対象は「英数字」「記号」「スペース」「カタカナ（。「」、・ を含む）」の 4 種類。
//! 半角カナの濁点・半濁点は 1 文字に合成／分解する（ｶﾞ ⇔ ガ）。

use crate::platform::Platform;

/// 変換対象の文字種。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidthTargets {
    pub alnum: bool,
    pub symbol: bool,
    pub space: bool,
    pub katakana: bool,
}

/// U+FF61..=U+FF9F（半角カナ）に対応する全角文字
const FULL_KANA: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン゛゜";
const HALF_KANA_START: u32 = 0xFF61;
const HALF_DAKUTEN: char = 'ﾞ';
const HALF_HANDAKUTEN: char = 'ﾟ';
/// 濁点を付けられる全角カナ（ウ は ヴ として別扱い）
const VOICEABLE: &str = "カキクケコサシスセソタチツテトハヒフヘホ";
/// 半濁点を付けられる全角カナ
const SEMI_VOICEABLE: &str = "ハヒフヘホ";

fn half_kana_to_full(c: char) -> Option<char> {
    let code = c as u32;
    if !(HALF_KANA_START..=0xFF9F).contains(&code) {
        return None;
    }
    FULL_KANA.chars().nth((code - HALF_KANA_START) as usize)
}

fn full_kana_to_half(c: char) -> Option<String> {
    if let Some(i) = FULL_KANA.chars().position(|f| f == c) {
        let half = char::from_u32(HALF_KANA_START + i as u32)?;
        return Some(half.to_string());
    }
    if c == 'ヴ' {
        return Some(format!("ｳ{HALF_DAKUTEN}"));
    }
    let code = c as u32;
    // 濁音・半濁音は全角カタカナのブロック内のみ
    if !(0x30A1..=0x30FA).contains(&code) {
        return None;
    }
    let base1 = char::from_u32(code - 1)?;
    if VOICEABLE.contains(base1) {
        return Some(format!("{}{HALF_DAKUTEN}", full_kana_to_half(base1)?));
    }
    let base2 = char::from_u32(code - 2)?;
    if SEMI_VOICEABLE.contains(base2) {
        return Some(format!("{}{HALF_HANDAKUTEN}", full_kana_to_half(base2)?));
    }
    None
}

/// 全角 → 半角。変換後の文字が `platform` でファイル名に使えない場合は全角のまま残す。
pub fn to_half(s: &str, t: &WidthTargets, platform: Platform) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let converted: Option<String> = match c {
            '\u{3000}' if t.space => Some(" ".to_string()),
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0)
                .filter(|h| {
                    if h.is_ascii_alphanumeric() {
                        t.alnum
                    } else {
                        t.symbol
                    }
                })
                .map(|h| h.to_string()),
            _ if t.katakana => full_kana_to_half(c),
            _ => None,
        };
        match converted {
            Some(h) if !h.chars().any(|x| platform.is_forbidden_char(x)) => out.push_str(&h),
            _ => out.push(c),
        }
    }
    out
}

/// 半角 → 全角。全角文字はどの OS でもファイル名に使える。
pub fn to_full(s: &str, t: &WidthTargets) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() * 3);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            ' ' if t.space => out.push('\u{3000}'),
            '!'..='~' => {
                let wanted = if c.is_ascii_alphanumeric() {
                    t.alnum
                } else {
                    t.symbol
                };
                match char::from_u32(c as u32 + 0xFEE0).filter(|_| wanted) {
                    Some(f) => out.push(f),
                    None => out.push(c),
                }
            }
            _ if t.katakana && half_kana_to_full(c).is_some() => {
                let full = half_kana_to_full(c).unwrap_or(c);
                let next = chars.get(i).copied();
                let composed = match next {
                    Some(HALF_DAKUTEN) if full == 'ウ' => Some('ヴ'),
                    Some(HALF_DAKUTEN) if VOICEABLE.contains(full) => {
                        char::from_u32(full as u32 + 1)
                    }
                    Some(HALF_HANDAKUTEN) if SEMI_VOICEABLE.contains(full) => {
                        char::from_u32(full as u32 + 2)
                    }
                    _ => None,
                };
                match composed {
                    Some(v) => {
                        out.push(v);
                        i += 1;
                    }
                    None => out.push(full),
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: WidthTargets = WidthTargets {
        alnum: true,
        symbol: true,
        space: true,
        katakana: true,
    };

    #[test]
    fn table_length() {
        assert_eq!(FULL_KANA.chars().count(), 0xFF9F - 0xFF61 + 1);
    }

    #[test]
    fn half_conversion() {
        assert_eq!(to_half("ＡＢＣ１２３　（テスト）", &ALL, Platform::Unix), "ABC123 (ﾃｽﾄ)");
        assert_eq!(to_half("ガギパヴ", &ALL, Platform::Unix), "ｶﾞｷﾞﾊﾟｳﾞ");
    }

    #[test]
    fn half_keeps_forbidden() {
        assert_eq!(to_half("Ａ：Ｂ？", &ALL, Platform::Windows), "A：B？");
        assert_eq!(to_half("Ａ：Ｂ？", &ALL, Platform::Unix), "A:B?");
        assert_eq!(to_half("ａ／ｂ", &ALL, Platform::Unix), "a／b");
    }

    #[test]
    fn full_conversion() {
        assert_eq!(to_full("AB1 (ｶﾞｷﾊﾟｳﾞ)", &ALL), "ＡＢ１　（ガキパヴ）");
        assert_eq!(to_full("ﾞｱ", &ALL), "゛ア");
    }

    #[test]
    fn targets_respected() {
        let only_alnum = WidthTargets {
            alnum: true,
            symbol: false,
            space: false,
            katakana: false,
        };
        assert_eq!(to_half("Ａ（ア）　", &only_alnum, Platform::Unix), "A（ア）　");
        assert_eq!(to_full("A(ｱ) ", &only_alnum), "Ａ(ｱ) ");
    }

    #[test]
    fn roundtrip_kana() {
        let s = "ガギグゲゴザジズゼゾダヂヅデドバビブベボパピプペポヴ";
        assert_eq!(to_full(&to_half(s, &ALL, Platform::Unix), &ALL), s);
    }
}
