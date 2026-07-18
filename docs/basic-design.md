# ORIGAMI2 基本設計書

## 1. 設計目標

本設計は、[要件定義書](requirements-definition.md)の初版要件を、段階的に実装・検証できる構造へ変換する。特に次を守る。

折り手順タイムラインの初期実装に関する永続モデル、折りモデル指紋、信頼境界、再生停止条件、上限は[折り手順タイムライン初期設計](instruction-timeline-design.md)を参照する。

手順ごとのSVG画像ZIPと複数ページPDFの投影、A4レイアウト、font、資源上限、stage、原子的保存は[折り手順書き出し契約](instruction-export-contract.md)を参照する。

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
│  ├─ Foldability / layer-order jobs                  │
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
- 現行の`vertical_parameter_v1`は、画面投影したヒンジ線の上下ドラッグで0〜180度の折り角パラメータを変更し、同じ検証経路へ渡す。紙面を掴んだ3D位置から角度を逆算する物理操作ではない。
- 現行UIの`physical_grab_v2`は、単一折りまたは木構造で選択した1ヒンジの従属面上にある表裏点とポインターrayから0〜180度の目標角を逆算する別契約であり、`vertical_parameter_v1`の意味を置き換えない。木構造では選択外ヒンジの完全角度vectorを固定し、連続衝突jobが認定した完全姿勢だけを原子的に3Dへ適用して終端安全角を確定する。複数ヒンジの同時運動と閉ループへの適用は未対応とする。
- フォーカス中の3D領域では、H/Shift+Hで表示順の次/前のヒンジ、F/Shift+Fで次/前の固定面を選び、Escapeでヒンジ選択を解除する。`KeyboardEvent.key`を文字入力の正本として特殊配列とOSキー再割当を尊重し、修飾キー、キーリピート、IME合成中、重複・空・過大ID、古いmodel/callbackは操作へ接続しない。実行可能なshortcutだけをARIAへ公開し、選択結果は常設した2つのpolite live regionを交互更新して同文も再通知する。

#### 8.1.1 折り重ね操作

SIM-010はVAL-003の平坦折り判定が管理する層順序を正本とし、その管理機能と、8.2節および[衝突接触ポリシーv2](collision-contact-policy-v2.md)で定める共有関係4種×交差証拠11種の全分類・境界回帰が完成した後に実装する。折り重ねUIを分類器より先に公開してはならない。現在の3D姿勢に対して一直線の折り線を指定し、その線をまたぐ重なり層を一括して動かす。成功した1操作は、同一command transaction内で次を確定する。

面系譜、層別山谷割当て、衝突停止、層順再証明、過去step移行、Undo/Redoを含む詳細なauthorityと失敗時不変条件は[折り重ね操作の原子的トランザクション設計](stacked-fold-design.md)を正本とする。表示用TS姿勢をauthorityにせず、native kinematics、current pose、衝突経路および場所別層順序transportを段階証明する境界は[native current applied pose設計](native-applied-pose-design.md)に固定する。

- 対象となった層、固定側、移動側および操作前後の完全姿勢
- 元の紙へ逆写像した層ごとの直線折り線と山谷割当て
- 展開図のrevision更新
- 折り手順タイムラインの1ステップと段階再生用snapshot

いずれかの逆写像、山谷割当て、層順序更新、衝突前停止またはtimeline記録に失敗した場合は全体を適用せず、展開図と3D姿勢を操作前に保つ。初版は折り線をまたぐ全対象層の一括操作だけを扱い、層を選んでめくる操作や中割り折り等の技法固有運動は将来拡張とする。

### 8.2 厚さモデル

初版では各面を一定厚さの薄い立体として衝突判定する。次を明示的に分類する。

- 離隔
- 接触
- 共有頂点だけと証明した許容接触
- 許容された共有ヒンジ接触
- 表裏の重なり接触
- 不正な貫通
- 交差の可能性・判定保留

折り曲げ半径や材料変形は計算しない。厚さによって理想ヒンジの周囲で幾何学的矛盾が生じる場合は、厚さモデル仕様で選んだオフセット規則を表示し、物理的完全再現とは表現しない。

#### 8.2.1 現在の中央面基準共有ヒンジモデル

初期実装`centered_mid_surface_v1`では、面の中央面を折り運動の正本とし、入力紙厚`t`の半分ずつを表裏へ押し出した三角柱で現在姿勢を判定する。表示を見やすくするために調整した厚さは判定へ使用しない。

共有ヒンジ接触を許容するには、次の条件をすべて要求する。

- adjacencyごとにEdgeId、幾何学的な左右面、共有辺両端のVertexIdと座標、厚さ規則が一対一で一致する。
- 共有辺が両面の実境界辺で逆向きに現れ、共有辺に接する左右三角形の第3頂点が展開時の支持線を挟んで反対側にある。
- 左右面の剛体変換後も共有辺の両端と軸が、有限ヒンジ長・紙厚・保存済み両端spanによる局所尺度だけから作る`topologyMargin`内で一致する。絶対world座標のULPや広域候補用global marginを共有軸identityの認証へ流用しない。大座標でのcorridor射影にはworld ULPを含む別の`numericalMargin`を用いるが、これは端点一致を認証する権限を持たない。
- 候補面間の全三角形対を処理上限内で走査し、判定に使う三角柱6頂点を静的面、現在姿勢、実紙厚から再現できる。
- 唯一の共有辺に接する三角形対は有限ヒンジ区間内にあり、それ以外の相互作用は許容領域内と証明できるか、許容領域外と証明できる。

generic `Matrix4`だけでは、巨大pivot回転のbinary64丸め差と実在する軸ずれを区別できない。真正な共有ヒンジでもworld精度枯渇により端点差が局所`topologyMargin`を超えた場合は、許容結果を推測せず`hinge_pose_mismatch`の判定保留へ退避する。この端点検査は広域候補抽出より先に全ポーズへ適用する。さらに、全adjacency面組を共有`VertexId`の有無と独立に広域候補へ照合し、共有ヒンジの一方の現在軸が離れた場合、または左右面で共有IDが壊れてAABB候補が0件になった場合も、`hinge_adjacent`かつ`pose_mismatch`の明示的な判定保留を返す。将来、kinematics provenanceまたは相対変換certificateを解析境界へ渡せる場合にだけ再認証する。

紙厚の半分を`h=t/2`、左右中央面法線間の角度を`θ`とすると、理想的な中央面基準half-slabの共有辺周辺重なりは半径`R=h/cos(θ/2)`に収まる。この解析条件を満たす共有辺三角形対だけを「モデルで許容した折り目境界接触」または「モデルで許容した折り目領域内重なり」とする。ただし、共有ヒンジの有限長を`L`とすると、初版がこの近似を許容する有限半径の閉境界は`R <= L`である。姿勢誤差を含む`outerMargin`はcorridor用`numericalMargin`と、局所`topologyMargin`内で認証済みの実測姿勢誤差から作り、corridor内外の数値分類だけに使用する。絶対world座標由来の数値marginで実在する軸ずれを吸収したり、`R > L`を許容側へ広げたりしない。これを超える深角、正厚の厳密な180度、または数値的なflat-fold特異域は、無限に広い許容corridorへ拡張せず`layer_offset_unmodeled`として判定不能で停止する。連続経路でも同じ有限半径条件を区間認定へ適用し、停止詳細には「層ずらし未再現」を専用理由として表示する。

衝突分類の共有関係4種×交差証拠11種、証拠の肯定条件、有限ヒンジcorridor、severity集約およびUI契約は[衝突接触ポリシーv2](collision-contact-policy-v2.md)を正本とする。厚さ0は正厚の退化三角柱として扱わず、理想三角形面どうしの交差次元を分類する。非隣接面の一点または境界辺だけの交差は`touching`、共面の正面積重なり、または両面内部を横断する正長線分は`penetrating`とする。正厚三角柱の体積重なりが0で接触集合が正面積の場合だけ`boundary_area_contact`とし、厚さ0からこの証拠を発行しない。共有VertexIdや共有EdgeIdだけで貫通を免除せず、同じrest座標を持つ共有点・共有辺と現在world位置が局所形状尺度由来の`topologyMargin`内で一致し、内部横断がない場合に限り、剛体変換の丸め残差をtopological contactの証拠として使う。共有ヒンジ候補の全三角形対が離間してもinteractionを省略せず、ヒンジ制約が無い場合はraw `touching`またはraw `penetrating`を許容・侵入の確定証拠として使わず、`missing_constraint`の判定保留へ倒す。絶対world座標のULPはSATやcorridorの数値安定化にだけ用い、共有identityの認証には使わない。正面積、正長、面間距離または点併合がmargin以下で次元を断定できない場合は`touching`へ落とさず`indeterminate`へ退避する。

`topology_contact_policy_v1`の4×10・全40セルは互換基準として凍結する。正厚の正面積境界接触を独立させた`topology_contact_policy_v2`の4×11・全44セルをnative証拠生成器の正本とし、frontendとnative `ori-collision`が同じ正規JSON corpusへ回帰する。これら純粋表の公開enumまたは戻り値は認証済み証拠ではなく、native実行系はcurrent poseとtriangle pairへ結合したprivate certificate/capabilityなしに衝突除外・ヒンジ許容・project mutationへ使用してはならない。runtimeで`same_face`がpair dispatcherへ到達した場合は表の`ignored_self`を採用せず、内部不整合として`indeterminate`へ閉じる。

native静的判定の最初の実装は、単一material face・no-hingeのunordered pair数0だけをopaqueなgeometry proofとして肯定する。exact model issuer、pose instance、紙厚bits、`centered_mid_surface_v1`、policy/model/proof IDへ結合するが、project、revision、current pose certificate identity、pose generationは証明しない。したがって単独ではcurrent collision certificateまたはmutation authorityではない。複数面は全pairの幾何証拠が揃うまでblocking errorとし、0件解析を安全完了へ読み替えない。

