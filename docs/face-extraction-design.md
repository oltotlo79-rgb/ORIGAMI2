# 面抽出・平面トポロジー設計

## 1. 目的

`ori-topology` は、2D 展開図を 3D 折り計算で使用できる面、ヒンジ、切断接続へ変換する。初期実装は局所更新の速さより正しさを優先し、常に入力全体から再構築する参照実装とする。

本設計が直接扱う要件は次のとおりである。

- PRJ-001/002: 一枚の紙と、直線辺からなる任意の単純紙ポリゴン
- PRJ-003/004: プロジェクト単位の切断可否と切断線
- EDT-001: 面の選択と、面を導出する頂点・辺の編集
- EDT-011/012: 不正な編集中状態の保存を許し、3D 移行時には遮断する
- VAL-001: 未分割交点、閉じていない面、輪郭外要素、接続関係の検証
- SIM-001/004: 面とヒンジ関係の構築、固定面から見た連動面群
- SIM-008: 切断後の接続切断と、一枚紙由来情報の保持
- SIM-009: 10,000 本以上を前提にしたデータ構造

`CreasePattern` の頂点・辺と `Paper` を正本とし、面は導出データとする。面を直接書き換える API は設けない。面に対する「分割」「削除」等の編集操作は、`ori-core` が頂点・辺のコマンドへ変換し、成功後に面を再抽出する。

## 2. 適用範囲

### 2.1 初期実装で扱う入力

- 紙外周は 3 頂点以上の任意の単純多角形で、凸・凹、CW・CCW のいずれも許可する。
- すべての幾何は有限な 2D 直線分とする。
- 交差、T 接続、多辺交点は事前に一つの共有 `VertexId` へ分割済みとする。
- 山折り、谷折り、外周、切断を平面埋め込みへ参加させる。
- 補助線は表示・作図要素であり、面を分割しない。
- 外周内の閉じた線群、切断による複数部品、面の穴、開いた切れ目を扱う。
- 切断の幅（kerf）は 0 とし、材料を面積として除去しない。

### 2.2 初期実装で扱わないもの

- 曲線、重なった線分、交差未分割のままの自動修復
- 複数枚の紙、接着、材料の追加・面積を持つ切り抜き除去
- 面の三角形分割、3D 変換、厚さ、衝突、折り拘束の解法
- 近接点を同一点とみなす許容誤差ベースのトポロジー変更
- 編集差分だけを更新する増分 DCEL

入力を暗黙に修復しない。たとえば近い二頂点を統合したり、交差位置へ頂点を自動追加したりせず、関連 ID を持つ診断を返して編集コマンド側の明示的な修復へつなげる。

## 3. 既存モデルとの契約

### 3.1 正本

```text
Paper.boundary_vertices       元の一枚紙の外周順
CreasePattern.vertices        座標付き頂点
CreasePattern.edges           原子的な直線分
EdgeKind::Boundary            紙外周
EdgeKind::Mountain / Valley   面を分けるヒンジ候補
EdgeKind::Cut                 接続を持たない切れ目
EdgeKind::Auxiliary           位相へ参加しない作図線
```

`Paper.boundary_vertices` の連続無向頂点対と `Boundary` 辺の多重集合が一致していることを前提にする。辺レコードの `start -> end` は外周順と同方向である必要はない。

### 3.2 crate の依存方向

```text
ori-domain
    ▲
ori-geometry    （有限性、向き、交差、包含の頑健な述語）
    ▲
ori-topology    （DCEL、面、隣接、切断後の材料成分）
    ▲
ori-core        （revision、コマンド、属性の引継ぎ、UI 向け facade）
```

`ori-topology` は UI、Tauri、Three.js、履歴管理へ依存しない。将来 `ori-numeric` が追加された場合、トポロジーを変える述語は `ori-geometry` 経由でその頑健な判定を使用する。

### 3.3 座標系

位相計算は保存された `(x, y)` の数値平面で行う。Canvas の Y 軸方向は UI の表示変換であり、アルゴリズムへ持ち込まない。外周入力の向きに依存せず、符号付き面積と half-edge の左側規約から出力向きを正規化する。

### 3.4 数値判定

トポロジーを変える符号判定は、f64 の生の determinant を無条件に採用しない。まず演算誤差境界付き f64 filter で符号を確定し、境界内なら adaptive exact predicate へ昇格する。exact backend が未実装の段階では、境界内を `PredicateIndeterminate` として fail closed してよく、ID 順や固定 epsilon で正負を推測してはならない。

