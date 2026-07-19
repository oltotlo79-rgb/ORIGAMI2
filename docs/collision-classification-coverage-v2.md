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

| 交差証拠 | 純粋表4セル | native private幾何基盤 | 現在の回帰 | production残作業 |
| --- | --- | --- | --- | --- |
| `separated` | 完了 | 厚さ0で実装 | exact三角形、面交換、頂点順 | 正厚full scanへ統合 |
| `point_contact` | 完了 | 厚さ0で実装 | 一般点、共有点以外、subnormal | 正厚境界との統合 |
| `boundary_line_contact` | 完了 | 厚さ0で実装 | 実外周、部分共有辺、人工対角線除外 | 正厚境界との統合 |
| `boundary_area_contact` | 完了 | 未実装 | 純粋表だけ | 正厚閉三角柱の正面積・正体積0証明 |
| `shared_feature_contact` | 完了 | 厚さ0の共有頂点・共有辺で実装 | exact singleton、完全共有辺、誤った共有点/部分辺拒否 | watertight tree姿勢と有限ヒンジへ結合 |
| `shared_feature_thickness_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚中央面再証明とprivate provenance |
| `shared_feature_flat_stack` | 完了 | native未実装 | frontend現行経路のみ | 厚さ0・厳密180度・有限ヒンジ範囲の証明 |
| `coplanar_area_overlap` | 完了 | 厚さ0で実装しproduction blockingへ接続 | 共有なし、共有頂点、共有ヒンジ、180度の共面正面積 | 正厚full scanとmulti-face safe proof |
| `transversal_crossing` | 完了 | 厚さ0で実装し、三角形どうしはCayley dual gate、非三角whole material faceは全pair認証済みexact集約からproduction blockingへ接続。有限な正厚では同じ三角形中央面dual gateを材料貫通の十分条件としてblocking専用接続 | exact binary64、近平行、悪条件、共有点外横断、非三角face、正厚0.1/1/3 mmの限定中央面横断 | 正厚の共面・非三角・三角柱full scanとmulti-face safe proof |
| `positive_volume_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚三角柱SATの肯定証明 |
| `indeterminate` | 完了 | 厚さ0で実装 | 退化、作業上限、共有姿勢不一致、同一面到達 | 正厚・有限ヒンジ・連続経路の全失敗理由 |

`no_shared_feature`で共有要素専用証拠が来るセル、共有identityと`separated`が同時成立するセル、`same_face`の全セルなどは、幾何生成の成功fixtureを作る対象ではない。これらは矛盾または走査前除外を表すため、純粋表とruntime fail-closed回帰で固定する。

本表の「厚さ0で実装」は、private幾何生成器が証拠種別を生成できることを表す。6.6.6ではexact tree `E`、測定元`E`へpointer identityで束縛したmeasured envelope、同じissuer-bound poseのbinary64 affine像をactual-mm有理数へ直接liftした実在triangle、および認証済みmodelから内部導出したtopologyを結ぶprivate actual-mm scanまで到達した。bit-exactな紙厚`+0.0`では、この三角形どうしの横断をcanonical exact `E`とdirect-lift `F`のdual gateで、180度の共面正面積重なりを認証済みzero-thickness集約の`CoplanarAreaOverlap`で、少なくとも一方が非三角whole material faceである横断を同集約の`TransversalCrossing`でproduction blockingへ接続している。全経路はexact pose instance、canonical face registry、全unordered face pair、全triangle-pairおよび有限workを照合してから肯定する。点接触・境界線接触・共有要素だけの接触は貫通へ昇格しない。三角形どうしの横断は旧集約だけでdual gateを迂回できない。measured envelopeはauthorityとdirect-lift点の包含再検証だけに使い、そのboxを貫通幾何として使わない。

desktop current-pose診断とproduction警告UIにも接続済みである。ゼロ厚のwire reasonとDTO fieldは`proven_zero_thickness_penetration`、`provenPenetratingPairs`、`firstProvenPenetratingPair`へ一般化し、旧`proven_transversal_penetration`、`provenTransversalPairs`、`firstProvenTransversalPair`をstrict parserで拒否する。Rust公開error variant名`ProvenTransversalPenetration`だけはcrate API互換のため維持する。

