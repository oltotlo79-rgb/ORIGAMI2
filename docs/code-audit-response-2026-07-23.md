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
|B-8|説明矛盾として妥当・修正済み|`6412942`。円は現状visual guideで自動snapしないと明記。circle intersection snap自体は新規要件F-2。|

## C: 改善

|項目|判定|対応|
|---|---|---|
|C-1|妥当な改善|カテゴリ優先ではなくscreen distanceを基準にする変更は挙動仕様変更を伴う。snap優先度の回帰行列を追加してから適用する。|
|C-2|一部妥当|境界edge除外は紙境界を通常creaseとして分割しない既存方針でもある。既存孤立頂点との接続は妥当だが、一括自動分割と同じ新規編集commandとして扱う。|
|C-3|新規要件|分数/N分割gridは現行要件にない。F-3として管理する。|
|C-4|妥当・修正済み|`4c28d50`。`fileOperationActive`を全編集gateへ追加。|
|C-5|妥当な改善|コンパイル済み文言のlocale key化が必要。物理証明の正否には影響しない。|
|C-6|妥当・修正済み|`ecc1dd3`。block authorityの2 schedule終端と保存target anglesをapply時にbit-exact再照合。WSL回帰1/1。|
|C-7|妥当・修正済み|`4c28d50`。visual JSON構造検証とGLB certificate export gateを追加。DOM 11/11。|

## D: 機能提案

F-D、F-E、F-A、F-1、F-2、F-3、F-6、F-10、F-11、EDT-009、交差一括分割、F-4、F-9はいずれも実装可能性の提案であり、監査基準時点の不具合ではなく新規要件である。AUT-101/AUT-005/SIM-010の一般解は監査記載どおり研究課題であり、短期完成条件へ混入させない。

## 検証

- frontend TypeScript/Vite build: 成功。
- frontend snapshot: 1,653/1,653。
- `StackedFoldPanel` DOM: 36/36。
- `InstructionTimelinePanel` DOM: 11/11。
- WSL `ori-core` A-4: 1/1、`ori-formats` A-5: 1/1。
- WSL `ori-instructions`: 40/40。
- WSL blockwise target-angle回帰: 1/1。
- desktop `cargo check`: 成功。無関係な既存warningは別所有差分として未変更。
