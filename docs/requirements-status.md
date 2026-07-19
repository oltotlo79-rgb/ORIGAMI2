# MUST要件 実装状況

更新日: 2026-07-19

`requirements-definition.md`のMUST 87件を、利用者がUIから実行できるかを基準に追跡する。配布・CI要件だけは、要件に明記された自動検証または成果物が存在するかを基準とする。内部基盤やテストだけが完成していても、利用者機能がUI未接続なら「実装済み」へは上げない。

- 実装済み: 要件の主要な利用経路がUIから動作する。配布・CI要件は、指定された自動検証または成果物がそろう
- 部分実装: 有用な一部が動作するが、要件に明記された範囲を満たさない
- 未着手: 要件固有の利用経路が存在しない

現在の行単位集計は **実装済み38 / 部分実装24 / 未着手25**。2026-07-18のオーナー決定によりSIM-010を追加し、macOS自動ビルド・CI検証へ確定したOPS-008を実装済みとして再評価した。2026-07-19に時間制限つき全体平坦折り3値判定、場所別層順序、進捗・中止・background worker・終端状態の利用者経路を接続し、VAL-003/005/006/007/009を実装済みへ更新した。同日に長さ表示単位の保存・換算・編集UIを接続してPRJ-008を、白黒でも識別できる5線種を画面・取込・SVG書出へ接続してLIN-003を、ライト・ダーク・OS連動とWindows/macOS共通標準shortcutを接続してUI-005/006を、2D/3D・プロパティ・折り手順領域の位置と大きさの変更・端末保存を接続してUI-004を、主要shortcutの変更・重複検出・端末保存を接続してUI-007を実装済みへ更新した。第三者監査本文の旧集計値は各時点の履歴であり、以下の87行の判定を正本とする。

2026-07-19追記: SIM-010行で未実装としていた`deep-chain stress`のうち、非平行
H8/H16のwatertight成功、H64の構造確認後の資源preflight即時拒否、subnormalの
GCD fallback exact/one-short、400 mm V、巨大平行移動、共有頂点fan、
precision collapseは実装・実行済みである。角起点・辺中点Vのfail-closed baselineも
加え、`ori-collision --lib`は74件全件成功。
未完了なのはH64を既定上限内で完全成功させる性能checkpoint、renderer有限包含、
厚さ0classifier接続以降である。これらが完了するまでSIM-010は未着手を維持し、
折り重ねUIへ進まない。

同じくSIM-010行の「frontend/native共通のv2純粋decision表完成」は、全44セルの
表とcorpus照合だけを指す。現行frontend production dispatcherと既存certificateは
凍結済みv1へ束縛されたままで、正厚`boundary_area_contact`生成器、v2 issuer、
pose/thickness再結合拒否は未実装である。単純なversion差し替えは行わず、証拠不足を
blockingな`indeterminate`へ閉じる。

2026-07-19追記: native binary64姿勢と有理Cayley姿勢の全境界直接差分観測は、
privateな`MeasuredBinary64AffineEnvelope`として実装した。同じissuer/pose instance、
共有頂点、hinge端点・中点、構造・保存・厳密算術上限に加え、測定元のexact `E`
そのものをpointer identity付きの安全なborrowで保持し、独立再生成`E1/E2`との
envelope組替えを拒否する。この`E`と閉有理envelopeを認証するprivate actual-mm scanも、
triangle dual-gate専用として三角形のmaterial faceだけを対象に実装した。scanは認証済みexact境界とhinge registryから
topologyを内部導出し、全canonical unordered face pairを走査する。肯定のdual gateは、
canonical exact `E`のzero-width厳密横断と、同じissuer-bound poseの保存binary64 affine係数・
rest頂点をactual-mm `BigRational`へ直接liftした実在triangleのzero-width厳密横断の双方である。
measured envelopeはexact pointer/pose authorityとdirect-lift各点のradius内包含を再検証する
だけで、独立per-face boxを共有点へweldしたり貫通幾何へ使ったりしない。
400 mm実fixtureでは角起点V字の正順/逆順・全root・指定全角度を貫通0、中点起点を
135/179度だけ貫通、90/91/180度はこのtriangle横断primitiveでは`Unresolved`として
固定した。入力厚さはbit表現まで`+0.0`だけを許可する。このprivate scan checkpoint
時点では、正厚、共有ヒンジ、非三角material faceの横断、public safe proof、
desktop current-pose診断・production UI・SIM-010への接続は未実装だった。後続の
公開入口は180度共面正面積重なりと非三角whole material face横断を別経路で補完したが、
SIM-010の状態と完成率は変更しない。

