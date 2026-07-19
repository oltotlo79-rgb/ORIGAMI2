# native current applied pose 設計

## 1. 目的

本書は、SIM-010「現在の3D状態に対する折り重ね操作」の前提となる、native側の current applied pose と証明境界を定める。

折り重ねは、現在の姿勢、衝突分類、場所別の層順序を同時に正しく扱えなければならない。表示中の姿勢やWebView内の診断値をそのまま編集権限へ昇格させず、immutable geometryからnative側で再構築・再検証した証明だけをauthorityとして使用する。

本書の設計は、次を必須条件とする。

- TS/Three.js側の姿勢、face transform、衝突結果、層列、fingerprint、certificateは観測・表示専用であり、project mutationのauthorityではない。
- native側でkinematics、衝突、連続経路、場所別層順序のtransportを証明する。
- stale、衝突、判定保留、制限超過、中止、内部失敗のいずれでもproject stateを変更しない。
- SIM-010の成功時だけ、pattern、current layer order、applied pose、timeline、revisionを一つの原子的editor commandで確定する。
- Undo/Redoでは意味状態を復元する一方、revisionとauthority generationは必ず単調増加させる。

## 2. 製品要件を縮小しない

VAL-003の `LayerOrderSnapshot` は、理想的なzero-thickness flat embeddingに対する場所別層順序の証明である。任意の非平坦3D姿勢における現在のstackを直接表すものではない。

全ヒンジが厳密に180度のflat姿勢は、native poseと既存layer-order certificateを最初に結合できる内部証明クラスとして実装してよい。no-hingeの単一面も同じ内部証明クラスに含めてよい。しかし、これは実装を段階化するためのbootstrapであり、SIM-010の製品要件を全体flat状態だけへ縮小するものではない。

製品として公開するSIM-010は、任意の現在3D状態で実際に局所的に重なっている層を扱わなければならない。そのためには、少なくとも次がnative側で完成している必要がある。

- 現在姿勢までのcontinuous collision証明
- 現在姿勢における場所別の重なり集合とcell-order transport証明
- 折り重ね候補経路全体のcontinuous collision証明
- target姿勢の場所別層順序とcertificate再検証
- 原子的commitとUndo/Redo

全180度flat互換だけが完成した段階では、内部テストと後続実装の基盤としてのみ使用する。任意3D姿勢のnative continuous collisionとcell-order transportが完成するまで、折り重ねUIを利用者へ公開しない。

## 3. authorityの原則

### 3.1 正本

次だけを正本とする。

- nativeの `ProjectState` と `EditorState`
- project instance ID、project ID、revision
- nativeが保持するpaper、crease pattern、timeline
- immutable geometryからnativeが再構築したtopology
- nativeが再計算した完全なface poseと全ヒンジ角
- nativeが発行したcurrent layer-order capability
- nativeが発行したcollision、continuous path、cell-order transportの各certificate

WebViewから受け取る値は、利用者の操作要求を表すuntrusted inputである。正本として受理しない。

### 3.2 IPC入力

current pose更新要求が受け取ってよい情報は、概念上次に限定する。

```text
NativePoseRequest {
    expected_project_instance_id,
    expected_project_id,
    expected_revision,
    fixed_face_id: Option<FaceId>,
    complete_hinge_angles[(edge_id, angle_degrees)]
}
```

`expected_project_instance_id` は遅延応答を同一ファイルの再open前後で区別するための相関値であり、それ自体をauthorityとして扱わない。nativeはcurrent `ProjectState`との一致を必ず再検証する。`fixed_face_id` はno-hingeの単一面だけ `None`、hingeを一つ以上持つtreeではcurrent face registry内の `Some` とする。

次をIPC入力として受理しない。

- face transform、hinge transform、Matrix4
- faceの移動側・固定側partition
- 現在の層列、対象層列、face lineage
- topologyまたはfold-model fingerprintの自己申告値
- collision結果、接触分類、continuous scan結果
- layer-order snapshotまたはcertificate
- native capabilityの内容を複製した構造体

必要なfingerprint、topology、material registry、transform、partition、層順序はすべてnativeのcurrent projectから導出する。

### 3.3 TS/Three.jsの責務

TS/Three.js側は次だけを担当する。

- 利用者入力とアニメーション
- 3D previewの描画
- nativeへ送る角度要求の構築
- nativeが返した状態の表示
- 追加の診断表示

既存の `FoldPreviewAppliedPoseSnapshot`、Three.js Matrix4、TS衝突分類、WeakSet/WeakMapで保護された証拠はWebViewプロセス内の観測値である。rendererが正しい表示を維持するために利用してよいが、native mutationの許可条件には使用しない。

TS側の衝突診断が成功、失敗、未実行のどれであっても、native authorityの結果は変化しない。native検証が未完了または不明なら、表示上安全に見えていてもmutationを許可しない。

## 4. native kinematicsの責務

`ori-instructions` にあるCPU側tree kinematicsから、表示・投影に依存しない部分を新しい `ori-kinematics` crateへ抽出する。

