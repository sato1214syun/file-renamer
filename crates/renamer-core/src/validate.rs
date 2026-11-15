use crate::platform::Platform;

/// 実行中の OS の規則でファイル名として有効かを検査する。
pub fn validate_name(name: &str) -> Result<(), String> {
    validate_name_for(name, Platform::current())
}

/// 指定した OS の規則でファイル名として有効かを検査する。
pub fn validate_name_for(name: &str, platform: Platform) -> Result<(), String> {
    if name.is_empty() {
        return Err("名前が空です".into());
    }
    if name == "." || name == ".." {
        return Err("「.」「..」は使えません".into());
    }
    if let Some(c) = name.chars().find(|c| platform.is_forbidden_char(*c)) {
        return Err(if (c as u32) < 0x20 {
            "制御文字は使えません".into()
        } else {
            format!("「{c}」は使えません")
        });
    }
    if name.len() > 255 {
        return Err("名前が長すぎます (255 バイト超)".into());
    }

    if platform == Platform::Windows {
        if name.ends_with('.') || name.ends_with(' ') {
            return Err("末尾のピリオド/空白は使えません".into());
        }
        let stem = name
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end()
            .to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit()
                && stem.as_bytes()[3] != b'0');
        if reserved {
            return Err(format!("「{stem}」は Windows の予約名です"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_rules() {
        let w = Platform::Windows;
        assert!(validate_name_for("a:b", w).is_err());
        assert!(validate_name_for("con.txt", w).is_err());
        assert!(validate_name_for("COM1", w).is_err());
        assert!(validate_name_for("name.", w).is_err());
        assert!(validate_name_for("COM0.txt", w).is_ok());
        assert!(validate_name_for("写真_001.jpg", w).is_ok());
    }

    #[test]
    fn unix_rules() {
        let u = Platform::Unix;
        assert!(validate_name_for("", u).is_err());
        assert!(validate_name_for("a/b", u).is_err());
        assert!(validate_name_for("a:b?.txt", u).is_ok());
        assert!(validate_name_for("con.txt", u).is_ok());
    }

    #[test]
    fn macos_rules() {
        assert!(validate_name_for("a:b", Platform::MacOs).is_err());
        assert!(validate_name_for("a?b", Platform::MacOs).is_ok());
    }
}
