# MUST要件 実装状況

更新日: 2026-07-20

現在の行単位集計は **実装済み86 / 部分実装1 / 未着手0**。

`requirements-definition.md`のMUST 87件を、利用者がUIから実行できるかを基準に追跡する。配布・CI要件だけは、要件に明記された自動検証または成果物が存在するかを基準とする。内部基盤やテストだけが完成していても、利用者機能がUI未接続なら「実装済み」へは上げない。

- 実装済み: 要件の主要な利用経路がUIから動作する。配布・CI要件は、指定された自動検証または成果物がそろう
- 部分実装: 有用な一部が動作するが、要件に明記された範囲を満たさない
- 未着手: 要件固有の利用経路が存在しない

現在の行単位集計は **実装済み57 / 部分実装25 / 未着手5**。2026-07-18のオーナー決定によりSIM-010を追加し、macOS自動ビルド・CI検証へ確定したOPS-008を実装済みとして再評価した。2026-07-19に時間制限つき全体平坦折り3値判定、場所別層順序、進捗・中止・background worker・終端状態の利用者経路を接続し、VAL-003/005/006/007/009を実装済みへ更新した。同日に長さ表示単位の保存・換算・編集UIを接続してPRJ-008を、白黒でも識別できる5線種を画面・取込・SVG書出へ接続してLIN-003を、ライト・ダーク・OS連動とWindows/macOS共通標準shortcutを接続してUI-005/006を、2D/3D・プロパティ・折り手順領域の位置と大きさの変更・端末保存を接続してUI-004を、主要shortcutの変更・重複検出・端末保存を接続してUI-007を実装済みへ更新した。さらに新規用紙の幅・高さへnative高精度数式入力、式保存、式/評価値切替を接続し、EDT-004/005を部分実装へ更新した。2026-07-20に11種幾何制約の保存・履歴・一覧・削除、水平/垂直の作成、直接矛盾の原因と判定保留表示を利用者経路へ接続し、EDT-008/009を未着手から部分実装へ更新した。同日にproject/session単位の履歴件数上限UIと、30秒周期の端末内自動保存、起動時の必須復元・破棄workflow、正常完了時の復旧slot整理を接続し、HIS-003/004/005/006を実装済みへ更新した。さらに通常`.ori2`と復旧checkpointへ認証済みUndo/Redo両stack・履歴件数上限を保存し、再読込後も利用できる利用者経路を接続してHIS-002を実装済みへ更新した。LIN-004はversion固定のproject layer文書、折り線edge assignment、編集command、履歴、通常保存・復旧・strict IPC snapshotに加えて、layer作成・改名・並べ替え・削除・選択折り線の割当UIまで接続したため部分実装を維持する。layer共通の表示・lock・透明度はLIN-005として接続済みだが、注釈・下絵object自体の作成・編集・描画は未実装のためLIN-004は実装済みにはしない。UI-001は言語設定の端末保存とライブ切替を、主要画面、ダイアログ、2D/3D、折り手順、ARIA、通知、固定native警告まで日英で接続したため実装済みへ更新した。さらに固定GitHub Releases APIへの明示的な手動更新確認、端末ごとの無効設定、プライバシー説明と日英状態表示を利用者経路へ接続し、OPS-001/002/003を実装済みへ更新した。同日に認証済みの現在3D姿勢をOBJ・バイナリSTL・GLBへ書き出す利用者経路を接続し、IO-007を実装済み、静的なBlender・3Dプリンター・Web等への受渡しをIO-008の部分実装へ更新した。名前付き複合折り技法は、日英の作成・厳格取込・複数技法選択編集・別名保存を1 MiB上限、通常ファイルno-follow読込、Rust/TypeScript二重検証、原子的保存へ接続してINS-009を実装済みとした。さらに技法情報・parameter・precondition・ordered operationを説明専用timeline案へ決定的に変換し、日英preview、明示確認、原子的追加、stale/取消/失敗時の無変更、一括Undo/Redoへ接続してINS-008を実装済みとした。技法から3D運動を生成・自動実行する機能はINS-008の実装根拠に含めない。展開フォルダー形式はstrict core、Windows/Unixのno-follow filesystem adapter、新規targetの原子的保存・読込、既存targetのimmutable phase journal差替え、native private registryによる起動時回復、pathless strict IPC、同一projectの安全な差替えと別projectの上書き拒否を明示する日英UIまで接続したためIO-003を部分実装として維持する。オーナー実施のWindows実機E2Eだけを残す。選択頂点を始点とする表示単位の正長・度単位角度・線種指定による終点と線の作図を一つのnative原子的commandへ統合し、一回のUndo/Redo、layer lock、切断許可、履歴永続化へ接続したためEDT-003を実装済みへ更新した。頂点の新規X/Y、既存頂点のX/Y移動、選択頂点からの長さ・角度にも原式とnative採用値のversion固定bindingを接続し、複数頂点差分を含むUndo/Redo、履歴上限、`.ori2`、展開folder、復旧保存・再読込、native再評価・bit一致検証まで完了したためEDT-005を実装済みへ更新した。第三者監査本文の旧集計値は各時点の履歴であり、以下の87行の判定を正本とする。