`ori-kinematics` の責務は次とする。

- immutable paper、crease pattern、topologyからhinge treeを構築する。
- fixed faceと完全なヒンジ角ベクトルから全faceのrigid transformを計算する。
- Mountain/Valley、canonical edge direction、rerootを一意に扱う。
- 0、90、180度のcardinal angleを決定的に扱う。
- 非有限値、範囲外角度、欠落、重複、余剰hinge、未接続、cycle、制限超過をfail closedで拒否する。
- native material座標を使用し、preview用のworld scale、camera、projectionを含めない。

材料座標の初版規約は `(paper_x, 0, -paper_y)` のmm単位とする。preview用の4.4 world-size正規化などは `ori-instructions` またはfrontend側へ残す。

非cardinal角の行列をJS実装とbit-exactに一致させることはauthority条件にしない。native内部では同じimmutable入力から常に再計算し、外部行列との比較で真正性を判断しない。

## 5. opaque capability、binding、generation

### 5.1 semantic pose

nativeの意味状態は概念上次とする。

```text
AppliedPoseV1 {
    model_id: "tree_absolute_hinge_angles_v1",
    fixed_face_id: Option<FaceId>,
    canonical_complete_hinge_angles
}
```

ヒンジ角はcanonical EdgeId順に保持し、`-0` は `0` へ正規化する。各値は有限かつ `0..=180` とする。no-hingeの単一面では角度列を空、`fixed_face_id` を `None` とする。hingeを一つ以上持つtreeでは全hingeを過不足なく一回ずつ列挙し、`fixed_face_id` をcurrent face registry内の `Some` とする。

### 5.2 binding

各certificateは、少なくとも次へ結合する。

- non-persisted project instance ID
- project ID
- source revision
- immutable topology inputの同一性
- fold-model fingerprint
- canonical material face registry
- canonical hinge registry
- kinematics model IDとversion
- fixed face
- 完全なヒンジ角ベクトルまたはそのcanonical digest
- paper thicknessのbinary64 bits
- thickness model ID
- collision/contact policy IDとversion
- pose generation
- 必要な場合はlayer-order generation

同じ値に見えるdeep clone、同じファイルの再open、同じ角度へ戻った姿勢、同内容の再解析を同じauthorityとして扱わない。

### 5.3 capability

`CurrentAppliedPoseCertificate` と `CurrentAppliedPoseCapability` のfieldはprivateとし、Serializeしない。capabilityはnativeプロセス内だけで使用する。

capabilityは概念上次を保持する。

- current authority slotへの `Arc`
- 発行元certificateへの `Arc`
- 発行時claims
- 発行時generation

mutation直前に、current slot内のcertificateと `Arc::ptr_eq` で同一性を確認し、claimsとcurrent projectも再検証する。構造的に同じcertificateのdeep cloneは拒否する。

UIへpose IDやgenerationを返す場合、それは表示・応答相関用でありauthority tokenではない。文字列や数値を送り返すだけでnative capabilityを復元できてはならない。

### 5.4 generation

pose generationはrevisionと分離する。姿勢変更はdocument revisionを変えない場合があり、同じ角度へ戻るABAも発生するためである。

次の成功時にだけchecked単調増加させる。

- 新しいnative pose certificateの採用
- revisionを進めるeditor command
- entryを実際に移動したUndo/Redo
- projectのnew、open、replace
- current poseの明示的invalid化
- authority slotの差し替え

失敗したcommand、空stackのUndo/Redo、stale要求および不採用candidateではgenerationを増やさない。editor commandがsemantic poseを保つ場合でもrevisionへ束縛した旧certificateは失効させる。overflow時は新しいauthorityを発行せず、editor、history、revision、semantic pose、slotを含む既存状態を完全に維持してfail closedとする。generationをJS numberとして公開する場合はsafe integer上限を超えない規約を使用する。

## 6. AからFまでの証明層

各層は直前の層を置き換えず、より強いclaimを追加する。後段のcertificateは、前段と同じbinding、certificate identity、generationへ結合しなければならない。

### A. NativeKinematicPoseCertificate

証明すること:

- current immutable geometryとtopologyからhinge treeを再構築した。
- fixed faceと全ヒンジ角が完全で妥当である。
- 全material faceのrigid transformをnativeで再計算した。

証明しないこと:

- 衝突がないこと
- 現実に到達可能な経路であること
- 現在の層順序
- SIM-010を実行してよいこと

このcertificate単独のmutation authorityは常にfalseとする。

### B. CurrentAppliedPoseCapability

証明すること:

- Aのposeが同じproject instance、revision、geometry、generationに対して現在もcurrentである。
- semantic poseとnative transformが同じcertificate objectに結合している。
- stale worker、同角度ABA、project reopen、deep cloneではない。

この段階でも衝突と層順序は未証明であり、SIM-010を許可しない。