有限な正厚については、`centered_mid_surface_v1`のclosed solidが自身の材料中央面を内部に含むため、同じissuer-bound poseのcanonical exact `E`とdirect-lift `F`がともに三角形中央面のstrict transversalを証明したpairだけを、専用の`ProvenPositiveThicknessPenetration`とwire reason `proven_positive_thickness_penetration`へ接続する。これは正厚材料貫通の十分条件であり、点・線・共有要素だけの接触、共面中央面、片側だけの肯定、独立envelope box、旧zero-thickness集約、三角柱SATの不確定または弱い証拠からは発行しない。UIは「紙厚を含む材料貫通・安全認定不可」を専用表示する。ただしpublic safe proofの成功集合は単一面・0 pairから広げず、正厚の全証拠行またはSIM-010を完成させない。

正厚のprivate checkpointでは、1ヒンジ・2三角形面の有限ヒンジ前提tokenを消費し、
同じCayley exact姿勢`E`とnative binary64 affine姿勢`F`の位置・材料法線差を、
各軸のexact有理数として測るE/F境界capabilityまで実装した。正厚solidの各軸上限は
`point[k] + h × normal[k]`で、`h`は紙厚をexact liftしてから2で割る。これは
componentwise包含であり、L∞欄もユークリッド距離ではない。exact pointer、native
pose instance、紙厚bits、左右面・hinge indexおよび全`F`係数bitsへprivateに結合し、
全構造・厳密算術counterをhard cap以下へ制限する。独立な成分boxを相関済み姿勢として
三角柱交差、共有軸weld、radial marginまたは分類へ用いてはならない。完全交差集合を
有限ヒンジcorridorへ含める`E`側proofと、direct-lift `F`側の同じproofが完成するまで、
表3のnative正厚productionは、上記のstrictな三角形中央面横断をblocking肯定する
限定経路だけが実装済みである。正厚三角柱の`boundary_area_contact`、
`shared_feature_thickness_overlap`、`positive_volume_overlap`、共面・非三角面、
finite-hinge admissionおよび安全証明は未実装のままである。

2026-07-19 checkpointでは、厚さ0幾何のbinary64入力をexact有理数へ持ち上げる処理から、三角形分割、面・線・区間演算、比較、全canonical unordered face pairの証拠集約までを、一つの単調な有限資源meterへ統合した。入力・保持clone・演算・中間bit・GCD fallback・出力に加え、exact kernelが明示的に生成するlogical BigInt payloadの件数・個別bit・累積bitをchecked加算し、資源超過時は保存済みpair表を発行しない。全pairのdispatchは準備時にcanonical順で事前計算し、後続の参照順によって作業量や結果が変わらない。

公開静的衝突境界は、blocking decisionを見つけても走査を短絡せず、全unordered face pairと全triangle-pairの期待数・解析数を照合する。資源超過または証拠取得失敗はその時点で原子的にfail-closedとする。複数面では有限ヒンジmodelが未完成なので、診断結果が非blockingでも`NativeStaticCollisionGeometryProof`を発行せず、`PairEvidenceUnavailable`を返す。安全証明constructorは単一面・0 pairの枝だけに残している。

blocking肯定の集約自体も同じexact pose instanceとcanonical face順へ束縛し、最初のpairをcanonical ID順で保持する。`CoplanarAreaOverlap`または対象を限定した`TransversalCrossing`以外のdecision/evidence不一致、肯定件数と最初のpairの不一致、非canonical registryは内部不整合として原子的に拒否する。

## 4. 利用者報告回帰