2026-07-19追記: Cayley有理核はallocation件数・個別bit・総bitを含む有限workの
原子的な再開・合算まで強化した。厚さ0幾何も入力、保持clone、演算、中間値、
GCD fallback、logical BigInt allocation、出力を一つの単調meterで全canonical unordered face pairへ累積し、
全pair dispatchを準備時に事前計算する。上記private scanでもexact側とbinary64 direct-lift側の
全pairおよびmeasured containment再検証が同じ再開・合算meterとatomic one-short境界を共有し、
各canonical pairの結果を欠落なく保持する。共有頂点では、共有occurrenceがbit-exactに同じ
zero-width singletonであり、そこから両面のrelative interiorへ正長区間が延びる場合に限る
証明を使う。独立box内に離間した実現姿勢が存在する監査反例も`Unresolved`へ固定した。
公開静的診断はblocking decisionで短絡せずpair数と
triangle-pair数を全件照合するが、複数面では有限ヒンジmodel未完成のため安全証明を
発行せず`PairEvidenceUnavailable`へ閉じる。これは分類基盤の内部品質であり、
MUST集計、SIM-010の未着手状態、完成率36.9%は変更しない。

2026-07-19追記: 上記private actual-mm scanを、nativeの公開静的衝突入口
`prove_static_collision_geometry`へblocking専用で接続した。bit-exactな紙厚`+0.0`
の複数面姿勢では、旧zero-thickness解析も同じexact pose instance、canonical face
registry、全canonical pairと全triangle-pairへ再結合し、件数照合を完了してから
blocking肯定を発行する。exactな`CoplanarAreaOverlap`は180度の共面正面積重なりとして
肯定し、`TransversalCrossing`は少なくとも一方が非三角whole material faceの場合に
旧集約から肯定できる。三角形どうしの横断は旧集約だけでは昇格せず、同じissuer/poseに
束縛したcanonical exact `E`とdirect-lift `F`の両zero-width gateを引き続き必須とする。
共有点・共有辺だけの点接触、線接触、共有要素接触はどちらの肯定経路にも入らない。
Rust公開error variantの`ProvenTransversalPenetration`は互換性のため維持するが、その
現在の意味は証明済みゼロ厚み面貫通・共面正面積重なりである。肯定0件または証拠不足は
`PairEvidenceUnavailable`、資源超過と姿勢不整合は既存のblocking errorへ閉じる。
安全証明のconstructorは従来どおり単一面・zero-pair枝の一箇所だけであり、複数面の
安全集合は拡張していない。

二段の解析は、triangle、pair、registry、境界関係、入力・保持clone・演算・GCD・
allocation・出力の全累積workを呼出側の一つの`StaticCollisionLimits`から差し引く。
Cayley側でもexact tree、measured envelope、blocking scanの三段をhard default以下へ
clampし、逐次残量と構造aggregate preflightを共有する。旧exact snapshotは全件照合後、
新exact姿勢の構築前に破棄し、ピーク保持量の二重化も避ける。400 mmの公開入口回帰は、
角起点山谷Vの片側10度と両45/90/91/135/179度を正逆source collection・全3 rootで
共有頂点接触の誤肯定0件、両180度を共面正面積重なり1件として分離した。辺中点山山Vは
135/179度だけを横断1件、90/91/180度を横断専用経路では判定保留として固定した。
非三角whole material faceの135度横断もsource順・全rootに依存せず1件となり、
三角形どうしはCayley dual gateを迂回できない。`-0.0`と正厚はこの肯定経路へ入らない。
正厚証拠、有限ヒンジcorridor、連続衝突、場所別層順transport、原子的
`ApplyStackedFold`は未完成なので、MUST集計、
SIM-010の未着手状態、完成率36.9%は変更しない。