2026-07-20再集計: LIN-004、EDT-001/007/008/009/010/012、VAL-002/004/008、SIM-003/006/007/008/009、INS-001/002/003/004/005/006/007/010、IO-002/003/008、OPS-007、PRJ-007/009の完了に加え、証明済み単一hinge厚さ0のSIM-010利用者経路を部分実装へ接続したため、現在の正本87行は **実装済み86 / 部分実装1 / 未着手0**。

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
三角形どうしはCayley dual gateを迂回できない。`-0.0`はこの肯定経路へ入らない。
正厚の完全な三角柱証拠、有限ヒンジcorridor、連続衝突、場所別層順transport、原子的
`ApplyStackedFold`は未完成なので、MUST集計、
SIM-010の未着手状態、完成率36.9%は変更しない。

2026-07-20追記: 初版正式仕様`centered_mid_surface_v1`では、有限な正厚solidが
自身の材料中央面を内部に含む。この包含関係を使い、同じissuer-bound poseの
canonical exact `E`とdirect-lift `F`がともに三角形中央面のstrict transversalを
証明した場合だけ、正厚でも材料貫通をblocking肯定する限定経路を公開静的衝突入口へ
接続した。専用reason `proven_positive_thickness_penetration`をdesktop診断と
赤系の安全認定不可UIへ渡し、厚さ`0.1 / 1 / 3 mm`の辺中点山山Vについて
135/179度だけを貫通、90/91/180度を証拠不足へ固定した。角起点山谷Vは同じ3厚さと
片側10度・両45/90/91/135/179/180度の全組で正厚貫通へ誤昇格しない。
この経路は中央面横断だけの十分条件であり、正厚の共面重なり、境界面接触、
正体積三角柱SAT、共有ヒンジ、非三角面、有限corridorまたは安全証明を完成させない。

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
| PRJ-007 | 実装済み | 表裏それぞれへ単色または組込みのドット・格子・縞模様を設定できる。模様は安定AssetIdとしてproject、通常保存、復旧保存、Undo/Redoへ保持し、2D展開図と3D紙面へ反映する |
| PRJ-008 | 実装済み | projectごとにmm/cm/inch/紙辺比を保存し、主要な寸法・座標・計測・紙厚・3D説明を一貫換算。紙辺比は一意な輪郭辺へ束縛し、失効警告、参照変更guard、Undo/Redo、未編集値保持、外部mm出力不変を回帰 |
| PRJ-009 | 実装済み | 頂点・線・面の安定IDごとに名前・任意色・メモを設定できる。version固定文書、文字数・件数・重複の検証、選択要素の編集UI、原子的native command、Undo/Redo、通常`.ori2`・展開folder・復旧保存、strict IPC snapshotへ接続 |

## 線種とレイヤー

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| LIN-001 | 実装済み | 5線種の作図・選択・削除 |
| LIN-002 | 実装済み | 初版対象を直線で保持 |
| LIN-003 | 実装済み | Boundary実線、Mountain一点鎖線、Valley破線、Auxiliary丸端点線、Cut二点鎖線をCanvas・取込見本・SVG書出へ共通適用し、白黒識別、紙面コントラスト、`stroke-linecap`取込cascadeと安全なwire伝搬を自動回帰・実画面検証 |
| LIN-004 | 実装済み | version固定のordered layer文書で、折り線・補助線をedge assignmentとして管理し、注釈・下絵には専用content kindのlayerを提供する。日英UIからlayerの作成・改名・並べ替え・削除・選択線の割当、表示・非表示、編集lock、透明度を操作できる。全geometry分割時の継承、Undo/Redo、`.ori2`、展開folder、通常open・復旧へ同じ文書を保存する |
| LIN-005 | 実装済み | layerごとの表示・非表示、編集lock、0〜100%透明度をUI・native command・Undo/Redo・`.ori2`保存/復元へ接続。非表示/透明度0のgeometryは描画・選択・snap・交差候補から除外し、共有頂点は可視edgeがあれば表示、incident edgeが1本でもlockなら移動/削除を禁止する。lock edgeへ現行UI/nativeが行える削除・分割・交差/T字/cluster・再割当を二重guardで原子的に拒否し、lock解除は常に可能。旧layer recordは`visible=true / locked=false / opacity=1`へ移行する。 |