| 回帰 | frontend現在経路 | native厚さ0基盤 | native正厚production |
| --- | --- | --- | --- |
| 角起点の山谷V: 厚さ`0 / 0.1 / 1 mm`×`片側10度左右2通り / 両側45 / 91 / 135度`の15姿勢 | 全15姿勢を`allowed_shared_vertex_contact`・貫通0で回帰 | 厚さ`+0.0`では`10/0, 0/10, 45/45, 90/90, 91/91, 135/135, 179/179`を共有頂点接触の誤肯定0件、`180/180`を別のexact共面正面積重なり1件として、通常・逆source collection、全3 rootで固定 | 厚さ`0.1 / 1 / 3 mm`の`10/0, 0/10, 45/45, 90/90, 91/91, 135/135, 179/179, 180/180`を専用中央面横断へ誤肯定しない |
| 報告A: 厚さ0、片側10度 | 共有頂点許容・貫通0 | exact共有頂点接触を回帰 | 対象外 |
| 報告B: 厚さ0、両側180度 | 共面正面積・貫通1 | `coplanar_area_overlap`・`penetrating`を回帰 | 対象外 |
| 辺中点の山山V: 厚さ`0 / 0.1 / 3 mm`×`90 / 91 / 135 / 179度` | 90/91度は`indeterminate`、135/179度は`penetrating`で全12姿勢を回帰 | 現行binary64 exact経路は共有点不一致により`indeterminate`。private actual-mm scanは通常・逆source collection、全3 rootで90/91/180度を`Unresolved`、135/179度だけを外側1 pairの`ProvenPenetrating`へ固定 | 厚さ`0.1 / 1 / 3 mm`×90/91/135/179/180度を通常・逆source collection、全rootで固定し、135/179度だけを`ProvenPositiveThicknessPenetration`、90/91/180度を証拠不足とする |
| 共有点外の横断: 厚さ`0 / 0.1 / 1 mm` | 全9姿勢を`penetrating`で回帰 | 厚さ0 exact横断を回帰 | 三角形中央面のdual gateで証明できる範囲だけblocking。正厚三角柱full scanは未実装 |
| 非三角whole material faceを含む山山V: 厚さ`+0.0`・両側135度 | 対象外 | 全pair認証済みexact集約から横断1件。通常・逆source collection、全rootで同一canonical pair | 正厚は未実装 |

従来のbinary64 exact経路で三角形どうしの山山V 135/179度が`indeterminate`であることは、安全側の一時退避である。private scanはissuer-bound exact tree `E`のzero-width triangleと、同じissuer-bound poseの保存binary64 affine係数・rest頂点をactual-mm `BigRational`へ直接liftしたzero-width triangleの両方が横断を肯定できる場合だけ、この偽陰性をblocking-onlyな`ProvenPenetrating`へ回復する。measured envelopeは両者のauthority結合とdirect-lift各点のradius内包含を再検証するだけで、box自体の肯定をdual gateへ数えない。共面正面積重なりと非三角whole material face横断は、dual-gate triangle scanとは別に全pair認証済みexact集約から肯定する。これらはnative公開静的衝突入口、desktop current-pose診断およびproduction UIへ接続済みだが、public safe proofの成功集合は単一面・0 pairから広げないため、SIM-010を完了扱いにしない。

## 5. 折り重ね前ゲート

純粋な4×11表と44セルcorpus照合は完了済みである。依存工程は次の順番を崩さない。

1. actual-mm閉有理区間のprivate blocking-only横断primitiveを、`ProvenPenetrating` / `Unresolved`だけのsealed結果、共有関係、strict境界、全資源上限で固定する。このprivate synthetic段階は完了済みだが、production decisionまたはpublic safe proofを発行しない。
2. exact tree `E`、同じissuer-bound poseのbinary64 affine係数・rest頂点のdirect lift、`E`を測定したenvelope、内部導出topology、canonical face pairだけからactual-mm入力を作るprivate scanと、角起点V・辺中点山山Vの三角material face実姿勢回帰、およびnative公開静的衝突入口へのblocking専用接続は完了した。canonical exact `E`側とdirect-lift実在triangle側の両方が同一pairを肯定した場合だけtriangle横断をblocking errorへ接続し、envelope boxを肯定幾何として使わず、multi-face safe setも広げない。全pair認証済みの180度共面正面積重なりと非三角whole material face横断、desktop current-pose診断およびproduction UIへの接続も完了した。H64既定上限内の完全成功とrenderer有限包含は後続である。
3. 正厚の有限ヒンジ前提とprivate E/F componentwise境界、および両中央面のstrict transversalを材料貫通の十分条件としてproduction blockingへ接続する限定経路は完了した。次に完全な三角柱交差集合を有限ヒンジcorridorへ含める`E`側proofとdirect-lift `F`側proofを別々に完成させ、両方を再結合してから、正厚`boundary_area_contact` / `shared_feature_thickness_overlap` / `positive_volume_overlap`、有限ヒンジ`shared_feature_flat_stack` / corridorをnative productionへ接続する。
4. current-pose apply・同generation静的診断・blocking表示への結合は完了した。multi-face safe certificate、native continuous collisionおよび全操作経路の停止へ結合し、`indeterminate`を貫通同等のblocking停止へ流す。
5. 全pair coverageと有限workを維持したままcell-order transportを結合し、atomic `ApplyStackedFold` command、最後に折り重ねUIを接続する。

