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
- 現行の`vertical_parameter_v1`は、画面投影したヒンジ線の上下ドラッグで0〜180度の折り角パラメータを変更し、同じ検証経路へ渡す。紙面を掴んだ3D位置から角度を逆算する物理操作ではない。
- 現行UIの`physical_grab_v2`は、単一折りまたは木構造で選択した1ヒンジの従属面上にある表裏点とポインターrayから0〜180度の目標角を逆算する別契約であり、`vertical_parameter_v1`の意味を置き換えない。木構造では選択外ヒンジの完全角度vectorを固定し、連続衝突jobが認定した完全姿勢だけを原子的に3Dへ適用して終端安全角を確定する。複数ヒンジの同時運動と閉ループへの適用は未対応とする。
- フォーカス中の3D領域では、H/Shift+Hで表示順の次/前のヒンジ、F/Shift+Fで次/前の固定面を選び、Escapeでヒンジ選択を解除する。`KeyboardEvent.key`を文字入力の正本として特殊配列とOSキー再割当を尊重し、修飾キー、キーリピート、IME合成中、重複・空・過大ID、古いmodel/callbackは操作へ接続しない。実行可能なshortcutだけをARIAへ公開し、選択結果は常設した2つのpolite live regionを交互更新して同文も再通知する。

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

静的分類単独では連続運動を保証しない。単一折りUIと、木構造の選択1ヒンジについて他ヒンジを固定する物理把持UIから連続経路を判定する。複数ヒンジvectorの同時連続運動は未検証として扱う。いずれも実際の折り癖、材料変形、層ずれを保証せず、OQ-002の物理的な厚さオフセット規則は引き続き未解決とする。UIでも中央面基準モデルによる許容であることを明示する。

#### 8.2.2 三角柱SAT witness seed

現在の`triangle_prism_sat_witness_v1`は、authoritativeな狭域SATが確定した一つの正厚三角柱対から、後続の衝突説明に使う有限なwitness seedを導出する純粋境界である。現在姿勢の狭域結果と、木構造で選択1ヒンジを動かした連続経路の危険姿勢へface・triangle identity付きで接続し、後者は停止詳細UIへも提示する。ただし、全体面の修正や安全な新姿勢を証明する機能ではない。

- 入力頂点はauthoritative SATが使用した同一順序・同一座標の6頂点snapshotでなければならず、呼出側で再構築・並べ替えしない。`authoritativeGeometryClass`は`touching`または`penetrating`だけを受理し、ローカル再導出したclassと一致しなければ`null`とする。`separated`と`indeterminate`からwitnessを作らない。
- 狭域結果の`witnessSamples`は、最終interactionが`touching`または`penetrating`へ確定した非隣接triangle-pairだけをface ID・両triangle index・classと結合する。hinge、`indeterminate`、ゼロ厚は対象外とし、witness導出が`null`でもauthoritative分類を変更しない。
- 説明導出は衝突判定の100万pair上限と独立した最大16 attemptに制限する。全候補中の`penetrating`を`touching`より優先し、同一severityではauthoritative走査順を維持する。上限を満たした後もSAT自体の必要な走査は省略しない。
- coverageは`eligible = attempted + omittedByLimit`、`attempted = witnessSamples + unavailable`を満たす。早期貫通終了または非隣接SAT未実行時は`authoritativePairScanComplete: false`とし、0件を「全対象を説明済み」と誤表示しない。
- SATの正規化、外積、長さ、射影はauthoritative経路と同じ演算順を保ち、近平行軸はより保守的に`null`へ退避する。共通剛体変換後の丸めによって利用不能になる場合も、法線を推測しない。
- 出力は「第2三角柱を第1三角柱から離す」向きの単位法線、単一pairが接触へ達するまでの`escapeDistance`、margin内ですでに離隔している`toleratedGap`、各4点以下のsupport、最大16点のsupport midpoint hullを持つ。位置候補の`sourcePose`はhint適用前の解析入力姿勢である。
- `localSeparationHint`のscopeは選択三角柱pairだけであり、`autoApplicable: false`を固定する。複数pair、共有ヒンジ、他面、連続経路、紙の変形を考慮した全体修正ではないため、安全停止姿勢やプロジェクトへtranslationを自動適用しない。
- 入力は公開境界で一度だけsnapshotし、頂点数、右手系frame、正厚、対応cap、有限値、support上限を検証する。結果は全階層を不変化し、Proxy、getter例外、overflow、退化、過大marginをfail-closedで`null`へ退避する。
- 連続運動層はproject/revision、固定面、選択ヒンジ、紙厚、request context、完全な開始・目標・危険角度vector、`blockingSampleTime`、危険側2面の変換をface・triangle identityへ結び付ける。終端時刻とblockerが一致した場合だけ説明snapshotを保持し、不一致や説明生成失敗ではauthoritativeな停止を維持したまま説明だけを破棄する。
- UI境界はrequest identity、全角度vector、2面変換、全witness、coverage方程式、primary witnessを再検証する。有効な場合だけ危険解析角、三角形番号、位置候補数、法線、局所分離距離を解析情報として提示し、保存した危険側の面行列を3D表示の更新には使用していないこと、候補が三角柱1組だけを対象とし自動適用できないことを明示する。時刻0では現在の表示姿勢自体と危険解析角が一致し得るため、「危険姿勢は表示されていない」とは表現しない。内部ID・context keyは表示しない。

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

現在の停止位置は「最初の衝突時刻の推定値」ではなく、時刻順探索で最後に連続安全を確認できた下限である。木構造の選択1ヒンジ経路では、実際に危険と判定した別姿勢へrequest identityと三角柱pair単位のwitnessを結合し、局所法線・分離距離までUI提示する。複数pairを同時に解消する全体修正、安全な修正後姿勢の再証明、複数ヒンジvectorの同時連続運動は次段階とする。

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
