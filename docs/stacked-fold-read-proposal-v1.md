# 折り重ね読み取り提案 v1

## 1. 目的

本書はSIM-010のうち、projectを一切変更せず、現在姿勢・現在層順序・一直線の折り候補を一つのnative captureへ再結合する最初の内部境界を定める。

この段階は折り重ね操作の実行ではない。対象cellとbottom-to-top層を列挙する読み取り提案だけを発行し、衝突安全、連続運動、材料面への逆写像、山谷割当、target layer order、timeline移行、`ApplyStackedFold`のいずれの権限も発行しない。

モデルIDは次で固定する。

- guard: `native_flat_stacked_fold_read_guard_v1`
- proposal: `native_linear_stacked_fold_read_proposal_v1`

## 2. 認証境界

guardは次を一つのopaque sealへ結合する。

- non-persisted project instance ID
- project ID
- source revision
- pose generation
- layer-order generation
- exact `MaterialTreeKinematicsModel` issuer
- exact `MaterialTreePose` instance
- exact current `LayerOrderSnapshot` object
- 再検証済みflat endpoint layer-order anchor

IDとgenerationは値が一致するだけではmutation authorityにならない。desktop bridgeはproject lock下でposeとlayer-orderのprivate current capabilityを取得し、lock外のblocking workerでguardとproposalを構築した後、応答直前にproject、pose slot、layer-order slotへ再照合する。編集、同角度pose再発行、同内容layer-order再解析、cancel、reopenのいずれでも古い結果を返さない。

guardのcloneは同じseal identityを保つ。同じproject、同じ角度、同じ層順序の値からguardを再captureしても新しいsealになり、古いproposalは新しいguardへ再結合できない。poseの同角度再solve、layer-orderの同内容再解析、project reopen、generation変更も同様に拒否する。

guardとproposalはSerialize不可とし、getterが返すID、候補線、cell key、face列、work countからopaque sealを復元できない。

desktopの`propose_current_stacked_fold_read`は、strictなproject/revisionと一直線候補を受け、model ID、generation binding、SHA-256 cell key、完全なbottom-to-top face列、target face和集合、有限work countだけをbounded DTOへ写す。応答は`authorizesProjectMutation=false`と`authorizesApplyStackedFold=false`を固定し、opaque guard、proposal、model、pose、layer snapshotをWebViewへ渡さない。このcommandはUIへ未接続であり、単独ではSIM-010の利用者経路を構成しない。

## 3. 成功対象

v1が成功できるcurrent poseは次の二クラスだけである。

1. 一つのmaterial face、hingeなし、fixed faceなし
2. 接続tree、全hinge角がbinary64のbit列まで厳密に`180.0`、fixed faceがcurrent layer-order certificateのreference face

どちらもimmutable paperとpatternからtopology、material registry、fold-model fingerprint、exact folded face、canonical overlap cell、cell coverage、cell-local order、任意のglobal orderを再検証する。

`90度`、`179度`、`180度-1 ULP`、角度混在、cycle、未接続、wrong rootは成功集合へ含めない。一般角は`unsupported`、真正性・coverage・certificate・resourceに関する不足は`indeterminate`へ閉じる。

既存の`native_single_face_cell_order_transport_proof_v1`は、no-hinge姿勢とstatic collision proofを結合する別の下位証拠である。ただしproject、revision、layer-order certificate、generationを持たないため、このv1 guardの代用にしない。本境界も同proofを再解釈せず、将来のC/E結合で同一bindingへ追加する。

## 4. 一直線候補

候補は次を保持する。

- current world平面上の相異なる二点で定める有向無限直線
- fixed side
- 正負の回転方向
- `0 < angle <= 180`の要求角

現在のflat bootstrapではworld平面を`Y = 0`とし、候補点がこの平面外なら拒否する。各certified overlap cellのexact有理境界に対し、直線の両側へ厳密に内部が存在する場合だけ、そのcellを横断したと認める。

直線がcellの辺と一致する場合、または内部を横断せず頂点・辺へ接するだけの場合は、対象層を推測せず`indeterminate`とする。一つもcellを厳密に横断しない場合もproposalを発行しない。

## 5. 読み取り結果

proposalは次だけを観測用に返す。

- guardと同じproject/revision/generation binding
- 正規化済み直線候補
- 厳密に横断したcanonical cell key
- 各cellの完全なbottom-to-top face列
- 横断cell全体のtarget face和集合
- 決定論的work count

cellごとの層順序を一つのglobal face列へ潰さない。離れたcellで順序または対象集合が異なる可能性を保持する。

target face和集合はface IDのcanonical byte順へ正規化する。material face数とretained target face上限の小さい方まで出力容量を先行予約し、各追加の上限確認後は再allocationを発生させない。

proposalが保証しないものは次のとおりである。

- current poseへ到達したcontinuous path
- staticまたはcontinuous collision safety
- 紙厚層offset
- 操作線の各材料面への逆写像
- 固定側・移動側partition
- 層別Mountain/Valley
- candidate patternまたはface lineage
- target pose、target layer order、timeline
- project mutationまたは`ApplyStackedFold`

guardとproposalの`authorizes_project_mutation`および`authorizes_apply_stacked_fold`は常にfalseである。

## 6. 資源と回帰

次をaggregateなchecked counterへ課金し、上限との一致は成功、one-shortは失敗とする。

- scanned cells
- total boundary vertices
- total layer records
- exact orientation tests
- exact arithmetic operations
- 一演算の予測最大integer bits
- 全orientationへ課金する予測integer bits合計
- retained crossed cells
- retained target faces

下位flat anchorの15上限（source vertex、source edge、paper boundary vertex、face、hinge、cell、cellごとのboundary vertex、全boundary vertex、layer record、face-pair order、supporting cell、exact payload byte、exact integer bit、包含orientation、cell分離orientation）も同じcapture内で適用する。15項目すべてについて上限一致の成功とone-shortの失敗を回帰する。上限到達、checked overflow、allocation失敗では部分proposalを返さない。

必須回帰は次とする。

- no-hinge単一面と全180度tree
- `90 / 179 / 180度-1 ULP`および三面treeの`[180, 179]`混在
- project ID、source revision、project instance、pose/layer generation
- same-angle pose ABA、same-content layer snapshot ABA、fresh guard ABA
- foreign model issuer、wrong root
- cell keyおよびlayer certificate改変
- candidateの第一点、第二点、fixed side、rotation direction、requested angleの各差
- cell境界一致、接線、対象cellなし
- proposal全counterのexact-limitとone-short、下位anchor全15上限のexact-limitとone-short
- checked count・exact bit-bound overflowおよび出力reserve失敗
- guard/proposalがSerialize不可かつmutation authorityを持たないこと