## 2D編集

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| EDT-001 | 実装済み | 頂点は選択・追加・数値/マウス移動・線分割による追加・削除、線は選択・追加・両端を同時に動かす全体移動・分割・削除を行える。topology faceは2D/3D共通選択に加え、全境界頂点の原子的平行移動、非隣接境界頂点間の線追加による面追加/分割、非輪郭共有線の削除による面削除/統合を行える。移動量は表示単位の数式入力に対応し、native再評価、project instance/revision、layer lock、幾何制約、履歴上限、Undo/Redo、通常/復旧保存へ接続 |
| EDT-002 | 実装済み | mouse作図に加え、日英プロパティUIから表示単位に従うX/Y座標を直接指定して頂点を作成できる。revision・project instance固定のnative編集、既定layer lock、Undo/Redo、新頂点選択を共用し、既存頂点の数値移動にも対応する |
| EDT-003 | 実装済み | 座標・角度補助に加え、選択頂点を始点として表示単位の正長・度単位の角度・線種から終点と線を一つのnative原子的commandで作成する。project instance/revision固定、既定layer lock、Cut許可、有限値・資源上限、履歴永続化をcoreで再検証し、一回のUndo/Redoで両要素を同時に戻す |
| EDT-004 | 実装済み | 新規プロジェクトの紙幅・紙高、頂点の新規X/Y座標、既存頂点のX/Y移動、始点からの長さ・角度、既存長方形用紙の幅・高さで、小数・分数・平方根・π・四則演算・括弧を含む式を入力できる。native高精度評価の隣接binary64区間だけを採用し、符号付き座標・角度、正長・正寸法、表示単位からmmへの有限変換を編集前に検証する |
| EDT-005 | 実装済み | 新規作成・既存長方形用紙resizeの幅/高さ、頂点の新規X/Y・既存X/Y移動、選択頂点からの長さ/角度について、表示単位込みの原式とnative採用値をversion固定schemaで保持する。native再評価・bit一致検証後に原子的変更し、複数頂点差分を含む式bindingのUndo/Redo両stackを全編集履歴と同期する。履歴上限、`.ori2`、展開folder、復旧保存・再読込、式/評価値切替表示まで接続済み |
| EDT-006 | 実装済み | 9種snap、個別切替、優先順位、空間索引 |
| EDT-007 | 実装済み | 選択頂点を中心に表示単位で半径を指定するコンパス円を最大64個まで重ね、消去できる。円は拡大率へ追従する補助線として2D上へ表示し、project置換時に破棄する。既存の角度二等分、垂線、平行線、長さ・角度指定、交点snapと組み合わせ、定規とコンパス相当の補助作図を提供する |
| EDT-008 | 実装済み | 固定長、固定角、水平、垂直、等長、平行、点の線上固定、鏡映対称、回転対称、角度二等分、長さ比率の全11種を、水平/垂直の直接buttonまたはstrict JSON作成UIから設定できる。unknown kind/field、非canonical ID、重複・dangling参照、非有限・範囲外値をfrontend/nativeで拒否し、project・`.ori2`・展開folder・復旧へ保存してUndo/Redo、一覧・対象選択・削除へ統合する |
| EDT-009 | 実装済み | 7種の直接矛盾をproject instance・project・revisionへ束縛して診断し、矛盾原因の種別とcanonicalな原因制約ID（各原因最大3件）をblocking表示する。完全solverが必要な系、資源上限、文書不正、通信失敗は矛盾なしと誤表示せず、目立つ判定保留へ閉じる |
| EDT-010 | 実装済み | 選択線を任意の垂直対称軸Xで左右反転、または任意の中心XY・角度で回転できる日英UIとstrict IPCを実装。全入力は数式評価値をnativeでbit一致再検証し、非有限値を拒否する。直角倍数はexact sin/cosを使い、両端点を1 commandで原子的に更新してlayer lock・幾何制約・Undo/Redo・座標数式履歴を維持する |
| EDT-011 | 実装済み | 不正・未完成状態の表示と保存を許可 |
| EDT-012 | 実装済み | 3D折り操作へ移行する前にnative topology検証を自動実行し、project/revisionへ束縛した結果が不合格なら3D modelを生成せずfail-closedで遮断する。全blocking/fatal問題を日英の理由別に表示し、関連する全頂点・全線IDを列挙して2D/3D共通選択へ移動できる |

