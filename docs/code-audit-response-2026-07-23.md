# 2026-07-23 Claudeコード監査への応答

基準は `docs/plans/code-audit-2026-07-23.md`。指摘を現行コード、回帰テスト、既存仕様に照合した。`docs/progress.md` の全体完成度は全CI成功まで更新しない。

## A: 不具合

|項目|判定|対応証拠|
|---|---|---|
|A-1/A-2|妥当・修正済み|`1863da8`, `0391d91`。cross-block cell/pairを無音破棄せず、保存pairを両block authorityが実際に証明した重複なし和集合へ限定し、authorityの認証済みpair数と再照合する。|
|A-3|妥当・修正済み|`b1efa98`。頂点ドラッグ時の未分割edge snapを拒否するDOM回帰を追加。|
|A-4|妥当・修正済み|`209ef1f`。角二等分を外積と方向残差の2行にし、反対向きを拒否。WSL 1/1。|
|A-5|妥当・修正済み|`209ef1f`。JSON writerでunderlayを検証し、ORI2 writerにも伝播。WSL 1/1。|
|A-6|妥当・修正済み|`372a38a`。正確180度の平坦終端診断と正厚経路の選択を回帰固定。|
|A-7/A-8/A-9|妥当・修正済み|`3911bf2`。専用transaction、技法切替時の旧証明破棄、schedule無しボタン無効化。DOM 36/36。|
|A-10|妥当・修正済み|`4c28d50`。declarative stepのSplit/Mergeを無効化。|
|A-11|妥当・修正済み|`d65de35`, `5ee7774`。lock保持経路のexpect/unreachableをfallible化。desktop check成功。|

## B: 矛盾

|項目|判定|対応|
|---|---|---|
|B-1/B-2/B-3/B-4/B-9|妥当な文書不整合・正本以外を是正済み|`13b1772`, `09c2e52`。公式進捗はCIゲート中のため79.3%を維持し、指示により`docs/progress.md`は変更しない。pending再評価は自動設計を60%へ上げず35%へ下げ、81.96%（表示82.0%）へ是正した。`requirements-status.md`は旧「現在」集計を履歴コメントへ隔離し、正本を85/2/0へ統一した。加重完成度とMUST行数は異なる指標であり同率には扱わない。|
|B-5|妥当・修正済み|`657f902`。山谷は表示名でなく選択crease assignmentと明示kindで識別。WSL `ori-instructions` 40/40。|
|B-6|一部妥当・修正済み|非連結blockを許す指摘は妥当。`ecc1dd3`でblock intersection graphをtreeに限定。公開型の死蔵は機能欠陥ではなく整理課題。|
|B-7|妥当・修正済み|`4c28d50`。split/merged noticeを追加。|
|B-8|説明矛盾として妥当・修正済み|`6412942`で当時の制限を正確に明記し、`77e8f1d`, `9d20cde`, `ed0ace0`で円×線・円×円の交点snapを頂点追加・辺分割へ接続し、接線・重複・非有限値・紙面外を回帰して案内文も実装へ再同期した。|

## C: 改善

|項目|判定|対応|
|---|---|---|
|C-1|妥当・修正済み|`4314420`。screen distanceと小さなcategory biasで全候補を比較。snap 101/101。|
|C-2|一部妥当|境界edge除外は紙境界を通常creaseとして分割しない既存方針でもある。既存孤立頂点との接続は妥当だが、一括自動分割と同じ新規編集commandとして扱う。|
|C-3|新規要件|分数/N分割gridは現行要件にない。F-3として管理する。|
|C-4|妥当・修正済み|`4c28d50`。`fileOperationActive`を全編集gateへ追加。|
|C-5|妥当・修正済み|`6c3f972`。認証済み技法の固定手順文を日英併記し、英語利用者へ日本語だけを返さない。WSL 40/40。|
|C-6|妥当・修正済み|`ecc1dd3`。block authorityの2 schedule終端と保存target anglesをapply時にbit-exact再照合。WSL回帰1/1。|
|C-7|妥当・修正済み|`4c28d50`。visual JSON構造検証とGLB certificate export gateを追加。DOM 11/11。|

## D: 機能提案