正厚三角柱SATの面内辺・法線・押し出し方向は、polygonの開始頂点とwindingから独立なcanonical rest三角形順、および検証済み剛体変換の線形成分から生成する。変換後world頂点同士の減算で方向を再構成して平行移動の相殺誤差を持ち込まない。射影上の厳密0も、対象軸について`2 × EPS`で評価した絶対world座標ULP上界が局所marginを支配する場合は真の境界接触を肯定できないため`indeterminate`とする。

三角柱SATまたは厚さ0の面交差分類が近平行軸・sub-margin距離によって`indeterminate`になる直前に、材料中央面のworld-space三角形3頂点を保存済みbinary64値の厳密な2進有理数として読み直す。各三角形が相手平面の正負両側に頂点を持ち、2本の平面交線区間が正長で重なることをBigInt演算で証明できた場合だけ、`binary64_transversal_triangle_intersection_v1`の横断交差として`penetrating`へ確定する。相手平面上の頂点は交線区間の端点として厳密に扱うが、正負straddleを省略しない。共有頂点については、両section区間の共通部分が指定共有頂点の射影ただ1点であることを別のexact certificateで証明できた場合だけ、`shared_feature_contact`または`shared_feature_thickness_overlap`へ進める。共有点または共有辺だけの一点・境界接触は横断交差にならず、許容marginも広げない。どの証拠も肯定できない場合は`indeterminate`を維持する。

BigIntによる厳密交差certificateは、横断証明と共有頂点のみの接触証明で一つの予算を共有し、同じ不変姿勢を対象とする1解析につき最大`MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS = 256`回とする。公開フィールド名は互換性のため維持する。この値は、最大1,000,000件のSAT pairがすべて任意精度演算へ進む最悪経路を約1/3906へ制限しつつ、A/B再現および回帰行列の少数候補を厳密化できる明示的な初版工学上限である。予算は解析入口で一度だけ生成し、全candidateとresumable stepで共有する。candidate切替やstep再開ではリセットせず、別の同期解析、one-shot解析、full-scan解析を開始したときだけ新しくする。cancel済み解析の会計を後続解析へ引き継がない。

許容差分類が厳密証明を要求した時点で1 attemptを消費する。上限後はBigInt helperを呼ばず、`skippedByLimit`を加算し、そのtriangle pairを必ず`indeterminate`とする。これは接触・安全・衝突なしを意味せず、UIでは貫通と同じblocking riskとして「交差の可能性・判定保留」を維持する。共有ヒンジcandidateでも予算超過由来の不確定を解析的な許容境界接触・corridor・平坦積層へ再分類しない。共有頂点のみの証明が枯渇したpairを、raw正厚重なりだけで`penetrating`へ付け替えない。同じface candidateの後続pairまたは後続candidateで通常SATがmarginに依存せず`penetrating`を確定できた場合は、予算枯渇後でもseverityを`penetrating`へ昇格してよい。`exactTransversalProofWork`はalgorithm、固定上限、実attempt数、上限により省略した数をresultと全job stepへdeep-frozen snapshotとして公開する。

厚さ0の共有ヒンジでは、通常角の境界辺接触を`boundary_contact`として許容する。`flat_surface_stack`へ昇格できるのは、左右法線がbinary64上で厳密に反平行で、かつ認証済み有限ヒンジ範囲内の三角形対に共面の正面積重なりが肯定証明された場合だけである。180度に数値的に近いだけの姿勢、点・線接触だけの姿勢、または共面正面積を確定できない姿勢は平坦積層として許容せず`indeterminate`へ退避する。非隣接面の平坦な正面積重なりは共有ヒンジ許容の対象外であり、引き続き`penetrating`である。複数共有ヒンジ、姿勢不一致、走査未完了、偽造・重複した三角形対、許容境界を安全に分類できない場合も`indeterminate`へ退避する。許容領域外の体積重なりを証明できた場合だけヒンジ外貫通、境界接触を証明できた場合だけヒンジ外接触とする。

利用者が指定した厳密な0、±90、±180度は、衝突診断と3D描画の双方でsin/cosを代数的な0、±1へ正規化した同一Matrix4を使う。`89.999999`度のような近傍値は丸めず、近平行またはsub-marginの不確実性を厳密角へ昇格しない。

2026-07-18の回帰基準は、400×400 mm紙、V頂点`(200, 0)`、左右折り線端点`(0, 200√3)`と`(400, 200√3)`の忠実fixture上で次の36組とする。「外側面」はV字の2折り線を直接共有しない2面を指す。「判定不能」は、近似仕様を越えて安全と断定せず衝突直前停止へ渡す正式な結果である。

| 外側2面を同角度で折る | 10度 | 90度 | 179度 | 180度 |
|---|---|---|---|---|
| 厚さ0 mm | 共有頂点許容接触 | 共有頂点許容接触 | 共有頂点許容接触 | 貫通 |
| 厚さ0.1 mm | 共有頂点許容接触 | 共有頂点許容接触 | 共有頂点許容接触 | 判定不能 |
| 厚さ3 mm | 共有頂点許容接触 | 共有頂点許容接触 | 共有頂点許容接触 | 判定不能 |

| 左だけを折り、右を0度に保つ | 10度 | 90度 | 179度 | 180度 |
|---|---|---|---|---|
| 厚さ0 mm | 共有頂点許容接触 | 判定不能 | 判定不能 | 判定不能 |
| 厚さ0.1 mm | 共有頂点許容接触 | 判定不能 | 判定不能 | 判定不能 |
| 厚さ3 mm | 共有頂点許容接触 | 判定不能 | 判定不能 | 判定不能 |

外側2面の完全な中央面交差が真正な共有頂点ただ1点であることに加え、両面の剛体変換local `+Y`から得る材料法線の内積が`1e-10`より大きいことを証明した場合だけ、初版の中央面基準仕様では専用の`allowed_shared_vertex_contact`として接触表示する。raw SAT分類は診断に保持し、exact singletonだけ、certificateが偽・不明、法線内積が0以下または境界内ならこの許容を与えず判定不能とする。certificateはprivate identityと発行時の三角柱pair・厚さ・raw分類へ結合し、clone、左右入替、別pairで再利用できない。さらに、同じ`VertexId`を持つ全ての面組を広域候補抽出に依存しない全ポーズ検証へ渡す。現在world座標がtopology marginを超えて離れた場合、またはmargin内なのに広域候補から欠落した場合は、非隣接面では通常の判定保留、共有ヒンジ面では`pose_mismatch`の判定保留を明示的に返す。正厚180度は層ずらし未再現、厚さ0・片側180度の外側2面は斜め軸変換後のsub-margin共面境界を安全に確定できないため判定不能とする。角起点の山折り・谷折りV字に対する厚さ0/0.1/1 mm×片側10度/両側45・91・135度の15姿勢は、衝突接触ポリシー9.1の必須回帰行列に従う。

| 長さ400 mmの共有ヒンジ | 10度 | 90度 | 179度 | 180度 |
|---|---|---|---|---|
| 厚さ0 mm | 許容境界接触 | 許容境界接触 | 許容境界接触 | 許容平坦積層 |
| 厚さ0.1 mm | 許容corridor | 許容corridor | 許容corridor | 層ずらし未再現・判定不能 |
| 厚さ3 mm | 許容corridor | 許容corridor | 許容corridor | 層ずらし未再現・判定不能 |

共有ヒンジの有限軸証明では、三角形対の少なくとも片方が両端点間へ完全に収まれば、その交差集合も両端点間に収まるものとする。両方とも端点外へ伸びる組はこの証明を得ず、端点外のconcave lobeを許容しない。厚さ0かつ180度未満では、検証済み共有軸、対向する材料半平面、有限軸の3証明がそろった数値的に限界の三角形対を境界接触として解決する。厚さ0の平坦積層は、厳密な反平行法線と共面正面積重なりの両方を肯定証明できる場合だけ同じ有限証明範囲内で許容する。正厚は0度の境界接触と180度未満かつ`R <= L`の有限corridorだけを許容し、`R > L`、厳密180度および数値的flat-fold特異域は`layer_offset_unmodeled`として判定不能にする。

利用者報告A/Bは上記の忠実fixtureを使い、各折り線長を400 mmに固定する。Aは厚さ0、左10度、右0度で、非隣接外側面を「貫通0・通常接触0・共有頂点のみと証明した許容接触1」、共有ヒンジ2件を許容境界接触、数値・方針不確定0とする。Bは厚さ0、左右180度で、非隣接外側面を「貫通1」とする。通常の非隣接`touching`は`penetrating`とは区別して表示するが、連続運動では紙同士の最初の接触として安全側に停止する。共有ヒンジモデルで明示許容した接触と、完全な交差が真正な共有頂点ただ1点であることをexact certificateで証明した`allowed_shared_vertex_contact`だけは停止対象から除く。

2026-07-19の追加回帰では、400×400 mm紙の一辺中点`M=(200, 0)`から遠い2角`(0, 400)`、`(400, 400)`へ山折りを引き、中央面を固定して外側2面を互いに反対符号で回転する。対象の非隣接外側面ペアは全組で広域候補に残し、次の分類を正本とする。横断開始は約`104.477512度 = acos(-1/4)`である。90度と91度はexact singletonでも材料法線のco-oriented条件を満たさないため、安全な共有頂点接触とは断定せず判定保留とする。

| 辺中点Mからの山山V字 | 90度 | 91度 | 135度 | 179度 |
|---|---|---|---|---|
| 厚さ0 mm | 判定不能 | 判定不能 | 貫通 | 貫通 |
| 厚さ0.1 mm | 判定不能 | 判定不能 | 貫通 | 貫通 |
| 厚さ3 mm | 判定不能 | 判定不能 | 貫通 | 貫通 |