## 検証と折り可能性

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| VAL-001 | 実装済み | 幾何・topology検証とUI結果表示 |
| VAL-002 | 実装済み | 紙内部の各単一頂点についてゼロ厚モデルの川崎定理・前川定理を全件検証し、両条件の成立・不成立と根拠をUI表示する。境界頂点、構造不正、次数上限など定理の適用条件外は、不合格と混同せず理由付きの対象外として表示する |
| VAL-003 | 実装済み | 凸material face対象の`convex_faces_facewise_v1`で、時間制限つきの可・不可・不明3値判定と、証明済みの場所別`facewise_layer_order_v1`をUIから実行・確認できる。厚さ、連続折り経路、対象外形状は保証せず、不明へ分離する |
| VAL-004 | 実装済み | 3Dのマウス把持・角度指定で用いる選択hingeの連続経路をCCD検証し、最初の衝突直前で自動停止して衝突面・位置・理由を表示する。衝突状態のendpointは適用しない。表示済みnative姿勢にも全unordered face pairの公開6分類と証明根拠を接続し、証明不能候補は安全扱いせず赤い判定保留へ閉じる。任意の無衝突経路探索は行わない |
| VAL-005 | 実装済み | 全体平坦折り判定で1〜300秒の時間制限を選択でき、単調phase・経過時間・上限付き件数を表示する。時間切れは不可でなく不明として返す |
| VAL-006 | 実装済み | 全体平坦折りpanelから実行中jobを中止でき、協調checkpointと世代照合により中止・再中止・旧job完了を現在結果へ混入させない |
| VAL-007 | 実装済み | immutable snapshot取得後にproject lockを解放し、native background workerで解析する。編集とUI操作を継続でき、進捗はpollingで受け取る |
| VAL-008 | 実装済み | VAL-004で停止した選択hinge操作は衝突状態へ適用せず、停止理由と、現在pose・modelへ束縛した修正候補を専用UIへ表示する。候補解析の作業中・候補なし・判定不能・認定済みを区別し、pose変更後のstale候補は直ちに無効化する |
| VAL-009 | 実装済み | 全体平坦折りpanelで可・不可・不明・中止・計算エラー・古い結果を独立した終端状態として文言と表示属性で区別し、閉じた理由と対象クラスを表示する |

## 3D折りシミュレーション

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| SIM-001 | 実装済み | 面・hinge構築と3D表示。閉路・切断には制限あり |
| SIM-002 | 実装済み | hinge選択、slider、数値角度操作 |
| SIM-003 | 実装済み | 3D上の紙面をマウスまたはpenで直接つかみ、選択hingeを軸とする物理grabとして折り角度を操作できる。front/backの把持側、固定面、camera、pointer競合を明示的に管理し、連続衝突検証後の安全なendpointだけを適用する |
| SIM-004 | 実装済み | 固定面選択、reroot、従属面連動 |
| SIM-005 | 実装済み | 表示厚と判定厚を分離し、正式仕様`centered_mid_surface_v1`で衝突へ反映。有限ヒンジ長を超えるcorridorは層ずらし未再現として判定不能へ退避 |
| SIM-006 | 実装済み | 3D把持・角度指定の連続経路を衝突直前で自動停止し、衝突した面・位置・原因を3D画面へ表示する。同一generationへ束縛したnative静的診断も全pairの6分類とproof provenanceを表示し、`penetrating`と証明不能な`indeterminate`を安全側のblocking表示へ統合する。busy・stale・改変応答は現在poseを変更しない |
| SIM-007 | 実装済み | 3D紙面の表裏へ個別の色と組込み模様textureを反映し、設定変更時にscene resourceを安全に再生成・解放する |
| SIM-008 | 実装済み | topology snapshotへ元紙ID付きの決定的material componentを追加し、境界間Cut、closed cut loop、紙内部または境界接続のopen/branched CutをDCEL walkから分類する。Faceは外周・時計回り穴・zero-area seamをversion互換表現で保持し、切断後の各pieceを同じsheet originへ束縛する。Cut incidenceは両岸faceを保持するがhinge adjacencyへ入れず、frontendは全face/component partition、穴・seam、面積を再検証し、穴を両面capと厚み壁へ反映して全pieceを静的3D表示する。切断禁止は従来どおり拒否する |
| SIM-009 | 実装済み | nativeで10,000本の実edgeを生成・転送して2D Canvasへ表示し、空間索引による選択・snapと描画FPS/p95を計測できる。性能データ上で頂点drag（incident edge座標を一括更新）と線削除の基本編集を実行できる。10,000要素・faceのbroad-phase、snap、parallel/angle補助も専用回帰と資源上限で検証する |
| SIM-010 | 部分実装 | 証明済みの単一hinge・厚さ0ケースについて、2D Canvasで選択した一直線、固定側、回転方向、角度を日英UIからnative read coordinatorへ渡し、材料面ごとのMountain/Valley候補、終端衝突、連続経路、target層順序、topology差分を表示する利用者経路を接続した。native private registryはcurrent pose・layer order・project instance/revision/fingerprint/generationへ結合した非直列化premiseをopaque transaction tokenとして保持し、成功ケースだけ明示確認後のtoken-only Applyを許可する。Applyはtarget pattern、paper、project layers、完全hinge角を持つtimeline 1 step、applied poseを一つのeditor履歴entryとして原子的に確定し、失敗時は文書を変更せず、成功時だけtokenを消費する。既存Undo/Redoで変更全体を一回で復元・再適用でき、入力・選択・revision変更、取消、再入、stale/ABAはfail-closedで失効する。一般姿勢へ広げた複数層transport、正厚の完全な衝突・共有hinge admission、多hingeの連続経路とclosureを含むApply、適用後を含む専用層順viewerは未完成であり、証明を発行できないケースではApplyを無効にして理由を表示するため部分実装とする |

