# 衝突接触ポリシー

## 1. 文書の地位と固定バージョン

本書は、折りプレビューにおける面同士の衝突・接触分類の初版正式仕様である。

- ポリシーバージョン: `topology_contact_policy_v1`
- 正厚モデル: `centered_mid_surface_v1`
- 対象: 一つの現在姿勢における、一組の材料面とその三角形対
- 言語間の正規corpus: [`collision-contact-policy-v1.json`](collision-contact-policy-v1.json)
- frontend実装表: [`foldPreviewTopologyContactPolicy.ts`](../apps/desktop/src/lib/foldPreviewTopologyContactPolicy.ts)
- native実装表: [`ori-collision`](../crates/ori-collision/src/lib.rs)
- 4×10固定表の回帰: [`foldPreviewTopologyContactPolicy.test.ts`](../apps/desktop/tests/foldPreviewTopologyContactPolicy.test.ts)

本書の「必須」「禁止」「〜してはならない」は規範要件である。`topology_contact_policy_v1`の表、証明条件、fail-closedの意味は固定し、意味を変える場合は新しいポリシーバージョンを発行しなければならない。正規corpus、frontend実装、native実装または本書の不一致は、v1の再解釈ではなく欠陥として扱う。両実装の全40セルは同じ正規corpusへ照合する。

このポリシーは、証拠を幾何計算から導出する関数ではない。証拠生成器が肯定的に証明した共有関係と交差種別を受け取り、decisionへ写像する。証明できなかったことを、離間、接触許容または共有要素由来の免除として扱ってはならない。

正厚材料どうしの正面積・正体積0の境界面接触はv1の10種類に含まれていなかった。この欠落はv1を再解釈せず、[`topology_contact_policy_v2`](collision-contact-policy-v2.md)の独立した`boundary_area_contact`列で補う。新しいnative証拠生成器は4×11のv2を使用し、v1は互換性検証用に凍結する。

## 2. 材料面と共有関係

紙厚を`t`とする。`t > 0`では材料中央面を正本とし、その表裏へ`t/2`ずつ押し出した閉じた三角柱を材料領域とする。表示上の厚さ、描画上のずらし、見やすさのための補正値を衝突判定へ使ってはならない。`t = 0`では中央面そのものを材料領域とする。

共有関係は、幾何形状が似ていることではなく、不変なトポロジーsnapshotから次のいずれか一つとして認証する。

| 共有関係 | 識別子 | 認証条件 |
| --- | --- | --- |
| 共有なし | `no_shared_feature` | 正規化済み面同士に、真正な共有頂点も共有ヒンジ辺もない |
| 共有頂点 | `shared_vertex` | 同じ正規`VertexId`が対応し、保存済み座標も一致する。異なる座標でのID再利用は不正入力である |
| 共有ヒンジ辺 | `shared_hinge_edge` | 一意な`EdgeId`、左右面、両端`VertexId`、保存済み両端座標、境界辺および厚さ規則が一対一に一致する |
| 同一面 | `same_face` | 同じ正規面identityである。別面の幾何学的一致を同一面としてはならない |

共有関係を一意に認証できない入力、複数の共有ヒンジ、重複ID、座標不一致または不正な隣接情報は、許容根拠を持たない。準備段階で拒否するか`indeterminate`へ退避する。

## 3. 交差証拠とdecision

交差証拠は次の10種類に固定する。

| 交差種別 | 識別子 |
| --- | --- |
| 離間 | `separated` |
| 点接触 | `point_contact` |
| 境界線接触 | `boundary_line_contact` |
| 共有要素のみ | `shared_feature_contact` |
| 共有要素近傍の中央面基準正厚重なり | `shared_feature_thickness_overlap` |
| 共有要素平坦積層 | `shared_feature_flat_stack` |
| 共面正面積重なり | `coplanar_area_overlap` |
| 横断 | `transversal_crossing` |
| 正体積重なり | `positive_volume_overlap` |
| 不明 | `indeterminate` |

decisionは次の意味を持つ。

| decision | 意味 |
| --- | --- |
| `separated` | 材料領域の離間が証明済み |
| `touching` | 非貫通の境界接触が証明済み。非隣接面の連続運動では最初の接触として停止対象である |
| `allowed_shared_vertex_contact` | 完全な交差が認証済み共有頂点だけであることを証明した、共有頂点固有の許容 |
| `requires_hinge_model` | 有限共有軸、材料半平面、厚さcorridor、平坦積層を追加判定する義務。これ自体は許容decisionではない |
| `penetrating` | 共面正面積、横断または正体積の侵入が証明済み |
| `indeterminate` | 安全にも貫通にも確定できないblocking状態 |
| `ignored_self` | 同じ正規面identity同士の自己組を走査対象から除外 |

