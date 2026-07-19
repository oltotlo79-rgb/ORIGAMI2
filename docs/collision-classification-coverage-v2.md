# 衝突分類 v2 実装カバレッジ台帳

## 1. 文書の目的

本書は、[`topology_contact_policy_v2`](collision-contact-policy-v2.md)の純粋な4×11 decision表と、そのdecisionへ入力するnative幾何証拠生成の完成度を分離して追跡する非規範の実装台帳である。純粋表が全44セルを返せることと、現在姿勢から全11種の証拠を発行できることを同じ「完成」と数えてはならない。

折り重ねは衝突分類を停止判定の正本として使用する。したがって、本書の「折り重ね前ゲート」が全て完了するまで、折り重ねUIを分類器より先に公開しない。

## 2. 4×11純粋表

次の三者は全44セルを一対一で照合済みである。

- 正規corpus: [`collision-contact-policy-v2.json`](collision-contact-policy-v2.json)
- native公開境界: [`ori-collision`](../crates/ori-collision/src/lib.rs)
- frontend公開境界: [`foldPreviewTopologyContactPolicy.ts`](../apps/desktop/src/lib/foldPreviewTopologyContactPolicy.ts)

native unit testとfrontend testは、4共有関係と11証拠の直積を正規corpusへ照合する。さらにnativeの公開APIだけを使う[`topology_contact_policy_v2_matrix.rs`](../crates/ori-collision/tests/topology_contact_policy_v2_matrix.rs)は、軸の重複、行の欠落、未知decision、重複セルを拒否し、`relation::evidence`の44個のcanonical cell IDが一度ずつ現れることを固定する。runtimeへ`same_face`が到達した44表上の11セルは、`ignored_self`を認可に使わず全て`indeterminate`へ閉じる。

結論: **純粋な4×11表は完成している。幾何証拠生成は未完成である。**

ここでいうfrontend完成は純粋decision表と正規corpusの一致に限る。現行の
`foldPreviewNarrowCollision` production dispatcherと、そのprivateな共有頂点・
共有ヒンジcertificateは、凍結済み`topology_contact_policy_v1`へ束縛されたままで
ある。v1 certificateのversion文字列や呼出関数だけをv2へ差し替えてはならない。
正厚の正面積・正体積0を肯定証明する`boundary_area_contact`生成器、v2専用issuer、
別厚さ・別姿勢・別solveへの再結合拒否が完成するまでは、frontend production経路を
v2実装済みと数えない。証拠不足またはversion不一致は`indeterminate`へ閉じる。

## 3. native幾何証拠の到達性

| 交差証拠 | 純粋表4セル | native現在姿勢からの生成 | 現在の回帰 | 残作業 |
| --- | --- | --- | --- | --- |
| `separated` | 完了 | 厚さ0で実装 | exact三角形、面交換、頂点順 | 正厚full scanへ統合 |
| `point_contact` | 完了 | 厚さ0で実装 | 一般点、共有点以外、subnormal | 正厚境界との統合 |
| `boundary_line_contact` | 完了 | 厚さ0で実装 | 実外周、部分共有辺、人工対角線除外 | 正厚境界との統合 |
| `boundary_area_contact` | 完了 | 未実装 | 純粋表だけ | 正厚閉三角柱の正面積・正体積0証明 |
| `shared_feature_contact` | 完了 | 厚さ0の共有頂点・共有辺で実装 | exact singleton、完全共有辺、誤った共有点/部分辺拒否 | watertight tree姿勢と有限ヒンジへ結合 |
| `shared_feature_thickness_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚中央面再証明とprivate provenance |
| `shared_feature_flat_stack` | 完了 | native未実装 | frontend現行経路のみ | 厚さ0・厳密180度・有限ヒンジ範囲の証明 |
| `coplanar_area_overlap` | 完了 | 厚さ0で実装 | 共有なし、共有頂点、共有ヒンジ、180度 | watertight tree姿勢へ結合 |
| `transversal_crossing` | 完了 | 厚さ0で実装 | exact binary64、近平行、悪条件、共有点外横断 | watertight tree姿勢と正厚full scanへ結合 |
| `positive_volume_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚三角柱SATの肯定証明 |
| `indeterminate` | 完了 | 厚さ0で実装 | 退化、作業上限、共有姿勢不一致、同一面到達 | 正厚・有限ヒンジ・連続経路の全失敗理由 |

