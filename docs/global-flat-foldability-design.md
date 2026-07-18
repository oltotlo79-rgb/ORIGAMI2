# 全体平坦折り判定と層順序管理の設計

## 1. 目的

本書はVAL-003、VAL-005、VAL-006、VAL-007、VAL-009を満たし、SIM-010の折り重ね操作が利用する層順序の正本を定義する。

- 設定された時間制限内で全体平坦折りを`possible / impossible / unknown`の3値で返す
- 時間切れ、資源上限、数値的曖昧、対象外、証明不足を`impossible`と誤表示しない
- 利用者による中止、計算エラー、古い結果を3値から分離する
- `possible`の根拠となった平面配置、重なり領域、面の層順序を後続の3D操作へ渡す
- 編集中のprojectをロックせず、判定中止や失敗で作品を変更しない

判定モデルIDは`convex_faces_facewise_v1`、層順序モデルIDは`facewise_layer_order_v1`とする。同じIDの意味を後から変更せず、対象クラスや制約を拡張する場合は新しいIDを追加する。

初版の紙厚仕様`centered_mid_surface_v1`は3D表示と衝突判定の近似仕様である。本判定は理想的な厚さ0の平坦折りだけを扱い、現実の紙厚、層ずれ、弾性、摩擦、折りやすさ、平坦状態へ至る連続経路を保証しない。

## 2. 根拠