この順序により、4×11表、blocking-only、認証済み厚さ0production blocking bridge、正厚・有限ヒンジ、continuous collision・層順transport、command、UIの依存関係を固定する。

current-pose apply・診断のheavy workerは`AppState`所有のprocess-wide RAII gateで一つに制限する。busy拒否ではproject、current/pending authority、generationを変えず、permitをblocking closureへmoveして待機futureのcancel中もworker終了まで保持する。blocking診断は元の非Cloneなexact B capabilityを保持し、project→pose lock下の再検証closure内だけでbinding付きDTOを作る。同角度再apply、編集、reopen、別slotでstaleになった結果はbindingなしへ閉じる。この境界は診断結果のABA混入を防ぐが、連続経路停止またはSIM-010 mutation authorityではない。

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

本節のうち未承認の直接差分観測はprivateな`MeasuredBinary64AffineEnvelope`として実装済みだが、正式admission、renderer接続およびcollision safe setは次工程であり、productionの実装済みカバレッジには数えない。この型は同じ`BoundMaterialTreePose` instanceに加え、測定元の`RationalCayleyTreePose`そのものを安全なborrowで保持し、参照同一性が異なる独立再生成`E`との組替えを拒否する。unsafeなaddress token、公開constructor、Clone、Serialize、証明model ID、admission判定およびsafe判定能力は持たない。演算数表、三角関数の厳密誤差上限およびhard ceilingの承認後に限り、正式model ID候補`material_tree_binary64_affine_containment_v1`を`rational_cayley_tree_pose_v1`、`material_tree_kinematics_mm_v1`および同一のissuer/pose instanceへprivateに結合する。proofとbuilderは公開constructorを持たず、caller提供matrix、公開IDの照合だけから再構成できてはならない。

2026-07-19時点のprivate観測は、全faceの保存済みbinary64係数をbit-exact有理数へ変換し、全boundary occurrenceで`F - E`の成分別最大値を測る。共有`VertexId`を全出現で再照合し、各hingeのstart/end/midpointをexact parent/child/stored pathとbinary64 parent/child/hinge-parent pathで検査する。単一面identity、斜め3-4-5軸37度、clone/ABA/foreign/reroot/1 ULP、全22個の構造・保存上限、厳密算術上限、canonical破損、checked overflow、および同じboundから独立再生成した`E1/E2`のenvelope相互組替え拒否を専用8件で固定した。private scanはこのenvelopeをexact pointer/pose authorityと、保存binary64 affine像を直接liftした各点が測定radius内にあることの再検証へ使う。envelope boxを共有点へweldしたり横断の肯定幾何にしたりせず、正厚、renderer、公開分類またはSIM-010へも渡していない。

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

#### 6.6.6 blocking-only横断区間証明

actual mm座標の閉有理区間だけを受け取るprivateな横断証明primitiveを用いる。source edge両端の相手平面に対する符号区間が厳密に反対で、交点parameterの分母が0を含まず、`t`全体が開区間`(0, 1)`に入り、得られた交点区間が相手三角形の安定な2次元射影で全3辺のstrict interiorにある場合だけ`ProvenPenetrating`とする。点・線接触、共面、離間、法線・分母・interiorのいずれかが0を含む場合、資源超過または入力不整合は全て`Unresolved`へ閉じる。このprimitiveの非zero-width区間fixtureは算術核の保守性を検査するsynthetic testに限る。認証scanではcanonical exact `E`とbinary64実在affine像をそれぞれactual-mm `BigRational`へ直接liftしたzero-width triangleだけを横断predicateへ渡し、独立なper-face radius boxを相関済み姿勢として扱わない。この二値結果から`Separated`、`Touching`、共有頂点許容またはpublic geometry proofを発行してはならない。