`no_shared_feature`で共有要素専用証拠が来るセル、共有identityと`separated`が同時成立するセル、`same_face`の全セルなどは、幾何生成の成功fixtureを作る対象ではない。これらは矛盾または走査前除外を表すため、純粋表とruntime fail-closed回帰で固定する。

## 4. 利用者報告回帰

| 回帰 | frontend現在経路 | native厚さ0基盤 | native正厚production |
| --- | --- | --- | --- |
| 角起点の山谷V: 厚さ`0 / 0.1 / 1 mm`×`片側10度左右2通り / 両側45 / 91 / 135度`の15姿勢 | 全15姿勢を`allowed_shared_vertex_contact`・貫通0で回帰 | `10/0, 45/45, 91/91, 135/135`を回帰。右だけ10度はfrontendでのみ固定 | 未実装 |
| 報告A: 厚さ0、片側10度 | 共有頂点許容・貫通0 | exact共有頂点接触を回帰 | 対象外 |
| 報告B: 厚さ0、両側180度 | 共面正面積・貫通1 | `coplanar_area_overlap`・`penetrating`を回帰 | 対象外 |
| 辺中点の山山V: 厚さ`0 / 0.1 / 3 mm`×`90 / 91 / 135 / 179度` | 90/91度は`indeterminate`、135/179度は`penetrating`で全12姿勢を回帰 | 4角度を走査するが、現行binary64 tree姿勢の共有点不一致により全て`indeterminate`。135/179度は正式期待値へ未到達 | 未実装 |
| 共有点外の横断: 厚さ`0 / 0.1 / 1 mm` | 全9姿勢を`penetrating`で回帰 | 厚さ0 exact横断を回帰 | 未実装 |

nativeの山山V 135/179度が`indeterminate`であることは、安全側の一時退避であり、正式期待値の達成ではない。`rational_cayley_local_rotation_v1`単体だけでこの行を完了扱いにせず、issuer-bound tree全体のwatertight姿勢へ合成してから`transversal_crossing`を再証明する。

## 5. 折り重ね前ゲート

次を順番に完了する。

1. 有理Cayley局所回転を同一issuerのtree traversalへ合成し、全共有頂点と共有ヒンジ端点がexactに一致するwatertight姿勢を作る。privateなcanonical BFS合成と完全一致検査は実装済みだが、renderer有限誤差包含、deep-chain stressおよび厚さ0classifierへの接続が未完了のため、このゲート全体は継続中とする。
2. 山山Vの厚さ0・135/179度を、pose mismatchではなく`transversal_crossing`・`penetrating`としてnativeで証明する。
3. 正厚の`boundary_area_contact`、`shared_feature_thickness_overlap`、`positive_volume_overlap`と有限ヒンジの`shared_feature_flat_stack`をnativeで実装する。
4. 角起点V、山山V、A/B、共有点外横断をnative production proofとdesktop current-pose certificate経路で回帰する。
5. `indeterminate`を貫通同等のblocking表示と停止へ結合し、全pair coverageとwork limitを維持する。

このゲート完了後に限り、層順序transport、atomicな折り重ねcommand、最後に折り重ねUIへ進む。

## 6. watertight exact tree pose checkpointの合格契約

### 6.1 private authorityとcertificate

`rational_cayley_local_rotation_v1`の局所回転が成功したことだけでは、tree pose完成とはみなさない。次の主張を一つのprivate authorityへ同時に結合する。