|項目|要件照合|判定|
|---|---|---|
|F-2 コンパス円交点snap|EDT-007 MUST|指摘は妥当。`77e8f1d`で円×線・円×円交点を既存snap契約と頂点追加・辺分割へ接続し、接線・重複円・非有限値・同一点候補・紙面外境界を追加回帰した。既存native commandを共有するためUndo/Redo・履歴永続化も同じ経路となる。|
|F-6 step camera取得|INS-004 MUST|`57ee8e5`, `818d9f6`, `70c4c7f`。3D previewの現在cameraを取得して選択stepの認証済みvisual更新へ渡し、camera未取得時は操作を無効化する。取得cameraをexact preview model keyへ束縛し、project・revision・model切替後の古いcameraを別stepへ保存できない。DOMとApp統合回帰を追加した。|
|EDT-009 最小不能部分集合|EDT-009 MUST|`a626dae`, `3b3a916`。「Gauss-Newtonのrankから一般MUSを容易に抽出できる」という実装提案は根拠不足。局所solverの`NonConvergent`、`RankDeficient`、資源超過は不充足証明ではなく、これをoracleにしたsubset縮約は偽の矛盾原因を表示しうる。soundな7種の直接矛盾がcanonical・削除最小な原因IDだけを返すことは回帰固定したが、production能力は増えていないため実装済みへ昇格せず85/2/0へ戻した。それ以外は`Unknown`へ閉じ、一般化にはsoundなunsat oracleを先に要する。|
|F-D/F-E|INS-001|作成・任意index移動は既に満たす。複製、先頭末尾button、DnDは新規shortcutでありMUST未達ではない。|
|F-A/F-11|PRJ-008/SIM系|既存の単線・寸法表示を越える新規計測モード。新規スコープ。|
|F-1|EDT-003|角度・長さ指定作図は実装済み。ray hit自動終端は新規スコープ。|
|F-3|EDT-006|grid snap自体は実装済み。分数/N分割UIは新規スコープ。|
|F-10|INS-008/009|preview・確認・適用は安全境界として意図的に分離。1操作chainは安全UX方針と衝突するため非採用。|
|交差一括分割|VAL-001|検出MUSTに修復commandは含まれず、新規スコープ。|
|F-4/F-9|該当MUSTなし|配列複製とonion skinはいずれも新規スコープ。|

AUT-101/AUT-005/SIM-010の一般解は監査記載どおり研究課題であり、短期完成条件へ混入させない。

## SIM-010の証明済み範囲と未証明境界

`1668933`, `1d167c0`, `7d0cc69`, `99ebfe6`により、既存の単一hinge・厚さ0経路だけでなく、bounded dyadic graph/cycleとpositive-thickness Treeの一部もproduction native routeへ到達している。Tree routeは正厚連続certificateとshared-vertex layer transportを別々に保持し、preview mintとApply直前にschedule端点、source/target、紙厚、層証拠、project instance/revision/fingerprint/generationを再照合する。Applyはone-shot token、明示確認、原子的履歴、Undo/Redo・再openへ接続済みである。

一方、これは任意の一般姿勢、任意多hinge schedule、一般共有hinge admission、完全な正厚衝突、一般複数層transport、全経路closure、専用層順viewerを証明しない。browser harnessのmock成功だけを能力証拠には用いず、native certificate fixtureと本番routeの双方がある狭いcaseだけを実装範囲とする。従ってSIM-010は部分実装を維持する。

## 検証

- frontend TypeScript/Vite build: 成功。
- frontend Node: 1,658/1,658、DOM: 335/335。
- `StackedFoldPanel` DOM: 36/36。
- `InstructionTimelinePanel` DOM: 11/11。
- WSL `ori-core` A-4: 1/1、`ori-formats` A-5: 1/1。
- WSL `ori-instructions`: 40/40。
- WSL blockwise target-angle回帰: 1/1。
- desktop `cargo check`: 成功。無関係な既存warningは別所有差分として未変更。
- compass intersection Node: 108/108、DOM: 10/10、production build: 成功。
- requirements design evidence: 3/3（正本85/2/0）。
- WSL `ori-core` EDT-009 direct witness回帰: 1/1（300件filter）。production変更なしのため要件昇格証拠には用いない。

検証件数は各対応コミット時点の対象suiteであり、異なる時点の件数を一つの全suite件数として合算しない。全CIが成功するまでは公式完成度を更新しない。
