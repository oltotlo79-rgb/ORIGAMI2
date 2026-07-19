# 衝突分類 v2 実装カバレッジ台帳

## 1. 文書の目的

本書は、[`topology_contact_policy_v2`](collision-contact-policy-v2.md)の純粋な4×11 decision表と、そのdecisionへ入力するnative幾何証拠生成の完成度を分離して追跡する非規範の実装台帳である。純粋表が全44セルを返せることと、現在姿勢から全11種の証拠を発行できることを同じ「完成」と数えてはならない。

折り重ねは衝突分類を停止判定の正本として使用する。したがって、本書の「折り重ね前ゲート」が全て完了するまで、折り重ねUIを分類器より先に公開しない。

## 2. 4×11純粋表

次の三者は全44セルを一対一で照合済みである。

- 正規corpus: [`collision-contact-policy-v2.json`](collision-contact-policy-v2.json)
- native公開境界: [`ori-collision`](../crates/ori-collision/src/lib.rs)
- frontend公開境界: [`foldPreviewTopologyContactPolicy.ts`](../apps/desktop/src/lib/foldPreviewTopologyContactPolicy.ts)

native unit testとfrontend testは、4共有関係と11証拠の直積を正規corpusへ照合する。さらにnativeの公開APIだけを使う[`topology_contact_policy_v2_matrix.rs`](../crates/ori-collision/tests/topology_contact_policy_v2_matrix.rs)は、軸の重複、行の欠落、未知decision、重複セルを拒否し、`relation::evidence`の44個のcanonical cell IDが一度ずつ現れることを固定する。runtimeへ`same_face`が到達した44表上の11セルは、`ignored_self`を認可に使わず全て`indeterminate`へ閉じる。

結論: **純粋な4×11表は完成している。幾何証拠生成は未完成である。**

## 3. native幾何証拠の到達性

| 交差証拠 | 純粋表4セル | native現在姿勢からの生成 | 現在の回帰 | 残作業 |
| --- | --- | --- | --- | --- |
| `separated` | 完了 | 厚さ0で実装 | exact三角形、面交換、頂点順 | 正厚full scanへ統合 |
| `point_contact` | 完了 | 厚さ0で実装 | 一般点、共有点以外、subnormal | 正厚境界との統合 |
| `boundary_line_contact` | 完了 | 厚さ0で実装 | 実外周、部分共有辺、人工対角線除外 | 正厚境界との統合 |
| `boundary_area_contact` | 完了 | 未実装 | 純粋表だけ | 正厚閉三角柱の正面積・正体積0証明 |
| `shared_feature_contact` | 完了 | 厚さ0の共有頂点・共有辺で実装 | exact singleton、完全共有辺、誤った共有点/部分辺拒否 | watertight tree姿勢と有限ヒンジへ結合 |
| `shared_feature_thickness_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚中央面再証明とprivate provenance |
| `shared_feature_flat_stack` | 完了 | native未実装 | frontend現行経路のみ | 厚さ0・厳密180度・有限ヒンジ範囲の証明 |
| `coplanar_area_overlap` | 完了 | 厚さ0で実装 | 共有なし、共有頂点、共有ヒンジ、180度 | watertight tree姿勢へ結合 |
| `transversal_crossing` | 完了 | 厚さ0で実装 | exact binary64、近平行、悪条件、共有点外横断 | watertight tree姿勢と正厚full scanへ結合 |
| `positive_volume_overlap` | 完了 | native未実装 | frontend現行経路のみ | 正厚三角柱SATの肯定証明 |
| `indeterminate` | 完了 | 厚さ0で実装 | 退化、作業上限、共有姿勢不一致、同一面到達 | 正厚・有限ヒンジ・連続経路の全失敗理由 |

`no_shared_feature`で共有要素専用証拠が来るセル、共有identityと`separated`が同時成立するセル、`same_face`の全セルなどは、幾何生成の成功fixtureを作る対象ではない。これらは矛盾または走査前除外を表すため、純粋表とruntime fail-closed回帰で固定する。

## 4. 利用者報告回帰

| 回帰 | frontend現在経路 | native厚さ0基盤 | native正厚production |
| --- | --- | --- | --- |
| 角起点の山谷V: 厚さ`0 / 0.1 / 1 mm`×`片側10度左右2通り / 両側45 / 91 / 135度`の15姿勢 | 全15姿勢を`allowed_shared_vertex_contact`・貫通0で回帰 | `10/0, 45/45, 91/91, 135/135`を回帰。右だけ10度はfrontendでのみ固定 | 未実装 |
| 報告A: 厚さ0、片側10度 | 共有頂点許容・貫通0 | exact共有頂点接触を回帰 | 対象外 |
| 報告B: 厚さ0、両側180度 | 共面正面積・貫通1 | `coplanar_area_overlap`・`penetrating`を回帰 | 対象外 |
| 辺中点の山山V: 厚さ`0 / 0.1 / 3 mm`×`90 / 91 / 135 / 179度` | 90/91度は`indeterminate`、135/179度は`penetrating`で全12姿勢を回帰 | 4角度を走査するが、現行binary64 tree姿勢の共有点不一致により全て`indeterminate`。135/179度は正式期待値へ未到達 | 未実装 |
| 共有点外の横断: 厚さ`0 / 0.1 / 1 mm` | 全9姿勢を`penetrating`で回帰 | 厚さ0 exact横断を回帰 | 未実装 |

nativeの山山V 135/179度が`indeterminate`であることは、安全側の一時退避であり、正式期待値の達成ではない。`rational_cayley_local_rotation_v1`単体だけでこの行を完了扱いにせず、issuer-bound tree全体のwatertight姿勢へ合成してから`transversal_crossing`を再証明する。

## 5. 折り重ね前ゲート

次を順番に完了する。

1. 有理Cayley局所回転を同一issuerのtree traversalへ合成し、全共有頂点と共有ヒンジ端点がexactに一致するwatertight姿勢を作る。
2. 山山Vの厚さ0・135/179度を、pose mismatchではなく`transversal_crossing`・`penetrating`としてnativeで証明する。
3. 正厚の`boundary_area_contact`、`shared_feature_thickness_overlap`、`positive_volume_overlap`と有限ヒンジの`shared_feature_flat_stack`をnativeで実装する。
4. 角起点V、山山V、A/B、共有点外横断をnative production proofとdesktop current-pose certificate経路で回帰する。
5. `indeterminate`を貫通同等のblocking表示と停止へ結合し、全pair coverageとwork limitを維持する。

このゲート完了後に限り、層順序transport、atomicな折り重ねcommand、最後に折り重ねUIへ進む。