## 4. 共有関係4種×交差種別10種の完全表

次の40セルを`topology_contact_policy_v1`の完全なdecision表とする。

| 共有関係＼交差種別 | 離間 | 点接触 | 境界線接触 | 共有要素のみ | 共有要素近傍の中央面基準正厚重なり | 共有要素平坦積層 | 共面正面積重なり | 横断 | 正体積重なり | 不明 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 共有なし | `separated` | `touching` | `touching` | `indeterminate` | `indeterminate` | `indeterminate` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 共有頂点 | `indeterminate` | `touching` | `touching` | `allowed_shared_vertex_contact` | `allowed_shared_vertex_contact` | `indeterminate` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 共有ヒンジ辺 | `indeterminate` | `indeterminate` | `indeterminate` | `requires_hinge_model` | `requires_hinge_model` | `requires_hinge_model` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 同一面 | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` |

`same_face`以外では、共有`VertexId`または共有`EdgeId`が存在しても、`coplanar_area_overlap`、`transversal_crossing`および`positive_volume_overlap`は必ず`penetrating`である。

認証済み共有頂点が同時に`separated`であること、または認証済み共有ヒンジ辺の完全交差が汎用の`separated`、`point_contact`、`boundary_line_contact`だけであることは、現在の剛体共有要素モデルでは矛盾セルである。これらを安全へ丸めず`indeterminate`とする。`same_face`の`ignored_self`は走査前除外を説明する表上の値であり、実行時dispatcherまで到達した同一面組は内部不整合として`indeterminate`へ倒す。

広域候補に入った`shared_hinge`面組で全三角形対が`separated`でも、interactionを省略してはならない。共有辺identityと離間が同時成立した矛盾として、ヒンジモデルの診断を付けた`indeterminate`を必ず発行する。

三角柱SATのraw `touching`またはraw `penetrating`は、それだけでは有限corridor内の許容重なりと共有要素外の侵入を区別する証拠ではない。ヒンジ制約・ヒンジ接触ポリシーが無い場合、現行のraw aggregateは独立した証明済み交差種別を保持しないため、全ての`hinge_adjacent`候補を`hingeDecision.reason = missing_constraint`の`indeterminate`へ倒す。将来、共有要素外の`coplanar_area_overlap`、`transversal_crossing`または`positive_volume_overlap`をprivate provenance付きで独立認証できた場合だけ、制約なしでも本表の`penetrating`を維持できる。

この純粋表が返す文字列だけでは、停止除外またはヒンジ許容を認可しない。実行時に許容へ進めるのは、解析器が肯定証明の直後に発行したprivate capabilityを同じ三角形対、紙厚状態、raw分類へ再結合でき、さらに本表のdecisionと一致した場合だけである。

## 5. evidence生成の肯定証明条件

### 5.1 共通条件

証拠生成器は、対象姿勢、面identity、三角形index、トポロジーsnapshot、紙厚、変換および数値marginを同じ不変requestへ結合しなければならない。全入力は有限値であり、姿勢変換は剛体変換でなければならない。

各証拠は次の肯定条件を満たした場合だけ生成する。

1. 対象の閉じた材料領域または中央面について、主張する交差次元と範囲を直接証明する。
2. より重大な交差を除外する。点を主張するなら正長の線・正面積・正体積を、境界線を主張するなら内部横断・正面積・正体積を除外する。
3. 共有要素固有の証拠では、完全な交差集合が認証済み共有要素、または本書で定めるその有限近傍だけに収まることを証明する。
4. margin境界、近平行、退化、走査未完了、作業上限、非有限値または証明失敗が残る場合は、安全側の推測をせず`indeterminate`とする。

「交差を見つけなかった」「共有IDがある」「数値的に小さい」だけでは肯定証明にならない。

狭域候補を走査する前に、直接隣接かどうかを問わず、同じ`VertexId`を持つ全ての面組について現在world座標を全ポーズ検証する。これには共有ヒンジ両端も含む。局所形状尺度から作るtopology marginを超えて不一致なら、AABBが重ならず広域候補が0件でも、その面組を明示的な`indeterminate` interactionとして返す。さらに、全adjacency面組を共有`VertexId`の有無とは独立に照合し、一つでも広域候補に存在しなければ、共有軸identityを狭域で認証できないため同じく`hinge_adjacent`の`pose_mismatch`とする。非隣接面組は`relation = non_adjacent`、共有ヒンジ面組は`relation = hinge_adjacent`、該当`hingeEdgeIds`および`hingeDecision.reason = pose_mismatch`を保持する。共有点がmargin内なのに当該面組が保守的広域候補に入っていない場合も、共有点と分離が同時成立した内部矛盾として`indeterminate`とする。同期解析、resumable解析およびone-shot互換経路は同じ結果を返し、全走査witness補正はこのトポロジー不正姿勢を入力として受理しない。

### 5.2 各evidenceの生成条件

#### `separated`

対象三角形対の閉じた材料領域が交わらないことを、marginを超える分離軸または同等の確定的な空隙証明で示す。候補全体を`separated`とするには、必要な全三角形対が離間していなければならない。margin内の正の隙間、走査漏れまたは計算不確実性を離間へ丸めてはならない。

#### `point_contact`

完全な交差集合が一点だけであり、その点が両材料領域の境界にあり、正長の線、面内部の横断、正面積および正体積を持たないことを証明する。共有identityを必要としない一般接触である。

#### `boundary_line_contact`

完全な交差集合が正の長さを持つ境界線分だけであり、両材料領域の内部へ入らず、共面正面積または正体積を持たないことを証明する。線分の一部でも面内部を横断する場合はこの証拠を生成してはならない。

#### `shared_feature_contact`

認証済み共有関係に対応し、完全な交差集合が次のいずれかだけであることを証明する。

- 共有頂点では、その一頂点だけ
- 共有ヒンジ辺では、その有限な共有辺上の境界接触だけ

共有要素の外に一点でも交差がある場合は生成禁止である。共有関係が`no_shared_feature`であるのにこの証拠が到着した場合、完全表は`indeterminate`へ退避する。

#### `shared_feature_thickness_overlap`

`t > 0`かつ`centered_mid_surface_v1`の場合だけ生成できる。押し出した三角柱同士に重なりがあっても、両方の材料中央面を再検査し、その完全な中央面交差が認証済み共有要素だけであることを先に肯定証明しなければならない。

- 共有頂点では、6節のexact shared-vertex-only certificateが必須である。
- 共有ヒンジ辺では、完全表の結果は`requires_hinge_model`に留まり、7節の有限corridor証明を別に完了しなければならない。
- 共有要素の外に中央面横断または体積重なりを証明した場合は、この証拠ではなく`transversal_crossing`または`positive_volume_overlap`とする。

正厚判定の正本は常に中央面である。共有IDだけを見て、通常のSAT正体積重なりをこの証拠へ付け替えてはならない。

#### `shared_feature_flat_stack`

認証済み共有ヒンジ辺、有限軸、対向材料半平面および完全な三角形対走査の内側で、`t = 0`の厳密な180度平坦折りに由来する積層だけであることを証明した場合に生成できる。非隣接面の共面正面積重なり、共有頂点だけの平坦重なり、有限ヒンジ範囲外の重なりには生成禁止である。

`t > 0`の180度では層ずらしが未実装であるため、この証拠を生成せず`indeterminate`とする。

#### `coplanar_area_overlap`

二つの中央面が共面であり、その面内部同士の交差がmarginを超えて正の二次元面積を持つことを証明する。認証済み共有ヒンジの`t = 0`平坦積層として有限範囲全体を別途証明した場合だけ`shared_feature_flat_stack`を用いる。それ以外の共面正面積は共有IDの有無にかかわらず`penetrating`である。

#### `transversal_crossing`

非平行な中央面が、点だけではなく正の長さを持って互いの内部を横断することを証明する。許容差で不明な場合のexact binary64証明は、両三角形が非退化・非平行で、各三角形が相手平面を厳密な正負符号でまたぎ、平面交線上の閉区間が正の長さで重なることを必要とする。共有点だけの接触はこの条件を満たさない。共有頂点から離れた部分を横断する場合は、共有頂点があっても必ずこの証拠を生成する。

近平行な浮動小数点法線が離間を示しても、それだけで`separated`を確定してはならない。さらに、三角形の最大辺に対する最小高度が小さい場合はsection区間計算の誤差をcondition ratioで拡大評価する。近平行または悪条件によって浮動小数点の離間証明が不安定なpairは、`separated`を返す前にexact transversal certificateへ渡し、証明できなければ`indeterminate`とする。

#### `positive_volume_overlap`

`t > 0`の二つの材料三角柱の内部交差が、全ての必要な分離軸上でmarginから独立した正の重なりを持ち、三次元の正体積を持つことを証明する。認証済み共有要素だけに由来する中央面基準重なりとして先に完全証明した場合を除き、共有IDの有無にかかわらず`penetrating`である。

SAT射影はpairに共通する決定論的な局所原点を使用し、共通平行移動による巨大な絶対座標を射影値から除く。面内辺方向と押し出し方向は、変換後world頂点同士を再減算せず、canonicalなrest三角形順と検証済み剛体変換の線形成分から作る。これによりpolygonの開始頂点・winding・平行移動をSAT軸生成へ混入させない。pair用marginは、局所寸法に基づくmarginと保存済みworld座標の有限ULP誤差を合わせ、解析全体の保守的marginを上限とする。

分離軸上の重なりが保存値で厳密に0でも、その軸成分について絶対world座標由来のULP誤差が局所marginを支配する場合、その0は正の隙間または正の重なりが量子化で潰れた可能性を持つため`touching`へ確定せず`indeterminate`とする。正規化軸`a`とpair各座標成分の最大絶対値`Sx / Sy / Sz`から、軸別上界を`2 × EPS × (|a.x|Sx + |a.y|Sy + |a.z|Sz)`として計算する。これはpair用の保守的world rounding marginとは別の、exact-zero証拠だけを拒否する境界である。この軸別上界が局所margin以下である厳密0だけを`touching`証拠にできる。符号付き隙間または正重なりが0ではないがpair用margin内の場合も`indeterminate`とし、全必要軸でpair用marginを超える正重なりを持つ場合だけ`penetrating`とする。

#### `indeterminate`

他の証拠を肯定証明できず、かつ貫通も否定できない場合に生成する。少なくとも次を含む。

- margin上またはmargin内の曖昧な隙間・重なり
- 近平行、退化、非有限値、座標精度の枯渇
- トポロジー、姿勢、三角形対geometryまたは共有ヒンジ制約の不一致
- 三角形対走査の未完了、重複、欠落
- exact certificateの作業上限超過
- 有限ヒンジcorridorの境界を内外いずれにも証明できない状態
- 正厚180度、または有限半径条件を満たさない`layer_offset_unmodeled`

`indeterminate`は「衝突なし」でも「接触許容」でもない。

## 6. shared-vertex-only exact証明

共有頂点固有の許容を与えるには、共有identityと幾何学の両方を次の手順で肯定証明する。

1. 二つの三角形に、保存済み座標まで一致する真正な共有`VertexId`がちょうど一つあることを確認する。IDが同じで座標が違う入力は拒否する。
2. 現在姿勢の二つの共有点が、局所形状尺度だけから作る`topologyMargin = min(globalMargin, localScale × EPS × 256)`内で一致することを確認し、同じcanonical binary64座標へ固定する。絶対world座標のULPは共有identityの認証に使用しない。したがって、同じ`VertexId`でも局所marginを超える非ゼロのworld位置差があれば、巨大な共通平行移動後も共有頂点許容へ進めず`indeterminate`へ退避する。
3. 三角形の全binary64座標を、共通の`2^-1074`単位を持つBigInt dyadicとして厳密にsnapshotする。
4. 指定頂点が厳密に一致し、両三角形が非退化、両平面が非平行で、指定頂点が両平面上にあることを証明する。
5. 厳密な平面交線へ各三角形の閉じたsectionを射影し、二つのsection区間の共通部分がちょうど一つのsingletonであり、その値が指定共有頂点の射影と厳密に等しいことを証明する。
6. 各面の検証済み剛体変換のlocal `+Y`から材料中央面法線を作り、二法線の内積がdimensionless境界`1e-10`より厳密に大きいことを証明する。三角形の開始頂点やwindingから法線向きを作り直してはならない。内積が境界以下、0付近、負または非有限なら、singletonだけが真でも`indeterminate`とする。
7. exact singletonと法線条件を満たした同じ呼出しだけがprivate geometry certificateを発行する。certificate identityを`WeakSet`で、発行時の第1・第2`TrianglePrism`参照と厚さ状態をprivate `WeakMap`で結合する。最終runtime evidenceもraw分類へ別途結合し、clone、左右入替、別prism、別raw分類または矛盾した`separated`での再利用を`indeterminate`へ倒す。

同側符号に退化したsingleton sectionも、上の厳密条件を満たす場合は扱える。exact singletonとco-oriented材料法線の双方を結合したcertificateが真正である場合だけ、`shared_feature_contact`または正厚時の`shared_feature_thickness_overlap`を生成できる。exact singletonだけでは初版の停止除外に十分ではない。

certificateが偽であることは、ただちに離間または貫通を意味しない。別の肯定証明で`transversal_crossing`、一般の点・境界接触、離間または正体積を確定できなければ`indeterminate`とする。共有`VertexId`だけで許容へ進めてはならない。

exact intersection certificateは一解析あたり最大256 attemptとする。上限後はexact helperを呼ばず、証明を必要とする当該三角形対を`indeterminate`とする。ただし、別の三角形対でmarginに依存しない`penetrating`を肯定証明できた場合は、8節のseverityに従い貫通へ昇格してよい。

## 7. 共有ヒンジの有限corridor

`requires_hinge_model`は未完了decisionであり、次の全条件を満たした場合だけ最終的な共有ヒンジ許容へ進める。

実行時は、共有ヒンジモデルが発行した`allowed_by_hinge_model`だけから、`boundary_contact`を`shared_feature_contact`、`corridor_overlap`を`shared_feature_thickness_overlap`、`flat_surface_stack`を`shared_feature_flat_stack`へ写像する。private runtime capabilityを元のhinge decision object、単一`EdgeId`組および集約前geometry classへ結合し、4×10表が`requires_hinge_model`を返すことを再確認してから面集約へ渡す。文字列だけの`requires_hinge_model`、別decision、複数辺、証拠種別不一致またはcapability不一致は`indeterminate`である。

### 7.1 トポロジーと姿勢の認証

- 対象面組に共有ヒンジ`EdgeId`がちょうど一つあり、一意な隣接recordとconstraintが対応する。
- constraintの左右面、開始・終了`VertexId`、保存済み座標および`centered_mid_surface_v1`が一致する。
- 共有辺は両面の実際のpolygon境界に一度ずつ現れ、境界方向が互いに逆である。
- 両面の第三頂点および材料三角形は、共有辺に対して互いに反対の材料半平面側にある。
- 両面の現在変換は剛体であり、変換後の開始点同士と終了点同士が、局所形状尺度だけから作る`topologyMargin = min(globalMargin, localScale × EPS × 256)`内で一致する。`localScale`は有限ヒンジ長・紙厚・保存済み両端点spanから求める。絶対world座標のULPと広域候補用global marginを共有軸identityの認証に使用しない。
- corridorの射影・距離計算には、既にbinary64座標へ保存された大座標丸めを覆う別の`numericalMargin = min(globalMargin, max(localScale × EPS × 256, worldScale × EPS × 32))`を用いる。`worldScale`は変換後両端4点の絶対座標から求める。この数値marginは共有端点の一致を認証する権限を持たない。
- 現在の共有軸は有限かつ非退化で、両中央面法線はその軸に直交する。
- 入力三角柱は、保存済み面、三角形index、姿勢変換および紙厚から再構成した頂点と一致する。偽造または重複したpairを受理しない。
- 期待される全三角形対を走査する。走査未完了のまま許容してはならない。

公開境界のgeneric `Matrix4`だけでは、巨大pivot回転で生じた共有端点のbinary64丸め差と、実在する軸ずれを区別できない。このため絶対world座標の精度が枯渇し、真正な共有ヒンジでも端点差が`topologyMargin`を超えた場合は、共通平行移動前と同じ許容結果を推測せず`pose_mismatch`による`indeterminate`へ退避する。将来、kinematics provenanceまたは相対変換certificateを解析境界へ渡せる場合に限り、この巨大pivotを別途再認証できる。

### 7.2 有限軸・半径条件

共有ヒンジの有限長を`L`、紙厚の半分を`h = t/2`、変換後の左右中央面法線間の角度を`θ`とする。正厚時の解析corridor半径は次である。

```text
R = h / cos(θ / 2)
```

`R`が有限であり、閉境界`R <= L`を証明できなければならない。姿勢誤差を含む`outerMargin`は、上記のcorridor用`numericalMargin`に、`topologyMargin`内で一致を認証済みの両端姿勢誤差を加えた値とし、各交差集合のcorridor内外を数値分類するためだけに用いる。有限半径上限を`L + outerMargin`へ拡張してはならない。したがって、共通平行移動で絶対world座標由来の数値marginが増えても、実在する軸ずれを一致へ変えたり、`R > L`を許容へ反転したりしない。corridorは無限直線ではなく、軸方向`[0, L]`と半径`R`に限定する。

各交差三角形対について、対向材料half-slab内であることと、交差集合が有限軸範囲および半径corridor内にあることを証明する。少なくとも片方の元三角形が両端間へ完全に収まる場合、その三角形対の交差集合も軸方向に収まる。両方が端点外へ伸びる場合は射影区間の共通部分を検査する。corridor境界を内外いずれにも確定できない場合は`indeterminate`である。

全pairについて次の優先順位を守る。

1. corridor外の貫通が一つでもあれば`outside_hinge_penetration`
2. 貫通がなくても未解決pairがあれば`indeterminate`
3. 未解決がなくcorridor外接触があれば`outside_hinge_contact`
4. 全相互作用が認証済み有限corridor内にある場合だけ`allowed_by_hinge_model`

したがって、共有ヒンジモデルはcorridor外貫通または未解決pairを消去する免除ではない。

### 7.3 角度上限と平坦積層

- `t = 0`かつ通常角: 検証済み有限共有軸上の接触を`boundary_contact`として許容できる。
- `t = 0`かつ左右法線がbinary64上で厳密に反平行: 検証済み有限範囲内の三角形対について共面の正面積重なりが肯定証明された場合だけ、`flat_surface_stack`として許容できる。180度近傍、点・線接触、または共面正面積を確定できない場合は昇格しない。
- `t > 0`かつ0度: 平坦な境界接触を`boundary_contact`として扱える。数値SATが貫通を返して解消できない場合は`flat_pose_penetration`による`indeterminate`とする。
- `t > 0`かつ`0 < θ < 180度`: 全条件を満たす有限領域だけを`corridor_overlap`として許容できる。
- `t > 0`かつ厳密180度、数値的なflat-fold特異域、または`R > L`: 無限corridorへ拡張せず、`layer_offset_unmodeled`による`indeterminate`とする。

正厚の層ずらし、層順、材料変形および実際の折り癖は未実装である。180度を安全に通過または到達できると推測してはならない。

この静的corridor証明は一姿勢だけの証明であり、連続運動の安全証明ではない。連続判定器は同じ共有軸を経路全体で再認証し、全区間で有限半径条件を満たすことを別途証明する。正厚経路の区間が180度特異点へ到達する場合、その区間は認定しない。

## 8. triangle-pair severity集約とUI

一つの面組に属する三角形対のraw geometry severityは、次の全順序で集約する。

```text
penetrating > indeterminate > touching > separated
```

- 一つでも`penetrating`なら面組は`penetrating`である。
- `penetrating`がなく、一つでも`indeterminate`なら面組は`indeterminate`である。
- 上二つがなく、一つでも`touching`なら面組は`touching`である。
- 全三角形対が`separated`の場合だけ相互作用なしとする。

非ヒンジ面組は、貫通を肯定証明した時点で残りのpairを省略してもseverityは変わらない。共有ヒンジ面組は有限corridorの完全性に全pairが必要なため、貫通pairを見つけても全pairを走査する。作業上限または走査未完了は、既により強い貫通証明がある場合を除き`indeterminate`とする。`same_face`は集約前に`ignored_self`として除外する。

共有ヒンジではraw geometryが`penetrating`でも、有限corridorの全証明を完了した場合に限り、別の`hingeDecision`で`corridor_overlap`または`flat_surface_stack`を許容表示できる。raw severityを書き換えて、corridor外貫通または未解決を隠してはならない。

`indeterminate`は貫通と同じblocking riskとして扱う。UIは次を満たさなければならない。

- 文言に「交差の可能性・判定保留」と「安全確認が必要」を含める。
- `layer_offset_unmodeled`では「層ずらし未再現のため判定不能」「貫通許可なし」を明示する。
- 現在姿勢のbadgeは、貫通と同じ高危険度の赤系border・background・太字を用いる。
- 3D上の該当面outlineは専用の判定保留色で保持し、非表示または安全色にしてはならない。
- clear、接触許容またはモデル許容の件数へ混入させてはならない。

`allowed_shared_vertex_contact`は通常接触・貫通・判定保留とは別の情報状態として「共有頂点の許容接触・貫通0」と表示する。連続判定から除外できるのは、解析器が発行したexact interaction objectとしてprivate capabilityに登録され、かつpolicy version、真正な共有頂点ID、pair件数、raw touching/penetrating/indeterminate件数および`exclusive: true`を再検証できた内部snapshotだけである。構造が同じclone、JSON往復、偽造object、欠損、重複、矛盾、例外を生じる入力は許容せずblocking側へ戻す。

## 9. 必須回帰matrix

回帰は入力順を変えても同じdecisionになり、各厚さ値を表示用厚さではなく材料厚として扱わなければならない。

### 9.1 角起点の山折り・谷折りV

中央面を固定し、一方を山折り、他方を谷折りとする。非隣接の外側2面は角の一共有頂点だけを持つ。次の15姿勢で、外側面組の期待値を全て`touching`、非隣接貫通件数を`0`、判定保留件数を`0`とする。

| 材料厚 | 左だけ10度<br>右0度 | 左0度<br>右だけ10度 | 両側45度 | 両側91度 | 両側135度 |
| --- | --- | --- | --- | --- | --- |
| 0 mm | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 |
| 0.1 mm | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 |
| 1 mm | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 | `touching` / 貫通0 |

正厚ケースの許容は、共有頂点identityではなく、完全な中央面交差がその一頂点だけであるという6節のexact certificateに依存する。

### 9.2 辺中点起点の山折り・山折りV

400×400 mm紙の一辺中点から遠い2角へ山折りを引き、中央面を固定して外側2面を互いに反対符号で回転する。90度と91度ではexact中央面交差が共有頂点singletonでも材料法線のco-oriented条件を証明できないため、許容へ進めずblockingの`indeterminate`とする。横断開始は約`104.477512度 = acos(-1/4)`であり、135度と179度ではexact transversal certificateにより`penetrating`とする。

| 材料厚 | 両側90度 | 両側91度 | 両側135度 | 両側179度 |
| --- | --- | --- | --- | --- |
| 0 mm | `indeterminate` | `indeterminate` | `penetrating` | `penetrating` |
| 0.1 mm | `indeterminate` | `indeterminate` | `penetrating` | `penetrating` |
| 3 mm | `indeterminate` | `indeterminate` | `penetrating` | `penetrating` |

90度と91度の判定保留は「安全」でも「許容接触」でもない。同期解析、resumable解析、正厚full scan、面入力順の正逆およびUI集計で同じblocking結果を必要とする。

### 9.3 共有点外の横断

二面が一つの真正な共有頂点を持っていても、その頂点から離れた部分で中央面が横断するfixtureは次を満たす。面入力順を反転しても同じである。

| 材料厚 | 期待evidence | 期待decision |
| --- | --- | --- |
| 0 mm | `transversal_crossing` | `penetrating` |
| 0.1 mm | `transversal_crossing`。正厚では正体積も伴う | `penetrating` |
| 1 mm | `transversal_crossing`。正厚では正体積も伴う | `penetrating` |

このmatrixは、共有`VertexId`が貫通の包括免除にならないことを固定する。

### 9.4 近平行・悪条件の横断

厚さ0でexact binary64横断が真となるpairは、浮動小数点の近平行分岐またはsection区間が離間を返しても`penetrating`とする。scale 1/400/1,000,000、面入力順の正逆を固定し、近平行fixtureは各scale 2組、最大辺/最小高度が約`6.67 × 10^9`の悪条件fixtureは角度90/179度を含む各scale 4組で回帰する。各セルはexact attempt 1、非隣接`penetrating` 1を必要とする。

### 9.5 正厚平行slabの符号境界と平行移動

厚さ0.1/1/3 mm、共通X平行移動0/`10^12`の全6組で、同じ平行slab pairを次の5セルに分類する。

| normal方向の関係 | 期待 |
| --- | --- |
| 厳密な接触 | `touching` |
| margin内の正重なり | `indeterminate` |
| marginを超える正重なり | `penetrating` |
| margin内の正の隙間 | `indeterminate` |
| marginを十分超える正の隙間 | 相互作用なし |

さらに、全厚さで0.01 mmの正体積重なりは原点とX=`10^12`で同じ`penetrating`とし、巨大な共通平行移動だけで`touching`へ変化させない。

world座標精度の枯渇回帰として、scale 400の同一三角形slabを全6頂点順×全6頂点順×面入力順正逆で検査する。共通`Rz(135度)`とX=`10^15`、厚さ0.1 mm、中央面間隔0.075 mmの正重なり0.025 mm、および共通`Rx(45度)`とXYZ=`10^15`、厚さ3 mm、中央面間隔3.25 mmの正の隙間0.25 mmは、保存射影値が0へ潰れても全72順を`indeterminate`とし`touching`へ確定しない。

SAT軸の入力順不変回帰として、scale 1の同一三角形slab、共通`Rx(45度)`、厚さ1 mm、中央面間隔0.75 mmの正重なり0.25 mmを、共通平行移動0 / `10^12` / `3 × 10^12`、全6頂点順×全6頂点順×面入力順正逆で検査する。剛体変換のlocal-Y basisから押し出し方向を作り、全216順を`penetrating`とする。world頂点の減算誤差で近平行軸を捏造して`indeterminate`へ分岐してはならない。

### 9.6 共有identityの平行移動不変性とexact予算枯渇

- 真正な共有頂点だけで接するpairは、共通平行移動0、`10^12`、`3 × 10^12`、`10^15`と面入力順の正逆で、許容または安全側の`indeterminate`となり、通常貫通へ誤昇格しない。
- 同じ`VertexId`でも現在共有点を0.1 mmずらしたpairは、上記全平行移動で`allowed_shared_vertex_contact`にならない。大座標のbinary64丸めでずれを確定できない場合も、許容ではなく`indeterminate`へ退避する。
- 現在共有点を100 mm離して両面AABBも離したpairは、厚さ0/0.1/3 mm、面入力順の正逆で広域候補0件のままでも、全ポーズ検証が一つの非隣接`indeterminate` interactionを発行する。同期、resumable、one-shotは同値であり、補正用full scanは`null`として拒否する。
- 共有ヒンジの一方の現在軸を100 mm離して両面AABBも離したpairも、厚さ0/0.1/3 mm、面入力順の正逆で広域候補0件のまま、一つの`hinge_adjacent`かつ`pose_mismatch`の`indeterminate` interactionを発行する。同期、resumable、one-shotは同値であり、補正用full scanは`null`として拒否する。
- adjacencyだけが存在し、左右面で共有`VertexId`を一つも認証できず、片面を100 mm離してAABB候補も0件にしたpairも、厚さ0/0.1/3 mmと面入力順の正逆で前項と同じ`pose_mismatch`を発行する。共有ID欠損を「ヒンジ候補なし」へ読み替えない。
- adjacency面組が広域候補には入るが全三角形対で離間するfixtureは、厚さ0/0.1/3 mmと面入力順の正逆でinteractionを省略せず、`missing_constraint`の`indeterminate`を発行する。
- 真正な共有辺で接触する面組でもヒンジ制約を渡さないfixtureは、厚さ0/0.1/3 mmと面入力順の正逆でraw分類を安全許容へ使わず、`missing_constraint`の`indeterminate`を発行する。同期、resumable、one-shotは全て同値とする。
- 共有ヒンジの一方の現在軸を0.1 mmずらしたpairも、X軸だけおよびXYZ全軸への`3 × 10^12`、`10^15`平行移動後を含めて`pose_mismatch`または`indeterminate`となり、有限corridor許容へ進まない。
- generic `Matrix4`から作った真正な共有ヒンジも、巨大pivotの丸め差で`topologyMargin`内の同一性を証明できなくなった場合は`hinge_pose_mismatch`によるblockingを許容する。これは平行移動不変な許容decisionの一般保証ではなく、world精度枯渇時の安全側退避である。
- 共有頂点のみのexact証明を257 pair必要とする解析は、走査順の正逆と同期・resumable実行で、最初の256 pairだけを許容し、残る1 pairを`skippedByLimit = 1`かつ`indeterminate`とする。予算枯渇をraw正厚重なりの`penetrating`へ付け替えず、UIに明示的な判定保留を残す。

## 10. 初版の非対象

`topology_contact_policy_v1`と`centered_mid_surface_v1`は、現在姿勢の中央面基準解析を定める。次は安全証明の対象外である。

- 厳密な層ずらし、層順および多層の積層順序
- 紙の塑性・弾性変形、実際の折り癖、圧縮および摩擦
- 複数ヒンジの同時運動、閉ループ一般経路
- 静的な一姿勢の判定だけから導く連続経路安全性

これらを未実装のまま共有接触許容へ読み替えてはならない。必要な証明が本書の範囲を超える場合は`indeterminate`として停止する。
