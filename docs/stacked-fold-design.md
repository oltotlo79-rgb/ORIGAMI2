# 折り重ね操作の原子的トランザクション設計

## 1. 対象

本書はMUST要件SIM-010の初版契約を定める。対象は、現在の3D状態で指定した一直線をまたぐ重なり層をすべてまとめて折り、その結果を展開図と折り手順へ一操作として反映する機能である。層を個別に選んでめくる操作、中割り折り等の技法固有運動、曲線折りは対象外とする。

利用者から見た一操作を途中状態へ分割しない。展開図だけへ折り線が増えた状態、3D姿勢だけが変わった状態、過去手順だけが古い面IDを参照する状態は、成功時にも失敗時にも公開しない。

実装順の前提は、VAL-003のcurrent layer-order slotと、互換用に凍結した`topology_contact_policy_v1`の4×10、およびnative証拠生成の正本である`topology_contact_policy_v2`の共有関係4種×交差証拠11種、共有頂点exact証明、有限共有ヒンジcorridor、判定保留表示および境界回帰が完成していることである。折り重ねはこの衝突分類を停止判定の正本として使用するため、分類仕様・実装・回帰を固定する前に折り重ねUIを公開しない。

## 2. 正本とauthority

入力の正本は次のnative current stateである。

- project instance、project ID、source revision
- source paper、crease pattern、履歴
- 3Dへ実際に適用済みの完全な面姿勢と全ヒンジ角
- VAL-003が発行したcurrent layer-order slot、そのimmutable binding、完全なcertificate
- world空間の操作直線、固定側、折り方向、要求角度

WebViewから渡されたface ID列、層順、face lineage、fingerprintまたはcertificateをauthorityとして受理しない。解析開始時に `project -> pose slot -> layer-order slot` の固定lock順でimmutable snapshotを取得し、重い計算はproject lock外で行う。commit直前にも同じ順序でproject instance、ID、revision、geometry、applied pose、current layer-order slotのobject identityとbindingを再照合する。一つでも変化していればstaleとして全候補を破棄する。既存のlayer-order guarded closure内からpose lockを後取りしてはならない。

current applied pose、native kinematics、衝突certificate、continuous path、場所別cell-order transport、generationおよびlock順の詳細は[native current applied pose設計](native-applied-pose-design.md)を正本とする。WebViewのMatrix4、表示用衝突結果または姿勢snapshotをnative mutation authorityへ昇格させない。

## 3. 成功トランザクション

将来の`ApplyStackedFold` commandは次を順に準備し、最後の一回だけ状態を確定する。

1. 現在姿勢と場所別stackから、操作直線を横切る全対象層、固定側、移動側を決定する。
2. world空間の直線を対象層ごとの材料面へ逆写像する。各線分は元の一枚紙上の有限な直線区間でなければならない。
3. 交点を分割したcandidate patternを構築し、層の表裏と回転方向から各新規区間へMountainまたはValleyを割り当てる。既存要素、既存線種、紙属性は許可した分割以外変更しない。
4. `face_lineage_v1`でsource faceからtarget descendant faceへの完全写像、包含、source別の厳密面積保存、revisionの一段更新を証明する。
5. 新しい直線を一つの集合ヒンジとして動かし、要求角までの連続経路を衝突判定する。貫通または判定保留では直前の認定済み角度で停止し、理由を保持する。
6. 終端姿勢から場所別層順序を再計算し、immutable target geometryからcertificateを再検証する。
7. 既存timelineの全stepをtarget topologyへ移行する。source faceのtransformは全descendantへ継承し、新しいヒンジは過去stepでは0度とする。参照を移行できないstepが一つでもあれば失敗する。
8. 実際に認定された終端姿勢、対象層、層別山谷線、停止理由を持つ新しいtimeline stepを一つ追加する。
9. candidate pattern、target layer order、applied pose、timeline、revisionを一つのeditor commandとしてcommitし、Undo/Redoにも一つの履歴entryだけを追加する。

要求角より前で安全停止した場合、展開図へ追加する折り線とtimeline stepは、実際に適用された非ゼロ終端角に対応させる。開始角から進めなかった場合は成功操作にせず、展開図もtimelineも変更しない。

### 3.1 証明済みApplyと投機的Apply

