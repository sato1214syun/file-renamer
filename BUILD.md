# 配布用ビルド（GitHub Actions）

| ワークフロー | タイミング | 内容 |
|---|---|---|
| `ci.yml` | push / pull request | Windows x64・macOS ARM64 でフォーマット確認、テスト、GUI ビルド |
| `release.yml` | 手動実行 / `v*` タグの push | Windows x64・Windows ARM64・macOS ARM64 の配布用 zip を作成。タグのときは GitHub Releases に公開 |

## 使い方

```powershell
git add .
git commit -m "Add GitHub Actions"
git push

# 手動でビルド: GitHub の Actions タブ → Release build → Run workflow
#   → 完了後、実行結果ページの Artifacts からダウンロード

# リリースとして公開
git tag v0.4.0
git push origin v0.4.0
```

## 成果物

| ファイル | 中身 |
|---|---|
| `flex-renamer-windows-x64.zip` | `flex-renamer-windows-x64.exe` |
| `flex-renamer-windows-arm64.zip` | `flex-renamer-windows-arm64.exe` |
| `flex-renamer-macos-arm64.zip` | `Flex Renamer.app`（アドホック署名） |

macOS 版は Apple の公証を受けていないため、初回はアプリを右クリック →「開く」、または
`xattr -dr com.apple.quarantine "Flex Renamer.app"` を実行してください。

## 手元の Mac で .app を作る場合

```sh
cargo build --release -p renamer-gui --target aarch64-apple-darwin
sh scripts/bundle-macos.sh aarch64-apple-darwin macos-arm64
```