`indeterminate`は安全を意味しない。UIでは「交差の可能性・判定保留」と明記し、貫通と同じblocking risk属性、赤系の枠・背景、太字で表示する。判定保留面の3D輪郭も維持し、無表示または安全色へ縮約しない。

静的分類単独では連続運動を保証しない。単一折りUIと、木構造の選択1ヒンジについて他ヒンジを固定する物理把持UIから連続経路を判定する。複数ヒンジvectorの同時連続運動は未検証として扱う。いずれも実際の折り癖、材料変形、厳密な層ずれを保証しない。2026-07-18のオーナー決定により、この`centered_mid_surface_v1`を初版の正式な厚さ仕様として確定し、厳密な層ずれ再現は将来課題とする。UIでも中央面基準モデルによる近似であることを明示する。

#### 8.2.2 三角柱SAT witness seed

現在の`triangle_prism_sat_witness_v1`は、authoritativeな狭域判定が確定した一つの正厚三角柱対から、後続の衝突説明に使う有限なwitness seedを導出する純粋境界である。現在姿勢の狭域結果と、木構造で選択1ヒンジを動かした連続経路の危険姿勢へface・triangle identity付きで接続し、後者は停止詳細UIへも提示する。ただし、全体面の修正や安全な新姿勢を証明する機能ではない。

- 入力頂点はauthoritative SATが使用した同一順序・同一座標の6頂点snapshotでなければならず、呼出側で再構築・並べ替えしない。`authoritativeGeometryClass`は`touching`または`penetrating`だけを受理し、ローカル再導出したclassと一致しなければ`null`とする。`separated`と`indeterminate`からwitnessを作らない。
- 狭域結果の`witnessSamples`は、最終interactionが通常の`touching`または`penetrating`へ確定した非隣接triangle-pairだけをface ID・両triangle index・classと結合する。共有頂点のみと証明した許容pair、hinge、`indeterminate`は対象外とする。厚さ0の通常確定pairもcoverage上はeligible/attemptedとして数えるが、正厚三角柱witnessを作れないため`unavailable`となりsampleは返さない。witness導出が`null`でもauthoritative分類を変更しない。
- 説明導出は衝突判定の100万pair上限と独立した最大16 attemptに制限する。全候補中の`penetrating`を`touching`より優先し、同一severityではauthoritative走査順を維持する。上限を満たした後もSAT自体の必要な走査は省略しない。
- coverageは`eligible = attempted + omittedByLimit`、`attempted = witnessSamples + unavailable`を満たす。早期貫通終了または非隣接SAT未実行時は`authoritativePairScanComplete: false`とし、0件を「全対象を説明済み」と誤表示しない。
- SATの正規化、外積、長さ、射影はauthoritative経路と同じ演算順を保ち、近平行軸はより保守的に`null`へ退避する。共通剛体変換後の丸めによって利用不能になる場合も、法線を推測しない。
- 出力は「第2三角柱を第1三角柱から離す」向きの単位法線、単一pairが接触へ達するまでの`escapeDistance`、margin内ですでに離隔している`toleratedGap`、各4点以下のsupport、最大16点のsupport midpoint hullを持つ。位置候補の`sourcePose`はhint適用前の解析入力姿勢である。
- `localSeparationHint`のscopeは選択三角柱pairだけであり、`autoApplicable: false`を固定する。複数pair、共有ヒンジ、他面、連続経路、紙の変形を考慮した全体修正ではないため、安全停止姿勢やプロジェクトへtranslationを自動適用しない。
- 入力は公開境界で一度だけsnapshotし、頂点数、右手系frame、正厚、対応cap、有限値、support上限を検証する。結果は全階層を不変化し、Proxy、getter例外、overflow、退化、過大marginをfail-closedで`null`へ退避する。
- 連続運動層はproject/revision、固定面、選択ヒンジ、紙厚、request context、完全な開始・目標・危険角度vector、`blockingSampleTime`、危険側2面の変換をface・triangle identityへ結び付ける。終端時刻とblockerが一致した場合だけ説明snapshotを保持し、不一致や説明生成失敗ではauthoritativeな停止を維持したまま説明だけを破棄する。
- UI境界はrequest identity、全角度vector、2面変換、全witness、coverage方程式、primary witnessを再検証する。有効な場合だけ危険解析角、三角形番号、位置候補数、法線、局所分離距離を解析情報として提示し、保存した危険側の面行列を3D表示の更新には使用していないこと、候補が三角柱1組だけを対象とし自動適用できないことを明示する。時刻0では現在の表示姿勢自体と危険解析角が一致し得るため、「危険姿勢は表示されていない」とは表現しない。内部ID・context keyは表示しない。

#### 8.2.3 非隣接pairの全走査witness集合

全体修正候補の入力には、通常解析が説明用に返す最大16件の`witnessSamples`をそのまま使用しない。通常解析は非隣接pairの最初の貫通で早期終了でき、最終face severityと異なる接触pairも説明対象外になるため、そのcoverageは全衝突pairの網羅を意味しない。

- prepared analyzerの`collectFullScanNonAdjacentWitnessSet`をオンデマンド診断として分離し、通常の`analyze()`、連続運動の各点判定、既存v1 witness順序・coverage・作業量を変更しない。
- 正厚の同一入力姿勢について、広域候補に残った全非隣接face pairの全triangle-pairを元の決定順で走査する。貫通を検出しても早期終了せず、同じface pair内の通常`touching`と`penetrating`をともに収録対象とする。共有頂点のみと証明した許容pairは別計数してwitness対象から除き、共有ヒンジpairはこの集合のscope外とする。
- 公開境界でface transform map、各16行列要素を一度だけ読み、切り離した倍精度行列snapshotを広域判定、剛体検証、三角柱構築、SATの全段へ渡す。状態依存Mapやgetterが段間で姿勢を差し替えてもcomplete認定しない。
- coverageは`expectedTrianglePairCount = trianglePairTests`、`trianglePairTests = aabbRejectedPairCount + satTests`、`satTests = satSeparated + allowedSharedVertex + touching + penetrating + indeterminate`、`eligible = attempted + omitted`、`attempted = available + unavailable`を必須とする。返却できた全走査結果だけが`authoritativePairScanComplete: true`を持つ。
- triangle-pair走査は最大1,000,000、witness導出は走査順の最大16件に固定する。未確定pair、17件目以降、witness導出不能のいずれかがあれば`kind: unavailable`とし、件数と理由だけを返して部分witnessを公開しない。完全な場合だけ`kind: complete`と全witnessをdeep freezeして返す。両variantとも`autoApplicable: false`である。
- `full_non_adjacent_prism_witness_scan_job_v1`はcandidate、first triangle、second triangleのcursorを保持し、AABB rejectを含むtriangle-pair visit 1件または選択済みwitness導出1件を1 work unitとして中断・再開する。exact pair数、最大witness導出数、合計work上限を不変`workBounds`で公開し、各`step`の増分、累積値、終端snapshotを照合してからだけ結果を確定する。cancel、再入、不正budget、走査例外は部分witnessを公開せず同一参照の終端へ退避する。
- 既存の同期full-scan APIは同じjobを最大1,000,016 unitsでdrainする互換wrapperとし、走査順・coverage・witnessを維持する。この初期jobのfactoryではtransform snapshot、broad phase、triangle-prism準備を同期実行するため、triangle-pair/witness部分だけが増分化済みであり、描画frame全体の時間上限はまだ主張しない。後続でbroad phaseとprism準備もcursor化する。
- `narrow_phase_sat_witness_cursor_job_v1`は通常の早期停止解析についてもcandidate、triangle pair、witnessのcursorを保持し、AABB rejectを含むpair visitまたは選択済みwitness導出を1 work unitとして中断・再開する。non-adjacentまたはpolicyなしhingeの最初のpenetrationではそのcandidateだけを早期終了し、後続candidateを継続する。最終face severityと一致するseedだけを集め、全penetrating、全touchingの順で最大16件を導出する。同期`analyze()`は同じjobを最大1,000,016 unitsでdrainし、one-shotとの出力・順序・coverageを維持する。
- 厳密横断証明はtriangle-pair visit内で必要時だけ完了まで同期実行するsubworkであり、追加cursor unitとして`totalWorkUnits`へ加算しない。その代わり、各stepの`exactTransversalProofWork`についてattempt/skippedが単調、attemptが256以下、同じstepで増えたrequest数が課金済みtriangle-pair増分以下であることを照合する。この上限はBigInt fallbackだけを制限し、既存の最大1,000,000 triangle-pair visitと最大16 witness導出を変更しない。
- 通常jobの`workBounds`はpotential pair数、実pair上限、witness上限、合計cursor上限に加え、`entireStepTimeBounded: false`、`synchronousFactoryPreparation: true`、`synchronousHingePolicyFinalization: true`、`synchronousResultFinalization: true`を固定する。potentialが100万を超えても早期停止により実訪問が上限内なら拒否せず、100万ちょうどは完了し、次のpairが必要な場合だけ未課金のまま`work_limit_exceeded`へ退避する。transform snapshot、broad phase、prism準備、hinge policy、完成resultの切り離し・deep freezeはまだ同期であり、真のframe時間上限にはこれらの追加cursor化またはworker分離が必要である。
- 通常/full-scan両jobはbudget検証前から再入guardを有効にする。budget検証、SAT、hinge policy、witness中の再入またはcancelは外側処理より優先し、cancel後に例外が発生しても`scan_error`で上書きしない。課金済みunitとwork boundsを照合してから同一参照の終端を公開する。
- `complete`が保証するのは、同じ解析姿勢で全非隣接衝突pairの局所制約を取り出せたことだけであり、出力は`requestIdentityBound: false`を固定する。次層でproject・revision・request・完全角度vector・危険時刻へ再結合するまではsolver入力にしない。固定側・可動subtreeのpartition、正のclearance、合法なヒンジ角への投影、新規衝突の全面再検証、そこまでの連続経路認定、層順・材料変形は別段階とし、raw translationや局所hintを3D姿勢またはプロジェクトへ適用しない。