2026-07-19追記: 表示中の完全姿勢をproject instance・project ID・revision・固定面・
全hinge角度とともにnative current-pose authorityへ適用するproduction commandと、
同じpose generationへ束縛した読み取り専用の静的衝突診断commandを接続した。
generationはJavaScript精度を失わない10進文字列で返し、frontendはapplyと診断の
binding 4項目が完全一致した結果だけを採用する。FoldPreviewが同じrenderで算出した
実描画姿勢keyを親の観測姿勢keyと照合し、新姿勢または移動中姿勢の最初のpaintから
旧い緑表示を隠す。frontendはapply→診断の二command transactionを一つずつ直列実行し、
待機要求は同一姿勢をまとめながら最新の異なる姿勢だけへcoalesceする。

native側にも、project置換では新しくならない`AppState`所有のprocess-wide RAII pose
worker gateを追加した。busy中は固定のredacted errorで拒否し、project、current/pending
pose authority、generationを変更しない。permitをblocking closureへmoveするため、
待機futureのcancel中も実workerが終わるまで保持し、通常完了・`Err`・panic/JoinErrorでは
必ず解放する。blocking診断は元の非Cloneなexact B capabilityを失敗結果に保持し、
project→poseの固定lock順で再検証したclosure内だけでbinding付きDTOを構成する。
同角度再apply、編集、reopenまたは別slotによりstaleになった結果はbindingなしの
`pose_authority_unavailable`となる。

C certificate発行後だけ「ゼロ厚み面貫通・重なりなし」、証明済みの横断または
共面正面積重なりは「ゼロ厚み面貫通・重なり・安全認定不可」とし、証拠不足・資源上限・
状態不整合は理由をバッジ本文にも表示する。wire reasonは
`proven_zero_thickness_penetration`、件数と最初のpairは`provenPenetratingPairs`と
`firstProvenPenetratingPair`へ一般化し、旧`proven_transversal_penetration`および
旧DTO fieldはfrontendのstrict parserで拒否する。確認中・姿勢確定待ちは
`status` / `polite`、終端blockingだけは`alert` / `assertive`とし、実行失敗時は
現表示姿勢を明示的に再試行できる。
診断は操作を巻き戻す権限を持たず、native連続停止は未接続である。

正厚については、exact lift後の半紙厚、有限軸、unit・軸直交のco-oriented材料法線、
`cos²(θ/2) > 2^-52`、`h² <= L² cos²(θ/2)`を証明するprivate scalarに加え、
1ヒンジ・2三角形面限定で、同じexact poseのissuer/version、左右共有辺の逆向き境界、
rest支持線の対向材料半平面、current共有端点、local `+Y`列、少なくとも一方の
三角形が有限軸区間内にあることを再認証するborrow tokenを追加した。tokenは
発行時の紙厚bit列を保持し、同一exact object・左右面/hinge index・同一紙厚bit列へ
private consumerが再結合できる場合だけ利用できる。全三角形対の
完全交差集合corridor包含、E/F位置・法線誤差、polygon・多ヒンジがそろうまで
本番許容へ接続しない。このprivate基盤は加算せず、利用者向けnative現在姿勢診断の
接続だけを3D領域へ計上して全体完成率を37.2%へ更新した。
MUST行の状態集計は **実装済み32 / 部分実装27 / 未着手28** のままである。

2026-07-19追記: 上記の有限ヒンジ前提tokenを消費する、正厚のprivate E/F境界
capabilityを追加した。同じ1ヒンジ・2三角形面について、全境界頂点の`F-E`位置差、
local `+Y`材料法線列の差、およびexact lift後の半紙厚`h`を用い、各軸で
`solid[k] = point[k] + h × normal[k]`を証明する。保存するL∞値は成分最大であり、
ユークリッド距離ではない。capabilityは前提token、exact object pointer、
native model/pose instance、紙厚bits、左右面・hinge index、全`F`係数bitsへ結合し、
ABA、foreign、reroot、角度・厚さ・matrixの1 ULP差および面swapを拒否する。
構造・厳密算術の全counterにはexact/one-shortとcaller非拡張hard capを固定した。
ただし、このcomponentwise boxは相関済みprism、共有軸weldまたは衝突分類の証拠ではない。
完全交差集合の有限hinge corridor包含、direct-lift `F`側の同一証明、
positive-volume/boundary-area分類、production safe proof、continuous collision、
layer transport、`ApplyStackedFold`は未完成なので、MUST集計と完成率は変更しない。