| certificateの主張 | 必須の肯定証明 | 失敗時 |
| --- | --- | --- |
| issuer | `MaterialTreeKinematicsModel::bind_pose`で同じprivate `PreparedTree` issuerを確認する | foreign model/poseとして拒否 |
| pose instance | 元の`MaterialTreePose`と`same_instance`で一致し、fixed face、canonical `(EdgeId, angle bits)`全件を保持する | 同角度再solveを含めABA拒否 |
| source completeness | canonical FaceId、EdgeId、VertexId、面境界、hinge、隣接および角度を期待数どおり一度ずつ再走査する | 部分authorityを発行せず拒否 |
| local rotations | 各hingeが同じ呼出しで発行されたCayley angle certificate、回転符号、軸両端およびworkを持つ | fallbackのbinary64局所回転を混入せずblocking |
| exact tree composition | root恒等変換と、全tree edgeの`T_child = T_parent ∘ L_hinge`を有理算術で再検証する | 全authorityを破棄 |
| watertight geometry | 全共有VertexIdの全face出現、共有hinge両端および有限共有辺が厳密に一致する | `pose_mismatch`のままblocking |
| renderer containment | 同じpose instanceのbinary64観測姿勢がexact poseの明示的な有限誤差包含内にある | exact衝突authorityと描画の対応を発行しない |
| resource completeness | 局所証明、tree合成、共有点走査、renderer照合の実workとversion固定上限を保持する | 一件でも上限超過なら原子的に失敗 |

authority、exact transform、BigRational、angle certificateおよびworkの内部値にはpublic constructorを設けず、永続化・Deserializeを禁止する。cloneは同じprivate identityを保持してよいが、同値入力からの再発行は別identityとする。文字列のmodel ID、公開FaceId/EdgeId、同値matrixまたは同値角度だけで再構築できてはならない。

現行APIでは`BoundMaterialTreePose`からcanonical face/hinge registry、fixed face、完全角度列および面境界をissuer付きで取得できる。rootからEdgeId順のBFSを行い、各stepの符号を次で固定する。

```text
assignment_sign = mountain:+1 / valley:-1
direction_sign  = left→right:+1 / right→left:-1
rotation_sign   = assignment_sign × direction_sign
```

非root faceとhingeをそれぞれ正確に`F - 1`回、一度ずつ訪問する。単一面はrootなし・0 stepとする。Observation pose、caller座標embeddingまたは公開IDだけからこの走査authorityを発行してはならない。

### 6.2 全fixture共通property

全ての成功fixtureで、許容差ではなくBigRationalの完全一致として次を検証する。

1. root faceのexact transformは恒等変換である。
2. 各face transformは`RᵀR = I`、`det R = 1`を満たし、各親子について`T_child = T_parent ∘ L_hinge`である。
3. 各hingeのstart、endおよび有理midpointを、親face、子face、hinge parent transformから写した3経路が同一点になる。
4. 同じVertexIdを持つ全face occurrenceを全組比較し、一点でも異なればauthorityを発行しない。直接隣接しない角起点V字の外側2面も省略しない。
5. 同じfaceの全頂点は一つのexact affine plane上にあり、材料法線はcanonical local `+Y`のexact像から作る。
6. 全face、全hinge、全共有vertex occurrence、全unordered face pairの期待数と解析数が一致する。
7. 同じ入力の再実行はexact transform、angle certificate、workおよびcanonical traversalがbit-for-bit同じになる。
8. sourceのface/edge/vertex格納順、境界cycle開始、edge方向、topology incidence/adjacency格納順を変えても、issuer identityは別のまま、観測可能なexact geometryとcollision decisionは同じになる。
9. fixed faceを別faceへ変更したposeは同一authorityではないが、基準root姿勢から一つのexact global rigid transform`G`を取り、全faceについて`T_base(face) = G ∘ T_reroot(face)`となる。全pairの交差証拠とdecisionも同じになる。
10. nativeの`MaterialTreePose` binary64 snapshotは、同じissuerとpose instanceへ束縛したexact poseに対し、全face境界の各成分についてversion固定の有限誤差包含を持つ。誤差上限は局所形状、tree深さ、各angle certificateおよびbinary64演算回数から導出し、任意に大きな包含を許可しない。巨大座標で上限内を証明できない場合は安全集合を広げない。現在のThree.js rendererは別経路で姿勢を再計算するため、このnative包含だけをrenderer完了証明とみなしてはならない。

### 6.3 既存fixtureの再利用

