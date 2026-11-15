//! 実行中の OS ごとのファイル名の制約。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// Linux など
    Unix,
}

impl Platform {
    /// ビルド対象の OS。
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOs => "macOS",
            Self::Unix => "Linux",
        }
    }

    /// ファイル名に使えない文字か。
    ///
    /// - 共通: `/` と NUL
    /// - Windows: `< > : " \ | ? *` と制御文字
    /// - macOS: `:`（Finder では `/` として表示されるため）
    pub fn is_forbidden_char(self, c: char) -> bool {
        if c == '/' || c == '\0' {
            return true;
        }
        match self {
            Self::Windows => {
                matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20
            }
            Self::MacOs => c == ':',
            Self::Unix => false,
        }
    }
}