## 折り手順

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| INS-001 | 実装済み | UIで折り手順timelineの作成・編集・並べ替え・削除、通常`.ori2`と復旧checkpointへの保存・読込、選択位置からの順次再生・停止が可能。各stepの姿勢適用をproject/revision/model fingerprintへ束縛し、stale・不正・適用失敗時は再生を停止する |
| INS-002 | 実装済み | 1 stepへ現在modelの全hinge角を一つの完全vectorとして保存し、固定面とともに一括適用できる。複数hingeの異なる角度を同じstepで同時に3D姿勢へ反映し、欠落・重複・未知hingeを拒否する |
| INS-003 | 実装済み | 各stepの姿勢へ固定面を記録・再適用し、同じstepの手指guideへ`pinch`の持つ位置、`hold`の押さえる位置、各位置からの方向を記録できる。前後stepでguideを変更することで持ち替えを明示し、通常のstep編集・並べ替え・Undo/Redo・保存・復旧で一体管理する |
| INS-004 | 実装済み | 折り角度、所要時間、説明文、注意事項に加え、任意の3D camera位置・注視点・上方向、複数の方向矢印、複数の注目箇所を手順ごとに保存・編集・Undo/Redo・復旧できる。旧手順は空visualへ後方互換読込し、件数・有限値・半径・labelをnative/frontend双方でfail-closed検証する。段階再生時はcameraを復元し、3D上へ矢印と注目範囲を表示する |
| INS-005 | 実装済み | 各手順の視覚注釈へ`pinch`（指先・つまみ）、`hold`（押さえ）、`push`（移動）を、3D位置・方向・label付きで追加できる。位置には接触ring、方向には矢印を3D上へ表示し、種別を色分けする。camera・折り方向矢印・注目箇所と同じ編集UI、Undo/Redo、通常`.ori2`保存・読込、復旧checkpointへ統合した。旧visualは空guideへ移行し、未知種別、非有限座標、ゼロ方向、過大・制御文字label、合計marker上限超過をRust/TypeScript双方で拒否する |
| INS-006 | 実装済み | 3Dへ実際に適用された現在姿勢を手動でtimelineへ登録できるほか、3D操作の自動記録を利用者が切り替え、初期化・再生を除く手動操作の完了ごとに通常の編集可能なstepとして追加できる。model fingerprint・pose identity・完了状態を検証し、重複観測やstale姿勢を登録しない |
| INS-007 | 実装済み | 3D panelの明示toggleをONにした後、手動で完了した3D姿勢変更を完全hinge vector・固定面付きの通常手順として自動追加する。ON直後の既存姿勢、timeline再生、running姿勢、stale binding、同一操作の重複は記録しない。通常の再編集に加え、一手順を時間比で隣接二手順へ分割し、隣接二手順を一つへ結合できる。分割・結合は周辺順序、pose、metadata、合計時間をcoreで再認証する単一commandであり、一回のUndo/Redo、永続履歴、通常`.ori2`と復旧保存へ接続する |
| INS-008 | 実装済み | 名前付き宣言技法の技法情報・parameter・precondition・ordered operationを欠落なく説明専用timeline案へ決定的に変換し、日英previewと明示確認後に一つの原子的commandで追加できる。project instance/revision/選択変更をstale拒否し、取消・失敗は無変更、全追加を一回でUndo/Redoできる。中割り・かぶせ・沈め・層選択・折り重ねは注意付き説明だけで、3D姿勢や物理commandを実行しない。`.ori2`/Project Folderの専用required feature、履歴・復旧、PDF/SVG placeholderを備える |
| INS-009 | 実装済み | 日英UIから独自技法を新規作成して初回保存し、既存の共有JSONを取り込み、最大64技法を選択編集して別名保存できる。path/raw bytesはWebViewへ渡さず、1 MiB上限、通常ファイルno-follow、strict V1 JSON、Rust正本検証・決定的read-back・TypeScript再検証、single-flight、原子的保存を適用し、取消・失敗・古い応答では現在状態を変更しない |
| INS-010 | 実装済み | timeline再生では各stepの所要時間に合わせ、開始poseから全hinge角を同時にanimation frame補間し、model・revision変更、手動操作、非表示、停止で取消する。各手順はcamera・折る方向矢印・注目箇所・手指guideとともに3D表示できる。さらに全stepを固定3D図、説明、注意事項付きのA4複数page PDFまたはpage別SVG画像ZIPとして書き出せる |