共有頂点pairでは、共有点にincidentなedgeを肯定witnessから除外する一方、共有点を含まない反対edgeが相手面のstrict interiorを横断する証明は許可する。反対edgeだけでは捉えられないrelative-interior正長segmentの専用証明は、両triangleの共有occurrenceがbit-exactに同じzero-width singletonである場合だけ使い、nonzeroまたは別々のboxを共有点へweldしない。これにより共有点だけの接触を貫通へ昇格させず、辺中点山山Vの135/179度に存在する共有点外の正長横断を証明可能にする。共有ヒンジ、same-face、180度共面重なりはこのprimitiveの対象外とし、それぞれ有限ヒンジmodel、runtime不整合、exact共面正面積classifierへ分離する。

private認証scanと、その後の本番接続は次を全て満たす。

1. 一方の入力はCayley exact pose `E`のactual-mm zero-width座標とする。もう一方は同じissuer-bound poseの保存binary64 affine係数とrest頂点をIEEE 754 bit列から`BigRational`へ直接liftし、そのaffine mapを無丸めで評価したactual-mm zero-width座標とする。厚さ0classifierの座標だけを`2^1074`倍したcommon-unit型を渡さず、determinant等の派生量と長さradiusを直接比較しない。
2. 実装済みのexact `E` borrow identityと`BoundMaterialTreePose`認証をbridgeでも必ず同時に再確認し、old/new `E`、同角度ABA、foreign issuer、別root、角度1 ULP差および別source順authorityの混在を拒否する。
3. measured envelopeは、同じexact pointer/pose authorityへの結合と、direct-liftした各boundary点が対応faceの測定radius内にあることのcontainment再検証だけに使う。per-face box同士には共有点の相関がないため、boxをweldした横断証明やdual gateの肯定には使わない。topology、共有vertex occurrenceとcanonical face/triangle pairは認証済みmodel・boundaryから内部導出し、caller提供enumを信用しない。結果も同じbound、`E`、envelope、pairへprivateに束縛する。
4. canonical exact `E`のzero-width predicateと、同じpairのdirect-lift binary64 actual triangle predicateがともに`ProvenPenetrating`の場合だけscanの肯定を採用する。片側だけの肯定、envelope boxだけの肯定、`PointContact`、`BoundaryLineContact`、`CoplanarAreaOverlap`または`Indeterminate`相当から本番貫通へ昇格しない。
5. 400 mm辺中点山山Vは90/91度を`Unresolved`、135/179度を`ProvenPenetrating`、180度をこのprimitiveでは`Unresolved`とする。角起点山谷Vの片側10度と両側45/90/91/135/179/180度は全てこのprimitiveでの肯定を禁止する。全root、pair交換、source順、dyadic scale、巨大共通平行移動、triangle頂点順、direct-lift containmentと全資源上限を回帰する。
6. private scanへ渡す幾何は紙厚にかかわらずzero-widthの材料中央面だけとし、紙厚を独立box、三角柱または有限corridorへ膨らませない。公開静的衝突入口は、有限な紙厚`0.1 / 1 / 3 mm`についても`centered_mid_surface_v1`のsolidが中央面を含むという包含だけを追加前提に、同じdual gateのstrict transversalを正厚材料貫通のblocking十分条件として利用できる。これ以外の正厚分類やmulti-faceのpublic safe proof成功集合を、単一面・0 pairから広げない。