| 既存fixture/test | 既に固定している性質 | exact tree checkpointで追加するassertion |
| --- | --- | --- |
| `cayley::zero_and_half_turns_are_exact`、`square_axis_ninety_degree_rotations_are_exact` | 0/90/180度の局所代数枝 | tree合成後も0度恒等、180度half-turn、全共有点一致 |
| `cayley::nonsquare_axis_and_deep_angles_have_strict_certificates`、angle boundary/property matrix | 非平方軸、subnormal、90度±1 ULP、179度、180度直前、符号・軸反転 | 同じ角度集合を2hinge treeへ通し、certificateを失わず合成 |
| `corner_v_model_and_pose` | 3面、共通頂点、斜め2hinge、山谷、外側面は頂点だけを共有 | 全共有頂点のexact一致、左右どちらだけ10度も許容、180度の共面正面積貫通 |
| `authenticated_corner_v_shared_vertex_stays_nonpenetrating_across_reported_angles` | `10/0, 45/45, 91/91, 135/135`の偽陽性防止 | `0/10, 0/0, 90/90, 179/179`を追加し、全rootと入力順で同じ結果 |
| `noncardinal_slanted_hinge_disagreement_is_explicitly_indeterminate` | binary64斜めhingeの端点不一致を安全側へ退避 | exact authority経路では37度hingeの両端・midpointが一致し、隣接pairは`SharedFeatureContact / RequiresHingeModel`。旧非authority経路の判定保留も残す |
| `authenticated_corner_v_at_full_fold_reports_real_area_overlap` | 角起点V字・厚さ0・180度の共面正面積 | exact half-turn authorityでも`CoplanarAreaOverlap / Penetrating` |
| `midpoint_mountain_model`と`midpoint_mountain_pair_is_explicitly_indeterminate_until_watertight_pose` | 3面、辺中点からの山山、90/91/135/179度を現状は全てpose mismatchへ退避 | 90/91度だけを非pose-mismatchの`Indeterminate`にし、135/179度を`TransversalCrossing / Penetrating`へ変更 |
| `non_commuting_fixture`と`non_parallel_multi_hinge_pose_composes_parent_before_local` | 非平行2hingeの親→局所合成順 | 同じ41/63度をexact合成し、逆順と不一致、正順とは完全一致 |
| `mountain_valley_sign_uses_canonical_left_right_and_reroots` | assignment、left/right、root変更時の符号 | exact local signを角度のf64乗算なしで再現し、全root姿勢がglobal exact合同 |
| `source_storage_topology_storage_and_edge_direction_do_not_change_pose` | source/topology/edge方向の順序不変性 | exact geometry digest、work、collision decisionの不変性。issuer identityは同一視しない |
| `material_face_boundaries_preserve_shared_vertex_and_shared_hinge_relations` | source上の共有頂点・共有辺registry | current exact occurrence registryの全件一致 |
| `material_pose_retains_private_issuer_identity_and_its_own_source_geometry` | clone、foreign issuer、同角度ABA | 元poseのcloneには同じauthorityを束縛でき、foreign/再solve/別root/1 ULP違いは拒否。exact authority自体をClone可能にすることは要求しない |
| Cayleyおよびzero-thicknessのone-short resource test | 局所核と幾何走査の個別上限 | tree全体の合算work、composition、occurrence、renderer照合のexact-limit成功/one-short原子失敗 |
| `arbitrarily_small_pose_mismatch_never_authorizes_false_transversal_or_coplanar_overlap` | 偽のpose mismatchをraw貫通へ昇格しない | exact authorityを持たない入力は今後も同じ判定保留。構造が似たcloneでauthorityを偽造できない |
| `subnormal_and_near_maximum_coordinates_keep_exact_classification` | exact triangle classifierの極端binary64入力 | tree pose側にもsubnormal pivot/angleと巨大pivotを追加し、局所核だけでなく合成・共有一致を検証 |

`non_commuting_fixture`は現在`ori-kinematics`のtest helperなので、production APIへ公開せず、collision private testへ同じ正規fixtureを移すかdev専用fixture moduleで共有する。

### 6.4 必須の姿勢・分類matrix

全行は紙厚0で実行し、外側2面だけでなく2件の隣接hinge pairも走査する。隣接pairが有限hinge model未実装で`RequiresHingeModel`に留まることは許すが、`SharedHingePoseMismatch`は一件も許さない。