#### 8.2.4 危険姿勢への終端full-scan結合

木構造の選択1ヒンジ経路では、通常の点判定中にfull-scanを実行しない。連続運動jobが`blocked`を確定し、危険時刻・blockerが保存seedと一致し、既存v1 blocking sampleの構築にも成功した後だけ、補助証拠としてterminal full-scanを一度試行する。

- 外側の`tree_single_hinge_blocking_sample_v1`を維持し、独立versionのnullableな`terminalFullScanBinding`を追加する。full-scanの例外、`unavailable`、identity不一致はこのfieldだけを`null`にし、block、停止時刻、v1 witness・coverage、motion stats、停止詳細UIを変更しない。反復`step()`はcache済みの同一terminal objectを返し、再走査しない。
- request identityがない純粋analyzer呼出ではfull-scanを開始しない。request付きでは開始角vectorから`sourcePoseRequestKey`を再確認し、危険角vectorから別の`blockingPoseRequestKey`を生成する。project、revision、固定面、選択ヒンジ、危険時刻、危険角、紙厚、start・target・sample全vectorを同じbindingへdeep freezeする。
- 完全構築・deep freezeしたbindingだけを公開前にprivate `WeakMap` provenanceへ登録し、analyzer準備時に受け取った元modelのexact参照を公開fieldにせず結合する。exact binding単独のguardに加え、exact modelとbindingの組だけをproperty非参照で受理するguardを設ける。structured clone、spread、prototype wrapper、同内容の別発行model・binding、hostile・revoked Proxy、primitive、binding以外のterminal要素は真正性を持たない。補正解析coordinatorは両guardを入力境界で通した後も、motion contextと全identity・pose keyを再照合する。
- `faceTransformsAt(危険角)`で全姿勢を再構成し、保存済みprimary blocker 2面の16要素と完全一致してからv2を呼ぶ。`kind: complete`、raw setの`requestIdentityBound: false`、紙厚、coverage全式、terminal blockerのface pair・class、v1 primary witnessのtriangle identityが一致した場合だけ、外側wrapperを`requestIdentityBound: true`とする。
- reroot済みのstationary・moving face集合は重複・交差・欠落を許さず全faceを一度だけ覆い、固定面と選択ヒンジのparentをstationary、childをmovingへ結合する。各witnessにはfirst/second面のbody所属と`cross_partition`、`stationary_internal`、`moving_internal`を記録する。
- same-body witnessが1件でもあれば説明bindingは保持するが、共通二体並進の入力には不適格とする。全witnessがcross-partitionの場合だけ`twoBodyTranslationInputEligible: true`にできる。ただし非隣接pair限定のため`wholeSceneConstraintsRepresented: false`、`hingeAdjacentPairsIncluded: false`、未生成・未再検証・未経路認定・`autoApplicable: false`を固定する。
- terminal同期処理には通常point budgetと独立した100,000 triangle-pair上限を置く。全face pair上限がこれを超える場合はv2を開始せずv1 blockを即時返す。将来はfull-scanと候補検証をincremental jobまたはworkerへ分離する。

#### 8.2.5 結合済み全制約からの二体並進候補

`two_body_translation_candidate_v1`は、危険姿勢へrequest結合済みで、全witnessがstationary/moving間をまたぐterminal full-scan bindingだけから、moving partition全体へ加える共通並進の解析候補を導出する純粋境界である。これは衝突局所制約を満たす三次元ベクトルであって、ヒンジだけで到達できる合法な折り姿勢ではない。

- 入力時にbinding version、project/revision、固定面、選択ヒンジ、危険時刻、紙厚、start・target・sample角度vectorを一度だけsnapshotする。startとsampleのpose keyを再計算してrequest keyと照合し、100,000 pair以下のcomplete coverage全式、class別件数、partition所属、連番witness index、安全フラグを再検証する。
- 同じ一回のbinding snapshotから、検証済み`rerooted_selected_hinge_partition_v1`のstationary/moving face ID列を`sourcePartition`として候補へ切り離して保持する。後続のモデル束縛層はraw bindingを再読せず、このdeep-frozen partitionとcontextから再導出した実subtreeを照合する。
- 各witnessは法線・escape・marginだけでなく、各4点以下の両support、最大16点のsupport midpoint generators、position source、局所hint translationまで再検証する。欠落、差し替え、非有限値、同一body、raw/unavailable集合、17制約以上は`null`へ退避する。
- 第2面がmovingならwitness法線、第1面がmovingなら反転法線を制約方向とする。必要射影は`escapeDistance + (numericalMargin - toleratedGap) + clearance`であり、非負減算と各加算を安全側へ上向き丸めする。clearanceは有限の正値を必須とする。
- 最大16制約についてactive setを1〜3本まで決定順に全列挙する。上限は`C(16,1)+C(16,2)+C(16,3)=696`で、Gram連立系と非負KKT multiplierから最小ノルムseedを求める。線形従属、負multiplier、矛盾、非有限値、作業上限超過は推測せずfail-closedとする。
- 通常の浮動小数点内積一致だけでは安全認定しない。各積と和を下向きに囲った射影下限が上向き丸め済み必要量を厳密に超えるまで、有限回だけ外向きscaleする。最大並進は上向き丸めしたL1ノルムをEuclideanノルム上限として判定し、候補には射影下限とノルム上限を保存する。
- `sourcePairConstraintsSatisfied: true`が意味するのは、このbindingに含まれる非隣接・cross-partition三角柱pairの線形制約を候補が満たしたことだけである。共有ヒンジ、同一body、全sceneの新規衝突、合法角度への射影、静的再判定、現在姿勢からの連続経路、層順・材料変形は未検証なので、対応する安全フラグと`autoApplicable`はすべて`false`に固定し、3D表示やプロジェクトへ適用しない。

#### 8.2.6 共通並進からの未検証1ヒンジ角度seed

`single_hinge_rotation_fit_seeds_v1`は、二体間の共通並進候補を、指定したworldヒンジ軸まわりの有限回転で近似できる角度候補へ変換する純粋な最小二乗境界である。この段階では軸・点・角度をproject、revision、選択ヒンジ、terminal bindingへ結合せず、候補を合法姿勢または安全姿勢とは認定しない。

- 入力はworld軸、child側の回転符号、危険角、許容する最大角度差、共通並進、moving側material pointを一度だけsnapshotする。軸方向は単位長許容差を確認してから同じ正規化値へ固定し、点IDを決定順へ並べる。重複ID、非有限値、ゼロ並進、空集合、100,000点超過、演算overflowは`null`へ退避する。
- 各点について、軸方向成分を除いたradial vectorと接線vectorを保存し、有限回転変位`radial × (cos φ - 1) + tangent × sin φ`と共通並進の残差二乗和を目的関数とする。微小角線形化ではなく、`cos φ`、`sin φ`、`cos 2φ`、`sin 2φ`を含む同じ有限回転目的を係数化する。単位軸では理論上ゼロになる二次調波については、幾何スケールの丸め境界と一次根の角度ずれ境界をともに満たす浮動小数点残差だけをゼロへcanonicalizeし、安定な一次調波解析根へ退化させる。
- 角度domainは折り角0〜180度と最大角度差の共通部分に限定する。両端点に加え、目的関数の導関数を二次調波三角多項式として正負の回転domainで全根列挙し、最大6候補だけを直接評価する。根の重複・数値曖昧性・作業上限では推測せず`null`へ退避する。
- 危険角でのbaselineより丸め許容差を超えて残差を減らす候補だけを、残差二乗、点数で正規化したRMS、改善量、改善率とともに返す。残差、回転量、角度の固定順で順位を付け、結果全体をdeep freezeする。
- 出力は`unverified_single_hinge_rotation_fit_seeds`であり、`modelIdentityBound`、`collisionConstraintsRevalidated`、`legalCorrectionPoseGenerated`、`staticCandidateRevalidated`、`continuousCandidatePathCertified`、`autoApplicable`をすべて`false`に固定する。次層でterminal bindingとworldヒンジ軸・moving頂点を再導出し、完全角度vectorを生成して全scene静的判定を通すまで、3D表示、motion owner、プロジェクトへ適用しない。

#### 8.2.7 モデル束縛済み1ヒンジ静的補正候補

`tree_single_hinge_static_correction_candidates_v1`は、terminal full-scanから得た二体並進候補と未検証角度seedを、同じ真正な木構造motion contextへ再結合する解析専用境界である。返却候補は指定角における全sceneの静的衝突再判定まで完了しているが、現在姿勢または危険姿勢から候補姿勢までの連続経路はまだ認定しない。

