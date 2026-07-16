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

## プロジェクトファイル

`.ori2`の開く・保存・別名保存はRust側の専用commandだけが実行します。WebViewへ汎用ファイルシステム権限は渡していません。保存は一時ファイルへの書込み、同期、コンテナ再検証、原子的置換の順で行い、失敗時は既存ファイルを維持します。

編集、検証、ファイル操作はクライアント側で直列化し、Rust側でもproject IDとrevisionを照合します。未保存のままウィンドウを閉じる場合は確認し、macOSの通常のCmd+Q／アプリメニュー終了も同じ未保存ガードへ通します。

### 既知の制約

macOSのDockメニューからの終了やOS終了要求は、利用中のTauriランタイムがアプリの終了要求イベントを迂回する場合があります。macOS配布完了の判定前に実機試験を行い、自動復旧保存またはネイティブdelegateによる保護を追加します。

ルートの[要件定義書](../../docs/requirements-definition.md)、[基本設計書](../../docs/basic-design.md)、[開発進捗](../../docs/progress.md)も参照してください。
