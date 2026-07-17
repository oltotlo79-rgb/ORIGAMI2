# ORIGAMI2 基本設計書

## 1. 設計目標

本設計は、[要件定義書](requirements-definition.md)の初版要件を、段階的に実装・検証できる構造へ変換する。特に次を守る。

1. UIと幾何計算を分離する。
2. 表示用近似値と判定用の正確な状態を混同しない。
3. 長時間処理は中止可能なジョブとして扱う。
4. 2D、3D、折り手順、自動設計で同一のプロジェクトモデルを使用する。
5. 10,000本規模で全件再計算せず、差分更新できる構造にする。
6. 初版を外部配布する前も、機能単位で内部確認可能にする。

## 2. システム構成

```text
┌──────────────── Desktop Application ────────────────┐
│                                                     │
│  React/TypeScript UI                                │
│  ├─ Workspace / Dock layout                         │
│  ├─ 2D Canvas renderer                              │
│  ├─ Three.js 3D renderer                            │
│  ├─ Timeline / Inspector / Validation panel         │
│  └─ Localized commands and shortcuts                │
│             │ typed commands/events                 │
│             ▼                                       │
│  Tauri boundary                                     │
│  ├─ Capability control                              │
│  ├─ Command transport                               │
│  └─ Job progress/cancellation events                │
│             │                                       │
│             ▼                                       │
│  Native Rust Core                                   │
│  ├─ Domain model and command history                │
│  ├─ Geometry / topology / constraints               │
│  ├─ Validation / fold kinematics                    │
│  ├─ Thickness-aware collision                       │
│  ├─ Foldability / path-search jobs                  │
│  ├─ Instructions                                    │
│  └─ Import / export / persistence                   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

## 3. リポジトリ構成案

```text
ORIGAMI2/
├─ apps/
│  └─ desktop/                 # Tauri + React application
│     ├─ src/                  # TypeScript UI
│     └─ src-tauri/            # desktop host only
├─ crates/
│  ├─ ori-domain/              # IDs, entities, commands, errors
│  ├─ ori-numeric/             # expressions, precision, robust predicates
│  ├─ ori-geometry/            # 2D/3D primitives and spatial indexes
│  ├─ ori-topology/            # planar graph, faces, cuts, adjacency
│  ├─ ori-constraints/         # geometric constraints and solver interface
│  ├─ ori-fold/                # fold states and kinematics
│  ├─ ori-collision/           # thickness and continuous collision
│  ├─ ori-validation/          # local/global validation
│  ├─ ori-planner/             # fold path search and cancellation
│  ├─ ori-instructions/        # timeline and techniques
│  ├─ ori-formats/             # ori2, FOLD, SVG, DXF, OBJ/STL/glTF/PDF
│  └─ ori-core/                # public facade coordinating the crates
├─ docs/
├─ fixtures/                   # license-cleared test patterns
└─ tools/                      # schema, fixture, benchmark utilities
```

依存方向は下位から上位への一方向とする。`ori-domain`と`ori-numeric`はUI、Tauri、Three.jsへ依存しない。`ori-core`だけがユースケースを束ね、desktop hostは`ori-core`の公開APIだけを呼ぶ。

## 4. ドメインモデル

### 4.1 ID

頂点、辺、面、制約、レイヤー、手順等は表示名と独立した不変IDを持つ。配列インデックスを永続IDとして使わない。

```text
ProjectId, VertexId, EdgeId, FaceId, ConstraintId,
LayerId, StepId, TechniqueId, AssetId, JobId
```

IDはプロジェクト内で一意とし、Undo/Redo、外部形式変換、差分同期で安定して参照できるものとする。

### 4.2 Project

```text
Project
├─ format_version
├─ metadata
├─ settings
│  ├─ locale-independent units
│  ├─ cutting_allowed
│  └─ numeric policy
├─ paper
├─ crease_pattern
├─ fold_state
├─ instruction_timeline
├─ techniques
├─ assets
└─ history
```

UI言語、テーマ、ウィンドウ配置、ショートカット、更新確認は作品データに含めず、端末の利用者設定へ保存する。

### 4.3 CreasePattern

2D上の平面埋め込みグラフとして表す。

- Vertex: 2D座標、式、属性
- Edge: 始点、終点、線種、属性、レイヤー
- Face: 境界ループ、穴、属性
- Constraint: 対象ID、式、強度、状態
- Layer: 表示、ロック、透明度

面は編集のたびに全面再構築せず、影響領域だけ再構築できるAPIを持つ。ただし初期PoCでは正しさを優先し、全面再構築版を参照実装として残す。

### 4.4 FoldState

```text
FoldState
├─ base_face
├─ fold_angles[EdgeId]
├─ face_transforms[FaceId]
├─ layer_order/contact graph
├─ collision_state
└─ validation_stamp
```

3D表示側の行列を正本にしない。折り角、固定面、拘束条件からRustコアが面変換を計算し、UIは描画スナップショットを受け取る。

### 4.5 InstructionTimeline

各ステップは開始状態、終了状態、操作、表示情報を分離する。

```text
InstructionStep
├─ operations[]
│  ├─ fold edge and angle curve
│  ├─ fixed faces
│  ├─ grips / presses / handoff
│  └─ technique reference
├─ duration
├─ camera
├─ annotations
└─ localized description
```

折り技法テンプレートは、複数操作、適用条件、パラメーター、表示名を持つ。共有ファイルを信頼できない入力として検証する。

## 5. コマンド・履歴設計

すべての永続変更はコマンドとして表す。

```text
Command
├─ command_id
├─ expected_revision
├─ payload
└─ author/time metadata (local only)