- raw terminal bindingは、private provenanceで真正なmotion contextとそのexact modelへ発行された同一objectであることをproperty非参照で確認してから、二体並進候補の導出時に一度だけ読む。その後は切り離された候補を使う。contextのversion、project、revision、固定面、選択ヒンジ、context key、紙厚、source完全角度vectorを再確認し、sourceとblockingのpose request keyを同じmodelから再計算する。clone、別model向けbinding、差し替え、失効getter、例外、非有限値、不一致は`null`へ退避する。
- contextから選択ヒンジの実moving subtreeとstationary complementを再導出し、保存済み`sourcePartition`と決定順を含めて一致させる。全faceを重複なく覆い、固定面・parentはstationary、childはmoving、選択ヒンジ以外のjointがpartitionを横断しないことを必須とする。
- blocking完全角度vectorから姿勢を再構成し、parent face transformと保存されたhinge transformの一致を確認してworldヒンジ軸を得る。moving subtreeのmaterial vertexをblocking姿勢へ変換し、同一vertex IDのworld位置が数値許容差内で一致する場合だけ、共通並進に対する有限回転seedを生成する。
- 各seedは選択ヒンジだけを置換した完全角度vectorへ戻し、新しいpose request keyと全face transformを生成する。まず全非隣接face pairのfull scanが`complete`で、未確定・接触・貫通・witnessが0、coverage全式が成立することを確認する。続いて通常のnarrow-phaseを実行し、残るinteractionがすべて共有ヒンジpairかつ`allowed_by_hinge_model`である場合だけ静的候補として保持する。
- 候補数は有限回転fitと同じ最大6件とする。全候補についてfull scanと通常解析を合わせた保守的triangle-pair上限を開始前に算出し、累積1,000,000を超えるモデルまたは実測値はfail-closedとする。計画上限、実測visit数、各scan回数をdeep-frozen結果へ保存する。
- `tree_single_hinge_static_correction_candidates_job_v1`は各seedを`full_scan_preparation`、`full_scan`、`narrow_scan_preparation`、`narrow_scan`へ分け、子job生成または子job終端の後は同じ公開`step`内で次段階を開始しない。full scanがclearでないseedでは通常解析を起動せず、全seedを元順位のまま完走してからだけ候補集合を公開する。途中候補はpending、cancelled、indeterminate、exhaustedのどのvariantにも含めない。
- 親jobはfull/通常解析のtriangle-pair累計とwitness導出累計を分離し、各子jobの不変work bounds、累積値、step差分、委譲budgetを照合する。再入またはcancelが子処理中に起きた場合は、子のcancelled終端を再観測して課金済みworkを親へ集計してから同一参照のcancelled終端を公開する。結果deep-freeze中の再入でもcompleteで上書きせず、真正context provenanceはcompleteの公開成功後にだけ、モジュール初期化時に固定したprivate `WeakMap`操作で付与する。
- `workBounds`は計画pair上限、witness上限、合計cursor上限に加え、`entireStepTimeBounded: false`と、二体solver・有限回転fit・全seed姿勢、子job factory、hinge policy、結果finalizationが同期であることを固定flagで明示する。従来の同期derive APIは同じjobを十分なbudgetでdrainする互換wrapperであり、結果・順位・作業集計を維持するが、描画frame全体の時間上限は主張しない。
- 出力はmodel identity、source/blocking pose、partition、完全合法角度vector、静的全scene、共有ヒンジ規則の再検証済みである。一方、候補までの連続経路、層順、材料変形、scene反映、undo可能なproject commandは未検証なので、`continuousCandidatePathCertified`、`sceneApplied`、`autoApplicable`をすべて`false`に固定する。解析結果をmotion owner、3D表示、プロジェクトへ直接適用しない。

#### 8.2.8 静的補正候補への連続経路認定

`tree_single_hinge_static_candidate_path_v1`は、同じ真正motion contextから生成された静的補正候補を残差順位で試し、source完全角度vectorから候補までの選択1ヒンジ線形経路を既存の連続区間証明で再検証する解析専用jobである。既知の衝突姿勢であるblocking角を開始点には使わない。

- 静的候補の成功結果は生成時にprivate provenanceへ登録し、同じcontext参照と組み合わせた未変更の結果だけを受理する。deep-frozen構造のclone、同値に見える別context、stale revision、hostile Proxyは安全flagの値にかかわらず拒否する。
- source pose keyをcontextの完全角度vectorから再生成し、project、revision、固定面、選択ヒンジ、context key、紙厚を照合する。prepared continuous analyzerのstationary/moving集合を保存済みpartitionと順序込みで一致させ、各候補が選択ヒンジだけを変えた完全角度vectorであることとtarget pose keyを再確認する。
- 最大6候補を残差順位で一件ずつ進め、現在候補のcontinuous childだけを必要時に生成する。`candidate_preparation`は専用の同期stepとしてchildを生成するだけでinterval workを進めず、続く`candidate_analysis`だけが`workBudget`を委譲する。衝突または未確定になった後も同じcall内で次候補を生成せず、明示的なphase境界を返す。最初に経路全体が`clear`となった候補で`certified`終了し、全候補が非認定なら`exhausted`とする。
- inner jobへ旧terminal request identityを渡さず、候補探索中のblocking説明用terminal full-scanを発生させない。候補単位のinterval pair・point triangle上限を候補数倍した保守的job上限と、interval test、point test、cache hit、最大深度の集計だけを公開し、未計測の実triangle visit数を実績として主張しない。
- 不正work budget、inner例外、結果形式または作業集計の後退は`indeterminate`、cancelと再入は現在のinner jobを停止して`cancelled`へ退避する。budget検証、課金済みchild callback、pending・terminal deep-freeze中の再入でも、観測済みworkを集計した同一のcancelled終端を優先する。certificateの真正provenanceは認定終端の公開に成功した後だけ、モジュール初期化時に固定したprivate `WeakMap`操作で付与し、実行時のprototype差し替えを登録窓へ入れない。終端後の`step`は同じ不変objectを返し、途中の認定候補を部分的な成功として公開しない。
- `workBounds`は候補数倍したinterval・interval pair・point triangle上限に加え、factory準備、child factory、成功結果finalizationが同期処理であり、公開step全体のwall-clock上限を主張しないことをliteral flagで明示する。表示DTOにも同じflagを切り離して保持する。
- `certified`だけがsource/target完全角度vector、両pose key、静的候補順位、連続解析statsと`continuousCandidatePathCertified: true`を持つ。これはその二姿勢間の単一線形角度経路だけの解析証明であり、現在の3D sceneがsource姿勢にあること、層順、材料変形、project適用は別境界とする。全variantで`sceneApplied: false`、`autoApplicable: false`を固定する。

#### 8.2.9 連続経路certificateの真正性と読み取り専用表示

連続経路を認定したcertificateはdeep-frozen構造だけを適用権限として扱わない。clear結果の完全性を確認してcertificateを不変化した後、生成元の真正motion context参照、source/target pose key、完全角度vector、候補順位の切り離したmetadataをprivate provenanceへ登録する。

- exact certificateとexact context参照の組だけがprovenance guardを通る。outer terminalのcloneが同じcertificate参照を保持する場合は有効だが、certificate自体のclone、JSON往復、同値の別context、hostile・revoked Proxy、primitive、clear以外の結果は権限を持たない。guardは入力propertyを読まずprivate mapだけを照合する。
- `tree_single_hinge_static_candidate_path_presentation_v1`はguardを通ったcertificateだけから表示DTOを生成する。project/revisionと選択ヒンジ、候補順位、source/target角、方向、連続解析statsと先行試行数、静的interaction集計、保守的作業上限だけをcopyし、seed種別、face ID、完全角度vector、pose key、application token、scene command、runtime stateを出力しない。
- badge単独でも「解析上」「静的・連続経路確認済み」「現在姿勢未照合」を示す。制限文は解析結果が現在も有効とは限らず、現在姿勢からの安全移動、層順、材料変形を証明せず、この表示から3D sceneまたは設計dataへ適用できないことを示す。DTOはdeep freezeし、`analysisOnly: true`、`runtimeRequestBound: false`、`activeRequestLeaseBound: false`、`startScenePoseMatched: false`、`sceneApplied: false`、`autoApplicable: false`を維持する。
- DTO単独を保存・再表示しない。UI coordinatorはowner/requestの非公開leaseとしてexact context、fixed face、collision thickness、generation/request sequence、source poseを保持し、各RAFの前後とpublish/render直前に現在値を照合する。新request、直接角度変更、model/revision・fixed face・selected hinge・thickness変更、disposeでは、古い表示を先にclearしてからjobをcancelする。lease不一致の結果はpresentation factoryへ渡さず公開しない。
- presentation moduleはThree.js、motion owner、runtime、scene pose適用、project commandをimportしない。将来の適用は、真正runtimeの現在完全角度vectorとsource pose keyを再確認し、新しいowner request・application tokenを発行して既存の原子的scene commitへ渡す別境界とする。

#### 8.2.10 補正解析requestの準備境界

`tree_single_hinge_correction_analysis_request_v1`は、blocked runner終端から補正解析coordinatorへ渡す内部専用requestである。raw bindingを最初にexact-object真正性guardへ通し、runner状態、危険時刻、start・target・sample完全角度vector、project/revision、固定面、選択ヒンジ、context key、source/blocking pose key、generation/request sequence、紙厚、二体並進適格性を一致確認する。

- 元のmotion contextの`appliedAngles`や現在sceneの安全停止角を解析開始姿勢に流用しない。terminal bindingの`angleVectors.start`から選択角だけをrebaseした新しい真正motion context shellを発行し、source pose keyを再計算して元のexact context keyを保持・再照合する。元contextのexact model・tree・非選択角は保持するため、2回目以降のrequestでもterminal bindingに結合されたmodel provenanceを失わない。解析requestは元terminal identityを検証済みでも、現在scene開始姿勢との一致を主張しない。
- clearance、最大並進量、最大角度差、連続経路の深度・interval・時間幅・triangle作業上限をversion付きpolicyとしてown data fieldから一度だけsnapshotし、後続変更から切り離す。untrustedなrunner/evidence/policy/角度配列はgetterを使わず各fieldを一度だけ読み、固定長のbracketと完全角度vectorは期待長を照合してからindexを読む。clone、wrapper、stale値、異常長配列、hostile・revoked Proxyは`null`へ退避する。
- bindingのprivate provenanceに結合された元modelとrebase前後のcontextが保持するmodelがexact参照で一致する場合だけ受理する。正規UIはmotion contextを先に準備し、その`context.model`からcontinuous analyzerを準備する。同じcontext key・project・revision・角度を持つ別発行contextでも、model参照が異なれば補正権限を継承しない。
- 返却値のown propertyはdetached scalar summary、policy、安全flagだけとし、fresh contextとexact terminal bindingはmodule-private `WeakMap` authorityに保持する。返却tokenのcloneまたはserialize結果は真正性とauthorityを回復できず、React state、保存data、表示DTOへexact context・bindingを流出させない。`activeRequestLeaseBound: false`、`startScenePoseMatched: false`、`sceneApplied: false`、`autoApplicable: false`を固定する。