semantic poseは `ori-core::EditorState` のruntime意味状態および必要なhistory transitionの対象とするが、初版ではpersisted documentへ保存せず、dirty判定の対象にしない。pose-only adoptionはrevisionまたはUndo/Redo entryを増やさない。in-process certificateは `ProjectState` 配下のauthority slotで管理し、persisted documentへsemantic pose、opaque fieldまたはprocess identityを保存しない。将来semantic poseを保存対象へ変更する場合も、保存値はuntrusted requestとしてnativeで再証明し、certificate自体は保存しない。

### C. NativeStaticCollisionCertificate

証明すること:

- Bと同じposeに対して、native collision engineが全対象face pairを走査した。
- 共有なし、頂点共有、辺共有、同一面と、離間、点接触、線接触、面接触、横断交差のpolicyをversion付きで適用した。
- 紙厚、hinge corridor、許容flat stackをbindingへ含めた。
- penetratingまたは未解決のindeterminateが残っていない。
- work accountingと対象pair coverageが完全である。

endpointのstatic検証だけでは、その姿勢へ到達する経路も層順序も証明しない。

### D. NativeContinuousMotionCertificate

証明すること:

- 直前のcurrent certified poseからcandidate poseまでの、指定された完全な角度経路を検証した。
- 経路全体でpenetrating、見逃した時間区間、未解決のindeterminateがない。
- 許可接触とhinge contactがversion付きpolicyに従う。
- deadline、中止、work limitを含む全区間のcoverageが完了した。

単一hingeの連続運動から実装を開始してよいが、未対応のmulti-hinge経路を安全と推定してはならない。未対応経路はunsupportedまたはindeterminateとして拒否する。

### E. CellOrderTransportCertificate

証明すること:

- current layer-order capabilityと同じproject instance、revision、fingerprint、topology、material registry、certificate identity、generationを使用した。
- 場所ごとの重なりface集合とbottom-to-top orderを、証明済みanchorから現在3D姿勢までtransportした。
- continuous path上で、層順序を変える横断、未記録接触、cell eventの見逃しがない。
- current poseで、折り重ね操作線と交差する局所cellの対象層を完全に列挙できる。
- target poseまでtransportした新しい場所別層順序をimmutable target geometryから再検証できる。

endpointの無衝突だけではEを発行しない。離れた場所で順序が異なり得るため、全体の単一face列だけでもEを発行しない。

最初の内部証明クラスとして、次を実装してよい。

- no-hingeの単一面
- current VAL-003 certificateと同一bindingを持つtree
- reference faceを同じroot gaugeとして使用
- 全hinge角が厳密に180度
- canonical cellとexact `source_to_flat` が再検証済み

このflat互換はEの限定された一実装であり、任意3D姿勢向けcell-order transportの代替ではない。

### F. ApplyStackedFoldCommitAuthority

AからEまでの必要なcertificateが同じcurrent bindingへ結合している場合だけ発行する。

Fが許可すること:

- 現在3D姿勢の局所cellから、操作線をまたぐ対象層を決定する。
- 層ごとの材料面へ操作線を逆像し、新しいMountain/Valley creaseを割り当てる。
- candidate patternとface lineageを構築する。
- 折り経路とtarget collisionを検証する。
- target layer order、target applied pose、timeline stepを構築する。
- 一つのeditor commandとしてcommitする。

Fは一回のcommitだけに使用できるone-shot authorityとする。同じFを再利用、複製、別project、別revision、別generationへ適用してはならない。

## 7. 紙厚とflat stack

初版の紙厚モデルは、オーナー決定どおり中央面基準の近似方式を正式仕様とする。厳密な層offsetは将来課題である。

そのため、全180度の層重なりを通常の「衝突なし」と表現してはならない。native collision policyは少なくとも次を区別する。

- 証明済みの許容flat stack
- hinge model内の許容接触
- layer offset未実装による近似状態
- 真のpenetration
- touching
- indeterminate

許容flat stackは、current layer-order certificateと同じbindingにより説明できる場合だけ許可する。単に近平行または同一平面だからという理由では許可しない。

紙厚値、そのbinary64 bits、thickness model IDはcollision、continuous motion、order transportの全certificateへ結合する。途中で紙厚が変わった場合は既存certificateをすべてstaleとする。

## 8. native collisionとorder transportの配置

新しい `ori-collision` crateを設け、少なくとも次へ依存させる。

- `ori-domain`
- `ori-topology`
- `ori-geometry`
- `ori-kinematics`
- exact binary64 predicateに必要な場合だけ `num-bigint`

`ori-collision` は次を担当する。

- broad phaseと全pair coverage
- zero-thicknessおよび中央面紙厚近似
- 共有関係と接触種別の完全な分類表
- exact transversal proof
- static collision
- continuous collision
- work limit、deadline、cooperative cancellation
- 証明結果のmodel/version付与

cell-order transportは、collision eventとlayer-order certificateの双方を必要とする。`ori-collision` 内のtransport module、または循環依存を生じない専用crateへ配置する。`ori-core` は証明アルゴリズムを複製せず、検証済みcandidateの意味状態と原子的commandを担当する。