この規則を orientation、ray の角度順、線分交差、point-on-segment、polygon containment、符号付き面積へ共通適用する。表示や距離スナップの許容誤差は、この位相規則とは別である。

## 4. 面・辺の意味

### 4.1 面

面は、位相参加辺を越えずに到達できる紙上の最大連結な 2D 領域である。0 幅の辺自身に面積はない。

- 山折り・谷折りの両側が異なる面なら、その二面間にヒンジを作る。
- 切断線の両側にはヒンジも材料接続も作らない。
- 外周の片側だけが材料面であり、反対側は紙の外部である。
- 補助線を追加・移動・削除しても面と `FaceId` は変わらない。

### 4.2 外周、穴、切れ目

`Face` の境界は座標列ではなく、向き付き half-edge の閉じた walk として保持する。これにより、凹形状、穴、および同じ切断線の両岸を通る開いた切れ目を失わない。

```text
Face
├─ outer: BoundaryWalk
├─ holes: Vec<BoundaryWalk>
└─ seams: Vec<BoundaryWalk>
```

- `outer` は面の材料側を左に見て進む外側 walk である。
- `holes` は同じ面に属する内側境界 walk である。
- `seams` は独立した 0 面積の切断岸 walk である。
- walk は位相的には閉じるが、切断先端を往復する場合は同じ幾何頂点を複数回通り得る。そのため単純多角形であるとは限らない。

外周 walk に接続された開いた切れ目は、外周 walk 内に二つの切断岸として現れる。紙内部に孤立した開いた切れ目は `seams` へ入る。レンダラーや将来の mesher は切断の両岸を別頂点として展開できるが、初期の面抽出は三角形分割を行わない。

### 4.3 「穴」は材料消失を意味しない

単純紙ポリゴン自体に穴はないが、内部の閉ループは面を入れ子にする。たとえば閉じた切断ループでは、外側面がそのループを `holes` に持ち、内側にも別の材料面が存在する。切り抜いた内側を自動的に廃棄してはならない。両方の材料成分は同じ元紙に由来する。

材料を実際に除去する将来機能を追加する場合は、`Cut` とは別の明示的な領域除去コマンドとデータ型を設ける。

### 4.4 非分離折り線

折り線の両 half-edge が同じ面へ戻る場合、その線は剛体面間ヒンジにならない。代表例は紙内部で途切れた折り線である。

面スナップショット自体は作成できるが、`NonSeparatingFoldEdge` を 3D 移行遮断診断として返す。切断線で同じ現象が起きる場合は有効な開いた切れ目として扱い、材料接続だけを切る。

### 4.5 材料成分

面をノードとし、山折り・谷折りの有効ヒンジだけを接続として Union-Find でまとめたものを `MaterialComponent` とする。`Boundary` と `Cut` をまたいで結合しない。

- 境界から境界まで達する切断や閉じた切断ループは、材料成分を分ける。
- 開いた切れ目の周囲に別経路があれば同じ材料成分のままである。
- 全成分に同じ `SheetOriginId` を付け、SIM-008 の「元の一枚紙に由来」を保持する。

現モデルには `SheetOriginId` がないため、初版はプロジェクト作成時の一枚紙に対して不変な namespace を `ori-core` から入力する。永続形式へ複数紙を導入するまでは、この値を `ProjectId` から決定的に導出してよい。

切断頂点では、同じ `VertexId` を共有するという理由だけで face や材料成分を結合しない。rotation system の各 sector と向き付き cut bank が接続の正本であり、3D mesher は `(FaceId, boundary walk occurrence)` ごとに頂点を複製できる。分割済みの分岐 Cut もこの規則で扱い、単に次数が 3 以上という理由では拒否しない。

## 5. DCEL データ構造

公開スナップショットは配列 index を永続参照にせず、ドメイン ID と決定的 key を使用する。内部 index は一回の構築中だけ使用できる。