2026-07-19追記: 上記正厚境界の次段として、2個のexact三角柱について6頂点・
5閉半空間を各々検証し、合計10平面の全`C(10,3) = 120`三つ組をCramer法で解き、
全10半空間membership、canonical dedup、affine rank、正体積、対向support facetを
有限work内で返すprivate kernelを追加した。呼出側の同一累積budgetへA局所hard
envelopeを事前予約し、全additive/max counterをlocal meterで強制した実測deltaだけを
resetなしでmergeする。全counterのexact/one-short、先行large maximum、overflow、
facet index witnessを固定し、独立監査はCritical / High / Medium / Lowすべて0件だった。
このkernelはproduction分類から隔離され、有限hinge corridorによる全交差集合包含と
E/F両側の同一包含証明をまだ発行しないため、SIM-010と3D完成率には加算しない。

2026-07-19追記: PRJ-008として、projectごとにmm、cm、inch、または明示選択した
輪郭辺を1とする紙辺比を保存し、幅・高さ、2D計測、頂点・線座標、紙厚、3D紙厚説明を
同じsnapshotの単位で表示・入力できる利用者経路を接続した。内部幾何、native IPC、
FOLD/SVG/PDF/DXFはmm正本を維持する。紙辺比は一意な正長Boundary辺へ束縛し、
参照辺の削除・分割を自動rebaseせず拒否する。不正な保存済み参照は警告とmm修復表示へ
閉じる。換算入力は未編集binary64 mm値を保持し、紙厚button/Arrowはどの表示単位でも
元の物理mm値へdecimal `0.01 mm` stepを適用する。これによりPRJ-008を実装済みとし、
現在集計を実装済み33 / 部分実装27 / 未着手27へ更新する。

## プロジェクトと紙

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| PRJ-001 | 実装済み | 一作品一枚紙のproject modelと新規作成 |
| PRJ-002 | 実装済み | 輪郭分割・頂点編集で任意単純多角形を保持 |
| PRJ-003 | 実装済み | project単位の切断許可設定と保存 |
| PRJ-004 | 実装済み | UIとcoreで切断禁止を強制 |
| PRJ-005 | 実装済み | 接着要素を持たない |
| PRJ-006 | 実装済み | 0以上の紙厚を保存し、初版正式仕様の中央面基準近似で表示・衝突へ反映。新規作成の既定値は0.10 mm、上下ボタンは0.01 mm刻みとし、数値の直接入力にも対応 |
| PRJ-007 | 部分実装 | 表裏の単色は動作。画像・模様を未実装 |
| PRJ-008 | 実装済み | projectごとにmm/cm/inch/紙辺比を保存し、主要な寸法・座標・計測・紙厚・3D説明を一貫換算。紙辺比は一意な輪郭辺へ束縛し、失効警告、参照変更guard、Undo/Redo、未編集値保持、外部mm出力不変を回帰 |
| PRJ-009 | 部分実装 | 安定IDは実装。要素の名前・色・メモなし |

## 線種とレイヤー

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| LIN-001 | 実装済み | 5線種の作図・選択・削除 |
| LIN-002 | 実装済み | 初版対象を直線で保持 |
| LIN-003 | 実装済み | Boundary実線、Mountain一点鎖線、Valley破線、Auxiliary丸端点線、Cut二点鎖線をCanvas・取込見本・SVG書出へ共通適用し、白黒識別、紙面コントラスト、`stroke-linecap`取込cascadeと安全なwire伝搬を自動回帰・実画面検証 |
| LIN-004 | 未着手 | project layer modelと管理UIなし |
| LIN-005 | 未着手 | 表示・lock・透明度を持つlayer未実装 |