## 9. 原子的prepareとcommit

重い計算をproject lock内で実行しない。

### 9.1 prepare

固定した `project (AppState) -> pose slot -> layer-order slot` のlock順で、次のimmutable snapshotとcapabilityを取得する。poseだけ、またはlayer-orderだけを中止する経路は単独slotを取得してよいが、複数lockが必要な経路で順序を逆転させてはならない。

- project instance、ID、revision
- paper、pattern、timeline
- topologyとfingerprint
- current applied pose certificate
- current layer-order certificate
- pose generationとlayer-order generation
- 紙厚と全policy/model ID

その後lockを解放し、AからFの準備計算をbackground workerで行う。

### 9.2 commit

commit直前に同じ `project -> pose -> layer-order` の固定lock順で必要なlockを取得し、次を再照合する。既存のlayer-order guarded closure内からpose lockを後取りせず、両certificateを必要とするcommitは専用combined helperでこの順序を保持する。

- project instance、ID、revision
- geometry、topology、fingerprint
- applied pose certificateの `Arc` identityとgeneration
- current layer-order certificateの `Arc` identityとgeneration
- 紙厚、kinematics、collision、transportのmodel/version
- workerの中止・期限・generation
- candidateが参照する全material registry

一つでも異なればstaleとしてcandidate全体を破棄する。

一致した場合だけ、一つのeditor commandで次を同時に更新する。

- crease pattern
- semantic applied pose
- current layer order
- instruction timeline
- revision
- Undo/Redo history

成功時のhistory entryは一つだけとする。

### 9.3 Undo/Redo

Undo/Redoはpattern、semantic pose、layer order、timelineを意味的に復元する。

semantic poseのhistoryは `PreserveCurrent` と `Restore { before, after }` を区別する。instructionまたは外観だけを変えるentryでは、実行後にpose-only adoptionされた最新姿勢を古いhistory値へ巻き戻さない。geometryを変えるentryではUndo/Redo直前の各側の最新semantic poseを対応する `before` または `after` へ取り込み、成功後に反対側を復元する。

ただし、古いworkerやcapabilityを再びcurrentにしてはならない。そのため、entryを実際に移動したUndo/Redoではrevision、pose generation、layer-order generation、必要なauthority generationをchecked単調増加させる。restored semantic poseが `Some` でnative再証明に成功した場合だけ新しいcertificate objectを発行する。`None`、未対応geometryまたは再証明失敗ではcurrent authorityを空のまま保つ。P0では安全側として旧certificateをclearし、P1の再証明経路が完成するまで折り重ねauthorityへ接続しない。

## 10. 失敗時不変条件

次のどの失敗でも、開始時とbit-exactに同じ意味状態を保つ。

- stale project、revision、pose、layer order
- invalid、欠落、重複、余剰入力
- unsupported topologyまたはmotion
- collisionまたは禁止接触
- indeterminate
- incomplete pair、time interval、cell coverage
- work limit、deadline、中止
- certificate再検証失敗
- generation exhaustion
- resource allocation失敗
- worker panicまたは内部失敗

不変対象は次とする。

- paperとcrease pattern
- semantic applied pose
- current layer-order slot
- timeline
- revision
- dirty baseline
- Undo/Redo stack
- 保存済みdocumentとpath
- current native authority

失敗したcandidate、部分的なface lineage、部分的な層順序、途中まで進んだposeをcurrent slotへ公開しない。

## 11. tests-first実装順

UI実装はAからFの完了後に行う。各段階で失敗テストを先に追加し、その段階が証明しないclaimもテストで固定する。

### A. native kinematics抽出

対象:

- `crates/ori-kinematics/Cargo.toml`
- `crates/ori-kinematics/src/lib.rs`
- `crates/ori-kinematics/src/tree.rs`
- `crates/ori-kinematics/src/transform.rs`
- `crates/ori-instructions/src/lib.rs`

先行テスト:

- Mountain/Valley sign
- fixed-face reroot
- 非可換multi-hinge
- 角度ベクトルの完全性
- cardinal angleの決定性
- edge direction、storage order不変
- cycle、未接続、resource limit拒否
- instruction exportの抽出前後golden一致

### B. semantic poseとopaque capability

対象:

- `crates/ori-core/src/applied_pose.rs`
- `crates/ori-core/src/editor.rs`
- `crates/ori-core/src/lib.rs`
- `apps/desktop/src-tauri/src/applied_pose.rs`
- `apps/desktop/src-tauri/src/lib.rs`

先行テスト:

- 外部transformとcertificateを受理しない
- material kinematics型だけをcertificate生成へ渡せ、caller embedding用observation型を渡せない
- native再計算
- stale instance、ID、revision、fingerprint、topology拒否
- 同角度ABA、deep clone、別slot、project reopen拒否
- generation単調増加とexhaustion時不変
- pose-only adoptionがrevision、history、dirty、保存bytesを変えない
- instruction・外観entryの `PreserveCurrent` とgeometry entryの `Restore { before, after }`
- edit、Undo、Redo時のsemantic poseとauthority。空stackと失敗時はgenerationも不変
- certificate/capabilityがSerialize不可