2026-07-19 checkpointではprivate synthetic testで、唯一の肯定横断、点・線・共面接触、共有頂点のincident-edge除外と反対edge横断、共有ヒンジ・same-face・不正共有index、分母・`t`・interiorのstrict境界、頂点順・pair方向、6軸置換×8反転、正scale・巨大平行移動、radiusの有理数上下bracket、非canonical・逆転区間、退化三角形、正radiusの共有点接触、肯定witness後の資源超過による全体`Unresolved`、work上限のexact/one-shortを固定した。さらに、zero-widthでは横断となる共有頂点fixtureでも一方を独立boxへ広げると共有点相関を失い、box内に離間した実現姿勢が存在する反例を`Unresolved`へ固定した。したがってsyntheticなnonzero interval肯定をmeasured envelopeの実姿勢証明へ転用しない。

同checkpointの次段として、測定元exact `E`へのpointer-bound envelopeと`BoundMaterialTreePose`を同時に再認証するprivate actual-mm scanを実装した。whole material boundaryが3頂点のfaceだけをpredicate対象とし、共有関係はexact boundaryの`VertexId`と認証済みhinge registryから内部導出する。全canonical unordered face pairへ結果recordを作り、exact tree work、measured containment work、全pairのcanonical exact `E` zero-width判定とdirect-lift binary64 actual triangle判定を一つの共有`WorkMeter`へ累積する。direct-lift各点が測定radius内であることも再検証し、boxはpredicate入力へ渡さない。全pair・topology照合・predicate call・結果recordの件数不一致またはどのone-shortでもscan全体を返さず、先に得た肯定を部分発行しない。

共有頂点pairには、反対edgeの通常piercingに加え、bit-exactに一致するzero-width共有点から両面のrelative interiorが同じ向きへ正長segmentとして重なることをstrictな有理算術で証明する経路を追加した。nonzeroまたは独立boxにはこのspecial caseを適用しない。これにより400 mm角起点山谷Vは通常・逆source collection、全root、指定全角度で肯定0件を維持し、辺中点山山Vは同じ変形軸で135/179度だけ1 pairを肯定、90/91/180度を`Unresolved`へ保つ。pairのforward/reverse参照も同じcanonical recordへ一致する。

このtriangle dual-gate scan自体は常にzero-widthの材料中央面を走査し、紙厚を幾何入力へ混ぜない。native公開静的衝突入口はbit-exactな`+0.0`では`ProvenTransversalPenetration`へ、有限な正厚では中央面を含むsolidの包含を追加確認して専用`ProvenPositiveThicknessPenetration`へblocking肯定を分岐する。`-0.0`と非有限値はどちらの肯定にも入らない。共有ヒンジ、非三角material face、点・線・共面または片側だけの肯定は、このdual-gate経路では`Unresolved`またはscan拒否へ閉じる。公開入口全体では、bit-exactな`+0.0`についてだけ、全pair認証済みzero-thickness集約が証明する180度の共面正面積重なりと非三角whole material face横断も別経路でblocking肯定し、desktop current-pose診断・production UIへ渡す。これにより本primitiveでの`180度 = Unresolved`や非三角拒否を、製品全体のゼロ厚最終分類と解釈してはならない。一方、正厚の共面・非三角・境界面・正体積証拠、有限ヒンジ、非三角faceのCayley区間横断、multi-face public geometry proofは未実装である。

### 6.7 resourceとfail-closed

tree checkpointでは局所Cayley上限に加え、少なくともface数、hinge数、vertex occurrence数、認証済み境界辺索引entry/単一走査operation数、local rotation数、exact composition数、point transform数、共有関係照合数、renderer containment数、合算interval/rational work、中間bit数および出力bit数を事前に上限検査する。境界辺をヒンジごとに線形再走査してはならず、全boundary occurrenceを一度だけ索引化して参照する。個別出力bit上限は分子・分母の大きい側へ適用し、総storage上限は両方のbit数をchecked加算する。一時的なlocal出力も最大個別bitの観測から除外しない。加算・乗算の`usize` overflowも`ResourceLimitExceeded`とする。

各実測上限は「exact値で成功、one-shortで失敗」を固定する。どの段階で失敗しても、部分face transform、部分certificate、共有頂点許容または135/179度の貫通証拠を返さない。より粗いbinary64結果へfallbackして安全集合を広げず、既存の`indeterminate`へ戻す。

