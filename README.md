# ORIGAMI2

ORIGAMI2は、展開図の精密編集、3D折りシミュレーション、折り手順制作を一つにまとめる折り紙設計支援アプリです。正式名称は公開前に再検討します。

現在は初期開発中です。完成版としての配布はまだ行っていません。

## 目標

- 任意の直線多角形からなる一枚紙
- 山折り・谷折り・補助線・輪郭線・切断線
- 数式と幾何制約を使う精密な2D編集
- FOLD 1.2、静的SVG、実寸PDF 1.7、DXF AC1021による一枚展開図の書き出し
- 紙厚と衝突を考慮した3D折り操作
- 折り可能性と折り経路の検証
- 折り手順の記録、編集、再生、複数ページの折り図PDF出力
- 将来は画像・3Dモデル・骨格から展開図と折り方を自動生成

実寸PDF 1.7は印刷・共有用の一ページ展開図です。折り工程、矢印、説明文、完成図を載せる将来の折り図PDFとは別機能です。

詳細は次の文書を参照してください。

- [要件定義書](docs/requirements-definition.md)
- [基本設計書](docs/basic-design.md)
- [技術調査・選定記録](docs/technical-research.md)
- [開発進捗](docs/progress.md)
- [FOLD/SVG/PDF/DXF展開図書き出し契約](docs/crease-pattern-export-contract.md)

## 技術構成

- Tauri 2
- React / TypeScript
- Three.js
- Rust計算コア

## 開発

### UI

```powershell
cd apps/desktop
npm install
npm run dev
```

### 検査

```powershell
cd apps/desktop
npm run build
npm run lint

cd ../..
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

TauriのWindowsビルドにはRust MSVC toolchain、Visual Studio Build ToolsのC++ワークロード、WebView2が必要です。macOSビルドにはXcode Command Line Toolsが必要です。

## ライセンス

GPL-3.0を第一候補としています。`GPL-3.0-only`または`GPL-3.0-or-later`の最終選択は初回公開前に確定します。
