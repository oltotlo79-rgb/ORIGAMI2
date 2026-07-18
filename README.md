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
- 折り手順の記録、編集、段階再生、A4複数ページ折り図PDF・SVGページ画像ZIP出力
- 将来は画像・3Dモデル・骨格から展開図と折り方を自動生成

実寸PDF 1.7は印刷用の一ページ展開図です。折り工程、説明文、注意事項、各手順の固定3D図を載せる複数ページ折り図PDFは別の書き出し機能です。初版の折り図は固定投影で、矢印、手指ガイド、滑らかな連続アニメーションは生成しません。

初版の正式配布対象はWindows 10/11です。macOSは自動ビルドとCI検証を維持し、実機検証環境を用意できた時点で正式対応を判定します。

詳細は次の文書を参照してください。

- [要件定義書](docs/requirements-definition.md)
- [基本設計書](docs/basic-design.md)
- [技術調査・選定記録](docs/technical-research.md)
- [開発進捗](docs/progress.md)
- [FOLD/SVG/PDF/DXF展開図書き出し契約](docs/crease-pattern-export-contract.md)
- [折り手順PDF・SVG画像書き出し契約](docs/instruction-export-contract.md)
- [全体平坦折り判定と層順序管理の設計](docs/global-flat-foldability-design.md)

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