### C. native static collision

対象:

- `crates/ori-collision`

`topology_contact_policy_v1`の純粋な4×10・全40セルは互換基準として凍結した。正厚三角柱の正面積境界接触を独立させた`topology_contact_policy_v2`の4×11・全44セルも、frontendとnativeが同じ正規JSON corpusへ照合する形で実装した。4×5の簡略表では、一般点と真正な共有点、部分線と完全共有辺、境界面と共面正面積、正体積および平坦積層を区別できないため、説明用の派生表示に限り、証拠生成の正本には使用しない。`boundary_area_contact`は正厚だけが発行でき、共有ヒンジでは表だけで許容せず有限corridorの全pair証明を要求する。runtimeで`same_face`がdispatcherへ到達した場合は内部不整合として`indeterminate`へ閉じる。

この時点で完成しているfrontend側v2は純粋表とcorpus照合である。既存の
`foldPreviewNarrowCollision` production dispatcher、共有頂点certificateおよび
有限ヒンジdiagnosticは、凍結済みv1 policyへ束縛されたままである。v1 capabilityの
versionだけをv2へ書き換えて再利用してはならない。正厚の正面積・正体積0を肯定する
`boundary_area_contact`証拠、v2 issuerおよびpose/thickness bindingが完成するまでは、
production v2接続を未実装と数え、証拠不足を`indeterminate`へ閉じる。

最初の下位証拠として、`ori-collision::NativeStaticCollisionGeometryProof`は単一material face・no-hingeに限り、unordered face pairの期待数と解析数がともに0である完全なzero-pair proofを発行してよい。proofはexact material model issuer、pose instance、紙厚のbinary64 bits、kinematics/thickness/contact policy/proof model IDへ結合する。複数面は全pair evidenceが無い限り`PairEvidenceUnavailable`でblockingとする。このgeometry proofはproject instance、revision、current pose certificate identityまたはpose generationを持たないため単独ではCではなく、project mutation authorityは常にfalseである。

desktopはBのexact certificate `Arc`とgenerationを`project -> pose` lock下で再検証し、同じgeometry proof objectへ封印したprivate `CurrentStaticCollisionCertificate`を発行する。発行前のcapture、lock外proof構築、発行直前のproject/revision/fingerprint/model/pose/紙厚/proof identity再検証を分離し、同角度再採用、編集、Undo/Redo、再open、別slot、別issuer、別proof、`-0`/`+0`差を拒否する。観察時にも同じidentityを両lock下で再検証し、観察callbackのpanicは両guardを正常dropしてから再送出する。これにより限定された単一面zero-pairについてCへ到達したが、mutation/SIM-010 authorityは付与せず、複数面は引き続きblockingとする。

厚さ0の下位幾何基盤は、finite binary64座標を`2^-1074`単位の整数へexact変換し、理想三角形対を`separated`、`point_contact`、`boundary_line_contact`、`coplanar_area_overlap`、`transversal_crossing`、`indeterminate`へ分類する。native kinematicsのprivate sourceから全material faceの単純境界を再検証し、凸・凹・collinear頂点へ決定論的なexact ear clippingを行い、面積、外周・内部辺incidence、全face pair・全triangle pair coverageを再照合する。全world頂点は同じbinary64 face transformをexact affineとして適用して構成し、一面の共面性を保持する。

共有関係はsourceのFaceId、VertexId、EdgeId、rest/current geometryおよびhinge registryから認証する。真正な共有一点だけの許容にはexact singletonに加え、検証済みface transformのlocal `+Y`から得た材料法線の内積がbinary64境界`1e-10`より厳密に大きいことを要求する。共有要素外の共面正面積または直接証明できる横断は`penetrating`を優先する。

triangle-localでは人工分割対角線上の境界接触に見える場合、二material faceの非平行support planeを再認証し、交線上の全triangle coverage、実material boundary区間およびその全endpoint eventをexactに正規化する。両coverage内かつ両実外周外にある正長open cellが存在する場合だけ`transversal_crossing`へ肯定し、全共通cellが少なくとも片面の実外周上なら`boundary_line_contact`、正長共通部がなければ非横断とする。凹形状の複数区間、片側/両側人工対角線、境界→内部遷移、面/triangle順および作業上限を回帰した。

全pairの厚さ0診断集約はここまで実装したが、watertightなexact rigid pose、厚さ0の有限ヒンジ許容および正厚証拠が未完成なため、productionの複数面geometry proofは引き続きblockingとする。source ID・edge・hinge registryが正しくcurrent座標だけが不一致の場合はprivateなpose mismatchとして分離し、全triangleと必要な面区間を走査して完全coverageを保った`indeterminate`を返す。rawの横断または共面正面積をそのまま肯定貫通へ優先する暫定案は採用しない。任意に小さい端点残差でも、本来は共有ヒンジだけで接する二面に偽のrelative-interior横断または薄い共面重なりを作れるためである。`2^-10`から`2^-50`までの反例を面順両方向で回帰した。watertight canonical pose、またはrawからcanonicalへの全face誤差包含と共有featureからの分離下限が証明されるまではこの明示的判定保留を維持する。