## ファイル入出力

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| IO-001 | 実装済み | 検証付き`.ori2`読込・保存・原子的置換 |
| IO-002 | 実装済み | 展開図、紙の見た目、全姿勢付き折り手順、独立した現在3D状態、project memo、安全な決定的SVG thumbnail、認証済みUndo/Redo両stackと履歴件数上限を格納。現在3D状態は再読込時にnative topology・kinematicsを再検証してauthorityを再発行する |
| IO-003 | 実装済み | `.ori2`と同じ`ProjectDocument` / `EditorHistoryV1`正本を、version固定JSON・画像（読取専用SVG preview）・strict manifestへ決定的に展開するfolder形式として保存・読込できる。hash/sizeを認証し、symlink/junction/reparse point/hard link・余分entry・差替えを拒否する。新規targetはno-replaceで原子的に公開し、同一projectの既存targetはimmutable phase journal、完全なWindows file ID、private registry、起動時回復を通す。path/bytes/FS名をWebViewへ出さず、日英UIから安全に利用できる |
| IO-004 | 実装済み | FOLD 1/1.1/1.2の2D `creasePattern`と、SVG 1.1/2共通の静的直線subsetを、縮尺・線種・外周・情報損失の確認後に新規未保存projectへ取り込める。各形式の対応範囲外は契約どおり拒否または警告する |
| IO-005 | 実装済み | SVGのstroke、dash、class、layer、`data-origami-kind`をsource groupとして表示し、全groupを6種へ明示割当する画面、外周選択、Cut許可、警告確認を提供 |
| IO-006 | 実装済み | 現在の一枚紙展開図をFOLD 1.2、静的SVG、実寸PDF 1.7、DXF AC1021へ書き出せる。4形式とも情報損失確認、revision固定のimmutable stage、native原子的保存を共用し、形式固有の意味・実寸・資源上限を固定している |
| IO-007 | 実装済み | 認証済みの現在3D姿勢をOBJ・バイナリSTL・glTF 2.0 GLBへ、形式別の単位・座標系と決定的な三角形分割で書き出せる。project instance・ID・revision・geometry fingerprint・pose generation・紙厚へ束縛し、編集または姿勢変更後の古いpreviewは保存できない |
| IO-008 | 実装済み | OBJ/GLBをBlender・Web・他3Dアプリへ、STLを3Dプリンター向けworkflowへ受け渡せる静的・texture・animation出力を実装。GLBはPBR material、PNG/JPEG埋込texture、UV、STEP morph-target animationを保持し、Khronos Validatorでerror/warning/info 0、固定revisionのKhronos Sample Viewer＋headless Chromium/WebGLでstatic/textured/animatedを実ロードして可視描画、console/runtime error 0、animation frame変化を自動受入する。固定Blender 4.5.11 LTS CLIでOBJ/STL/GLBを実importし、mesh・material・image・animation・morph target・軸・mm/m変換・bounds・open-sheet manifold metadataを検査する。正厚STLは固定PrusaSlicer 2.9.6 CLIでmanifold、10×10×2 mm寸法、200 mm³体積、12 triangle、repair warning/error 0を確認し、open sheetと未証明non-manifold meshを期待失敗にする。G-codeも6 layer、有限XYZ、model Z boundsを検査する。紙厚正GLBは表裏capと境界wallを持つclosed solidへ分類し、binary64 exact一致する共面隣接faceと認証済み限定2面1hingeの内部wallを除去してwatertight外殻へ統合する。形式固有の独立reader、release生成、checksum固定外部ツール、CI gateを含めて受渡し要件を満たす |
| IO-009 | 実装済み | 現行の全9形式（展開図FOLD/SVG/PDF/DXF、折り手順PDF/SVG ZIP、3D OBJ/STL/GLB）で、形式固有の情報損失・省略・非保証事項を保存前に表示する。各dialogは明示確認まで保存を無効化し、native commitも確認値を独立に強制する。将来形式は追加時に同じ回帰matrixへ加える |

