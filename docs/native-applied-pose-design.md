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

`topology_contact_policy_v1`の純粋な4×10・全40セルは互換基準として凍結した。正厚三角柱の正面積境界接触を独立させた`topology_contact_policy_v2`の4×11・全44セルも、frontendとnativeが同じ正規JSON corpusへ照合する形で先行実装した。この段階は入力evidenceを証明せず、共有ヒンジを許容せず、衝突certificateも発行しない。次の先行テストと実装で、current native poseに束縛した共有関係・交差証拠の肯定証明、全pair coverage、有限ヒンジモデルおよび作業上限を追加して初めてCのcertificate境界とする。`boundary_area_contact`は正厚だけが発行でき、共有ヒンジでは表だけで許容せず有限corridorの全pair証明を要求する。runtimeで`same_face`がdispatcherへ到達した場合は内部不整合として`indeterminate`へ閉じる。

最初の下位証拠として、`ori-collision::NativeStaticCollisionGeometryProof`は単一material face・no-hingeに限り、unordered face pairの期待数と解析数がともに0である完全なzero-pair proofを発行してよい。proofはexact material model issuer、pose instance、紙厚のbinary64 bits、kinematics/thickness/contact policy/proof model IDへ結合する。複数面は全pair evidenceが無い限り`PairEvidenceUnavailable`でblockingとする。このgeometry proofはproject instance、revision、current pose certificate identityまたはpose generationを持たないためCではなく、project mutation authorityは常にfalseである。desktopがBのexact certificate `Arc`とgenerationを`project -> pose` lock下で再検証し、同じproof objectへ封印したprivate current static collision certificateを発行して初めて、限定された単一面についてCへ到達する。

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