| 角起点・山谷Vの角度vector | 外側2面の期待 |
| --- | --- |
| `0/0` | `SharedFeatureContact / AllowedSharedVertexContact` |
| `10/0`, `0/10` | `SharedFeatureContact / AllowedSharedVertexContact` |
| `45/45`, `90/90`, `91/91`, `135/135`, `179/179` | `SharedFeatureContact / AllowedSharedVertexContact` |
| `180/180` | `CoplanarAreaOverlap / Penetrating` |

| 辺中点・山山Vの角度vector | 外側2面の期待 |
| --- | --- |
| `0/0`, `10/10` | `SharedFeatureContact / AllowedSharedVertexContact` |
| `90/90`, `91/91` | topologyは真正な`SharedVertex`。材料法線条件によりblockingの`Indeterminate`だが、理由をpose mismatchにしてはならない |
| `135/135`, `179/179` | `TransversalCrossing / Penetrating` |
| `180/180` | exact half-turn後の`CoplanarAreaOverlap / Penetrating` |

上記各行を少なくとも次で反復する。

- root: 3面全てを一度ずつfixed faceにする
- source順: baseline、face/edge/vertex反転、境界cycle回転、hinge adjacency反転
- pair順: 全unordered pairのforward/reverse診断が同値
- exact再実行: 同じpose instanceで決定論的、同角度再solveは別authority

斜め共有hingeは角起点fixtureの`37/0`と`37/73`、非可換fixtureの`41/63`を用い、共有辺start/end/midpoint、親子合成、材料法線、全root合同を検査する。

### 6.5 新規に必要なfixture

| 不足fixture | 入力 | 固定する失敗モード |
| --- | --- | --- |
| 忠実400 mm報告corpus | 角起点山谷Vは正式仕様の400×400 mm座標、辺中点山山Vは`M=(200,0)`と遠い2角を使用 | 現行nativeの0〜10座標簡略fixtureだけでscale・非平方軸・実報告形状を代表したと誤認しない |
| 巨大共通平行移動 | dyadic座標の角起点V・山山VをX/Zへ`0, ±10^12, ±3×10^12, ±10^15`移動 | pivotの桁落ちで共有点を壊さない。exact分類は不変。renderer包含不能なら明示的blocking |
| tree-level subnormal | 斜め2hinge treeで角度を最小subnormal、最大subnormal、最小normalとし、pivot一成分にも最小subnormalを含める | 非0角を恒等へ潰さない、符号を失わない、合成後も共有点一致 |
| deep chain | 非平行軸を持つ複数hinge chain、角度`10/91/135/179`を反復 | 分母・bit長の累積、合成順、total work上限、途中成功の漏出防止 |
| shared-vertex fan | 4面以上が一つのVertexIdを共有するtree | 直接隣接pairだけ検査して非隣接occurrenceを取りこぼす実装を検出 |
| renderer containment exhaustion | 斜めhinge、巨大pivot、179度および180度直前 | exact poseとbinary64観測姿勢の差を無制限marginで吸収しない |
| certificate adversary | clone、同角度ABA、別root、角度1 ULP差、独立prepare、同一IDで異なる座標、別local certificate差し替え | value equalityからauthorityを偽造・再利用できない |
| aggregate limit boundary | deep chain/fanの実測workを各上限のexact値とone-shortで再実行 | 局所上限をhingeごとにリセットして総workを無制限化しない |
| precision-collapse rejection | distinct source頂点がbinary64保存時に同一点へ潰れる巨大座標または退化hinge | 不正sourceからexact authorityを発行しない |

巨大平行移動の不変比較には、加算後も差分がbinary64で正確に保存されるdyadic座標を使う。入力保存時点で既に頂点が潰れるケースは平行移動不変fixtureへ混ぜず、`precision-collapse rejection`として分離する。

### 6.6 native binary64 exact-affine包含とrenderer境界（次工程設計）

