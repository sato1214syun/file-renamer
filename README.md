# Flex Renamer

Rust + [GPUI](https://gpui.rs) で作る、Windows / macOS / Linux 対応の一括リネームツール（Flexible Renamer 風）。

## 機能（v0.4）

| 区分 | 内容 |
|---|---|
| 入力 | フォルダを開く / ファイルを選択（入れ替え）、ドラッグ＆ドロップ（追加）、クリア |
| 一覧の編集 | クリックで選択（Ctrl/⌘ で追加、Shift で範囲）、Delete で一覧から削除、Ctrl/⌘+A、Esc |
| ルール | 置換 / 正規表現 / 先頭・末尾に追加 / 挿入 / 連番 / 文字数で削除 / 大文字小文字 / 全角半角 / 拡張子 |
| 設定の操作 | 選択肢は**ドロップダウン**、ON/OFF は**チェックボックス** |
| 親フォルダ名 | 置換後・追加する文字・挿入する文字・連番の区切りで `\f0`〜`\f9` が使える（`\f0` = 入っているフォルダ、`\f1` = その 1 つ上 …、`\\` = `\`） |
| 正規表現 | `$1`〜`$9` は 1 桁のグループ参照、`$$` で `$`、エラーは 1 行の日本語で位置付き |
| 全角/半角 | 英数字・記号・スペース・カタカナを個別に指定。その OS で使えない文字には変換しない |
| 名前チェック | 実行中の OS の規則で自動チェック |
| レイアウト | 左右の境界・各領域の下端・列の境界をドラッグでサイズ変更（保存される） |
| スクロール | 見切れる領域はスクロールバー（つまみをドラッグ）とマウスホイールで移動。一覧は横スクロールも可（Shift+ホイール） |
| プリセット / セッション | ルール＋オプションの保存・読み込み・削除、前回の状態とレイアウトを自動復元 |
| 実行 / Undo | 一時名を経由する 2 段階リネーム、失敗時は全件ロールバック、直近 1 回分の Undo |

## 構成

```
crates/
├── renamer-core/   ルール・プラン生成・検証（OS / GUI 非依存）
│   └── src/{lib,error,model,rule,plan,platform,token,validate,width,util}.rs
├── renamer-fs/     走査・2 段階リネーム・Undo・プリセット
│   └── src/{lib,scan,execute,undo,preset}.rs
└── renamer-gui/    GPUI フロントエンド（バイナリ名 flex-renamer）
    └── src/{main,app,text_input,session,theme,components}.rs
        views/{toolbar,rule_panel,file_list,status_bar,dropdown,layout}.rs
```

## ビルド

Rust **1.100** 固定（`rust-toolchain.toml`）。

```sh
cargo test -p renamer-core -p renamer-fs
cargo run
```

### Windows (ARM64)

`ring` のビルドに AArch64 対応の clang が必要です。`.cargo/config.toml` に次を置いてください。

```toml
[env]
CC_aarch64_pc_windows_msvc = "C:\\Program Files\\LLVM\\bin\\clang.exe"
AR_aarch64_pc_windows_msvc = "C:\\Program Files\\LLVM\\bin\\llvm-lib.exe"
```

## 保存場所

`%APPDATA%\flex-renamer`（Windows）、`~/Library/Application Support/flex-renamer`（macOS）、
`~/.local/share/flex-renamer`（Linux）に `presets/*.json`、`session.json`、`last-undo.json`。
以前のバージョンのプリセット・セッションはそのまま読み込めます（列幅のみ初期値に戻ります）。
