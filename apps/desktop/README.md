# ORIGAMI2 Desktop

Tauri 2、React、TypeScript、Three.jsで構成するORIGAMI2のデスクトップUIです。作品データと編集履歴の正本はRust側が保持し、React側は表示、入力途中の状態、Tauri command呼び出しを担当します。

## 開発コマンド

```powershell
npm ci
npm run dev
npm run build
npm run lint
```

Tauriウィンドウを起動する場合は、Rust toolchain、WindowsではVisual Studio Build ToolsとWebView2、macOSではXcode Command Line Toolsを準備してから実行します。

```powershell
npm run tauri dev
```

## ディレクトリ

- `src/`: React UIと2D/3D表示
- `src/lib/coreClient.ts`: Rustコアとの型付きcommand境界
- `src-tauri/`: Tauriホスト、ネイティブ状態、ファイル操作
- `src-tauri/icons/`: Windows/macOS/Linux向けアプリアイコン

ルートの[要件定義書](../../docs/requirements-definition.md)、[基本設計書](../../docs/basic-design.md)、[開発進捗](../../docs/progress.md)も参照してください。