本節のうち未承認の直接差分観測はprivateな`MeasuredBinary64AffineEnvelope`として実装済みだが、正式admission、renderer接続およびcollision safe setは次工程であり、productionの実装済みカバレッジには数えない。この型は同じ`BoundMaterialTreePose` instanceだけを受理し、証明model ID、admission判定およびsafe判定能力を持たない。演算数表、三角関数の厳密誤差上限およびhard ceilingの承認後に限り、正式model ID候補`material_tree_binary64_affine_containment_v1`を`rational_cayley_tree_pose_v1`、`material_tree_kinematics_mm_v1`および同一のissuer/pose instanceへprivateに結合する。proofとbuilderは公開constructorを持たず、`Serialize`、caller提供matrix、公開IDの照合だけから再構成できてはならない。

2026-07-19時点のprivate観測は、全faceの保存済みbinary64係数をbit-exact有理数へ変換し、全boundary occurrenceで`F - E`の成分別最大値を測る。共有`VertexId`を全出現で再照合し、各hingeのstart/end/midpointをexact parent/child/stored pathとbinary64 parent/child/hinge-parent pathで検査する。単一面identity、斜め3-4-5軸37度、clone/ABA/foreign/reroot/1 ULP、全22個の構造・保存上限、厳密算術上限、canonical破損およびchecked overflowを7件で固定した。測定値は分類器、正厚、rendererまたはSIM-010へ渡していない。

#### 6.6.1 面境界全域の成分別包含

各faceについて、native `MaterialTreePose`の`RigidTransform`を構成する全binary64係数をIEEE 754 bit列からdyadic有理数へ完全変換し、その係数を無丸めの有理算術で評価する理想affine mapを`F`とする。同じsource boundary vertex `v`をexact tree map `E`と`F`へ適用し、各成分`k ∈ {x,y,z}`について次を有理算術で直接求める。ここで証明するのは保存済みmatrix係数が表す理想affine mapであり、JavaScriptやGPUでmatrixを点へ適用する際の追加丸めは6.6.4の別gateで扱う。

```text
delta(face, v, k) = abs(F(v)[k] - E(v)[k])
radius(face, k)    = max(delta(face, v, k) for every boundary occurrence v)
```

`F - E`はaffine mapであり、単純多角形の全点は境界頂点のconvex hull内にある。したがって、成分絶対値の凸性から、凹faceを含む材料面の全点`p`で`abs(F(p)[k] - E(p)[k]) <= radius(face, k)`が成立する。三角形分割頂点だけ、AABB cornerだけ、または代表点だけの検査で代用してはならない。

同じ`VertexId`の全face occurrenceは一つのexact点に対して個別に包含を検査する。共有hingeのstart、endおよびmidpointも親face、子face、hinge transformの全経路で検査し、一件でも欠落・重複・不一致があればface単位の部分proofを残さず全体を失敗させる。正厚へ拡張する際は位置radiusだけでは足りず、材料法線列の差も包含し、少なくとも`point_error + (thickness / 2) * normal_error`を別のversion付き正厚gateで証明する。

#### 6.6.2 version固定admission budgetとhard ceiling

上記の直接差分は包含そのものを証明するが、極端に広い対応を認可しないため、独立したadmission budgetを設ける。v1候補は次とし、実装前に演算数表、定数のbinary表現およびhard ceilingを同じpolicy versionへ固定する。

```text
u       = 2^-53
eta     = 2^-1074
S       = max(1, source point/pivot L1 norms in mm)
M_d     = S * (2*d + 1)
N_d     = N_axis + d*(N_local + N_compose) + N_affine
gamma_d = (N_d*u) / (1 - N_d*u), requiring N_d*u < 1/2

B_angle(d) = 2*M_d*sum(path angle_certificate.max_error_radians)
B_fp(d)    = gamma_d*M_d + N_d*eta
             + 2*M_d*d*TRIG_ABS_ALLOWANCE_V1
B_raw(d)   = B_angle(d) + B_fp(d)
require      B_raw(d) <= 2^-20 mm
B_admit(d) = B_raw(d)
```

各boundary occurrence・各成分の直接`delta`は、そのface深さに対応する`B_admit(d)`以下でなければならない。`2^-20 mm`はv1 hard ceiling候補であり、owner承認とversion固定前には実装済み仕様と扱わない。caller設定、紙厚、表示倍率または座標scaleによって拡張できず、budgetはcollisionの安全集合を膨らませる幾何許容値にも流用しない。`N_d*u >= 1/2`、checked算術overflow、budget導出不能またはhard ceiling超過は全てblockingとする。

