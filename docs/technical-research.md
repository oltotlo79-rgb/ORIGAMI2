# ORIGAMI2 技術調査・選定記録

## 1. 結論

ORIGAMI2の初版は、以下を基準構成とする。

| 領域 | 採用候補 | 方針 |
|---|---|---|
| デスクトップ基盤 | Tauri 2 | Windows/macOSの配布、OS機能、権限制御を担当 |
| UI | React + TypeScript | 2D/3D/タイムライン/設定画面を構築 |
| 状態管理 | Zustand相当の軽量ストア + コマンド履歴 | UI状態と永続ドメイン状態を分離 |
| 2D描画 | Canvas 2Dを起点に、必要箇所をWebGLへ移行 | 10,000本編集に備えDOM/SVG依存を避ける |
| 3D描画 | Three.js | WebGLを基準とし、WebGPUは任意の高速経路に限定 |
| 計算コア | Rust | 幾何、トポロジー、制約、検証、折り、衝突、探索を担当 |
| 線形代数 | nalgebra | 点、ベクトル、行列、回転、変換 |
| 衝突候補 | parry3d-f64 | 広域判定、距離、接触、連続衝突判定の基盤候補 |
| 高精度数値 | rug等を比較検証 | 数式評価・検証用。ホットパスではf64と適応的精度を使い分ける |
| シリアライズ | serde + JSON + ZIPコンテナ候補 | `.ori2`と展開フォルダー形式の共通スキーマ |
| 並列計算 | Rustネイティブスレッド/Rayon相当 | 中止可能なジョブとして実行 |
| テスト | Rust unit/property test + UI component/E2E test | 計算コアをUIなしで検証 |

## 2. 選定理由

### 2.1 Tauri 2

TauriはOSのWebViewとRustバックエンドをメッセージ通信で接続する構成であり、Windows/macOS向けの小型なデスクトップアプリを作成できる。権限はcapabilityとして限定できるため、ファイル、更新確認等の必要機能だけを公開する。

- 公式アーキテクチャ: <https://v2.tauri.app/concept/architecture/>
- 公式Capability仕様: <https://v2.tauri.app/security/capabilities/>

採用上の注意:

- WindowsとmacOSでWebView実装が異なるため、描画・ファイル選択・印刷を両OSで継続試験する。
- UIから計算コアへ巨大JSONを頻繁に渡さない。差分コマンドまたはバイナリ転送を設計する。
- capabilityは最小権限とし、任意シェル実行を公開しない。

### 2.2 React + TypeScript

複数パネル、インスペクター、タイムライン、設定画面の構築に使用する。Reactをドメインモデルの正本にはせず、Rustコアが検証済み状態を所有する。編集中の一時状態と表示状態だけをTypeScript側で管理する。

### 2.3 Three.js

Three.jsは3D表示、選択、カメラ、材質、glTF連携に適する。大量オブジェクトにはBufferGeometry、バッチ化、インスタンス描画を用いる。公式のInstancedMeshは同一形状の大量描画でドローコールを減らせるが、折り紙の面は形状が異なるため、面ごとのMesh乱立ではなく共有バッファとグループ更新を基本とする。

- InstancedMesh: <https://threejs.org/docs/pages/InstancedMesh.html>
- WebGPU capability: <https://threejs.org/docs/pages/WebGPU.html>

WebGPUは必須にしない。WebGPU非対応環境でもWebGLで全機能を利用できることを優先し、将来の計算・描画高速化経路として扱う。

### 2.4 ネイティブRust計算コア

長時間の厳密探索、中止可能なバックグラウンド処理、マルチコア利用、ファイル変換を安定して行うため、計算コアはTauriのRustプロセス内に置く。WebAssemblyは将来のWeb版向けアダプター候補とし、初版の主計算経路にはしない。

Rust公式資料でもWebAssemblyのマルチスレッド利用は追加設定を必要とするため、初版のデスクトップ要件ではネイティブ実行が適する。

- Rust wasm32 intrinsics: <https://doc.rust-lang.org/core/arch/wasm32/>

### 2.5 nalgebra

nalgebraはRustの線形代数・幾何変換ライブラリであり、点、ベクトル、回転、等長変換を型として扱える。Apache-2.0ライセンスでGPL-3.0プロジェクトと組み合わせられる。

- 公式ドキュメント: <https://www.nalgebra.rs/docs/>
- 点と変換: <https://www.nalgebra.rs/docs/user_guide/points_and_transformations/>

