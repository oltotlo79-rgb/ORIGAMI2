# 折り重ね操作の原子的トランザクション設計

## 1. 対象

本書はMUST要件SIM-010の初版契約を定める。対象は、現在の3D状態で指定した一直線をまたぐ重なり層をすべてまとめて折り、その結果を展開図と折り手順へ一操作として反映する機能である。層を個別に選んでめくる操作、中割り折り等の技法固有運動、曲線折りは対象外とする。

利用者から見た一操作を途中状態へ分割しない。展開図だけへ折り線が増えた状態、3D姿勢だけが変わった状態、過去手順だけが古い面IDを参照する状態は、成功時にも失敗時にも公開しない。

## 2. 正本とauthority

入力の正本は次のnative current stateである。

- project instance、project ID、source revision
- source paper、crease pattern、履歴
- 3Dへ実際に適用済みの完全な面姿勢と全ヒンジ角
- VAL-003が発行したcurrent layer-order slot、そのimmutable binding、完全なcertificate
- world空間の操作直線、固定側、折り方向、要求角度

WebViewから渡されたface ID列、層順、face lineage、fingerprintまたはcertificateをauthorityとして受理しない。解析開始時にimmutable snapshotを取得し、重い計算はproject lock外で行う。commit直前にproject instance、ID、revision、geometry、applied pose、current layer-order slotのobject identityとbindingを再照合する。一つでも変化していればstaleとして全候補を破棄する。

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

逆写像、対象層決定、差分検証、face lineage、山谷割当て、衝突証明、層順再計算、certificate再検証、timeline移行、stale再照合、資源上限、中止のいずれが失敗しても、次を開始時とbit-exactに同じまま保つ。

- paper、crease pattern、revision、dirty baseline
- 現在の3D姿勢と選択
- current layer-order slot
- timeline
- Undo/Redo stack
- 保存済みprojectと書き出しstage

panic payload、作品座標、path、raw OS errorはIPCへ出さず、固定categoryと利用者向け理由だけを返す。

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
- Undo後にpattern、層順、姿勢、timeline、revisionが全て操作前へ戻る
- target patternの無関係な頂点、辺、線種、紙属性変更を拒否する
- stale pose、stale revision、偽造layer snapshot、lineage不一致を拒否する
- 貫通は直前停止し、判定保留は安全成功として沈黙させない
- 厚さ`0 / 0.1 / 3 mm`と深角で、展開図更新と衝突表示が同じ終端姿勢を参照する
- 途中の各失敗、中止、期限切れ、panicで全project stateが不変である