```rust
pub struct HalfEdgeRef {
    pub edge: EdgeId,
    pub origin: VertexId,
    pub destination: VertexId,
}

pub struct BoundaryWalk {
    pub half_edges: Vec<HalfEdgeRef>,
    pub signed_double_area: f64,
    pub classification: WalkClassification,
}

pub enum WalkClassification {
    Outer,
    Hole,
    CutSeam,
}

pub struct Face {
    pub id: FaceId,
    pub key: FaceKey,
    pub outer: BoundaryWalk,
    pub holes: Vec<BoundaryWalk>,
    pub seams: Vec<BoundaryWalk>,
    pub area: f64,
}

pub enum EdgeIncidence {
    Boundary {
        material: FaceId,
    },
    Hinge {
        left: FaceId,
        right: FaceId,
        assignment: FoldAssignment,
    },
    Cut {
        left: FaceId,
        right: FaceId,
    },
    NonSeparatingFold {
        face: FaceId,
        assignment: FoldAssignment,
    },
    AuxiliaryIgnored,
}

pub struct FaceAdjacency {
    pub edge: EdgeId,
    pub first: FaceId,
    pub second: FaceId,
    pub assignment: FoldAssignment,
}

pub struct MaterialComponent {
    pub key: MaterialComponentKey,
    pub sheet_origin: SheetOriginId,
    pub faces: Vec<FaceId>,
}

pub struct TopologySnapshot {
    pub source_revision: u64,
    pub faces: Vec<Face>,
    pub edge_incidence: Vec<(EdgeId, EdgeIncidence)>,
    pub hinge_adjacency: Vec<FaceAdjacency>,
    pub material_components: Vec<MaterialComponent>,
}
```

`Cut { left, right }` は二面が同じ場合も許す。これは開いた切れ目であり、接続を意味しない。`FaceAdjacency` は実際のヒンジだけを含めるため、3D 側が誤って切断を伝播経路に使えない。

`left/right` は元辺レコードの保存方向ではなく、endpoint の canonical bytes が小さい頂点から大きい頂点へ向かう canonical edge direction に対する左右とする。元辺の `start/end` だけを反転しても公開 snapshot は変わらない。左右を必要としない一覧では二面を `FaceKey` 順の `first/second` に正規化する。

`FoldAssignment` は既存 `EdgeKind::Mountain/Valley` から導出する。山谷の変更は面境界を変えず、ヒンジ属性だけを変える。

## 6. 公開 API

編集途中の不正状態を保存できる要件と、3D で不正な面を使わない要件を両立するため、診断 API と strict API を分ける。

```rust
pub struct FaceExtractionInput<'a> {
    pub identity_namespace: FaceIdentityNamespace,
    pub sheet_origin: SheetOriginId,
    pub source_revision: u64,
    pub paper: &'a Paper,
    pub pattern: &'a CreasePattern,
}

pub struct FaceExtractionReport {
    // Fatal がある場合は None。BlocksSimulation だけなら調査用 snapshot を返せる。
    pub snapshot: Option<TopologySnapshot>,
    pub issues: Vec<TopologyIssue>,
}

pub fn analyze_faces(input: FaceExtractionInput<'_>) -> FaceExtractionReport;

pub fn extract_faces_strict(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, FaceExtractionRejected>;
```

- `analyze_faces` は UI の検証表示向けで、入力を変更しない。
- `extract_faces_strict` は `BlocksSimulation` または `Fatal` が一件でもあれば失敗し、3D へ部分結果を渡さない。
- 問題が複数あっても安全に判定可能な範囲で全件を返す。
- `source_revision` を結果へそのまま保持し、非同期化後に古い結果を適用しない。
- API は入力順を変更せず、内部キャッシュや乱数に依存しない純粋関数とする。

既存 `FaceId` は v4 UUID の新規生成しか公開していない。実装時には `ori-domain` に、UUID の canonical bytes を読むアクセサと、検証済み決定的 UUID から各 ID を構築する crate 内向け API が必要になる。面抽出側で ID 型を複製しない。

## 7. 構築アルゴリズム

### 7.1 Stage 0: 入力解決と検証

次を順に検証し、診断順も固定する。

1. 頂点 ID と辺 ID が一意である。
2. 全参加辺の端点が一意な頂点レコードへ解決できる。
3. 座標が有限で、辺の幾何長が 0 でない。
4. `Paper` 外周が単純で面積を持ち、外周辺レコードと一致する。
5. 切断禁止時に `Cut` が存在しない。
6. 参加辺同士に未分割交点、T 接続、正長の共線重複がない。
7. 外周以外の参加辺の全開区間が紙の閉包内にある。
8. 同一無向頂点対を複数の参加辺が占有しない。