CommandResult
├─ new_revision
├─ changed entity IDs
├─ validation dirtiness
├─ render delta
└─ inverse command / history record
```

- UIは状態を直接変更せず、コマンドをRustコアへ送る。
- Rustコアは検証後に適用し、差分を返す。
- UIは楽観的表示を行えるが、拒否時にコア状態へ戻す。
- 永続Undo/Redoはコマンドログと定期スナップショットを組み合わせる。
- 履歴容量上限を超えた場合、古いログをスナップショットへ圧縮する。
- 未完成状態を許す2D編集コマンドと、必ず有効状態を要求する3D操作コマンドを区別する。

## 6. 数値・幾何設計

### 6.1 数値表現

```text
ScalarExpression
├─ source_text       # "1/3 + sqrt(2)"
├─ parsed_ast
├─ unit_dimension
├─ evaluated_value   # high precision/f64 cache
└─ evaluation_error
```

- 永続座標は式と評価値の関連を保持する。
- ドラッグ操作はf64を生成し、必要に応じて利用者が式へ置換できる。
- 単位は内部基準へ正規化し、表示時に変換する。
- 許容誤差は用途別ポリシーを通して渡し、暗黙のグローバルepsilonを禁止する。

### 6.2 頑健な述語

トポロジーを変更する判定では、単純な`abs(x) < epsilon`だけに依存しない。

- orientation2d/3d
- incircle
- segment intersection
- point-on-segment
- polygon containment
- coplanarity

高速なf64フィルターで確定できないケースだけ高精度評価へ昇格する。判定結果は`Positive / Negative / Zero / Indeterminate`等、曖昧性を型で表す。

### 6.3 空間索引

- 2D: R-treeまたはBVHでスナップ、交差候補、選択候補を絞る。
- 3D: 動的BVHで面・厚み形状の衝突候補を絞る。
- 変更された局所領域のみ索引を更新する。

## 7. 2D編集設計

### 7.1 描画レイヤー

1. 紙・表裏背景
2. 下絵
3. 面塗り
4. 輪郭・切断線
5. 山折り・谷折り
6. 補助線・作図図形
7. 制約記号・寸法
8. 選択・ホバー・スナップ候補

表示用の線はバッチ化し、画面内要素を優先して描画する。ピッキングは色IDバッファまたは空間索引と距離判定を比較検証する。

### 7.2 ツール状態機械

各ツールは`Idle → Preview → Commit/Cancel`を共通契約とする。右クリック/Escapeで安全にキャンセルし、Commit時だけドメインコマンドを生成する。

### 7.3 制約

制約ソルバーはインターフェースで分離する。

```text
solve(current geometry, constraints, edited targets, policy)
  -> Solved | UnderConstrained | OverConstrained(conflict set) | Failed