watertight姿勢の第一checkpointは、`ori-collision`内部だけで使用する非公開の`rational_cayley_local_rotation_v1`とする。入力の軸端点と角度はfinite binary64の保存値から有理数へbit-exactに変換し、山谷および親子向きから得る回転符号は角度とのbinary64乗算を介さず独立した`+1 / -1`として適用する。0度は恒等変換、180度は`R = -I + 2ddᵀ/(d·d)`のexact half-turn、その他は有理Cayley parameterによる回転と`p - Rp`の平行移動を構成する。発行前に`RᵀR = I`、`det R = 1`、`Rd = d`および両軸端点の不動を有理算術で再検証する。

一般角の超越値をexact値と偽らない。Machin公式によるπ、有理整数平方根区間、単調区間上のsin/cos Taylor包含から`tan(θ/2)`を上下界で囲み、指数追従のdyadic Cayley parameterを選ぶ。実現角と要求角の差は有理数の上界として証明し、その上界が入力角度の隣接binary64間隔の4分の1より**厳密に**小さい場合だけ局所回転を発行する。これにより、最小subnormal角を0度へ潰さず、180度直前をhalf-turnへ丸めず、同じbinary64角度へ丸め戻せる余裕をcertificateに保持する。pinned `libm`値は表示用候補に利用できても証明根拠にはしない。

区間精度、Machin項数、三角関数項数、整数平方根反復、区間演算数、shift量、中間整数bit数および出力分子・分母bit数にはversion固定の上限を設ける。各演算は割当て前に上限を検査し、上限到達、包含不能または不変条件不成立では部分matrixやfallback姿勢を発行せずblocking errorとする。この第一checkpointは局所回転核の証明だけであり、既存のbinary64 face transformを置き換えず、複数面geometry proofの成功集合も広げない。

第二checkpointの内部段階である`rational_cayley_tree_pose_v1`では、`BoundMaterialTreePose`へ束縛したcanonical BFSで局所回転を全faceへ合成し、exact pose instance、fixed face、完全角度列、全face・hinge・boundary coverageを保持する。全VertexIdの全face occurrence、各共有ヒンジのstart・end・midpoint、全合成回転の直交性と行列式をBigRationalで完全照合する。同角度再solve ABA、foreign issuer、別rootを区別し、root変更は全faceに共通する一つのexact global frame changeであることを回帰する。境界辺は全boundaryの単一走査で認証済み索引へ変換し、tree全体のlogical storage、索引走査、一時局所値、Machin/Taylor項、sqrt反復および有理演算量を固定上限へ合算する。個別出力は分子・分母の大きい側、総storageは両方のbit数で課金し、各上限はexact値成功・one-short失敗とする。

この内部段階だけでは第二checkpointは完了せず、既存のbinary64 renderer姿勢も置き換えず、collision safe setへ未接続である。raw renderer姿勢へのversion固定誤差包含、deep-chainでの分母・work累積、厚さ0の面証拠生成器への結合を完成し、同じauthorityから分類対象geometryを発行できた時点でのみ、pose mismatchの判定保留を再分類する。

#### C.1 `MaterialTreePose` exact-affine包含（次工程設計）

この項のうち未承認の直接差分観測はprivateな`MeasuredBinary64AffineEnvelope`として実装済みだが、Cの完成やcollision safe setの拡張を表さない。この型は同じ`BoundMaterialTreePose` instanceだけを受理し、proof model ID、admission判定およびsafe判定能力を持たない。全boundary occurrence、共有`VertexId`、各hingeのstart/end/midpointと親/子/hinge-parent経路、clone/ABA/foreign/reroot/1 ULP、構造・保存・厳密算術のexact-limit/one-shortをprivate testで固定した。演算数表、三角関数の厳密誤差上限およびhard ceilingの承認後に限り、正式proof model ID候補`material_tree_binary64_affine_containment_v1`を`rational_cayley_tree_pose_v1`、`material_tree_kinematics_mm_v1`、exact material model issuerおよび同じpose instanceへ結合する。proof/build authorityはopaque、non-`Serialize`、public constructorなしとし、caller matrix、ID文字列または値が等しい別solveから再構成できないようにする。

各faceのnative binary64 `RigidTransform`の12係数をIEEE 754 bit列からdyadic有理数へ完全変換し、その係数を無丸めの有理算術で評価する理想affine map `F`とする。同じsource boundary vertexをexact Cayley tree map `E`へ適用し、全boundary occurrenceについて次を直接計算する。これは保存済みmatrix係数が表す理想mapの包含であり、JavaScriptやGPUでmatrixを点へ適用する際の追加丸めはC.2の別gateで扱う。