## 2D編集

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| EDT-001 | 部分実装 | 頂点・線の主要編集は動作。面編集と線全体移動を残す |
| EDT-002 | 部分実装 | mouse作図と既存頂点数値編集あり。数値からの直接作図なし |
| EDT-003 | 部分実装 | 座標と角度補助あり。長さ指定作図なし |
| EDT-004 | 未着手 | 分数・平方根・π・四則式parserなし |
| EDT-005 | 未着手 | 式の保持、高精度評価、式/評価値切替なし |
| EDT-006 | 実装済み | 9種snap、個別切替、優先順位、空間索引 |
| EDT-007 | 部分実装 | 平行・垂直等の補助あり。円・compass作図なし |
| EDT-008 | 未着手 | 永続的な11種幾何制約systemなし |
| EDT-009 | 未着手 | 制約矛盾の原因特定なし |
| EDT-010 | 未着手 | 左右・回転対称編集なし |
| EDT-011 | 実装済み | 不正・未完成状態の表示と保存を許可 |
| EDT-012 | 部分実装 | 3D移行のfail-closed遮断あり。問題位置・理由の網羅表示を残す |

## 検証と折り可能性

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| VAL-001 | 実装済み | 幾何・topology検証とUI結果表示 |
| VAL-002 | 部分実装 | 紙内部の単一頂点・ゼロ厚モデルで川崎・前川条件を全頂点へ検証し、対象外・構造遮断・次数上限と両条件の根拠をUI表示。指定山谷の局所十分性、他の局所条件、厚さモデルを残す |
| VAL-003 | 実装済み | 凸material face対象の`convex_faces_facewise_v1`で、時間制限つきの可・不可・不明3値判定と、証明済みの場所別`facewise_layer_order_v1`をUIから実行・確認できる。厚さ、連続折り経路、対象外形状は保証せず、不明へ分離する |
| VAL-004 | 部分実装 | 選択1ヒンジの線形CCDでは衝突直前停止と理由表示が動作。加えて表示済みnative姿勢の厚さ`+0.0`について、三角形どうしのdual-gate横断、非三角whole material face横断、180度の共面正面積重なりを厳密診断し、安全認定不可・判定保留を表示する。共有点・共有辺だけの接触は貫通へ昇格しない。native診断自体による連続経路停止、正厚、全3D折り操作経路への適用を残す |
| VAL-005 | 実装済み | 全体平坦折り判定で1〜300秒の時間制限を選択でき、単調phase・経過時間・上限付き件数を表示する。時間切れは不可でなく不明として返す |
| VAL-006 | 実装済み | 全体平坦折りpanelから実行中jobを中止でき、協調checkpointと世代照合により中止・再中止・旧job完了を現在結果へ混入させない |
| VAL-007 | 実装済み | immutable snapshot取得後にproject lockを解放し、native background workerで解析する。編集とUI操作を継続でき、進捗はpollingで受け取る |
| VAL-008 | 部分実装 | 選択1ヒンジでは衝突状態への適用遮断・停止理由・補正候補の解析専用UI（作業中・候補なし・判定不能・認定済み、stale無効化）が動作。候補3Dプレビュー、明示適用、全3D折り操作種別への適用を残す |
| VAL-009 | 実装済み | 全体平坦折りpanelで可・不可・不明・中止・計算エラー・古い結果を独立した終端状態として文言と表示属性で区別し、閉じた理由と対象クラスを表示する |