## 編集履歴と復旧

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| HIS-001 | 実装済み | command単位Undo/Redoとdirty連動 |
| HIS-002 | 実装済み | 通常`.ori2`へproject ID・`project.json` SHA-256に束縛したUndo/Redo両stackと1〜128件の履歴上限を保存し、全27 command・20 inverseの安全な巻戻し・再生、生成inverseと現在文書のbit-exact照合、currentと全Undo/Redo到達endpointのinstruction pose検証後にrevision 0で復元する。既定128件の空履歴はentryを省略してlegacy 2-entry bytesを維持し、旧archiveは既定空履歴として開く。不正・付替え履歴は既存project不変で拒否し、dirty復旧checkpointにも同じ履歴と上限を保存する |
| HIS-003 | 実装済み | 現在のproject/sessionについて1〜128件の履歴上限を、project instance・project ID・revisionへ厳密に束縛したUIから明示適用できる。縮小時はUndo/Redo両stackの最古を即時trimし、revision・document・dirty・3D poseを変えない。増加しても破棄済み履歴は復元しない |
| HIS-004 | 実装済み | native timerが30秒周期でdirty documentとcheckpoint時点のUndo/Redo・履歴上限を同じsnapshotへ取得し、project lock解放後にcurrent 1状態とUndo/Redo到達先合計最大256状態（総検証最大257状態）を検証して、通常`.ori2`と同じstrict archive reader・履歴意味再認証・hard limitsでアプリ専用の固定1 slotへ自動保存する。history-only変更はdocument dirtyを変えず、明示保存またはdocument dirty時のcheckpointで永続化する。dirty中のrevision不変な履歴差分もUndo/Redo・履歴上限全体のSHA-256 digestで検出して次のcheckpointへ反映し、digestまで同一の場合だけ重複I/Oを省略する。verified atomic publish、latest-one generation fence、single writerにより古い世代を公開しない。保存healthは匿名の3状態DTOだけを5秒single-flight監視し、失敗・監視不能をassertive警告、再開をpolite通知する |
| HIS-005 | 実装済み | 単一instanceを先に確立し、起動時に復旧slotを`none / available / invalid`へ分類する。履歴のhash、project binding、意味再認証または全到達endpointのinstruction pose topology検証の失敗も`invalid`とし、候補がある間は自動上書きを止め、利用者が復元または破棄を完了するまで編集画面を解放しない。復元はcheckpoint時点のUndo/Redo・履歴上限も維持し、runtime poseだけは復元しない |
| HIS-006 | 実装済み | 復元は保存済みproject IDとcheckpoint履歴を維持しつつfresh instance・pathなし・revision 0・dirtyとして開くため、元の保存ファイルを暗黙に上書きしない。正常なsave/new/open/FOLD・SVG importは完了時bindingが現在値と一致する場合だけslotをclearする。通常終了は初回要求を止め、nativeの10秒・1回限りtokenへproject instance・project ID・revision・clean/破棄確認を束縛し、2回目の要求で再検証してから5秒以内にclearできた場合だけ終了する。stale・取消・失効・clear失敗では自動保存を継続する。不正entryは最終要素をfollowせず固定8 quarantine名へ排他的に退避してentryだけを削除し、枠不足時はactive entry不変で失敗へ閉じる。未処理候補は保持し、raw path/errorの表示と外部通信を行わない |