```text
delta(face, vertex, component) =
    abs(F(vertex)[component] - E(vertex)[component])
radius(face, component) =
    max(delta(face, vertex, component) for every boundary occurrence)
```

`F - E`はaffineであり、単純多角形の材料面は境界頂点のconvex hullに含まれるため、各成分の絶対差はface内部全域で`radius`以下になる。この証明は凹faceにも適用し、ear-clipping後の一部頂点、AABB cornerまたは代表点だけの検査へ縮小しない。同じ`VertexId`の全face occurrenceを一つのexact点へ照合し、各hingeのstart、end、midpointも親、子、hingeの全経路で包含する。正厚では別gateとして材料法線列も比較し、位置包含へ少なくとも`(thickness / 2) * normal_error`を加える。位置radiusだけで正厚を認証しない。

直接差分による包含とは別に、広すぎる対応を拒否するversion固定admission budgetを置く。v1候補は次とし、定数、演算数表、binary表現およびhard ceilingをowner承認後にmodel versionへ凍結する。

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

全boundary occurrence・全成分の実測exact差分がface深さに対応する`B_admit(d)`以下である場合だけproofを発行する。`2^-20 mm`はv1 hard ceiling候補であり、承認・version固定までは正式実装値としない。budgetはcaller、紙厚、表示scaleまたは座標scaleで拡張できず、collision marginや共有feature許容へ転用しない。`N_d*u >= 1/2`、budget算出のchecked overflow、差分超過またはhard ceiling超過はblockingである。

version bindingには`DEGREES_TO_RADIANS`のbit列、binary64→有理変換とround-to-nearest-ties-to-even規約、pinned `libm 0.2.16`、canonical traversal/order、座標規約、`N_axis / N_local / N_compose / N_affine`の演算数表、`TRIG_ABS_ALLOWANCE_V1`、hard ceilingおよび全resource limitを含める。一件でも変更する場合はversionを上げ、旧proofを再解釈しない。

受理するposeはexact tree authorityと同じissuer、model、pose instance、fixed face、全angle bits、generation/versionを持つものだけとする。valid clone以外の同角度再solve ABA、foreign issuer/model、reroot、角度1 ULP差を拒否する。face、hinge、boundary occurrence、transformの欠落・重複・非canonical順、非finite係数、cycle、共有点/hinge端点検査の欠落も拒否する。

work accountingはface数`F`、hinge数`H`、最大深さ/深さ合計、transform scalar `12F`、boundary occurrence `O`、point component `3O`、共有occurrence、hinge endpoint、certificate read、admission演算、exact rational演算、入力・中間・出力の最大/合計bit数をaggregateなchecked counterへ課金する。全limitにexact-limit成功/one-short失敗を設け、face/hingeごとにresetしない。一件でも失敗した場合はproof、radius、matrixを全て破棄し、partial resultやlegacy binary64 safe resultへfallbackせず、Cはblockingな`indeterminate`に留める。

#### C.2 Three.js rendererへの最終DTO橋渡し

上記が包含するのはnativeの`MaterialTreePose`である。現在のTS/Three.js側は`Math.PI`と`Matrix4`で姿勢を別計算しているため、native proofが成功しても画面上のrenderer poseまで証明したことにはならない。Cのrenderer checkpointを完了するには、認証済みnative face/hinge transformをcanonical ID順のdetachedかつversion付きDTOとして発行し、committed poseでのTS側の三角関数・tree再計算を停止する。

Three.js列優先4×4 matrixのDTO layoutは次で固定する。この配列は`Matrix4.elements`または`Matrix4.fromArray`用であり、row-major引数の`Matrix4.set(...dto)`へそのまま渡してはならない。

```text
[r00,r10,r20,0, r01,r11,r21,0, r02,r12,r22,0, tx,ty,tz,1]
```

DTOはproject instance、revision、pose generation、fixed face、全angle bits、model/proof IDとversion、geometry digestへ結合する。ただしDTOとrenderer ACKは観測値であり、native capabilityやcommit authorityを生成しない。native proofの直接範囲は発行したDTO payloadのbinary64 bit列までとする。UIは全ID、length、finite値、version/digestを一時領域で検証し、`fromArray`後の`Matrix4.elements`がDTO bit列と一致することをclient-side bridge testで確認してから全matrixを原子的に交換する。このclient検証をnative authorityへ逆輸入しない。欠落、重複、並べ替え、stale generation/digest、NaN/Infまたはcopy途中失敗では旧表示を維持する。GPU f32、camera、rasterization、pixel誤差は別の表示品質検証とする。local drag previewを残す場合はuntrusted previewとして明示し、collision certificateまたはmutation authorityへ使わない。