凹紙ポリゴンでは両端が紙内でも線分中間が紙外へ出ることがあるため、端点包含だけで合格させない。外周との交差で分割した区間ごとに、頑健な polygon containment で内外を判定する。

既存の `validate_crease_pattern` と `validate_paper` は Stage 0 の一部を満たす。`ori-topology` は将来、検証済みトークンを受け取って二重計算を省けるようにしてよいが、未検証の公開入力を無条件に信頼しない。

### 7.2 Stage 1: 位相参加辺の選別

`Boundary`、`Mountain`、`Valley`、`Cut` ごとに二つの half-edge を作り、互いを `twin` とする。`Auxiliary` は DCEL へ入れず、出力の `AuxiliaryIgnored` 対応だけを作る。

内部 half-edge は最低限、次を持つ。

```text
edge_id, origin, destination, twin, next, incident_walk
```

元辺の保存方向は幾何 half-edge 集合に影響しない。`start -> end` を反転した同じ入力でも、同じ二つの向き付き half-edge が得られる。

### 7.3 Stage 2: 頂点の rotation system

各頂点から出る half-edge を、数値平面上の反時計回りに並べる。

- `atan2` の丸め値だけで順序を決めない。
- 上下半平面の分類と頑健な orientation 述語を使う。
- 述語が `Indeterminate`、overflow になった場合は ID 順で推測せず、診断して構築を止める。
- 同じ ray に二辺がある場合は、事前検証済みなら共線重複または未分割状態なので入力エラーとする。

half-edge `h` の終点で `twin(h)` の直前、すなわち反時計回り配列上の一つ時計回り側の half-edge を `next(h)` とする。これにより、walk の面は常に進行方向の左側になる。

### 7.4 Stage 3: 境界 walk の列挙

未訪問 half-edge から `next` をたどり、開始 half-edgeへ戻るまでを一つの walk とする。

- 全 half-edge はちょうど一つの walk に一度だけ現れる。
- `2E` 回を超えても閉じない場合は入力診断ではなく `InternalInvariantViolation` とする。
- walk の開始位置は canonical rotation へ正規化する。
- walk の符号付き面積を頑健に計算する。切断岸の往復は面積が相殺される。

### 7.5 Stage 4: 紙外部、outer、hole、seam の分類

walk ごとに「最初の頂点から少し左」といった任意 epsilon の点を作らない。half-edge と頂点の角度順が定める左側の開いた sector を記号的 witness とし、頑健な point-location へ渡す。

1. 紙外周の外側に属する非有界面を一つだけ特定し、出力面から除外する。
2. 正面積の material walk を面の `outer` 候補とする。
3. 負面積 walk は、左側 witness を含む最小の outer 候補へ `hole` として割り当てる。
4. 0 面積の cut-only walk は、左側 witness が属する面の `seam` へ割り当てる。
5. 0 面積 walk に折り線が含まれる場合も所有面を決め、各該当辺へ `NonSeparatingFoldEdge` を付ける。この walk は有効な face 境界や `FaceKey` には含めない。

包含候補は walk の AABB を空間索引で絞る。共有頂点や弱単純 walk があって通常の点包含だけで一意に決まらない場合も、rotation system の sector を使って同じ面の境界成分をまとめる。座標の微小オフセットを導入してはならない。

各 material face の面積は `outer + holes` の符号付き和から求め、正かつ有限でなければ `DegenerateMaterialFace` とする。`seams` は面積へ寄与しない。

### 7.6 Stage 5: 辺 incidence とヒンジ

各元辺について二 half-edge の所属先を調べる。

| 線種 | 必須 incidence | 出力 |
|---|---|---|
| Boundary | 一方が material、他方が exterior | `Boundary` |
| Mountain/Valley | 両方 material、異なる面 | `Hinge` + `FaceAdjacency` |
| Mountain/Valley | 両方が同じ material 面 | `NonSeparatingFold` + 遮断診断 |
| Cut | 両方 material | `Cut`。同一面も可 |
| Auxiliary | DCEL 非参加 | `AuxiliaryIgnored` |

上表に合わない incidence は `InvalidEdgeIncidence` であり、3D へ渡さない。

### 7.7 Stage 6: 材料成分と決定的整列

有効ヒンジを Union-Find で結び、切断をまたがない材料成分を作る。その後、公開配列を次の key で整列する。