## 3D折りシミュレーション

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| SIM-001 | 実装済み | 面・hinge構築と3D表示。閉路・切断には制限あり |
| SIM-002 | 実装済み | hinge選択、slider、数値角度操作 |
| SIM-003 | 部分実装 | 3D把持は選択1ヒンジ回転へ限定 |
| SIM-004 | 実装済み | 固定面選択、reroot、従属面連動 |
| SIM-005 | 実装済み | 表示厚と判定厚を分離し、正式仕様`centered_mid_surface_v1`で衝突へ反映。有限ヒンジ長を超えるcorridorは層ずらし未再現として判定不能へ退避 |
| SIM-006 | 部分実装 | 1ヒンジの安全停止・原因表示に加え、同一generationへ束縛したnative静的診断を3D画面へ表示。process-wide RAII worker gate、busy時の全状態無変異、blocking結果のexact B再検証、stale結果のbinding除去を実装し、厚さ`+0.0`の証明済み面貫通・共面正面積重なりを一般化したwire/UIでblocking表示する。native診断は読み取り専用で、全操作経路の適用遮断・巻戻し、正厚には未対応 |
| SIM-007 | 部分実装 | 表裏色は反映。画像・模様textureなし |
| SIM-008 | 未着手 | 切断後の由来・接続を3Dへ反映しない |
| SIM-009 | 部分実装 | 1万辺の生成・2D表示・索引検証あり。基本編集・3D全体を未検証 |
| SIM-010 | 未着手 | 現在3D状態の一直線による複数層一括折り、層別山谷線の展開図追加、1 step記録を未実装。先行条件として、衝突分類v1/v2の全組合せ表と共通corpus、private layer-order capability、current pose certificate/generation失効、tree kinematics、認証済みexact境界・三角形分割・全pair coverage・共有関係分類、有理Cayley tree poseと有限資源meterまで完成した。bit-exactな厚さ`+0.0`では、三角形どうしのcanonical exact `E`＋direct-lift `F` dual-gate横断、非三角whole material faceのexact横断、180度のexact共面正面積重なりをnative公開静的入口へblocking専用で接続し、desktop current-pose apply、process-wide worker gate、同generation診断、stale binding除去、一般化したproduction警告UIまで到達した。正厚では有限半径scalar、1ヒンジ・2三角形面限定の共有境界・材料半平面・current共有端点・材料法線・有限軸方向範囲を同じexact poseへ束縛するprivate前提token、および同じ`E/F`の位置・法線差から正厚solidの成分別上限を封印するprivate capabilityまで固定した。さらに2個のexact三角柱の完全閉交差集合を全120平面三つ組から構成し、rank・正体積・対向support facetを有限累積budgetで返すprivate kernel、exact `E`側の全canonical交差頂点を閉じた有限ヒンジ回廊へ包含するprivate能力token、およびdirect-lift `F`側でcanonical affine half-prismへの完全包含・有界性・有限半径を示すC2診断まで完成した。C2は共有端点driftを許容するauthorityではなく、`ContainedUnadmitted`以外の安全認定を発行しない。全boundary point/normal/solidを束縛する共有hinge admission、native exact topology margin、正厚の本番証明、共有ヒンジ一般、H64既定上限内の完全成功、native連続衝突、cell-order transport、原子的`ApplyStackedFold` commandは未実装である。これらの完成前は折り重ねUIへ着手しない |

## 折り手順

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| INS-001 | 部分実装 | UIで手順の作成・編集・並べ替え・削除・`.ori2`保存・読込・段階再生が可能。連続動作の補間と経路安全認定を残す |
| INS-002 | 部分実装 | 1ステップへ現在の全hinge角を完全vectorとして保存・一括適用できる。複数hingeの連続同時動作は未実装 |
| INS-003 | 部分実装 | 固定面は姿勢とともに記録・再適用できる。持つ位置、押さえる位置、持ち替えは未実装 |
| INS-004 | 部分実装 | 折り角度、所要時間、説明文、注意事項を保存。camera位置、矢印、注目箇所を残す |
| INS-005 | 未着手 | 3D手指guideなし |
| INS-006 | 部分実装 | 3Dへ実際に適用された現在姿勢を手動stepとして登録できる。操作軌跡や連続動作の登録は未実装 |
| INS-007 | 未着手 | 3D操作の自動記録・再編集なし |
| INS-008 | 未着手 | 名前付き複合折り技法なし |
| INS-009 | 未着手 | 独自技法file共有なし |
| INS-010 | 部分実装 | timelineの各手順を固定3D図、説明、注意事項付きのA4複数ページPDFまたはページ別SVG画像ZIPとして書き出せる。滑らかなanimation、折る方向の矢印、手指guideを残す |

## ファイル入出力

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| IO-001 | 実装済み | 検証付き`.ori2`読込・保存・原子的置換 |
| IO-002 | 部分実装 | 展開図、紙の見た目、全姿勢付き折り手順を格納。独立した現在3D状態、memo、thumbnail、編集履歴を残す |
| IO-003 | 未着手 | 展開folder形式なし |
| IO-004 | 実装済み | FOLD 1/1.1/1.2の2D `creasePattern`と、SVG 1.1/2共通の静的直線subsetを、縮尺・線種・外周・情報損失の確認後に新規未保存projectへ取り込める。各形式の対応範囲外は契約どおり拒否または警告する |
| IO-005 | 実装済み | SVGのstroke、dash、class、layer、`data-origami-kind`をsource groupとして表示し、全groupを6種へ明示割当する画面、外周選択、Cut許可、警告確認を提供 |
| IO-006 | 実装済み | 現在の一枚紙展開図をFOLD 1.2、静的SVG、実寸PDF 1.7、DXF AC1021へ書き出せる。4形式とも情報損失確認、revision固定のimmutable stage、native原子的保存を共用し、形式固有の意味・実寸・資源上限を固定している |
| IO-007 | 未着手 | OBJ/STL/glTF exporterなし |
| IO-008 | 未着手 | 外部3D用途向け出力workflowなし |
| IO-009 | 部分実装 | FOLD/SVG/PDF/DXF書き出し前に、紙の見た目、ID・履歴、3D表示、折り手順、切断許可と形式固有の損失を表示し、明示確認をnativeでも強制する。将来の3D exporterへの共通適用を残す |