連続clearance certificateを得た経路は従来どおり証明済みApplyを使う。一方、bounded native samplingで全sampleがnonblockingだが連続区間のcertificateを発行できない場合に限り、利用者へ日英で未証明であることを明示して確認を取り、別commandの投機的Applyを許す。sampling結果を安全証明へ昇格させない。

投機的proposalは、少なくとも次を全て満たさなければ発行しない。

- sampled poseが1件以上あり、全件がnonblockingで、blocking sampleがない
- endpointの`hasBlockingHold`がfalse、`penetratingPairCount`と`indeterminatePairCount`がともに0
- targetの非平坦layer orderをnative geometryから再検証できる
- project instance、project ID、revision、geometry fingerprint、pose generation、request generation、紙厚bitが解析開始時と一致する

投機的権限は非`Clone`・非Serde・one-shotの`SpeculativeUnprovenFoldTokenV1`で表し、証明済みauthorityとは別型にする。両者の`From`、`Into`、`as_*`変換を設けない。tokenのpublic issuerはopaqueな`PreparedStackedFoldRequestedPoseV1`を受け取り、bounded collision diagnosticを内部で再実行する。raw metadataからtokenを発行する関数とtokenからhistory bindingを取り出す関数はcore外へ公開しない。

Apply直前には上記live bindingとcurrent pose/layer capabilityを再認証し、candidate document、applied pose、timeline、未証明markを単一history entryとして確定する。target pose authority、pair-proof cache epoch、current layer evidence、保存baseline、Undo/Redoまたはdirty状態の後段installに失敗した場合は、同じproject lock内で全てを操作前へ戻す。proposalをregistryへinstallした後も、response公開直前にgeneration、live binding、apply contractを再検査し、失敗時はその正確なtokenだけをABA-safeに撤去する。

未証明markはUndoでunapplied redo側へ移り、Redoでapplied側へ戻る。保存時は`.ori2`と展開folderの`required_features`へ`speculative_unproven_fold_v1`を必ず立て、旧readerをfail closedさせる。事後証明が`Blocked`または`Unknown`でも自動Undoせずmarkの状態だけを更新し、利用者が明示的に戻す場合だけ既存Undoを使う。

成功した投機的Applyは、history bindingに加えてsource/targetのopaque prepared pose、正確な紙厚、target geometry fingerprint、target pose generationをWebViewへ出さないbounded native registryへ発行する。V1 registryは最大8件、全体8 MiB、1件2 MiB、未開始保持5分、開始後30秒を上限とし、同じjob tokenとrun generationによるsingle-flightで`sample_intervals = 16, 32, 64`を順に実行する。累積上限は112 interval-workであり、各pollとhistory解決直前にproject instance、project ID、revision、target fingerprint、paper thickness、pose generation、face/hinge集合、fixed face、全hinge angle、および元のone-shot token bindingを再認証する。samplingだけを安全証明にせず、V1が明示的にallowlistした既存continuous certificate modelが完全に成立した場合だけ`Certified`、追加sampleのnative blocking witnessを得た場合だけ`Blocked`、全段を使い切れば`Unknown(EvidenceInsufficient)`とする。deadline、cancel、resource failure、staleまたはABA不一致は肯定結果より優先してfail closedし、自動Undoは行わない。Apply rollback中はregistryへ発行せず、Apply commit後のregistry容量・lock・allocation failureは確定済み編集を巻き戻さず、未証明markをそのまま保持する。

このregistryとprepared premiseはV1では意図的に非永続である。再open後は新しいproject instanceとなるため旧jobを再開せず、未証明markだけを保存済み状態として表示する。editor historyはsource documentのinverseとtarget documentを保持でき、documentはcurrent target poseも保持するが、history persistenceはruntime applied-pose transitionを保存せず、任意のsource fixed faceとsource poseを一般には一意に復元できない。したがってlatest stacked-fold predecessorだけから完全な連続経路premiseを再構成できるとは主張しない。再open再開を追加する将来版では、source poseを含むsealed premiseのversioned persistenceと、読込後の全geometry・pose・binding再生成比較を別仕様として導入する。

## 4. face lineage version 1

`ori-core::prepare_face_lineage_v1`は上記4の読み取り専用基盤である。sourceとtargetのtopologyをimmutable geometryから再構築し、現在の対象クラスに合わせて凸source faceだけを扱う。全target faceをただ一つのsource faceへ厳密に包含させ、sourceごとの面積をbinary64値から正確な2進有理数へ持ち上げて保存する。