- walk: canonical な向き付き edge token 列
- holes / seams: walk key
- faces: `FaceKey`
- edge incidence: `EdgeId` canonical bytes
- adjacency: `(min FaceKey, max FaceKey, EdgeId)`
- material components: 構成 `FaceKey` の整列列

`HashMap` の反復順、入力配列順、スレッド数を公開結果へ漏らさない。

`MaterialComponentKey` は `"ORIGAMI2_MATERIAL_COMPONENT_KEY_V1"`、`SheetOriginId`、整列済み `FaceKey` 列を長さ付きで連結した canonical bytes の SHA-256 とする。したがって同じ切断状態の全面再構築で成分 key も安定する。

## 8. 決定的で安定した Face ID

### 8.1 FaceKey

half-edge を次の token にする。

```text
(EdgeId canonical bytes, origin VertexId canonical bytes,
 destination VertexId canonical bytes)
```

walk は進行方向を反転せず、token 列の辞書順最小 cyclic rotation を canonical 表現とする。面の canonical bytes は以下を domain separator と長さ付きで連結する。

```text
"ORIGAMI2_FACE_KEY_V1"
outer walk
sorted hole walks
sorted seam walks
```

`FaceKey` はこの完全表現の SHA-256 とする。`FaceId` はプロジェクト固有 `FaceIdentityNamespace` と `FaceKey` から UUID v5 で導出する。完全 canonical bytes または SHA-256 key もスナップショットに保持し、異なる key が同じ `FaceId` になった場合は `FaceIdentityCollision` として fail closed する。

線種は key に入れない。同じ原子的境界の山谷変更や Fold から Cut への変更では面の幾何 ID を維持し、incidence と材料成分だけを更新する。`Auxiliary` は token 自体を作らない。

### 8.2 保証範囲

初期の全面再構築版は次を保証する。

- 同じ namespace、同じ参加エンティティ ID と接続なら、プロセス、OS、入力配列順、辺の保存方向に関係なく同じ `FaceId` になる。
- 座標移動だけで rotation system と接続が変わらなければ同じ `FaceId` を保つ。
- 補助線だけの編集では全 `FaceId` を保つ。
- Undo が元の頂点・辺 ID と接続を復元すれば元の `FaceId` に戻る。
- 局所編集と無関係な境界 walk の face は同じ ID を保つ。

辺分割では原子的 edge token が変わるため、幾何領域が同じでも隣接 face の key は変わり得る。コマンド実行中の選択・面属性をより長く保つための lineage 対応は `ori-core` の責務とし、初期 extractor が座標重なりによる推測をしない。

永続的な面属性は `FaceId` を key にした overlay として保持する。再抽出で消えた ID の属性は直ちに別面へ移さず orphan として履歴内に保持し、Undo で ID が戻った場合に再接続する。

## 9. 診断と失敗契約

### 9.1 重大度

```rust
pub enum TopologyIssueSeverity {
    Warning,
    BlocksSimulation,
    Fatal,
}
```

- `Warning`: snapshot の意味は確定しており、UI で注意を示す。
- `BlocksSimulation`: 調査用 snapshot は返せるが、strict API は拒否する。
- `Fatal`: 面所属を一意に決められず、snapshot は `None`。

利用者のキャンセル、資源上限、内部不変条件違反を「折れない」や「入力不正」と混同しない。

### 9.2 主な診断

| 分類 | 例 | 重大度 |
|---|---|---|
| 構造 | DuplicateVertexId / DuplicateEdgeId / MissingEndpoint | Fatal |
| 数値 | NonFiniteCoordinate / PredicateIndeterminate / ArithmeticOverflow | Fatal |
| 外周 | InvalidPaperBoundary / MissingBoundaryEdge / BoundarySelfIntersection | Fatal |
| 埋め込み | ZeroLengthEdge / UnsplitIntersection / CollinearOverlap / DuplicateEmbeddedEdge | Fatal |
| 紙内外 | ActiveEdgeOutsidePaper | Fatal |
| 切断規則 | CutNotAllowed | Fatal |
| 面 | DegenerateMaterialFace / AmbiguousBoundaryComponent | Fatal |
| 折り | NonSeparatingFoldEdge | BlocksSimulation |
| incidence | InvalidEdgeIncidence | Fatal |
| ID | FaceIdentityCollision | Fatal |

各診断は可能な限り `VertexId`、`EdgeId`、`FaceKey`、紙外周 index を構造化フィールドで持つ。表示文だけを機械処理させない。診断配列は stage、元文書 index、canonical ID の順に安定整列する。