## 編集履歴と復旧

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| HIS-001 | 実装済み | command単位Undo/Redoとdirty連動 |
| HIS-002 | 未着手 | `.ori2`へ履歴を保存・復元しない |
| HIS-003 | 未着手 | 内部安全上限は最新128件。利用者が件数・容量を設定するUIは未実装 |
| HIS-004 | 未着手 | 定期autosaveなし |
| HIS-005 | 未着手 | crash recoveryなし |
| HIS-006 | 未着手 | recovery data workflowが未実装 |

## UI・アクセシビリティ

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| UI-001 | 未着手 | i18n基盤と日英切替なし |
| UI-002 | 実装済み | mouse操作と数値編集を同一画面に提供 |
| UI-003 | 実装済み | 2D/3Dの並列表示 |
| UI-004 | 実装済み | 2D/3Dの左右入替え、プロパティの左右移動、2D比率・プロパティ幅・折り手順高さのpointer/keyboard変更、初期化、version付き端末保存を実画面へ接続。3境界はARIA separatorと範囲・現在値・操作対象を公開 |
| UI-005 | 実装済み | system/light/darkの端末設定、起動前適用、OS変更追従、manual永続化、購読解除、native select、実効theme表示、`data-theme` CSS、WCAG contrast回帰と実画面確認 |
| UI-006 | 実装済み | Undo/Redo・削除・3D操作に加え、Ctrl/Cmd+N/O/S/Shift+Sを共通strict resolverから新規・開く・保存・別名保存へ接続し、ARIA、IME/repeat/modal/busy guardとWindows上macOS mapping testを維持 |
| UI-007 | 実装済み | 新規・開く・保存・別名保存・Undo・Redoのkey/Alt/Shift変更、Windows/macOS重複と固定Ctrl+Y別名の事前検出、version付き端末保存、動的title/ARIA、IME・repeat・変換key code fallbackを実画面へ接続 |
| UI-008 | 実装済み | mouse・trackpad相当pointer・keyboardを提供 |
| UI-009 | 部分実装 | 2D/3D hinge同期あり。頂点・面の相互強調を残す |

## 更新・診断・公開

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| OPS-001 | 未着手 | GitHub Releases更新確認なし |
| OPS-002 | 未着手 | 更新確認設定なし |
| OPS-003 | 未着手 | 明示更新workflow未実装 |
| OPS-004 | 実装済み | 固定schemaのredacted JSONだけをアプリ専用領域へ端末内保存し、明示操作で選んだ端末内ファイルにも保存できる。通信・自動送信なし |
| OPS-005 | 実装済み | 固定15 scopeの粗い件数区分だけを保存・表示し、作品名・形状・内容・path・ID・座標・時刻・アプリ版・OS・CPU・GPUを含めない |
| OPS-006 | 実装済み | Tauri版の診断ダイアログで正確なJSONを読取専用表示し、内容選択と同一bytesの手動保存、GitHub Issuesへ利用者自身で添付する案内を提供 |
| OPS-007 | 部分実装 | Windows CIは動作。Windows用パッケージのGitHub Releases正式配布を残す |
| OPS-008 | 実装済み | macOSでRust test・Clippyとfrontend production buildを含む`.app`生成をCI検証。オーナー決定どおり実機E2E・正式配布は初版対象外 |

## 更新ルール

各機能checkpointで該当IDだけを更新し、状態変更の根拠となるUI経路、保存形式、test、制限を短く追記する。内部品質だけの変更では状態を上げない。要件自体を変更する場合は`requirements-definition.md`と本表を同じcommitで更新する。