証明はproject ID namespace、source/target revision、source/target fold-model fingerprintへ結合し、少なくとも一面が実際に分割された場合だけ返す。保存順、無向辺方向、紙境界cycleの開始点・向きには依存しない。

この証明が保証しないものは次のとおりである。

- candidate差分が一本の直線折りだけであること
- 層ごとのMountain/Valley割当て
- layer-order certificateの真正性
- 連続折り経路と衝突直前停止
- timeline移行とproject mutationのauthority

したがって`FaceLineageV1`だけを根拠に展開図を変更してはならない。公開transport型である`LayerOrderSnapshot`のfield一致も、native current slotの認証の代用にしない。

## 5. 失敗時不変条件

逆写像、対象層決定、差分検証、face lineage、山谷割当て、層順再計算、certificate再検証、timeline移行、stale再照合、資源上限、中止、またはApply後のtarget authority installのいずれがcommit前後の原子的区間で失敗しても、次を開始時とbit-exactに同じまま保つ。

- paper、crease pattern、revision、dirty baseline
- 現在の3D姿勢と選択
- current layer-order slot
- timeline
- Undo/Redo stack
- 保存済みprojectと書き出しstage

panic payload、作品座標、path、raw OS errorはIPCへ出さず、固定categoryと利用者向け理由だけを返す。

ただし、利用者が明示確認して投機的Applyを完了した後の事後証明失敗は、この「失敗時不変条件」の対象ではない。そこで無断rollbackは行わず、未証明markを`ProofBlocked`または理由別`ProofUnknown`へ更新する。

## 6. 資源、期限、中止

face lineage version 1は頂点、辺、面、半辺、face pair、厳密包含判定へ決定論的件数上限を持つ。件数上限はheap/RSSのhard上限とは呼ばない。UI commandでは全準備段階をproject lock外のbackground jobで実行し、deadlineとcooperative cancellationを加える。中止、期限切れ、上限到達、証明不足を「折れない」へ変換せず、変更なしの判定保留として扱う。

## 7. 実装段階

1. face lineageと面積保存の純粋証明
2. 一本の直線だけを許すcandidate edit-delta検証と層別山谷割当て
3. 現在3D直線の材料面への逆写像と対象stack決定
4. 集合ヒンジの連続衝突停止とtarget layer-order再証明
5. timeline全step移行と原子的`ApplyStackedFold`
6. background job、進捗、中止、UI操作、段階再生

段階1は内部基盤であり、単独ではSIM-010の利用者経路または製品完成率へ計上しない。

## 8. 受入試験

- 二層以上を横切る直線で、全対象層だけが一括して折れる
- 層の表裏に応じたMountain/Valleyが展開図へ追加される
- 既存面が複数に分割されても過去stepを同じ姿勢で再生できる
- 一操作がtimelineとUndo/Redoで常に一entryになる
- Undo後にpattern、層順、姿勢、timelineの意味内容が操作前へ戻る。一方、entryを実際に移動したUndo/Redoではrevisionとauthority generationが必ず単調増加し、古いworker、pose token、layer captureまたはcertificate `Arc`を再びcurrentにしない。復元したsemantic poseとlayer orderはcurrent geometryから新しいcertificateへ再証明する
- target patternの無関係な頂点、辺、線種、紙属性変更を拒否する
- stale pose、stale revision、偽造layer snapshot、lineage不一致を拒否する
- 貫通は直前停止し、判定保留は安全成功として沈黙させない
- blocking sample、endpoint hold、penetrating pairまたはindeterminate pairが1件でもあれば投機的tokenを発行しない
- 投機的Applyは明示確認なし、stale revision/pose/layer generation、紙厚1 ULP差、誤ったrequest generation、cross-mode tokenで変更なしに拒否される
- 投機的Applyの後段pose再発行失敗はdocument、history、dirty、保存baseline、pose/layer capability、pair-cache内容とepochを操作前へ戻す
- 未証明markはUndo/Redo、`.ori2`、展開folder、recoveryを往復し、未知featureの旧readerは読込を拒否する
- 事後証明失敗はmarkだけを更新し、自動Undoしない
- 厚さ`0 / 0.1 / 3 mm`と深角で、展開図更新と衝突表示が同じ終端姿勢を参照する
- 途中の各失敗、中止、期限切れ、panicで全project stateが不変である