補助線や孤立した非参加頂点の紙外判定は、全作品を扱う `ori-validation` の責務として別途報告する。`ori-topology` はそれらを面構築の Fatal 条件にはせず、位相参加要素だけについて `ActiveEdgeOutsidePaper` を返す。

### 9.3 原子性

面抽出は読み取り専用なので失敗時に project、revision、dirty、Undo/Redo を変更しない。`ori-core` は入力 revision と現在 revision が一致する場合だけ結果をキャッシュし、古いジョブ結果は破棄する。

## 10. 正しさの不変条件

成功した `TopologySnapshot` は次をすべて満たす。

1. 各参加辺から二つ、逆向きの half-edge が作られる。
2. 各 half-edge の `twin(twin(h)) == h` である。
3. 各 half-edge はちょうど一つの閉じた walk に属する。
4. 各 material face は一つの outer と 0 個以上の holes/seams を持つ。
5. material face の面積は正かつ有限である。
6. 全 material face の面積和は 0 幅切断前の紙面積と一致する。これは測定値比較であり、位相判定に epsilon を使うことを意味しない。
7. `Boundary` の material 側は一つだけである。
8. 有効ヒンジは異なる二面を結び、切断は `hinge_adjacency` に現れない。
9. 全 face と材料成分が同じ入力 `SheetOriginId` を保持する。
10. canonical 出力を再入力なしで serialize した結果は、同一入力に対して byte 単位で決定的である。

## 11. 計算量と 10,000 辺への方針

交差分割済みであることを検証済みトークンにより再利用できる定常経路では、目標計算量を次とする。

| 処理 | 時間 | 追加メモリ |
|---|---:|---:|
| 頂点・辺解決 | `O(V + E)` expected | `O(V + E)` |
| rotation system | `O(Σ deg(v) log deg(v))` | `O(E)` |
| walk 列挙 | `O(E)` | `O(E)` |
| containment | `O(W log W + K)` average | `O(W)` |
| incidence・成分 | `O(E α(F))` | `O(E + F)` |

`W` は境界 walk 数、`K` は AABB 包含候補数である。すべてが入れ子になる病的入力では containment が `O(W²)` になり得るが、正しさを落とす上限打ち切りはしない。長時間化した場合は将来のジョブ化で中止可能にする。

未分割交差検証は空間索引または sweep broad phase を使用し、通常の疎な 10,000 辺入力で全組合せを列挙しない。性能受け入れ値は専用 fixture を Windows/macOS CI の release benchmark で測定してから固定し、初期実装では少なくとも次を満たす。

- 10,000 辺 fixture を資源上限や stack overflow なしに完了する。
- peak 構造メモリが `V + E + F` に対して線形である。
- 同 fixture の繰返しで ID と canonical 出力 hash が一致する。
- benchmark に検証、DCEL、包含、ID 化を分けた時間と peak 件数を記録する。

## 12. 増分再計算との境界

全面再構築を捨てず、将来も正しさの oracle として残す。

```rust
extract_faces_full(input) -> report

// 将来 API
extract_faces_incremental(previous, input, topology_delta) -> report
```

`TopologyDelta` は追加・削除・接続変更・座標変更・線種変更された ID と影響 AABB を持つ。増分版は次を守る。

- 変更頂点の star、交差候補、影響 walk、包含親、隣接材料成分までを再計算する。
- 影響範囲を一意に閉じられない場合は全面再構築へフォールバックする。
- canonical key、FaceId、診断順、公開配列順は全面版と完全一致させる。
- テストでは各編集列のたびに増分結果と全面結果を構造比較する。
- 全面版と異なる高速近似を正本にしない。

内部 DCEL index を project の永続 ID にしないため、全面版と増分版を切り替えても UI・履歴・保存形式の参照は変わらない。

## 13. テスト受け入れ条件

### 13.1 基本面

- 凸四角形、凹多角形、CW/CCW 外周から一つの同じ face を得る。
- 外周頂点の cyclic shift、頂点・辺配列の全順列、各辺の start/end 反転で同じ canonical snapshot を得る。
- 四角形を境界間の一本の折り線で分け、二面と一つのヒンジを得る。
- 分割済み X 交点で四面、共有中心頂点 degree 4 を得る。
- 分割済み T 接続で期待する面数と三本の incidence を得る。
- 未分割 X/T、共線重複、同位置別 ID、0 長辺は Fatal になり snapshot を返さない。