## UI・アクセシビリティ

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| UI-001 | 実装済み | 日本語・英語を端末設定として保存し、再読込後も維持する。主要画面、全ダイアログ、2D/3D、折り手順、制約・平坦折り、ARIA、通知、既知native警告を意味状態からライブ再翻訳し、未知警告・例外・pathは固定安全文言へ閉じる。全TSX静的監査、Node 1,310件、DOM 165件、lint・型検査・production buildとブラウザ実画面で回帰 |
| UI-002 | 実装済み | mouse操作と数値編集を同一画面に提供 |
| UI-003 | 実装済み | 2D/3Dの並列表示 |
| UI-004 | 実装済み | 2D/3Dの左右入替え、プロパティの左右移動、2D比率・プロパティ幅・折り手順高さのpointer/keyboard変更、初期化、version付き端末保存を実画面へ接続。3境界はARIA separatorと範囲・現在値・操作対象を公開 |
| UI-005 | 実装済み | system/light/darkの端末設定、起動前適用、OS変更追従、manual永続化、購読解除、native select、実効theme表示、`data-theme` CSS、WCAG contrast回帰と実画面確認 |
| UI-006 | 実装済み | Undo/Redo・削除・3D操作に加え、Ctrl/Cmd+N/O/S/Shift+Sを共通strict resolverから新規・開く・保存・別名保存へ接続し、ARIA、IME/repeat/modal/busy guardとWindows上macOS mapping testを維持 |
| UI-007 | 実装済み | 新規・開く・保存・別名保存・Undo・Redoのkey/Alt/Shift変更、Windows/macOS重複と固定Ctrl+Y別名の事前検出、version付き端末保存、動的title/ARIA、IME・repeat・変換key code fallbackを実画面へ接続 |
| UI-008 | 実装済み | mouse・trackpad相当pointer・keyboardを提供 |
| UI-009 | 実装済み | 2D/3Dで線・頂点・面のID選択を相互同期。2D面塗り、3D面輪郭・頂点marker、共有頂点の全instance強調、現在姿勢変換、collision優先表示を実装し、cross-view/picking/DOM回帰と実ブラウザで検証 |

## 更新・診断・公開

| ID | 状態 | 現在の根拠・不足 |
|---|---|---|
| OPS-001 | 実装済み | 「今すぐ確認」の明示操作時だけ固定GitHub Releases latest APIへ資格情報・referrer・bodyなしで接続し、10秒timeout・128 KiB上限、strict SemVerと公式release URL検証後に、更新なし・更新あり・公開releaseなし・確認不能を日英表示する。起動・画面開閉・言語切替では通信せず、自動取得・自動導入を行わない |
| OPS-002 | 実装済み | 更新確認の有効・無効をversion固定の端末設定として保存し、UIから切り替えられる。破損・読取不能は無効へ閉じ、書込失敗はこの起動中だけの設定であることを画面へ表示する |
| OPS-003 | 実装済み | 更新確認前に、標準的な接続metadata以外の作品データ・利用状況・インストール済み版を送信せず、自動ダウンロード・自動インストールもしないことを日英で明示する。表示する外部linkは検証済み公式GitHub release pageだけに固定する |
| OPS-004 | 実装済み | 固定schemaのredacted JSONだけをアプリ専用領域へ端末内保存し、明示操作で選んだ端末内ファイルにも保存できる。通信・自動送信なし |
| OPS-005 | 実装済み | 固定15 scopeの粗い件数区分だけを保存・表示し、作品名・形状・内容・path・ID・座標・時刻・アプリ版・OS・CPU・GPUを含めない |
| OPS-006 | 実装済み | Tauri版の診断ダイアログで正確なJSONを読取専用表示し、内容選択と同一bytesの手動保存、GitHub Issuesへ利用者自身で添付する案内を提供 |
| OPS-007 | 実装済み | `v*` tagまたは既存tagを指定する手動dispatchから、tag・commit・全versionを固定してfrontend/Rust全検証、unsigned Windows NSIS生成、resource・release notes・SHA-256再検証を行う。保護environmentと最小`contents: write`権限のpublish jobは、既存Releaseの上書きを拒否し、同一の検証済みartifactをdraft作成後に正式なlatest GitHub Releaseとして公開する。契約testは署名状態、trigger、権限、tag ABA、asset集合、取消不能publishを回帰する |
| OPS-008 | 実装済み | macOSでRust test・Clippyとfrontend production buildを含む`.app`生成をCI検証。オーナー決定どおり実機E2E・正式配布は初版対象外 |

## 更新ルール

各機能checkpointで該当IDだけを更新し、状態変更の根拠となるUI経路、保存形式、test、制限を短く追記する。内部品質だけの変更では状態を上げない。要件自体を変更する場合は`requirements-definition.md`と本表を同じcommitで更新する。