```

リアルタイム対称編集は単なる複製ではなく、対応要素の関係を制約として保持する。

## 8. 3D・衝突設計

### 8.1 折り運動

- 利用者が固定面と操作折り線を選ぶ。
- 切断されていない面接続グラフから移動側候補を求める。
- 木構造はヒンジ変換を伝播する。
- 閉ループは拘束問題として解き、解がない操作を拒否する。
- 3Dドラッグは目標姿勢から折り角を求め、同じ検証経路へ渡す。

### 8.2 厚さモデル

初版では各面を一定厚さの薄い立体として衝突判定する。次を明示的に分類する。

- 離隔
- 接触
- 許容された共有ヒンジ接触
- 表裏の重なり接触
- 不正な貫通

折り曲げ半径や材料変形は計算しない。厚さによって理想ヒンジの周囲で幾何学的矛盾が生じる場合は、厚さモデル仕様で選んだオフセット規則を表示し、物理的完全再現とは表現しない。

#### 8.2.1 現在の中央面基準共有ヒンジモデル

初期実装`centered_mid_surface_v1`では、面の中央面を折り運動の正本とし、入力紙厚`t`の半分ずつを表裏へ押し出した三角柱で現在姿勢を判定する。表示を見やすくするために調整した厚さは判定へ使用しない。

共有ヒンジ接触を許容するには、次の条件をすべて要求する。

- adjacencyごとにEdgeId、幾何学的な左右面、共有辺両端のVertexIdと座標、厚さ規則が一対一で一致する。
- 共有辺が両面の実境界辺で逆向きに現れ、共有辺に接する左右三角形の第3頂点が展開時の支持線を挟んで反対側にある。
- 左右面の剛体変換後も共有辺の両端と軸が数値許容範囲内で一致する。
- 候補面間の全三角形対を処理上限内で走査し、判定に使う三角柱6頂点を静的面、現在姿勢、実紙厚から再現できる。
- 唯一の共有辺に接する三角形対は有限ヒンジ区間内にあり、それ以外の相互作用は許容領域内と証明できるか、許容領域外と証明できる。

紙厚の半分を`h=t/2`、左右中央面法線間の角度を`θ`とすると、理想的な中央面基準half-slabの共有辺周辺重なりは半径`R=h/cos(θ/2)`に収まる。この解析条件を満たす共有辺三角形対だけを「モデルで許容した折り目境界接触」または「モデルで許容した折り目領域内重なり」とする。

ゼロ厚、180度またはその数値特異近傍、複数共有ヒンジ、姿勢不一致、走査未完了、偽造・重複した三角形対、許容境界を安全に分類できない場合は`indeterminate`へ退避する。許容領域外の体積重なりを証明できた場合だけヒンジ外貫通、境界接触を証明できた場合だけヒンジ外接触とする。

これは現在姿勢に対する近似分類であり、実際の折り癖、材料変形、層ずれ、連続運動中の衝突を保証しない。OQ-002の物理的な厚さオフセット規則は引き続き未解決とし、UIでも中央面基準モデルによる許容であることを明示する。

### 8.3 衝突前停止

現在角から目標角までの連続運動を判定し、最初の衝突時刻`0..1`を求める。安全余白手前で停止し、面ID、接触点、法線、折り角、分類をUIへ返す。

## 9. 検証・探索ジョブ

### 9.1 ジョブ契約

```text
Job
├─ id
├─ immutable input snapshot
├─ cancellation token
├─ progress events
├─ checkpoints (when supported)
└─ typed result
```

- 編集中のプロジェクトとジョブ入力を分離する。
- 実行中に編集された場合、結果へ入力revisionを添え、古い結果として表示する。
- 中止は協調的に行い、一定間隔でトークンを確認する。
- `Impossible`、`Cancelled`、`ResourceLimit`、`InternalError`を混同しない。

### 9.2 検証パイプライン

```text
Schema
  → topology
  → local geometry
  → face construction
  → local foldability
  → global flat-foldability
  → kinematic/path feasibility
