# 局所平坦折り条件の設計

## 1. 目的

VAL-002の初期実装として、各頂点に対する川崎条件と前川条件を検証する。結果は局所的な必要条件の情報であり、指定済み山谷割当ての局所十分性、展開図全体の平坦折り可能性、厚さ付き紙の折りやすさを保証しない。VAL-003の全体平坦折り判定とは別の結果として扱う。

根拠は次の単一頂点平坦折りの資料へ照合する。

- Thomas C. Hull, [The Combinatorics of Flat Folds: a Survey](https://arxiv.org/abs/1307.1065)
- Koji Ouchi and Ryuhei Uehara, [Efficient Enumeration of Flat-Foldable Single Vertex Crease Patterns](https://doi.org/10.1587/transinf.2018FCP0004)

## 2. 対応モデル

固定model名を`interior_single_vertex_zero_thickness_v1`とする。

- 保存されたbinary64座標を設計値の正本とする。
- 理想的な厚さ0、幅0の直線折り線を対象とする。
- 紙内部で、十分小さい近傍が一枚の材料diskとなる頂点だけを川崎・前川条件の適用対象とする。
- MountainとValleyだけを折り線rayとして数える。
- Auxiliaryは作図補助であり、未割当折り線として数えない。
- 紙外周、Boundary接続、Cut接続、折り線なしの頂点は理由付き`not_applicable`とする。
- 同一ray、長さ0、重複ID、重複辺、未分割交差、紙外の参加辺等で正しい回転順を構築できない入力は、成立・不成立を推測せずreport全体を`blocked`とする。

プロジェクト内の全頂点をcanonical `VertexId`順で返す。これにより「各頂点」を満たし、対象外も黙って省略しない。

## 3. 状態

report全体は次の5状態を持つ。

| 状態 | 意味 |
|---|---|
| `blocked` | 前段の構造・紙・参加辺検証を通せず、頂点判定を開始していない |
| `not_applicable` | 全頂点が局所定理の対象外 |
| `necessary_conditions_satisfied` | 一つ以上の適用対象があり、全対象で今回実装した必要条件を認定 |
| `violated` | 一つ以上の頂点で必要条件違反を認定 |
| `indeterminate` | 違反は認定していないが、資源上限等により一つ以上を確定できない |

頂点と各定理は`satisfied / violated / not_applicable / indeterminate`を持つ。総合優先順位は`violated > indeterminate > satisfied > not_applicable`とする。資源上限で川崎条件が`indeterminate`でも、前川条件の違反を整数で認定できた場合は頂点とreportを`violated`とする。

`blocked`は頂点の`indeterminate`へ変換しない。前段入力を確定できない場合と、正当な頂点を計算したが上限へ達した場合を区別する。

## 4. 川崎条件

検証済みの反時計回りrayを`r0..r(2n-1)`、連続sector角を`αi`とする。偶数sectorの和と奇数sectorの和がともにπであるときだけ成立する。奇数次数は偶数次数条件を満たさないため`violated`とする。次数2は二つのrayが正反対の場合だけ成立する。

角度を`atan2`で求めてepsilon比較しない。各保存座標を共通単位`2^-1074`の`BigInt`へ正確に変換してから、端点と中心の差を取る。方向を非正規化complex整数`zi = xi + i yi`とし、

```text
A = product(z0, z2, ...)
B = product(z1, z3, ...)
```

を計算する。`A * conjugate(B)`の虚部がexact zeroで、実部が負の場合だけ川崎条件成立とする。各rayの正の長さscaleは積の偏角を変えない。積はbalanced treeで計算し、各段で実部・虚部に共通する2の冪を除いて整数肥大を抑える。

exact計算の上限は一頂点256折り線とする。超過は数学上の対象外ではなく、`fold_degree_limit`による`indeterminate`である。上限値はreportへ含め、UI側で別の上限を決めない。

## 5. 前川条件

現行schemaでは全折り線にMountainまたはValleyが割り当てられている。山折り数を`M`、谷折り数を`V`とし、`|M - V| = 2`の場合だけ成立する。これは必要条件であり、一般角度の指定済み山谷割当てに対する十分条件ではない。

将来、未割当折り線を追加するときはAuxiliaryを流用しない。折り線roleと山谷assignmentを分離し、未割当数を含む補完可能性と「現在の割当てで成立」を別状態にする。

## 6. admissionと決定性

局所判定はfaceやexterior walkの完成を前提にしない。degree 1等をface構築失敗として遮断すると、前川条件違反を利用者へ示せないためである。

処理順は次で固定する。

1. 全VertexId・EdgeIdの一意性
2. 紙境界と紙設定の検証
3. 切断可否policy
4. Boundary/Mountain/Valley/Cutだけから作る参加graphの交差・端点検証
5. active edgeの紙内包含
6. exact orientationによるDCEL反時計回りrotation
7. 各頂点の適用条件、川崎条件、前川条件

ray順をIDや入力配列順で決めない。同一rayをIDで並べて判定へ進めず、前段で遮断する。結果の頂点順だけをcanonical IDで安定化する。edge保存方向、edge配列順、全体平行移動、正の2冪scale、全Mountain/Valley交換で条件結果が変わらないことを回帰する。

## 7. DTO

reportは固定fieldだけを持つ。

```text
LocalFlatFoldabilityReport
├─ model
├─ max_exact_fold_degree
├─ status
├─ total_vertices
├─ applicable_vertices
├─ satisfied_vertices
├─ violated_vertices
├─ not_applicable_vertices
├─ indeterminate_vertices
└─ vertices[]
   ├─ vertex
   ├─ fold_degree
   ├─ mountain_count
   ├─ valley_count
   ├─ verdict
   ├─ reason
   ├─ kawasaki
   └─ maekawa
```

`reason`は`paper_boundary / cut_incident / no_incident_fold_edges / fold_degree_limit`または`null`に限定する。巨大なexact整数、角度近似値、内部error文字列、`foldable: true`のような十分性を誤認させるfieldは返さない。

`blocked`では、前段入力の頂点集合を信頼済み結果として返さず、verticesを空、全件数を0とする。それ以外では次の整合を要求する。

```text
total_vertices = vertices.length
total_vertices =
  satisfied_vertices + violated_vertices
  + not_applicable_vertices + indeterminate_vertices
applicable_vertices =
  satisfied_vertices + violated_vertices + indeterminate_vertices
```

## 8. UI

既存の幾何検証`is_valid / issues`へ局所条件違反を混ぜない。同じ検証操作の同一revision応答へ、別の`local_flat_foldability`結果として含める。

- 合格: 「川崎・前川の局所必要条件を満たします」
- 違反: 条件名、次数、山折り数、谷折り数と対象頂点を表示
- 対象外: 外周、切断接続、折り線なし等の固定理由を表示
- 判定不能: 計算上限等を表示
- blocked: 「前段の幾何・紙・参加辺を確定できないため未判定」と表示

常に「理想的な厚さ0の局所条件であり、指定山谷割当ての十分性や展開図全体の平坦折りを保証しない」と表示する。

問題一覧は最大20件をDOMへ表示し、残件数を明記する。不成立頂点は赤い実線ring、判定不能頂点は黄色い破線ringをCanvasの最大2 batchで描く。色だけに依存せず、一覧の状態文と線種を併用する。編集でprojectまたはrevisionが変わった結果は消去し、benchmark表示へ通常projectの結果を重ねない。

## 9. 完了条件

- 川崎条件をepsilonなしのexact predicateで判定する。
- 前川条件を整数で判定する。
- 全頂点に結果または対象外理由を持つ。
- 境界・切断・構造不正・資源上限を成立や不成立へ誤変換しない。
- 合格を全体平坦折り可能と表現しない。
- degree 1/2/3/4/6、非対称角、1 ULP差、巨大・極小座標、入力順・向き不変、境界、切断、Auxiliary、上限超過、10,000要素を回帰する。
- stale結果を表示・Canvas強調へ使用しない。