version bindingには少なくとも次を含める。

- `rational_cayley_tree_pose_v1`、`material_tree_kinematics_mm_v1`および本proof model ID
- binary64から有理数へのbit-exact変換、round-to-nearest-ties-to-even規約、`DEGREES_TO_RADIANS`のbit列、pinned `libm 0.2.16`
- canonical traversal/order、座標系、`N_axis / N_local / N_compose / N_affine`の演算数表
- `TRIG_ABS_ALLOWANCE_V1`、`2^-20 mm`候補を含むbudget定数と全resource limit

いずれかを変更する場合はmodel versionを上げ、旧proofを新versionへ再利用しない。

#### 6.6.3 authority、fail-closedおよびwork accounting

admissionはexact tree authorityと同じnative solveの`MaterialTreePose`だけを受理する。cloneは元のprivate bindingを保持する場合だけ許可し、同角度の再solve ABA、foreign model/issuer、別fixed face、角度1 ULP差、generationまたはversion差を拒否する。missing/duplicate/noncanonicalなface、hinge、boundary occurrence、transform、非finite係数、cycle、共有点またはhinge端点検査の欠落も拒否する。

少なくともface数`F`、hinge数`H`、最大深さと深さ合計、transform scalar `12F`、boundary occurrence数`O`、point component数`3O`、共有occurrence照合、hinge endpoint照合、angle certificate読取、budget演算、exact rational演算、入力・中間・出力の最大/合計bit数をaggregateなchecked counterへ課金する。各上限はexact-limit成功とone-short失敗を対にし、hingeまたはfaceごとにcounterをresetしてはならない。

一件でも失敗した場合はproof全体を破棄し、部分radius、部分matrix、legacy binary64の安全結果へfallbackしない。分類はblockingな`indeterminate`に留める。

#### 6.6.4 Three.jsへの最終DTO橋渡し

native包含が直接証明する対象はnative `MaterialTreePose`である。現行UIは`Math.PI`とThree.js `Matrix4`を用いて別に姿勢を計算するため、native proofだけでは画面に表示されたmatrixの包含を証明できない。renderer checkpointの最終工程では、認証済みnative face/hinge matrixをcanonical ID順のdetachedかつversion付きDTOとして発行し、committed poseでの独立した三角関数・tree再計算を停止する。

Three.js列優先4×4のface matrixは次の順を固定する。この配列は`Matrix4.elements`または`Matrix4.fromArray`へ渡すlayoutであり、row-major引数を取る`Matrix4.set(...dto)`へそのまま渡してはならない。

```text
[r00,r10,r20,0, r01,r11,r21,0, r02,r12,r22,0, tx,ty,tz,1]
```

DTOはproject instance、revision、pose generation、fixed face、全angle bits、model/proof versionおよびgeometry digestへ結合する。ただしDTO自体とclient ACKはauthorityではなく、native capabilityを生成できない。native proofの直接範囲は発行したDTO payloadのbinary64 bit列までとする。UIは全IDとmatrixを一時領域で検証し、`fromArray`後の`Matrix4.elements`がDTO bit列と一致することをclient-side bridge testで確認してから原子的に交換する。このclient検証をnative authorityへ逆輸入しない。欠落、重複、順序違反、stale digest、NaN/Infまたはcopy途中失敗では旧表示を維持する。GPU f32変換、camera、rasterizationおよびpixel errorは別の表示品質問題として区別する。local drag previewを残す場合は非認証previewと明示し、安全certificateやcommit authorityへ使用しない。

matrix bit列の一致だけでは、`Vector3.applyMatrix4`等が行うbinary64乗算・加算後のworld点まで包含したことにならない。CPUでface頂点を変換する経路を残す場合は、version固定の演算順で実際のbinary64結果をbit-exact有理数へ戻し、理想`F(v)`との差`radius_cpu_apply`を全境界頂点で証明して`radius + radius_cpu_apply`を表示対応の上限とする。より単純な最終経路として、nativeが計算済みworld頂点DTOも発行し、UIがそのbit列を再計算せず使用してよい。どちらも未実装の間はrenderer containmentを完成扱いにしない。GPU側だけでmatrixを適用する経路はcollision authorityへ使わず、f32変換以降を表示品質検証として分離する。