現行のCayley核は、従来のinterval、級数、平方根、shift、中間値、GCD、出力に加え、有理数allocationの件数・個別bit・総bitを有限上限へ含めた。保存済みworkの再開・合算は、加算counterをchecked加算し最大値counterをmax合成した後に全上限を一括検証し、失敗時は元meterを変更しない。算術結果と内部scratchはoperation数・反復数・中間bit上限で拘束し、算術operationを伴わないclone・符号反転、比較の交差積およびBigUint補助一時値をallocation件数・個別bit・総bitへ課金する二層契約である。このcounter単独を実heap allocationの完全計数または厚さ0核の全logical payload計数と解釈してはならない。認証scanのresume設定はこのworkで再検証できるexact 12項目だけを公開し、exact pose発行時にだけ意味を持つprecision round、guard bit、candidate bitの3項目を含めない。内部`WorkMeter`へ変換するときはこの3項目を0固定し、scan経路から新しい三角関数candidateを生成できないようにする。構造15項目とexact 12項目には、運用defaultから独立した固定hard ceilingと、その全27項目の直上拒否回帰を持つ。

厚さ0幾何側にも独立した有限上限を設け、binary64入力とその総storage、保持clone総量、有理演算数、中間bit、GCD回数・入力bit総量、logical BigInt allocationの件数・個別bit・累積bit、出力bit・総storageを、一姿勢の準備と全canonical pairへ通して累積する。生成後の観測ではなく、保守的な上界を生成前に一括検査してからcounterを確定する。`num-bigint = 0.4.8`と`num-rational = 0.4.2`を厳密固定し、依存内部scratchは実heap byte quotaへ混同せず、演算数・中間bitとGCD回数・入力bitで別に有限拘束する。Cayley側のcanonical equalityは正規化済み分子・分母のallocation-free構造比較でmeterを進めず、大小順序だけを`WorkMeter`付き比較でpreflight・GCD・補助allocationへ課金する。厚さ0側の一致は`RationalWorkMeter::equal`がoperationとinput bitを課金してから同じ構造比較を行い、sortはmetered ordering、dedupはmetered equalityを用いる。全pairは同じmeterで事前計算し、資源超過では姿勢authority全体を返さない。保存後のpair参照は再計算もmeter更新も行わない。exact-limit/one-short、16k-bit比較、GCD境界、counter overflow、canonical不変条件、除算不能、pair参照順不変を回帰する。

公開静的診断はblocking pairだけを理由に途中成功せず、canonical pair列を最後まで走査してpair数とtriangle-pair数を検算する。ただし資源超過・証拠欠落は直ちに全体失敗とする。複数面へ安全証明を発行する条件はまだ満たしていないため、この全走査は診断完全性のcheckpointであり、`NativeStaticCollisionGeometryProof`のsafe set拡張ではない。

### 6.8 checkpoint完了条件

次を全て満たした時点だけ、watertight exact tree poseを完成扱いにする。

1. 6.1と6.2のcertificate/propertyが全てprivate testで肯定される。
2. 角起点山谷V、辺中点山山V、斜め共有hingeが6.4の全行・全root・入力順で一致する。
3. 山山Vの90/91度は`pose_mismatch`ではない判定保留、135/179度はexact貫通になる。
4. 0度はidentity、180度はexact half-turnとしてtree全体で処理され、角起点および山山Vの180度正面積重なりを失わない。
5. 巨大平行移動、subnormal、deep chain/fan、resource one-short、renderer containment、ABAを全てfail-closedで回帰する。
6. exact authorityなしの従来経路と、偽造・期限切れ・resource不足の経路は安全結果へ昇格しない。
7. このcheckpointでは正厚、有限hinge許容、連続衝突、SIM-010用の任意3D姿勢に対するcell-order transport、または折り重ねUIを完成扱いにしない。VAL-003で既に発行するcurrent layer-order capabilityとは区別する。

本節の内部基盤だけでは実装完成度を加算しない。current-pose診断とproduction UI接続で別途計上した現在の全体完成度37.2%は、今回の分類精度・worker境界強化では変更しない。折り重ねUIは本checkpoint、正厚・有限ヒンジ、層順序transport、atomic commandの後に実装する。
