# 2026-07-23 Claudeコード監査への応答

基準は `docs/plans/code-audit-2026-07-23.md`。指摘を現行コード、回帰テスト、既存仕様に照合した。`docs/progress.md` の全体完成度は全CI成功まで更新しない。

## A: 不具合

|項目|判定|対応証拠|
|---|---|---|
|A-1/A-2|妥当・修正済み|`1863da8`。cross-block cell/pairを無音破棄せず、証明済みpairだけを保存する。|
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
|B-1/B-2/B-3/B-4/B-9|妥当な文書不整合|公式進捗はCIゲート中のため79.3%を維持する。自動設計60%への上方修正は採用せず、一般tree生成の実証範囲を再計上する。旧集計は次の公式更新時に単一表へ統合する。|
|B-5|妥当・修正済み|`657f902`。山谷は表示名でなく選択crease assignmentと明示kindで識別。WSL `ori-instructions` 40/40。|
|B-6|一部妥当・修正済み|非連結blockを許す指摘は妥当。`ecc1dd3`でblock intersection graphをtreeに限定。公開型の死蔵は機能欠陥ではなく整理課題。|
|B-7|妥当・修正済み|`4c28d50`。split/merged noticeを追加。|
|B-8|説明矛盾として妥当・修正済み|`6412942`で当時の制限を正確に明記し、`77e8f1d`と後続境界回帰で円×線・円×円の交点snapを頂点追加・辺分割へ接続した。|

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
|F-6 step camera取得|INS-004 MUST|camera保存自体は既存visual JSON編集で可能。専用取得buttonは新規UXだが既存スコープを具体化するため実装対象。|
|EDT-009 最小不能部分集合|EDT-009 MUST|既存スコープ内の未達。EDT-009を部分実装へ是正し、実装対象とする。|
|F-D/F-E|INS-001|作成・任意index移動は既に満たす。複製、先頭末尾button、DnDは新規shortcutでありMUST未達ではない。|
|F-A/F-11|PRJ-008/SIM系|既存の単線・寸法表示を越える新規計測モード。新規スコープ。|
|F-1|EDT-003|角度・長さ指定作図は実装済み。ray hit自動終端は新規スコープ。|
|F-3|EDT-006|grid snap自体は実装済み。分数/N分割UIは新規スコープ。|
|F-10|INS-008/009|preview・確認・適用は安全境界として意図的に分離。1操作chainは安全UX方針と衝突するため非採用。|
|交差一括分割|VAL-001|検出MUSTに修復commandは含まれず、新規スコープ。|
|F-4/F-9|該当MUSTなし|配列複製とonion skinはいずれも新規スコープ。|

AUT-101/AUT-005/SIM-010の一般解は監査記載どおり研究課題であり、短期完成条件へ混入させない。

## 検証

- frontend TypeScript/Vite build: 成功。
- frontend snapshot: 1,653/1,653。
- `StackedFoldPanel` DOM: 36/36。
- `InstructionTimelinePanel` DOM: 11/11。
- WSL `ori-core` A-4: 1/1、`ori-formats` A-5: 1/1。
- WSL `ori-instructions`: 40/40。
- WSL blockwise target-angle回帰: 1/1。
- desktop `cargo check`: 成功。無関係な既存warningは別所有差分として未変更。