#### 8.2.11 補正解析job・UI coordinator

`tree_single_hinge_correction_analysis_job_v1`は真正な補正解析requestのprivate authorityだけを受け取り、静的候補準備、静的候補解析、候補経路準備、候補経路解析の4段階を順に進める解析専用jobである。UI coordinatorはこのjobを`requestAnimationFrame`ごとに1 work unitだけ進め、Reactへは切り離した状態と表示DTOだけを公開する。

- coordinatorは開始時に`working`を同期公開し、job factoryと各`step(1)`をRAFへ遅延する。generationと予約frameを照合し、同期callback、取消中の再入、旧世代の遅延callback、dispose後の完了を公開しない。
- exact leaseはterminal runtime、motion owner、真正context、表示中の安全停止pose keyと完全角度vector、project/revision、固定面、選択ヒンジ、紙厚、request sequenceを非公開で保持する。job生成、step、終端変換、publishの前後で同じleaseを再検証し、新しい角度要求、直接角度変更、model/revision・固定面・選択ヒンジ・紙厚・表示姿勢の変更、disposeでは旧結果を`stale`へ無効化してjobを取消す。
- UI状態は`idle`、`working`、`stale`、`no_candidate`、`indeterminate`、`certified`を区別する。`no_candidate`は指定したclearance、最大並進量、最大角度差、作業上限と候補生成方式の範囲内で認定候補がなかったことだけを表し、作品が折り不可能であることを意味しない。経路解析に未確定な試行が残る場合は`no_candidate`へ縮約せず`indeterminate`とする。
- `certified`でも示せるのは同じsource完全角度vectorから候補までの選択1ヒンジ線形経路を、現在の静的・連続解析が認定したことだけである。現在sceneとの一致、層順、材料変形、複数ヒンジ同時運動、閉路、切断由来の一般経路は対象外とする。
- 全状態で解析専用を維持し、認定表示DTOも`analysisOnly: true`、`sceneApplied: false`、`autoApplicable: false`を固定する。候補の3Dプレビュー、sceneまたは設計dataへの自動適用、明示適用commandはこの境界に実装しない。

### 8.3 衝突前停止

最終目標では、現在角から目標角までの連続運動を判定し、連続安全を証明できた下限と、その直後の危険または未確定な探索区間を求める。確認済み境界で停止し、対象面、接触点、法線、折り角、分類をUIへ返す。

現在の初期実装は`centered_mid_surface_v1`による単一折りと、他ヒンジを固定して選択1ヒンジだけを動かす木構造の物理把持UIを対象とし、現在の実表示角から指定角までを次の線形経路で扱う。

```text
angle(t) = appliedAngle + t × (requestedAngle - appliedAngle), 0 ≤ t ≤ 1
```

- 連続衝突ジョブは`requestAnimationFrame`ごとに`step(1)`だけを実行し、UIスレッドを一括探索で占有しない。
- `vertical_parameter_v1`と`physical_grab_v2`の開始角には、指定値ではなくrunnerの実表示角を使う。ドラッグ移動中は未確認目標だけを表示し、pointerup時に最終ポインター位置から目標を再計算して1回だけrunnerへ要求する。3D描画はrunnerが認定した角度だけを受け取る。
- `vertical_parameter_v1`は開始前に画面投影したヒンジの可視深度、最小表示長、ポインター距離を再検証し、横優勢gestureを角度操作へ誤接続しない。両方式ともpointer captureを取得してからOrbitControlsを無効化し、複数pointer、capture喪失、blur、resize、範囲外、revision・固定面・紙厚の変更では要求を送らず取消す。
- `physical_grab_v2`の開始点は、移動面の表または裏のcapだけを受理する。Raycasterから得たworld座標とobject-local座標を切り離した不変snapshotにし、表示厚の表裏位置、固定面から求めた移動面、履歴非依存の正規姿勢で再構成したworld座標が一致することを確認する。側面、固定面、古い姿勢、特異な座標尺度は開始させない。
- 把持点をヒンジ軸へ射影した中心・半径と正方向接線から円軌道を定義し、ポインター半直線との距離の停留点を解析的に列挙する。端点0度・180度も候補へ含め、投影の曖昧性、感度不足、ray範囲外、軌道から遠い点、1 sampleで45度を超えるbranch jump、固定作業量超過は推測せず拒否する。
- 紙面ドラッグは把持点の画面上軌道半径8 CSS px以上、1度あたり移動量0.2 CSS px以上を開始条件とし、mouse/penで6 CSS px、touchで10 CSS pxを超えた後だけ成立する。1 DOM eventにつきcurrentを含む最大32件のcoalesced moveを順に処理し、上限超過または拒否されたsampleでは古い表示目標を消す。runner stateの同一snapshot、camera行列・投影行列・注視点・viewport、指定角、project/revision/固定面/紙厚contextを開始時から照合し、どれかが変われば要求を送らず取消す。
- 木構造の1ヒンジ把持準備では、固定面へ再root化した完全角度vectorから「選択角だけ0度」の姿勢を作り、下流ヒンジ角を保った従属面上の点と、親面に従うworldヒンジ軸を復元する。現在姿勢との差が同じ軸回りの単一円軌道になることをcore solverでも再検証し、非可換な親子回転、固定面変更、山谷符号、表示hingeと運動hingeの不一致を安全側に遮断する。
- 重なった従属面群のsurface pickingは、画面上で全体最前面にあるhitと数値的に同一深度の優先面だけを候補にする。優先集合内は距離順、距離が等しい場合は指定順で決定し、固定側の手前面を透過して背後の従属面を掴まない。
- pointer reducerの副作用は純粋coordinatorが順序付きcommandへ変換し、表示予約の破棄、capture解放、context・runner・view guard消去、camera復帰、UI同期を終えた後にだけ最終角度を1回要求する。複数terminal effectや抑止pointerが残る不正・未完了状態では要求を送らない。
- ジョブが返す正の`certifiedSafeThrough`で新たに確認された境界だけを正方向または逆方向の角度へ補間し、未確認の指定角を3Dへ直接適用しない。
- `blocked`または`indeterminate`では最後に確認できた正の時刻境界で停止する。時刻0の場合は開始姿勢自体が安全確認済みとは限らないため、表示角と安全確認の有無を分けて示す。
- `unsafeBracket`は区間全体が衝突している意味ではなく、その探索範囲内で衝突姿勢を検出したことを表す。`unresolvedBracket`は衝突を意味せず、安全を証明できなかった探索範囲を表す。
- `blocked`結果は、実際に点判定が衝突を返した経路時刻を`blockingSampleTime`として必須保持し、同じ値から`unsafeBracket[1]`を生成する。core、runner、表示、詳細の各境界は、欠落、非有限、0〜1の範囲外、区間上端との不一致を受理しない。
- bracketは逆方向の折りでも経路順を維持する。`[0,0]`は開始姿勢そのものを確認できない状態、`[0,u]`は開始点の点判定だけ通過して正の長さの経路を確認できない状態として区別する。
- 新しい角度指定、revision、固定面、紙厚の変更では旧ジョブと予約フレームを破棄し、世代番号が一致しない遅延callbackを無効化する。
- 現在姿勢の狭域判定と連続経路判定は別の状態・バッジ・読み上げとして提示し、どちらの結果かを混同しない。
- 停止・判定不能時は開始角、指定角、実表示角、探索角範囲、対象面番号、相互作用分類、確認済み進捗を`details`へ表示する。内部面IDと未知の内部reasonは生表示せず、読み上げのlive通知は短い終端結果だけにする。
- `clear`表示にも中央面基準・単一ヒンジ・線形経路だけの確認であることを可視表示し、材料変形、実際の折り癖、層ずれ、複数ヒンジは未保証とする。`physical_grab_v2`は物理的な把持軌道から目標角を作る入力方式であり、材料変形や手指との力学を保証するシミュレーションとは表現しない。

VAL-004でいう「衝突直前」の停止位置は、時刻順探索で最初の危険姿勢を検出した区間について、最後に連続安全を確認できた下限とする。危険側の時刻を安全姿勢として推定適用しない。木構造の選択1ヒンジ経路では、実際に危険と判定した別姿勢へrequest identityと三角柱pair単位のwitnessを結合し、局所法線・分離距離までUI提示する。複数pairを同時に解消する全体修正、安全な修正後姿勢の再証明、複数ヒンジvectorの同時連続運動は次段階とする。

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

全体平坦折り判定は、設定された時間制限内で「可」「不可」「不明」を返し、可否を証明できる対象クラスと根拠を結果に含める。対象外、時間切れ、処理上限到達または証明不足を「不可」と断定しない。初版の対象クラス、version付きprovenance、facewise制約、場所別層順序、native background job、時間・資源上限の正本は[全体平坦折り判定と層順序管理の設計](global-flat-foldability-design.md)に置く。3D操作の運動検証は一般経路を完全探索せず、要求された操作経路で最初に検出した衝突の直前へ停止し、対象面・位置・理由を表示する。

### 9.4 局所平坦折り必要条件

初期モデルは`interior_single_vertex_zero_thickness_v1`とし、紙内部で近傍が一枚の材料diskとなる頂点に川崎条件と前川条件を適用する。MountainとValleyだけをfold rayとして数え、Auxiliaryは未割当折り線へ流用しない。紙境界、Boundary接続、Cut接続、折り線なしは理由付き`not_applicable`とし、参加graphのID、紙、切断policy、交差、包含、rotationを確定できない場合はreport全体を`blocked`とする。