matrix bit列の一致だけでは、`Vector3.applyMatrix4`等のbinary64乗算・加算後のworld点を直接包含しない。CPUで頂点変換する経路を残す場合は、version固定の演算順で実際のbinary64結果をbit-exact有理数へ戻し、理想`F(v)`との差`radius_cpu_apply`を全境界頂点で証明し、表示対応の上限を`radius + radius_cpu_apply`とする。別案としてnative計算済みworld頂点DTOを発行し、UIが再計算せずそのbit列を使用してもよい。いずれも未実装の間はrenderer containmentを完成扱いにしない。GPUだけでmatrixを適用する経路はcollision authorityへ使わず、f32変換以降を表示品質検証へ分離する。

#### C.3 先行test matrix

実装前に少なくとも次を失敗testとして固定する。

| 軸 | 必須case |
| --- | --- |
| tree | single face、one hinge、非可換2 hinge `41/63`、角起点V、山山V、chain depth `1/2/32/max`、shared-vertex fan、全face reroot |
| axis/angle | X/Z、3-4斜め、非平方、subnormal、`0`、最小正値、`90度±1 ULP`、`90/135/179度`、`180度-1 ULP`、`180度`、山/谷とleft/right反転 |
| coordinate | 400 mm、共通平行移動`0/±10^12/±3×10^12/±10^15 mm`、巨大pivot+短hinge、precision collapse |
| containment | 全境界頂点、凹face内部barycentric sample、root radius 0、共有occurrence、hinge endpoint、各matrix scalar `±1 ULP` |
| authority | valid clone、同角度ABA、foreign issuer/model、reroot、angle 1 ULP差、stale generation/version |
| resource | 全counterのexact-limit/one-short、最大/合計bit、checked overflow、`N_d*u = 1/2`、hard ceiling直下/一致/直上 |
| DTO | row/column layout、ID欠落/重複/順序違反、NaN/Inf、stale digest、atomic rollback、committed poseの独立再計算禁止、CPU `applyMatrix4`丸め包含またはnative world頂点bit照合 |

標準座標では全faceのexact点が成分radius内にあること、同じ入力とstorage reorderでradius/DTOが決定論的であること、cardinal branchとshared occurrenceが一致することを期待する。巨大座標でceilingを超えるcaseは包含幅を緩めず明示的blockingを期待する。これらが完了して最終DTOが実表示へ接続されるまで、native containmentとrenderer containmentの両方を未完了と数える。

先行テスト:

- 共有なし、頂点共有、辺共有、同一面
- 離間、点接触、線接触、面接触、横断交差
- 紙厚 `0 / 0.1 / 3 mm`
- 角度 `10 / 90 / 179 / 180度`
- 共有頂点の偽陽性と近平行・zero-thicknessの偽陰性
- 全pair coverage、work accounting、indeterminate
- frontend/nativeのv1 4×10およびv2 4×11分類表と同じcanonical fixture corpus

### D. native continuous collision

対象:

- `ori-collision` のcontinuous module
- desktop background job境界

先行テスト:

- single-hingeの全時間区間
- collision直前停止
- narrow interval内の衝突
- endpoint安全・途中衝突
- deadline、中止、work limit
- stale completion
- indeterminateを安全と扱わない

### E. cell-order transport

対象:

- current layer-order capabilityとのnative結合
- canonical cell再検証
- order-transport module

先行テスト:

- 同じcertificate `Arc` とgenerationだけを受理
- material registry、provenance、reference face不一致拒否
- no-hingeと全180度flat bootstrap
- 90度、179度、角度混在をflat certificateへ誤結合しない
- 局所的に順序が異なる複数cell
- path中のcontact/crossingでorderが変わるケース
- endpoint無衝突でもtransport未証明なら拒否
- incomplete cell coverageとindeterminate拒否

### F. atomic SIM-010 authority

対象:

- `apps/desktop/src-tauri/src/simulation_authority.rs`
- `ori-core` の原子的 `ApplyStackedFold`

先行テスト:

- AからEの各certificate欠落時にmutation前拒否
- model、binding、generation、`Arc` identityの各不一致拒否
- worker計算中のedit、Undo/Redo、open、pose変更
- commit直前stale
- 成功時にpattern、layer order、pose、timelineが一履歴entry
- 全失敗経路で完全不変
- Undo/Redo後もauthority generation単調増加
- 固定lock順の並行テスト
- one-shot authorityの再利用拒否

## 12. UI公開ゲート

折り重ねUIを公開してよいのは、次がすべて満たされた場合だけとする。

- AからFがnativeで実装済み
- 任意の対応対象3D姿勢についてcontinuous collisionとcell-order transportが完了する
- unsupportedとindeterminateを安全側へ表示・停止できる
- stale、cancel、deadline、panicを含む失敗時不変テストが通る
- Windows実機E2Eで、現在の局所重なり層、展開図への層別山谷線、timeline一step、Undo/Redoを確認できる

全180度flat bootstrapだけの段階、TS衝突診断だけの段階、native endpoint collisionだけの段階では公開しない。

本設計追記は未実装の次工程を固定したものであり、全体完成度は36.9%のままとする。折り重ねUIは分類、exact/native包含、renderer DTO、continuous collision、cell-order transportおよびatomic authorityの完了後に最後に実装する。