```

前段に失敗した場合、依存する後段は`BlockedByEarlierError`とする。

### 9.3 完全性

全体判定と経路探索は、アルゴリズムが完全性を保証できる対象クラスを結果に含める。対象外入力に対して、時間をかけた近似探索の失敗を「不可能」と断定しない。要件上の厳密判定を満たせる対象範囲はPoC後に確定し、要件定義書へ反映する。

## 10. UI設計

### 10.1 ワークスペース

- 2D、3D、タイムライン、インスペクター、検証パネルをドッキング可能にする。
- 並列表示とタブ表示を切り替える。
- 2D/3D選択を同期する。
- 作業レイアウトは利用者設定として保存する。

### 10.2 操作モード

初版は作家モードのみを公開する。将来の簡単設計モードも同じコマンドとドメインモデルを使用し、生成された展開図を作家モードで編集可能にする。

### 10.3 エラー表示

- 問題要素を2D/3Dで強調する。
- 人間向け説明、要件/規則名、関連ID、修正候補を表示する。
- 編集途中の警告と3D移行を阻止するエラーを明確に区別する。

## 11. 保存・復旧設計

### 11.1 `.ori2`

ZIP系コンテナを候補とし、最低限以下を格納する。

```text
manifest.json
project.json
history/
assets/
thumbnails/
```

- `manifest.json`に形式バージョン、必要機能、ファイルハッシュを持たせる。
- 保存は同一ディレクトリの一時ファイルへ書き、検証後に置換する。
- 展開フォルダー形式は同一スキーマを使用する。
- 未知フィールドは可能な限り保持し、破壊的な旧版保存を警告する。

### 11.2 自動復旧

- 通常ファイルとは別の端末内領域へコマンドログとスナップショットを保存する。
- 正常終了時は不要な復旧データを整理する。
- 起動時に復旧候補を示し、元ファイルを上書きせず復元コピーを開く。

## 12. インポート・エクスポート

各形式アダプターは共通の中間モデルを介する。

```text
external format ↔ format adapter ↔ ORIGAMI2 project/domain
```

変換結果には次を返す。

- errors: 変換不能
- warnings: 情報損失、近似、未対応属性
- mapping: 外部IDと内部IDの対応
- report: 単位、座標系、線種割り当て

3D出力では座標系、単位、表裏、厚さ、アニメーション対応差を出力画面に示す。

## 13. セキュリティ設計

- Tauri capabilityをファイル選択、設定、更新確認等へ限定する。
- UIから任意パスや任意コマンドをRustへ渡せる汎用APIを作らない。
- ZIP bomb、巨大画像、再帰参照、不正JSON、NaN/Infinity、ID衝突を検査する。
- SVG内スクリプトや外部リソースを実行・自動取得しない。
- 診断ログは作品座標・パス・内容を標準で記録しない。
- 更新確認はGitHubの公開情報だけを取得し、無効化可能にする。

## 14. 実装フェーズ

外部配布は全MUST要件完成後とするが、内部確認可能な垂直スライスを以下の順で作る。

### Phase 0: 技術PoC

- Tauri/Rust/Reactの通信
- 10,000本線描画と選択ベンチマーク
- 頑健な線分交差・面抽出
- 厚さ付き2面の連続衝突
- Windows/macOSビルド

完了条件: 採用技術と性能目標を具体値で確定できる。

### Phase 1: プロジェクト骨格

- ドメインID、紙、頂点、線、レイヤー
- コマンド、Undo/Redo、revision
- `.ori2`最小保存と自動復旧
- 日本語/英語、テーマ、基本レイアウト

完了条件: 任意多角形の紙と直線を作成、保存、再読込できる。

### Phase 2: 精密2Dエディター

- スナップ、補助作図、数式入力
- 面抽出、切断
- 幾何制約、矛盾表示、対称編集
- SVG/FOLD入出力

完了条件: 既知の展開図を数値的に再現し、検証前状態を保存できる。

### Phase 3: 3D折り

- 面グラフ、固定面、角度操作
- 3Dドラッグ
- 紙厚、衝突、衝突前停止
- 2D/3D選択同期

完了条件: 基本パターンを指定角へ折り、厚さ衝突を説明付きで停止できる。

### Phase 4: 検証・探索

- 構造検証、川崎・前川条件
- 全体平坦折り判定
- 中止可能な折り経路探索
- 進捗、結果根拠、対象クラス表示

完了条件: 合意した対象クラスで既知の可能・不可能例を正しく分類する。

### Phase 5: 折り手順

- タイムライン、同時折り、固定面、持ち替え
- 3D操作記録、編集、再生
- 簡略指ガイド、技法テンプレート
- 画像/PDF出力

完了条件: 一作品の折り方を最初から最後まで記録、再生、PDF化できる。

### Phase 6: 互換性・性能・配布

- DXF、OBJ、STL、glTF
- 10,000本性能最適化
- Windows/macOSパッケージ
- 更新確認、ログ、利用文書
- 全受け入れ試験

完了条件: 要件定義書の初版受け入れ条件をすべて満たす。

### Future: 初心者向け自動設計

- TargetShape/Skeleton/Part
- 画像・3D認識
- 候補生成と評価プロファイル
- 展開図・手順自動生成

## 15. 最初の技術スパイク

実装開始時は、製品UI全体を作る前に次の最小検証アプリを作る。

1. Rust側で10,000本の折り線と面データを生成する。
2. UIへ初期スナップショットを送り、CanvasとThree.jsで描画する。
3. 2Dの線を選択し、Rustへ選択/編集コマンドを送る。
4. Rustから差分だけを返し、2D/3Dを更新する。
5. 2面を一つのヒンジで折り、厚さ衝突の直前で停止する。
6. 長時間ダミージョブを開始・進捗表示・中止する。
7. 同一コードをWindowsとmacOSでビルドする。

このスパイクで通信量、描画方式、ジョブモデル、衝突ライブラリを確定する。結果はADRとベンチマークとして保存する。

## 16. 設計上の未解決事項

| ID | 項目 | 解決時期 |
|---|---|---|
| OQ-001 | 全体平坦折り判定の保証対象クラス | Phase 0〜4前 |
| OQ-002 | 厚さ付きヒンジのオフセット規則 | Phase 0 |
| OQ-003 | 高精度数値ライブラリ | Phase 0 |
| OQ-004 | 制約ソルバーと競合制約抽出 | Phase 2前 |
| OQ-005 | 2D Canvas/WebGL切替基準 | Phase 0 |
| OQ-006 | `.ori2`スキーマと履歴圧縮 | Phase 1前 |
| OQ-007 | PDF図記号とレイアウト規則 | Phase 5前 |
| OQ-008 | 各外部形式の対応バージョン | Phase 2/6前 |
| OQ-009 | GPL-3.0-only/or-later | 公開前 |
| OQ-010 | 正式名称と商標 | 公開前 |