#### 6.6.5 必須test matrix

次の軸をpairwise省略ではなく、記載した境界値とadversaryを含む形で固定する。

| 軸 | 必須case |
| --- | --- |
| tree | 単一face、1 hinge、非可換2 hinge `41/63`、角起点V、山山V、chain depth `1/2/32/max`、shared-vertex fan、全faceへのreroot |
| hinge | X/Z軸、3-4斜め軸、非平方軸、subnormal軸成分、山/谷、left/right反転 |
| angle | `0`、最小正値、`90度±1 ULP`、`90/135/179度`、`180度-1 ULP`、`180度` |
| coordinate | 400 mm、共通平行移動`0/±10^12/±3×10^12/±10^15 mm`、巨大pivotと短いhinge、入力precision collapse |
| authority | valid clone、同角度ABA、foreign issuer/model、別root、角度1 ULP差、stale version/generation |
| resource | 全counterのexact-limit/one-short、最大/合計bit上限、checked overflow、`N_d*u = 1/2`境界、hard ceiling直下/一致/直上 |
| DTO | row/column layout、ID欠落/重複/並べ替え、NaN/Inf、stale generation/digest、copy失敗時rollback、committed poseでの独立再計算禁止、CPU `applyMatrix4`丸め包含またはnative world頂点bit照合 |

肯定caseでは全boundary vertexと凹face内部のbarycentric sampleが成分radius内、root radiusが0、全共有occurrenceとhinge endpointが同じexact hull内、同じ入力のradius/DTOがbit-for-bit決定論的であることを検査する。各matrix scalarを`±1 ULP`改変したadversary、storage順変更、cardinal angleのexact branchも含める。巨大座標でhard ceilingを超えるcaseは成功期待へ緩和せず、明示的blockingを期待する。

### 6.7 resourceとfail-closed

tree checkpointでは局所Cayley上限に加え、少なくともface数、hinge数、vertex occurrence数、認証済み境界辺索引entry/単一走査operation数、local rotation数、exact composition数、point transform数、共有関係照合数、renderer containment数、合算interval/rational work、中間bit数および出力bit数を事前に上限検査する。境界辺をヒンジごとに線形再走査してはならず、全boundary occurrenceを一度だけ索引化して参照する。個別出力bit上限は分子・分母の大きい側へ適用し、総storage上限は両方のbit数をchecked加算する。一時的なlocal出力も最大個別bitの観測から除外しない。加算・乗算の`usize` overflowも`ResourceLimitExceeded`とする。

各実測上限は「exact値で成功、one-shortで失敗」を固定する。どの段階で失敗しても、部分face transform、部分certificate、共有頂点許容または135/179度の貫通証拠を返さない。より粗いbinary64結果へfallbackして安全集合を広げず、既存の`indeterminate`へ戻す。

### 6.8 checkpoint完了条件

次を全て満たした時点だけ、watertight exact tree poseを完成扱いにする。

1. 6.1と6.2のcertificate/propertyが全てprivate testで肯定される。
2. 角起点山谷V、辺中点山山V、斜め共有hingeが6.4の全行・全root・入力順で一致する。
3. 山山Vの90/91度は`pose_mismatch`ではない判定保留、135/179度はexact貫通になる。
4. 0度はidentity、180度はexact half-turnとしてtree全体で処理され、角起点および山山Vの180度正面積重なりを失わない。
5. 巨大平行移動、subnormal、deep chain/fan、resource one-short、renderer containment、ABAを全てfail-closedで回帰する。
6. exact authorityなしの従来経路と、偽造・期限切れ・resource不足の経路は安全結果へ昇格しない。
7. このcheckpointでは正厚、有限hinge許容、連続衝突、SIM-010用の任意3D姿勢に対するcell-order transport、または折り重ねUIを完成扱いにしない。VAL-003で既に発行するcurrent layer-order capabilityとは区別する。

本節の設計追記時点では実装完成度を加算せず、全体完成度は36.9%のままとする。折り重ねUIは本checkpoint、層順序transport、atomic commandの後に実装する。