### 2.6 parry3d-f64

Parryは距離、交差、接触、shape cast、非線形剛体運動の衝突時刻等を提供する。紙の厚さ付き三角形/凸形状の候補衝突判定に利用可能か、プロトタイプで評価する。

- crate: <https://docs.rs/parry3d/latest/parry3d/>
- query API: <https://docs.rs/parry3d/latest/parry3d/query/>

ただし、紙面同士の接触、共有ヒンジ、許容接触、厚さオフセットは一般剛体衝突とは異なる。Parryを判定の正本にせず、ORIGAMI2固有の接触分類層を上に設ける。

### 2.7 高精度数値

入力式は原文と構文木を保持し、評価値を別に持つ。全計算を任意精度で行うと10,000本規模に不向きなため、次の混合方式を採る。

1. 表示・ドラッグ・3D変換は原則f64。
2. 位相を決めるorientation/intersection等は頑健な述語を使う。
3. 境界付近または証明が必要な判定だけ高精度へ昇格する。
4. 数式入力は有理数・高精度浮動小数として評価可能にする。

Rugは任意精度整数・有理数・浮動小数を提供するが、GMP/MPFR/MPC依存とクロスビルド負担があるため、正式採用前にWindows/macOS CIで検証する。

- Rug: <https://docs.rs/crate/rug/latest>

## 3. 見送る構成

### 3.1 Electronのみで計算する構成

UI開発は容易だが、計算コアの正確性・長時間処理・ネイティブ並列化・配布サイズの要件に対し追加のネイティブ層が結局必要となるため、主構成にしない。

### 3.2 すべてをWebAssemblyで実装

将来のWeb版には有利だが、初版はデスクトップのみで、マルチスレッド、巨大メモリ、ネイティブライブラリ、長時間ジョブ管理の制約が増える。ドメインコアはWebAssemblyへ移植可能な依存方向に保つが、初版はネイティブRustを使う。

### 3.3 SVGを2D編集画面の主描画に使用

小規模図面では扱いやすいが、10,000本、選択ハイライト、ドラッグ、レイヤー、補助表示でDOM更新負荷が増える。SVGは入出力形式とし、編集画面はCanvas/WebGLを使用する。

### 3.4 WebGPU必須

対象PCの対応差とWebView差があるため必須にしない。WebGLを互換経路とし、WebGPUは検出後に有効化する高速化機能とする。

## 4. 技術検証が必要な項目

優先順位順に、コードベースを大きくする前に検証する。

1. TauriでRustコアとUI間に10,000本規模の差分を低遅延で同期できるか。
2. 頑健な2D交差・面抽出・切断更新をf64＋適応的精度で実装できるか。
3. 厚さ付き面、共有ヒンジ、接触許容を含む連続衝突判定モデル。
4. 固定面からの折り伝播と閉ループ制約の扱い。
5. `.ori2`の増分保存、履歴、クラッシュ復旧の容量と速度。
6. Three.jsで10,000本以上の線と多数面を選択・更新する描画方式。
7. Rugまたは代替高精度ライブラリのWindows/macOSビルドとGPL互換性。
8. PDF生成、フォント埋め込み、日本語・英語、山谷線種の品質。
9. FOLD/DXF/glTF等の意味変換と情報損失表示。
10. 任意展開図の全体平坦折り判定・経路探索で保証可能な対象クラス。

## 5. ライセンス方針

- プロジェクト: GPL-3.0を第一候補とする。
- 依存関係: MIT、BSD、Apache-2.0、ISC、Zlib、LGPL等を個別確認する。
- GPLと非互換、用途制限付き、商用禁止、出典不明のコード/素材を混入させない。
- Rust crate、npm package、フォント、アイコン、サンプル展開図を台帳管理する。
- GPL-3.0-only / GPL-3.0-or-laterは公開前に確定する。

## 6. 選定ステータス

| 判断 | 状態 |
|---|---|
| Tauri 2 | 採用 |
| React + TypeScript | 採用 |
| ネイティブRustコア | 採用 |
| Three.js/WebGL | 採用 |
| WebGPU | 任意高速化、必須ではない |
| nalgebra | 採用候補、PoC後確定 |
| parry3d-f64 | 採用候補、厚さ衝突PoC後確定 |
| rug | 比較候補、クロスビルドPoC後確定 |
| 具体的な制約ソルバー | 未選定 |
| 全体平坦折り・経路探索アルゴリズム | 研究・PoC後に対象クラスを確定 |