### 13.2 任意紙外周

- 深い凹部を持つ単純多角形を正しく抽出する。
- 両端が紙内でも中間区間が凹部の外へ出る辺を拒否する。
- 外周上の既存頂点へ接続する折り線と、分割済み外周 T 接続を受理する。
- 自己交差外周、外周辺欠落・重複、0 面積外周を拒否する。

### 13.3 穴と非連結埋め込み

- 紙内部の閉じた折り線ループから、内側面と、一つの hole を持つ外側面を得る。
- 二重・三重の入れ子ループで containment 親を誤らない。
- 複数の離れた閉ループを、それぞれ外側面の hole として決定的に整列する。
- 凹 loop の頂点平均が loop 外へ出る例でも、symbolic witness により正しく所属させる。

### 13.4 切断

- `cutting_allowed == false` で Cut が一つでもあれば Fatal にする。
- 境界から境界への Cut で二面・二材料成分を作り、ヒンジは作らない。
- 閉じた Cut で内外二面・二材料成分を作り、外側面に hole、両方に同じ sheet origin を付ける。
- 境界から内部への Cut で一面・一材料成分を保ち、両岸が同じ FaceId の Cut incidence になる。
- 紙内部の孤立した開 Cut を seam として同じ面へ所属させる。
- 分割済みの分岐 Cut を rotation sector ごとに分け、切断を越えて face・材料成分を誤結合しない。
- 未分割接触、共線重複、sector 所属を一意に決められない Cut は Fatal にする。

### 13.5 stable ID

- 100 回の再構築、異なる HashMap seed、Windows/macOS で同じ FaceId と snapshot hash を得る。
- 座標移動だけで接続と角度順を保つ場合、FaceId を保つ。
- 補助線の追加・削除・線種内変更で FaceId を保つ。
- 山谷反転で FaceId を保ち、hinge assignment だけが変わる。
- Undo で元 entity ID と接続を復元した場合、元 FaceId が戻る。
- 局所編集の非隣接面は ID を保つ。
- hash/UUID 衝突をテスト用注入器で発生させ、黙って面を統合せず Fatal にする。

### 13.6 プロパティ・差分試験

- 生成した有効平面グラフで twin、walk 所属、incidence、正面積、面積和の全不変条件を検査する。
- exterior を含む DCEL の参加辺とその端点について `V - E + F = 1 + C` の Euler 関係を検査する。
- 同じ graph の入力順・辺方向をランダム化し、canonical snapshot を比較する。
- NaN、±Infinity、`f64::MAX` 付近、subnormal、極端に近いが異なる点を fuzz し、panic、無限 loop、推測による成功がないことを確認する。
- 将来の増分版は、ランダム編集列の各 step で全面版と完全一致させる。

### 13.7 回帰と統合

- 既存の矩形紙、辺分割、proper 交点、T 接続、交点クラスタの全テストを維持する。
- 失敗時に project、paper、pattern、revision、dirty、Undo/Redo が不変であることを `ori-core` 統合試験で確認する。
- Tauri 応答は revision 付き snapshot を返し、古い revision の結果を UI が適用しない。
- 3D facade は `extract_faces_strict` の成功結果以外を受け取れない型または API 境界にする。

## 14. 実装順

1. `ori-domain` に決定的 ID 構築・canonical bytes の最小 API を追加する。
2. `ori-geometry` に誤差境界付き filter と、exact 未対応時に曖昧性を返す述語契約を追加する。
3. `ori-topology` crate と診断型、入力・snapshot 型を追加する。
4. 既存 validation を再利用して Stage 0 の strict 入力境界を作る。
5. half-edge、rotation system、walk 列挙を実装する。
6. 紙外部除去、包含、holes/seams の grouping を実装する。
7. incidence、ヒンジ隣接、材料成分、stable FaceId を実装する。
8. fixture、property test、canonical determinism test を追加する。
9. `ori-core` に revision-aware cache と 3D 移行用 strict facade を追加する。
10. 10,000 辺 benchmark を両 OS CI で記録し、性能合格値を確定する。
11. 全面版を oracle として維持したまま、編集 profiler の結果に基づいて増分版へ進む。

この順序では、面抽出が未完成でも現在の 2D 編集・保存を妨げず、面の意味が確定した時点から 3D、折り手順、衝突へ同じ topology snapshot を段階的に接続できる。