facewise層順序と制約は、Akitaya、Demaine、Kuによる[Computing Flat-Folded States](https://erikdemaine.org/papers/FlatFolder_OSME2024/paper.pdf)を基礎とする。この方式は、凸面からなる展開図について、正の面積で重なる面対の上下関係へ反対称、非交差、推移等の有限個の条件を課す。

実装構造とfixtureの参考として、MIT Licenseの[Flat-Folder](https://github.com/origamimagiro/flat-folder)をcommit `d50004815fb738d009e5b87b2307fbaefa717ef0`へ固定して調査した。ただし同実装が入力正規化にepsilonを用いる部分は移植しない。ORIGAMI2では保存済みbinary64を正確な有理数へ変換し、資源上限内で証明できない場合は`unknown`とする。

層順序の交換形式は[FOLD 1.2](https://edemaine.github.io/fold/doc/spec.html)の`faceOrders`を参照する。ただしFOLDの面対順序だけではSIM-010に必要な場所別の重なりを失うため、内部正本は重なりcellと各cellのstackも保持する。

## 3. 対象クラス

`convex_faces_facewise_v1`が`possible`またはconstraint探索完了による`impossible`を返せる対象は、次をすべて満たす展開図とする。

1. 現在revisionの`TopologySnapshot`がsimulation-readyである
2. 切断線、穴、非単純面、未接続材料を含まない
3. 全material faceが正の面積を持つ凸多角形である
4. 全内部ヒンジがMountainまたはValleyへ割り当て済みである
5. 全頂点の局所必要条件が適用対象外、成立、または明示的な違反として確定している
6. 平面反射、重なりcell、制約、探索を第11章の時間・資源上限内で完了できる

対象外の入力は`unknown`であり、`impossible`ではない。将来、非凸面を証明付きで凸分割する場合も、同名モデルの意味を暗黙に広げず新しいモデルIDを追加する。

初版実装は次の順で対象を広げ、4段階すべてを実装済みとする。

1. 0ヒンジ: 1面の自明な`possible`
2. 1ヒンジ: Mountain/Valleyに対応する2面の`possible`
3. 局所必要条件の明示的違反: `impossible`
4. 複数凸面: 平面配置、重なりcell、facewise制約、時間制限探索

複数凸面を含む第4段階をUIから利用でき、時間切れ・中止・計算エラーを区別し、証明済み層順序をnativeへ保持できるため、VAL-003を実装済みとする。対象クラス外の入力を`unknown`にする契約は変わらない。

## 4. immutable入力とprovenance

### 4.1 core provenance

coreの判定結果と層順序は、次の`GlobalFlatFoldabilityProvenance`を正本として共有する。

```text
GlobalFlatFoldabilityProvenance
├─ identity_namespace: ProjectId
├─ source_revision
├─ source_fingerprint: FoldModelFingerprintV1
└─ model_id: convex_faces_facewise_v1
```

`FoldModelFingerprintV1`はdomain separatorを持つversion 1のSHA-256である。頂点・辺をstable IDでcanonical sortし、無向辺の端点方向、紙境界cycleの開始位置と反転を正規化する。頂点座標と紙厚は保存済みbinary64のIEEE-754 bit列をそのまま含め、線種、切断可否、紙境界を含める。表裏色、作品名、折り手順等、折りモデルを変えない見た目・説明dataはfingerprint対象外とする。符号化項目または正規化を変える場合は同じversionを再利用しない。

geometryを伴う`GlobalFlatFoldabilityInput`は`identity_namespace`、paper、crease patternを必須とする。coreはfingerprintを協調checkpoint付きで再計算し、同じimmutable geometryからtopologyと局所平坦折りreportを独立に再生成する。呼出側から渡されたtopology/local artifactと完全一致しない場合は証明へ進まず`unknown`にする。geometryまたはidentityのない互換入力経路は証明根拠を持たないため、可・不可を返さない。

`possible`と`impossible`はidentity、revision、fingerprint、proof modelがすべて存在し一致する場合だけ公開できる。fingerprint計算前のdeadlineまたはsource上限で終わった`unknown`はfingerprintを持たなくてよいが、fingerprintを持つ`unknown`が期待値と異なる場合はnative境界で拒否する。

### 4.2 native instance bindingとABA防止

native側は開始時にproject mutex下で次を一つのbindingへ取得し、mutexを解放してからbackground workerへ渡す。

```text
GlobalFlatFoldabilityBinding
├─ project_instance_id
├─ project_id
├─ revision
├─ topology_input (paper + crease pattern + identity + revision)
└─ fold_model_fingerprint
```

`project_instance_id`はproject生成・再openごとに変わるopaque IDである。WebViewからの開始commandにはproject ID、revisionに加えて64桁小文字hexの期待fingerprintを必須とし、通常経路ではnative再計算値と一致しなければsnapshotを取得しない。bindingは`Arc`一個だけを生成し、active job、completion context、current layer-order slotがcloneではなく同じ`Arc`を共有する。完了採用時はjob IDに加えて`Arc::ptr_eq`、project instance、project ID、revision、`topology_input.is_current_for`、fingerprintを再照合する。これにより、同じrevisionへ戻る編集、同一project IDの別内容、project再open、旧worker完了によるABAを拒否する。

source頂点、source辺、紙境界頂点、合計record数は、canonical fingerprint、snapshot clone、topology/local index構築より先に件数だけを確認する。ここで上限を超えた場合に限り、未登録の`completed / unknown / work_limit_reached`を即時返す。この例外経路は新しいactive jobを設置せず、既存jobをcancelせず、current layer-order slotも変更しない。可・不可または層順序を生成しないためfingerprint内容照合は行わないが、project ID/revisionと要求fingerprint形式の検証は先に行う。

WebViewへproject instance ID、fingerprintの構成material、全座標、探索stack、raw errorを渡さない。UIへ返すのはopaque job ID、閉じたphase、上限付き件数、判定値、固定理由、表示用の面番号と証明概要だけとする。

## 5. 実行状態と3値判定

実行状態と判定値を別の型にする。

```text
JobState =
  queued
  | running
  | completed(GlobalFlatFoldabilityResult)
  | cancelled
  | failed(ClosedErrorCategory)
  | stale

GlobalFlatFoldabilityVerdict =
  possible
  | impossible
  | unknown
```

### 5.1 `possible`

`possible`は、対象クラス内で平面配置と全facewise制約を満たす層順序を少なくとも一つ構成し、そのcertificateを再検証できた場合だけ返す。必ず`LayerOrderSnapshot`を伴う。

### 5.2 `impossible`

`impossible`は次のいずれかを有限の根拠として確定した場合だけ返す。

- currentかつcompleteな局所必要条件reportに明示的な違反がある
- 平面反射を異なる経路で伝播した結果がexactに矛盾する
- Mountain/Valleyから固定された層順とfacewise制約が矛盾する
- 全未確定変数の有限探索を完了し、解が一つもない

前段のtopology不正、未対応cut、数値上限、panic、I/O失敗、時間切れは`impossible`の根拠にならない。

### 5.3 `unknown`

`unknown`は少なくとも次を区別する閉じたreasonを持つ。

- `unsupported_topology`
- `non_convex_face`
- `time_limit_reached`
- `work_limit_reached`
- `exact_number_limit_reached`
- `overlap_arrangement_limit_reached`
- `constraint_limit_reached`
- `proof_not_completed`
- `local_conditions_indeterminate`

`unknown`は層順序の正本を更新しない。途中で得た候補順序を後続操作へ流用しない。

## 6. exact平面配置

### 6.1 数値表現

保存済みbinary64座標を値の変更なしに符号付き整数と2の冪の分母へ変換する。反射、交点、内外判定には既約な有理数を使い、四則演算はchecked resource accountingを通す。

- NaN、Infinity、負のzeroを含む前段不正は解析へ入れない
- 分子・分母のbit長、生成値件数、演算回数を会計し、bit長・演算回数・保持storageを上限化する
- 上限到達は`unknown / exact_number_limit_reached`
- f64 epsilonで近い点を同一化しない
- 表示用f64はcertificate完成後に別途作り、正本判定へ戻さない

### 6.2 面の伝播

canonical `FaceKey`が最小の面をreference faceとし、元の2D座標をそのまま平面配置する。面隣接graphをcanonical順で走査し、Mountain/Valleyのいずれでも幾何配置は共有ヒンジ直線に関する反射として伝播する。Mountain/Valleyは層順制約へ使用し、反射幾何自体を変えない。

閉路で同じ面へ複数経路から到達した場合、全頂点のexact座標と表裏parityが一致しなければ`impossible / inconsistent_flat_embedding`とする。計算上限で一致を確認できない場合は`unknown`とする。

全source edgeは、両incident faceから得た写像が同一のfolded segmentになることを再検証する。面のorientation parityは反射ごとに反転する。

## 7. 重なりcell

正の面積で重なる面対だけに層順変数を作る。点または線だけの接触は面対変数を作らないが、折れ線同士および折れ線と面内部の交差制約を作る際には保持する。

折り畳み後の全face boundaryが属するcanonical supporting lineをexactに平面分割し、正面積かつstrict convexなarrangement atomをoverlap cellとする。同じcovering face集合を持つ隣接atomでも、その境界にcanonical supporting lineが存在する場合は別cellのままとする。複数atomを論理上一領域として扱う必要がある場合は、cell自体を結合せずcanonical cell key集合を持つ上位regionを別に作る。

```text
OverlapCell
├─ stable_cell_key
├─ exact_boundary
├─ covering_faces
└─ ordered_faces
```

- cell keyはcanonical boundaryとcovering FaceKeyから生成する
- covering faceはFaceKey順、完成stackはbottom-to-top順
- 面内部にある代表点をexactに構成し、covering集合を再検証する
- 同一面が一つのcellへ二重登録される、cell境界が自己交差する、coverageが一致しない場合はfail closed
- cell全体を構成できるまでは層順候補を公開しない

certificate再検証はimmutable geometryから全canonical supporting-line arrangement atom集合をexactに再構築し、提出されたcell集合と`cell_key`、`exact_boundary`、`covering_faces`が完全一致することを要求する。cellの保存順は証拠に含めないが、人工分割、canonical supporting lineをまたぐ結合、欠落、重複は`certificate_reverification_failed`として拒否する。strict convex、face全面積・face pair面積coverage、cell間の正面積非交差の従来検証も併用する。このcanonical cell照合は完成したが、同一slot capability、current applied poseとのnative結合、原子的commandが完成するまでは、cell単独をSIM-010のmutation authorityとして使用しない。

## 8. facewise制約

正の面積で重なるcanonical面対`[A, B]`を一つの二値変数とする。

```text
FacePairOrder =
  A_above_B
  | B_above_A
```

制約は次を含む。

- 反対称
- overlap cell内の推移
- taco-taco
- taco-tortilla
- tortilla-tortilla
- Mountain/Valleyと面orientation parityから固定される隣接面順

初版のmaterial separatorはMountain/Valleyだけである。`Auxiliary`はtopology上
`AuxiliaryIgnored`となり面を分割しないため、初版にunfolded creaseは存在せず、
tortilla-tortilla制約の生成件数は0となる。unfolded creaseで分割された面を将来
導入するときは、対象クラスとmodel IDを更新してから同制約を有効化する。

taco-tacoは固定参照実装commit
`d50004815fb738d009e5b87b2307fbaefa717ef0`の有向面対順
`AB, CD, CB, AD, AC, BD`と16個の許容tupleを使う。canonical面対の二値変数へ
変換するときは、有向面対がcanonical順と逆なら該当bitを反転する。4個のcross
relationの和だけを検査してはならない。その条件だけでは、他制約の存在を仮定
しない限り交差する割当ても許してしまうためである。

1ヒンジのcanonical規則は、`FaceKey`が小さい面をreference faceとしたとき、Mountainならcanonical second faceを上、Valleyならcanonical first faceを上とする。入力配列順、edgeの保存方向、UUID生成順に結果を依存させない。

各制約は関係する面、cell、folded segmentのstable keyだけをcertificateへ記録する。内部探索indexやhash map列挙順を永続化しない。

## 9. 伝播と探索

1. Mountain/Valleyで確定する変数をcanonical順に投入する
2. 各制約の許容tupleから単一候補になった変数をBFSで伝播する
3. 矛盾すれば`impossible`
4. 未確定変数と制約の二部graphを連結成分へ分割する
5. 小さい成分、canonical variable keyの順で決定論的DFSを行う
6. 一つの完全解が得られたら全制約を独立に再検証し`possible`
7. 全分岐を調べ終えて解がなければ`impossible`
8. deadline、中止、資源上限へ到達したら候補を破棄し`unknown`または`cancelled`

探索順序は性能のためのheuristicを使ってよいが、同点はcanonical keyで解決する。hash seed、thread数、OS、localeにより最初の採用解を変えない。

複数threadで探索する場合も、採用するsolutionはcanonical solution keyが最小のものとする。初期実装は正しさと再現性を優先して一つのworker threadでよい。

初版のDFSは再帰を使わず、明示的な`SearchFrame` stackとassignment trailの巻き戻しで実装する。変数数がOS thread stackの大きさへ依存しないようにし、validation、伝播、連結成分分解、探索では少なくとも1,024 recordごとにdeadline/cancel checkpointを通す。探索node上限到達またはstack/trail allocation失敗を不可の証明へ変換しない。

## 10. 層順序正本

`possible`だけが次のsnapshotを生成する。

```text
LayerOrderSnapshot
├─ model_id
├─ source_revision
├─ material_faces[] (FaceKey canonical registry)
├─ global_bottom_to_top? (global DAGの線形拡張が存在するときだけ)
├─ reference_face
├─ folded_faces[]
│  ├─ face_id
│  ├─ face_key
│  ├─ source_to_flat
│  └─ orientation(front_up / back_up)
├─ overlap_cells[]
│  ├─ cell_key
│  ├─ exact_boundary
│  └─ bottom_to_top_faces
├─ face_pair_orders[]
│  ├─ lower_face
│  ├─ upper_face
│  └─ supporting_cells
└─ proof_summary
```

`LayerOrderSnapshot.provenance.source`はidentity namespace、revision、version付きsource fingerprint、proof modelを完全に保持する。native側はsnapshotを`CurrentLayerOrder { binding: Arc<GlobalFlatFoldabilityBinding>, snapshot }`として保持し、current slotへの採用時と利用時の両方で同じbindingを再検証する。

`face_pair_orders`は場所を失わないようsupporting cellを必須にする。全体の一つのDAGだけを正本にしない。重ならない面対へ上下関係を作らない。

異なるcellの局所順序を連結すると全体cycleになる場合がある。このcycleだけを
`impossible`の根拠にしてはならない。その場合もcell stackとface pair orderを
正本として保持し、`global_bottom_to_top`は`None`にする。`material_faces`は層順
ではなく、常にFaceKey順のmaterial face registryである。0ヒンジと1ヒンジでは
global orderが一意に構成できるため`Some`となる。

certificate再検証では、生成済みconstraint/cellをそのまま信用しない。immutable
geometryから全unordered face pairと全face tripleをexactに再交差し、全hinge×face、
全hinge×hinge、Mountain/Valley固定を別経路で検査する。cellについては全canonical
supporting-line arrangement atom集合を再構築し、保存順と独立に`cell_key`、
`exact_boundary`、`covering_faces`の完全一致を確認する。さらにstrict convex、
coverage、一意性、相互の正面積非交差を再確認し、各overlap pairについてcovering
cellのexact面積総和がface intersectionのexact面積と一致することを要求する。
再構築中は提出済みarrangementを保持したまま検証用arrangementを作るため、両方の
live exact storageと検証用順序bufferを同じ論理証明storage budgetへ同時計上する。
資源上限、deadlineまたはcancelに到達した場合は一時storageだけを解放し、候補証明を
公開しない。

nativeのcurrent layer-order slotはproject instance、project ID、revision、topology input、fold model fingerprint、proof/layer model、snapshot provenance、material registry、checked単調generationへ結合する。snapshot cloneではなく、同一slotとcertificateの`Arc` identityを封印した非Serialize・非Cloneのprivate capabilityだけを捕捉し、deep clone、同内容再解析ABA、編集後Undo、project reopen、別slot、世代差を拒否する。観測用の借用再検証はmutation authorityにしない。mutation境界では`AppState`からlayer slotの固定lock順で両lockを保持したまま再認証済みclosureを実行し、cancelまたは再解析が再認証とcommitの間へ入るTOCTOUを許さない。

編集またはproject置換後は利用時のbinding照合で無効として返さず、通常の新規判定開始、判定job置換、判定`unknown/impossible`、cancel、errorではslotを明示的に消去する。完了済みcertificateに対する明示cancelもslotを失効させる。source件数preflightで即時終了する未登録例外は既存slotへ触れない。stale snapshotを3D表示またはSIM-010へ渡さない。

## 11. 時間・資源上限

利用者は1秒以上300秒以下の時間制限を選べる。初期UIのpresetは5秒、30秒、120秒とし、既定値は30秒とする。deadlineはnativeのmonotonic clockで測り、wall clock変更の影響を受けない。

初期上限は次を正本とし、実測後にmodel IDを変えず上限だけを厳しくしない。

| 項目 | 初期上限 |
|---|---:|
| source vertex | 100,000 |
| source edge | 100,000 |
| 紙境界vertex | 100,000 |
| material face | 2,048 |
| face boundary half-edge総数 | 100,000 |
| hinge | 100,000 |
| edge incidence record | 500,000 |
| 局所平坦折りvertex record | 100,000 |
| 全構造record合計 | 2,000,000 |
| overlap face pair | 500,000 |
| arrangement segment | 1,000,000 |
| overlap cell | 500,000 |
| constraint | 5,000,000 |
| search node | 10,000,000 |
| exact整数1値のbit長 | 65,536 |
| exact演算回数 | 100,000,000 |
| 論理証明storage budget・完成certificate | 128 MiB |

各上限はちょうどの入力を受理し、`+1`を作業開始前または最初に超えた時点で全体`unknown`とする。checked addition/multiplicationを使い、overflowを上限超過として扱う。

128 MiBは、OS allocatorが使用する実heapまたはprocess RSSのhard上限ではなく、初版が対応する64-bit Windows/macOS target間で同じ結果を返す決定論的な論理証明storage budgetである。完成JSONの長さだけでなく、exact値のcanonical分子・分母payload、明示的に会計する平面配置・arrangement・snapshot・certificate構造、facewise変数、`TupleConstraint`本体と各`variables`・`allowed_rows`・`faces` payload、固定代入、solverが要求するdomain・連結成分・明示stack・trail、元問題を保持したまま再生成する検証用制約問題、最終serializationの実測bytesを同じbudgetへ含める。solverの連結成分は可変長nodeを持つ内側heapを面数分作らず、上限を事前算出できるUnion-Find、単一の連続variable buffer、範囲表で求める。対象bufferはchecked算術で検査して`try_reserve`し、solver終了、再生成照合終了等、所有値を実際に破棄したscopeでだけ会計を解放する。再確保は旧bufferと新bufferが同時に存在するpeakも論理budgetへ含め、size計算overflowは上限が`usize::MAX`でも失敗とする。

この128 MiB値は、`BigInt` / `BigRational` object header、limb allocatorの余剰capacity、`Vec` / `HashMap` header、allocator metadata・padding、すべての前処理workspaceを実測して拘束するものではない。これらはsource record、face、pair、segment、cell、constraint、exact整数bit長、exact演算回数の独立上限で無制限化を防ぎ、明示的に`try_reserve`する会計対象bufferの確保失敗は`unknown`へ閉じる。すべてのRust container確保がfallibleであるとは主張しないため、OS全体のOOMを判定結果へ変換する保証はない。実heapは128 MiBを超え得る。厳密なheap/RSS上限が必要になった場合は、facewise専用arenaまたはaccounting allocatorを別versionとして導入し、この論理budgetと同一視しない。論理storage上限、算術overflow、明示的な確保失敗、deadline、cancelのいずれでも候補certificateを`possible`として公開しない。

完成certificateは`serde_json` writerで実際にserializationしてbytesを数え、certificate内の`certificate_bytes`と固定点になるまで照合する。serialization中も一定bytesごとにdeadline/cancelを確認し、最終照合後にもcheckpointを置く。上限、deadline、cancel、serializer failureのどれかへ到達した候補は`possible`として返さない。

deadlineとcancel tokenは、少なくとも次の境界で確認する。

- 1,024 exact arithmetic operationごと
- face、segment、cell、constraintの各外側loopと、pair×cell、face×face等の内側batch
- constraint伝播の各queue batch
- iterative DFSの各node・validation batch
- certificate構築・再検証の各cellと順序付け比較batch
- canonical境界構築とcertificate serializationの一定bytesごと

初版の時間制限と中止は協調的で、OS threadの強制停止ではない。大規模vectorのin-place canonical sortは追加heapを持たない代わりにsort内部へcheckpointを挿入できず、その直前と直後で確認する。このため最大規模入力では、指定時刻または中止操作から現在の1回のsortが終わるまで終端通知が遅れる可能性がある。ただしsort後のcheckpointを通過せず`possible`を公開することはない。厳密な応答時間上限は、順序を変えないcancel可能なradix/bucket sort等を別versionで導入してから保証する。

明示的な`try_reserve`が失敗した場合は資源上限の`unknown`へ閉じる。確保直前のcheckpoint後からallocatorが失敗を返すまでにcancel tokenが変化する競合では、初版は確保失敗を先に返すことがある。どちらも数学的な可・不可にはならず、候補certificateを公開しない。

## 12. 進捗と中止

phaseは単調に次へ進む。

```text
capturing
→ validating_local_conditions
→ building_flat_embedding
→ building_overlap_arrangement
→ building_constraints
→ propagating
→ searching
→ verifying_certificate
→ completed
```

進捗DTOはphase、完了work、既知なら総work、経過時間、判定済み件数だけを含む。総量が未確定なphaseで架空の百分率を表示せず、「重なり領域を構築中 12,340件」のように表示する。

中止要求はidempotentとし、tokenを立てた直後から新しいphaseへ進めない。workerが終了するまでUIは「中止しています」を表示できるが、編集操作は判定jobと独立して継続できる。中止結果は`cancelled`であり、`unknown`へ変換しない。

## 13. UI契約

局所平坦折り条件の下に「全体平坦折り判定」panelを置く。

- 時間制限を選択
- 判定開始
- 実行phaseと件数をpolite live regionで通知
- 常に到達できる中止button
- `可 / 不可 / 不明 / 中止 / 計算エラー / 古い結果`を色だけでなく文言とicon形状で区別
- model、対象クラス、経過時間、面数、重なりcell数、制約数、探索node数を表示
- `possible`では層数、最大ply、reference face、nativeに証明済み層順正本があるかを表示する。専用の層順3D viewerは別機能であり、初版VAL-003の完了範囲に含めない
- `impossible`では証明種別と上限付きの対象面番号を表示
- `unknown`では時間切れ、対象外、資源上限、数値証明不足を個別表示

`impossible`の対象面番号は、判定に使用したimmutable `TopologySnapshot`のmaterial
faceを`(FaceKey, FaceId)`のcanonical順へ並べた1始まりの番号とする。公開件数は最大
20件で、空配列は有効な不可根拠として扱わない。理由別の範囲は次のとおりとする。

- 局所必要条件違反: 違反頂点にincidentなmaterial faceの和集合
- 平面配置矛盾: `InconsistentFlatEmbedding`が保持する1面
- 層順序制約矛盾: 矛盾したconstraintが保持する面集合
- 全探索解なし: 全material faceを対象とし、表示にはcanonical順の先頭20面を用いる

reasonが参照する面・頂点・ヒンジを同じtopologyへ照合できない場合、重複したreason
face、重複したFaceId/FaceKey、reportとface countの不一致がある場合はDTOを公開せず、
内部整合性エラーとしてfail closedにする。UIは全探索解なしを「全体」と明記し、
20面を超える場合は省略面数を表示する。

「可」は厚さ付きで折れる、手で折りやすい、安全な連続経路がある、という意味ではないことを常時表示する。

## 14. nativeジョブ

Tauri commandは次の閉じた経路とする。

```text
begin_global_flat_foldability(
  expected_project_id,
  expected_revision,
  expected_fold_model_fingerprint,
  time_limit_ms
)
get_global_flat_foldability_progress(job_id)
get_global_flat_foldability_result(job_id)
cancel_global_flat_foldability(job_id)
```

- 開始はproject ID、revision、64桁小文字hex fingerprint、1〜300秒の時間制限を閉じた入力として検査する
- source件数preflightを通った通常開始だけopaque job IDをactive slotへ登録し、旧世代をcancelしてcurrent layer orderを無効化する
- source件数上限超過は未登録の完了済み不明結果を返し、既存jobとcurrent layer orderへ触れない
- 通常開始は一つの`Arc<GlobalFlatFoldabilityBinding>`へsnapshotを取得し、mutexを解放してからworkerを起動する
- workerはpanicをcatchして`failed/internal_failure`へ閉じる
- raw panic、path、座標、UUID、探索値をIPC errorへ含めない
- progress phaseはcompare-and-swapで単調にする
- result claimはone-shotとし、別jobや別revisionから取得できない
- `possible`のlayer snapshotはnative内部へ保持し、WebViewへは要約だけ返す
- completionはactive jobと同じ`Arc` identity、project instance、project ID、revision、topology input、fingerprintを再照合する
- stale resultはcurrent layer-order slotへ採用せず、旧completionで現在jobを完了させない

## 15. SIM-010への接続

SIM-010を実装する前に、層順序に加えて次を用意する。

1. 折り線追加前の面から分割後の面へのface lineage
2. 新規ヒンジを過去stepへ0度で追加するtimeline移行規則、またはstep別model snapshot
3. 展開図更新、層別Mountain/Valley割当て、層順更新、timeline 1 step追加を一つにする原子的`ApplyStackedFold` command
4. current applied poseへrevision-boundな面変換とlayer-order snapshot IDを追加
5. 逆写像、衝突直前停止、山谷割当て、lineage、timeline移行の全成功後だけcommitするtransaction

現在の`FaceId`は面境界に依存し、折り線追加で変わり得る。SIM-010は古いFaceIdを推測で再利用せず、明示的lineageを検証する。

## 16. 受入試験

### 16.1 core

- 0ヒンジを`possible`、1面stackとして返す
- Mountain 1ヒンジとValley 1ヒンジで上下が反転する
- 面、辺、頂点の入力配列順とsource edge方向で結果が変わらない
- currentかつcompleteな局所必要条件違反を`impossible`とする
- 前段不正、局所`indeterminate`、未対応cutを`impossible`にしない
- 平面反射の閉路矛盾を`impossible`とする
- 既知の複数面可・不可fixtureを正しく区別する
- taco-taco、taco-tortilla、tortilla-tortilla、推移の各矛盾を検出する
- deadlineと各資源上限が`unknown`になる
- 中止が`cancelled`、panicが`failed`になる
- 上限ちょうどを受理し、`+1`を拒否する
- source geometryからtopology/localを再生成し、staleまたは偽造されたartifactを不明として拒否する
- vertex/edge保存順、無向辺方向、紙境界cycle開始・反転によらず同じversion 1 SHA-256 fingerprintを返し、折りモデルの全対象field変更でfingerprintが変わる
- identity namespace、revision、fingerprint、proof modelのどれかが異なるprovenanceをcurrentとして受理しない
- 5万変数規模でも再帰stackを使わず、iterative DFS・伝播・連結成分処理を完了または上限付きで中断する
- certificateをimmutable geometryから独立に再検証し、全canonical supporting-line atom集合とのkey・exact boundary・covering faces完全一致を要求する。人工分割、結合、欠落、重複、cell key、coverage、pair order、supporting cell、derivation、証明集計の改ざんを拒否する
- 会計対象のexact canonical payload、arrangement、snapshot、certificate構造、制約・solver buffer、検証一時値の合算論理storageについて上限ちょうどを受理し、`-1`上限、算術overflow、構築途中超過を不明へ閉じる。これはallocator実heapの上限試験とは呼ばない
- 実certificate JSONのserialization bytesと`certificate_bytes`を一致させ、実測値ちょうどを受理し、1 byte不足を拒否する。serialization中のdeadline/cancelも終端候補を公開しない

### 16.2 native

- 編集中もUI threadとproject editを止めない
- 同時開始で最新世代だけが採用される
- cancel、再cancel、完了直前cancel、旧job callbackが安全
- revision変更、同revision別内容、project再openをstaleとして拒否する
- 開始要求のfingerprint欠落、大文字、桁数違い、非hex、期待値不一致を閉じたrequest/snapshot errorとして拒否する
- normal captureでactive、completion、current layer-orderが同じ`Arc` bindingを共有し、cloneまたは別bindingを採用しない
- source頂点、source辺、紙境界、合計recordの上限ちょうどを通常経路へ通し、`+1`はfingerprint計算・snapshot clone前に未登録の不明結果とする
- source上限の未登録結果が既存active jobをcancelせず、既存current layer-orderを消去しない
- fingerprint計算前に終わるdeadline/source上限の不明だけはfingerprint欠落を許し、可・不可では必須、存在する不一致fingerprintは判定値に関係なく拒否する
- `unknown/impossible/cancelled/failed`でcurrent layer slotを更新しない
- raw error、path、座標、内部IDをIPCへ露出しない

### 16.3 UI

- keyboardだけで時間選択、開始、中止、結果確認ができる
- phase更新を過剰に読み上げず、終端状態を必ず通知する
- 200%表示で結果理由と中止buttonへ到達できる
- `可 / 不可 / 不明 / 中止 / 計算エラー / stale`が混同されない
- native開始にproject ID、revision、現在のfold model fingerprintが渡され、未登録source上限結果だけを安全な即時完了として受理する
- 中止後も展開図編集と再判定ができる