- 川崎条件は保存済みbinary64座標を共通`2^-1074`単位の整数へ正確に変換してからray差分を作る。反時計回りrayの偶数番積`A`と奇数番積`B`をbalanced complex productで計算し、`A * conjugate(B)`の虚部がexact zeroかつ実部が負の場合だけ成立とする。角度・epsilon比較は使用しない
- 前川条件はMountain数`M`、Valley数`V`に対して`|M-V|=2`を整数で判定する
- 川崎条件の厳密計算上限は一頂点256折り線とする。超過時は`fold_degree_limit`による`indeterminate`とするが、前川条件の違反を認定できる場合は`violated`を優先する
- reportはproject内の全頂点をcanonical `VertexId`順で返す。頂点と条件の優先順位は`violated > indeterminate > satisfied > not_applicable`、reportは`blocked / not_applicable / necessary_conditions_satisfied / violated / indeterminate`を区別する
- 既存の幾何検証`is_valid/issues`へ局所条件違反を混ぜず、同じproject/revision応答の独立fieldとして返す。編集、検証失敗、benchmark表示では旧結果を消去または非表示にする
- UIは不成立・判定不能頂点を色だけでなく実線・破線でも区別し、一覧から頂点選択できるようにする。成立は今回の局所必要条件だけを意味し、指定山谷の局所十分性、全体平坦折り、層順、厚さ、折り経路を保証しない

厳密数値手順、DTO、上限、表示契約の詳細は`docs/local-flat-foldability-design.md`を正本とする。

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
- container v1のwriterはentry順を`manifest.json`、`project.json`に固定し、Deflate level 6、DOS timestamp `1980-01-01 00:00:00`で書く。同一`ProjectDocument`の反復保存と、読込後に変更せず再保存した結果はbyte-for-byteで一致する。頂点・辺・手順等の`Vec`順はprojectの順序付きデータとして保持し、別の保存順を同一視するcanonical化は行わない。
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

### 12.1 FOLD取込

現在のFOLD adapterは、FOLD 1/1.1/1.2のtop-level 2D `creasePattern`をORIGAMI2の一枚紙projectへ変換する。対応subset、線種割当、警告、上限の正本は[FOLD取込契約](fold-import-contract.md)とする。

```text
native file dialog
  → 16 MiB上限付きread
  → strict/bounded parse
  → geometry・単一境界検証
  → 1世代だけmemory stage
  → preview / scale / assignment / warning確認
  → 同一bytes再parse・再変換
  → project instance・ID・revision照合
  → 新規未保存projectへatomic replace
```

- `vertices_coords`、`edges_vertices`、`edges_assignment`を必須とし、2D座標、parallel array、添字、有限性、退化・重複・交差、単一の単純`B`境界をRust adapterで検証する。3D `foldedForm`、非ゼロZ、穴・複数紙・複数frameは拒否する。
- `mm`、`cm`、`m`、`in`、`pt`、`um`、`nm`はmm換算値を提示し、単位なし、`unit`、独自単位は利用者へmm/単位の入力を要求する。提示値を含め、適用する有限・正の倍率は確認画面で変更できる。
- `B`はboundary、`M`はmountain、`V`はvalleyへ固定する。`C`はcut/ignore、`F`と`J`はauxiliary/ignore、`U`はmountain/valley/auxiliary/ignoreから選択し、曖昧なassignmentを自動決定しない。
- previewは最大5,000辺に制限するが、用紙境界を先に含め、他assignmentを決定論的に抽出する。表示の省略は変換対象を省略しない。
- 面、折り角度、frame metadataなど永続化しない情報は具体名を警告し、明示確認を適用条件とする。未知JSONの内容、実ファイル名、path、raw JSONはWebViewへ渡さない。
- stageはopaque ID、開始時のproject instance・ID・revision、検証対象bytesだけを保持する。適用直前の再照合に失敗したstale結果、変換失敗、取消ではprojectを変更しない。
- 成功時は履歴、手順、保存先、保存済みbaselineを引き継がず、revision 0・dirtyの新規projectへ一度だけ置換する。旧projectへ戻るUndoは作らない。
- 上限はファイル16 MiB、頂点1万、辺1万、境界1,414辺、交差候補100万件、変換後の内部包含判定100万件とする。未知JSONは展開・保持せず読み捨て、縮尺適用後にも作業量と幾何を再検証する。

### 12.2 SVG取込

静的SVG 1.1/2の直線subsetを、色だけで線種を推測せず、利用者がsource groupと用紙外周を確認して一枚紙projectへ変換する。許可する要素・path command・style、縮尺、交差分割、情報損失、資源上限の正本は[SVG取込契約](svg-import-contract.md)とする。

```text
native file dialog
  → 16 MiB上限付きread
  → network非依存のstrict/bounded XML parse
  → 直線geometry・style group・閉路候補をpreview
  → 全線種 / 外周 / 一様縮尺 / Cut許可 / 警告確認
  → stageした同一bytesを再parse・平面graph化
  → project instance・ID・revision照合
  → 新規未保存projectへatomic replace
```

- `line`、`polyline`、`polygon`、非角丸`rect`と直線commandだけの`path`を扱い、nested affine transformを反映する。曲線、text、画像、`use`、script、animation、外部resourceは実行・取得・flattenしない。
- presentation attribute、inline style、継承、単純な`.class` CSSをsource groupへ集約する。`data-origami-kind`だけは初期候補にできるが、全groupを`Boundary / Mountain / Valley / Auxiliary / Cut / Ignore`へ明示的に割り当てる。
- 明示的な閉路候補、viewBox矩形の生成、またはBoundary割当のいずれか一つで外周を選ぶ。最大面積や線色から外周を自動確定しない。
- 倍率、全線種割当、外周選択をRust側で非破壊検証し、最終用紙幅・高さと変換後のCut有無を表示する。opaqueな検証IDをstage token、project identity/revision、倍率のbit列、canonical mapping、外周へ束縛し、設定変更や別世代では適用を拒否する。
- X/T接点と外周接点をexactな交点で分割する。許容差snap、隙間補修、共線重複の推測統合は行わない。
- DTD/entity宣言を拒否し、raw XML、path、実ファイル名をWebViewへ渡さない。opaque stage IDと開始時project identityを再照合し、取消・失敗・stale操作では既存projectを変更しない。

### 12.3 FOLD/SVG/PDF/DXF展開図書き出し

現在の一枚紙projectを、FOLD 1.2、静的SVG直線図、PDF 1.7の一枚展開図、またはDXF AC1021として書き出す。形式別field、線種対応、情報損失、資源上限、stage、原子的保存、受入試験の正本は[展開図書き出し契約](crease-pattern-export-contract.md)とする。

```text
project instance・ID・revision・形式を固定
  → bounded validation / deterministic serialization
  → native memoryへ最新1世代だけstage
  → opaque ID・件数・サイズ・警告をpreview
  → 利用者が情報損失を明示確認
  → native保存dialog
  → identity/revision/tokenを再照合
  → 同一directoryの一時fileへwrite・sync・同一handle再読込
  → atomic replace
```

- WebViewへraw FOLD/SVG/PDF/DXF bytes、保存path、project path、file handleを渡さず、保存commandもopaque ID、期待project ID・revision、警告確認flagだけを受け取る。
- FOLDはmmの2D `creasePattern`として`B/M/V/F/C`を出力する。SVGは1 unitを1 mmとして、各直線へcanonicalな`data-origami-kind`を付ける。
- PDFは`application/pdf`のPDF 1.7として、一ページのベクター展開図だけを実寸1:1・四辺10 mm余白で出力する。`PrintScaling=None`、黒一色の固定線種、pageの幅・高さを各14,400 point以下、real number tokenを64文字以下とする上限を適用し、未参照頂点は拒否する。これは折り工程や説明を載せる折り図PDFではない。
- DXFは`image/vnd.dxf`の`AC1021`テキスト形式として、UTF-8・CRLF・mm単位と固定headerで出力する。5線種を固定した`ORIGAMI_*` layer、ACI色、line typeへ分け、辺・頂点配列順、endpointの向き、UUIDに依存しないcanonical順へ並べる。全group pairを100,000、real number valueを64文字、出力全体を16 MiBに制限する。作品名の改行その他の制御文字は拒否し、group code `999`の安全なcommentだけに決定論的に分割する。
- 紙の見た目、ID、履歴、3D表示、camera、折り手順、線がない場合の切断許可など形式が保持しない情報を保存dialogより前に表示する。
- 生成中の旧世代完了、旧token、別project、別instance、stale revision、未確認警告を拒否する。dialog取消では同一stageを再試行でき、成功時だけ一度消費する。
- 書き出しは`.ori2`保存とは独立し、projectのdirty、保存先、revision、Undo/Redoを変えない。
- この4形式で要件IO-006に列挙された展開図書き出しを満たす。折り工程、矢印、説明文、完成図を含む複数ページの折り図PDFは要件INS-010の別機能として扱い、この一枚展開図PDFには含めない。OBJ・STL・glTFの完成形3D出力は要件IO-007で別に扱う。

### 12.4 手順画像・複数ページ折り図書き出し

折り手順タイムラインから、A4縦の複数ページPDF 1.7または手順ページごとのSVGを収めたZIPを書き出す。投影profile `orthographic_isometric_v1`、各stepを新しいpageから開始する規則、長文continuation、PDFとSVGで共有するcanonical plan、同梱Noto Sans JP/OFL、全体stale拒否、資源上限、opaque stage、cancel、原子的保存、未保証事項、受入試験の正本は[折り手順書き出し契約](instruction-export-contract.md)とする。

- 現在の3D cameraやGPU描画結果に依存せず、CPU側の固定orthographic isometric投影を全stepへ適用する。
- PDFとSVG ZIPは同じcanonical planを消費し、page break、文字位置、図形bounds、変更ヒンジを一致させる。
- 一件でもstaleなstepがあれば全体を拒否し、skip、truncate、部分保存を行わない。
- 最終出力128 MiB、総page 2,048、一page 4 MiB、総glyph 500,000、総投影点visit 1,000,000を初版上限とする。
- exact bytesはnative memoryへ最新一世代だけstageし、WebViewにはopaque IDとbounded metadataだけを返す。保存dialog取消では再試行でき、成功時だけ一度消費する。
- アプリ内animationが未実装のため、現時点のINS-010は部分実装として扱う。作家指定camera・矢印・注目箇所、手や指のguideはINS-004/INS-005の別作業である。

