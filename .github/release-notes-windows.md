# ORIGAMI2 {{VERSION}} - Windows

## 日本語

ORIGAMI2 {{VERSION}} Windows正式版です。

- 対応環境: Windows 10/11 x64
- 配布形式: NSISインストーラー
- ソースコミット: `{{COMMIT_SHA}}`

重要: このインストーラーはコード署名されていません。Windows SmartScreen等の警告が
表示される場合があります。実行前に、SHA256SUMS.txtとGet-FileHashでSHA-256を照合してください。
ORIGAMI2は更新の自動インストール、Releaseの自動ダウンロード、テレメトリー送信を行いません。

```powershell
Get-FileHash .\{{INSTALLER_NAME}} -Algorithm SHA256
```

macOS版は正式配布対象ではありません。macOSは自動ビルドとCI検証だけを維持し、
実機動作と利用者サポートは初版の保証対象外です。

## English

This is the ORIGAMI2 {{VERSION}} official Windows release.

- Supported systems: Windows 10/11 x64
- Package: NSIS installer
- Source commit: `{{COMMIT_SHA}}`

IMPORTANT: This installer is not code signed. Windows SmartScreen or another security
control may show a warning. Verify its SHA-256 with SHA256SUMS.txt and Get-FileHash.
ORIGAMI2 does not automatically install updates, download releases, or send telemetry.

```powershell
Get-FileHash .\{{INSTALLER_NAME}} -Algorithm SHA256
```

macOS is not an official release artifact. The macOS target is maintained only through
automated builds and CI checks; initial release support does not cover macOS.