## 13. セキュリティ設計

- Tauri capabilityをファイル選択、設定、更新確認等へ限定する。
- UIから任意パスや任意コマンドをRustへ渡せる汎用APIを作らない。
- ZIP bomb、巨大画像、再帰参照、不正JSON、NaN/Infinity、ID衝突を検査する。
- SVG内スクリプトや外部リソースを実行・自動取得しない。
- 診断ログは作品座標・パス・内容を標準で記録しない。
- 更新確認はGitHubの公開情報だけを取得し、無効化可能にする。

### 13.1 TypeScript内部の信頼境界

- ファイル、Tauri IPC、将来のimport・pluginなど外部由来の値は入口で一度検証し、検証済みDTO以降を通常のTypeScript内部値として扱う。内部関数すべてで汎用的なhostile Proxy・getter対策を重ねることは既定方針にしない。
- 有限性、上限、退化、作業量など幾何計算そのものの安全条件は各数値境界に残す。非同期処理のgeneration、request sequence、revision、現在scene poseのstale検査も状態整合性のため維持する。
- private `WeakMap`/`WeakSet`によるexact-object authorityは、scene変更、motion owner、committed terminal lease、解析certificateなど「読み取り結果を副作用へ接続できる境界」に限定する。単なる内部DTOの全段へ真正性flagと構造再検証を増殖させない。
- 既存の重複した`deepFreeze`、`isRecord`、own-data snapshot、hostile入力回帰は一括削除せず、UI接続とruntime分割時に信頼境界を確認しながら共通化・縮小する。安全停止、原子的scene commit、stale結果非公開の回帰は保持する。
- 補正解析authority chainと解析専用UIの接続は上記の副作用境界で完結した。`FoldPreview` runtime分割の第一段階では、authorityを入力に持たないscene・camera・renderer・照明・grid・紙/輪郭材質の所有と破棄だけをReact非依存境界へ分離した。次は補正解析の新段階や新しい一般DTO防御を追加せず、残るcamera/入力runtimeで入口検証と副作用authority以外の重複防御を段階的に整理し、redacted diagnosticsを独立境界として追加する。

### 13.2 Redacted diagnostics境界

- frontendの診断生成APIは`reportUnexpected(scope)`だけとし、raw error、message、stack、cause、任意contextを受け取らない。scopeは固定allowlistをTypeScript型と実行時検査の両方で制限する。
- 15 scopeの件数だけをメモリ内で集計する。各件数は65で飽和し、外部へ公開するのは`0`、`1`、`2_4`、`5_16`、`17_64`、`65_plus`の粗い区分だけとする。snapshotは固定順・8 KiB以下で、作品名、座標、寸法、entity ID、revision、ファイル名・パス、時刻、アプリ版、OS、CPU architecture、GPU等の環境情報を含めない。
- 計測対象は、利用者向け安全停止へ移るグローバル例外と上位の起動・解析・3D runtime境界に限定する。キャンセル、stale結果の破棄、入力/編集拒否、ファイル権限・破損、作業上限、`indeterminate`、独立資源のbest-effort cleanupを予期しない障害として一括記録しない。
- frontend runtimeはメモリ内集計を先に完了し、Tauri環境でだけ`record_unexpected_diagnostic({ scope })`をfire-and-forgetで呼ぶ。任意文字列、error、context、作品snapshot、保存先pathはIPCへ渡さない。frontendとnativeの両方でscope別65回/sessionへ制限し、同期例外・非同期拒否・永続化失敗を診断機能自身へ再帰させない。
- Rustはfrontendと完全に同じ`origami2.redacted-diagnostics.v1`の`{schema, unexpected}`だけをstrict enum・unknown field拒否・固定15件・固定順で再検証し、Tauriのアプリ専用log領域にある`redacted-diagnostics-v1.json`だけへ保存する。別形状を同じschema名で扱わず、アプリ版やOS等のnative metadataも加えない。
- 保存は8 KiBを上限にbucketが変わる時だけ行う。同じdirectoryのcreate-new一時ファイルへ書込み、同期、同一handle再読込、bytes一致確認後に原子的置換し、POSIXでは親directoryも同期する。Unix系の新規file modeは`0600`を上限とし、既存owner modeがより厳しい場合はそれを保持する。24時間を超えた診断用一時ファイルだけを起動時に最大512件走査・32件削除し、通常ファイル以外と無関係な名前には触れない。
- 読込時は8 KiB超過、JSON破損、unknown/欠落field、scope順序・重複・件数不一致を空の内部状態へfail closedし、元ファイルは次の正当な記録まで変更しない。永続化に一度失敗したsessionは以後のdisk I/Oを停止する。commandは真のasync functionで非同期gateを取得してから明示的にblocking poolへ移し、blocking jobを常時1本に制限する。WebViewのasync worker上ではmutex待ちや`sync_all`を実行しない。
- 共有用previewはnative側で永続化gate内の集計snapshotからcanonical JSONを一度だけ生成し、単調増加する`preview_generation`、UTF-8 byte長、JSON文字列を返す。nativeは最新1世代の正確なbytesをprivate cacheへ保持し、frontendはschema、固定field、15 scopeの順序、bucket、byte長、8 KiB上限、canonical再serialize一致をIPC入口で検証する。
- 保存commandがfrontendから受け取るのは検証済み`preview_generation`だけとする。nativeはcache済みの同一bytesだけを、native保存ダイアログで利用者が選んだ場所へ原子的に保存する。保存先path、file handle、JSON本文、raw errorをWebViewとのIPCへ渡さず、旧世代、改変preview、byte長不一致、dialog失敗、I/O失敗を固定errorへ退避する。キャンセルは書込みを行わず、世代・byte長・キャンセル済みの固定metadataだけを返す。
- Tauri版だけに診断ダイアログを表示し、背景を`inert`にしたmodal内で、保存対象そのものを読取専用textareaへ表示する。利用者は内容を全選択するか、nativeダイアログからJSONファイルとして保存できる。保存中は閉じる操作と重複保存を止め、Escape、focus trap、開始focus、呼出元へのfocus復帰、stale request無効化、狭幅overflowを明示的に処理する。
- 通信、console、Web Storage、自動送信、自動clipboard操作は行わない。表示JSONと保存bytesは同一世代の同一内容であり、利用者自身が内容を確認してGitHub Issuesへ手動添付する。GitHub Issues、telemetry、クラッシュ報告への自動送信は今後も行わない。

## 14. 実装フェーズ

外部配布は全MUST要件完成後とするが、内部確認可能な垂直スライスを以下の順で作る。

### Phase 0: 技術PoC

- Tauri/Rust/Reactの通信
- 10,000本線描画と選択ベンチマーク
- 頑健な線分交差・面抽出
- 厚さ付き2面の連続衝突
- Windowsビルドと、macOSの自動ビルド・CI検証

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

### Phase 4: 検証・層順序

- 構造検証、川崎・前川条件
- 時間制限つき全体平坦折り3値判定
- 平坦折り判定と一体化した層順序管理
- 進捗、中止、結果根拠、対象クラス表示

完了条件: 合意した対象クラスで既知の可・不可例を正しく分類し、時間切れ・証明不足を「不明」として区別し、後続の折り重ね操作が使用できる層順序を保持する。

### Phase 5: 折り手順

- 一直線による複数層の折り重ね、展開図への層別山谷線追加、1 step記録
- タイムライン、同時折り、固定面、持ち替え
- 3D操作記録、編集、再生
- 簡略指ガイド、技法テンプレート
- 画像/PDF出力

完了条件: 一作品の折り方を最初から最後まで記録、再生、PDF化できる。

### Phase 6: 互換性・性能・配布

- DXF、OBJ、STL、glTF
- 10,000本性能最適化
- Windows正式パッケージ
- macOSの自動ビルド・テスト・`.app`生成CI
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
7. Windowsでビルドし、同一コードのmacOSビルド・自動テスト・`.app`生成をCIで検証する。

このスパイクで通信量、描画方式、ジョブモデル、衝突ライブラリを確定する。結果はADRとベンチマークとして保存する。

## 16. 設計上の未解決事項

OQ-001は、全入力を時間制限つきの「可／不可／不明」で返し、証明可能な対象クラスと根拠を明示する契約として初版解決した。OQ-002は`centered_mid_surface_v1`を初版正式仕様とし、厳密な層ずれを将来課題とすることで解決した。OQ-004は、外部汎用solverを採用せず、exact facewise制約のBFS伝播と明示stackによる決定論的DFS、独立certificate再検証を`convex_faces_facewise_v1`として実装したため初版解決した。詳細は[全体平坦折り判定と層順序管理の設計](global-flat-foldability-design.md)を正本とする。

OQ-007「PDF図記号とレイアウト規則」は、[折り手順書き出し契約](instruction-export-contract.md)の`instruction_export_v1`として初版解決した。将来profileで図記号やlayoutを拡張する場合も、初版profileの決定論的出力は変更しない。

| ID | 項目 | 解決時期 |
|---|---|---|
| OQ-003 | 高精度数値ライブラリ | Phase 0 |
| OQ-005 | 2D Canvas/WebGL切替基準 | Phase 0 |
| OQ-006 | `.ori2`スキーマと履歴圧縮 | Phase 1前 |
| OQ-008 | 各外部形式の対応バージョン | Phase 2/6前 |
| OQ-009 | GPL-3.0-only/or-later | 公開前 |
| OQ-010 | 正式名称と商標 | 公開前 |
