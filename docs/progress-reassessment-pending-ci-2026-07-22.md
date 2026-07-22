# 完成度再評価案（CI確認待ち、2026-07-22）

## 位置づけ

次の3監査を統合した、CI確認待ちの完成度再評価案である。

- `beginner-custom-target-evidence-2026-07-22.md`
- `path-technique-evidence-2026-07-22.md`
- `project-design-evidence-2026-07-22.md`

本書は提案値であり完成度の正本ではない。remote CIの全必須jobが同一headでterminal greenになるまで`docs/progress.md`を編集せず、公式表示は79.3%のままとする。

## 統合後の領域別試算

| 領域 | 全体比率 | 正本値 | 提案値 | 提案後の全体寄与 | 変更根拠 |
|---|---:|---:|---:|---:|---|
| 要件・基本設計・技術検証 | 5% | 70% | 85% | 4.25% | trust/proof/resource/persistence境界をversioned codeと回帰へ固定 |
| プロジェクト・保存・履歴 | 8% | 78% | 94% | 7.52% | strict `.ori2`/folder/recovery、認証済みUndo/Redo、autosave authority |
| 2D展開図エディター | 15% | 100% | 100% | 15.00% | 据え置き |
| 数式・幾何制約 | 9% | 100% | 85% | 7.65% | 11種solverは実装済みだが、直接矛盾certificateは限定family。一般充足可能性と同一視しない |
| 3D折り・紙厚・衝突 | 17% | 99% | 75% | 12.75% | Tree/限定cycle/限定正厚は実装済み。一般正厚・一般多面・任意self-contactは未証明 |
| 折り可能性・経路探索 | 18% | 45% | 78% | 14.04% | dyadic 3/5/9、Tree/cycle issuer proof、preview、atomic apply |
| 折り手順・PDF | 10% | 75% | 92% | 9.20% | named compiler、認証preview/apply、保存、PDF/SVG ZIP |
| 入出力・互換性 | 5% | 100% | 100% | 5.00% | 据え置き。安全summary exportを重複加算しない |
| 多言語・設定・配布・QA | 5% | 75% | 75% | 3.75% | 据え置き。外部release authorityは未完 |
| 初心者向け自動設計 | 8% | 55% | 35% | 2.80% | bounded custom target・一般木候補・画像/GLB consensusは存在するが、一般treeは骨格の線形写像と交互M/Vであり平坦可解性を合成しない |
| **合計** | **100%** | — | — | **81.96%** | **表示82.0%** |

## 加重計算

正本79.32%の領域入力にはClaude再監査で過大計上が確認されたため、単純差分加算ではなく全領域の寄与を再合計する。

```text
4.25 + 7.52 + 15.00 + 7.65 + 12.75
+ 14.04 + 9.20 + 5.00 + 3.75 + 2.80 = 81.96%
```

表示値は小数第1位へ丸めて **82.0%** とする。ただしCI gateまでは正本79.3%を維持する。

## 監査後に追加された提案値の裏付け

以下は81.96%の内訳を増額せず、既に提案へ含めた保存・path・3D・instructions・QA境界の証拠密度を上げる回帰である。

- 5/8ヒンジ実証明は`.ori2`、expanded-folder、recoveryの復元後に独立再計算され、保存certificateと一致する。両保存形式は正規再保存が決定的で、未認証改ざんとhistory binding不一致をfail-closedする（`d9b3da5`、`41017dd`、`df1ba4d`、`3e543c8`、`405b355`、`82dd5e7`、`e8cfc89`）。
- M/V割当またはface geometryを変更した同型treeは元certificateを再利用できず、pathと3D simulation inputへの結合を負例で確認した（`4252b21`、`f6eb215`）。一般正厚・一般self-contactの証明には数えない。
- 実証明付きinstruction poseは適用対象fold model fingerprintへ結合され、ApplyおよびUndo→Redo後にpose validationを通る（`63cd9e2`）。
- 5/8ヒンジassessmentは各8回のserialized DTOが一致し、現行と旧2世代history envelopeの全3世代でtyped certificate保持とcanonical resaveを確認した（`50c0f7a`、`684337a`、`ori-formats` 307/307）。QA領域の提案値は据え置きで、正式compatibility policy完成とは扱わない。

## 二重計上監査

- Treeの正厚・層順certificateは3D領域の一般正厚完成として加算せず、「経路探索」でissuer-bound pathを成立させた分だけ評価した。
- named compilerのPDF/SVG出力は「折り手順」で利用者経路を評価し、既に100%の「入出力・互換性」へ追加加算しない。
- custom general treeの生成・consensusは「初心者向け自動設計」で評価し、同じdyadic proofを「経路探索」へ再度成果量として加算しない。経路領域では汎用native proof/apply境界だけを評価した。
- consensus/profile provenanceの`.ori2`・recovery通過は「プロジェクト・保存・履歴」の新しいschema対応証拠だが、初心者領域では機能利用者経路としてのみ評価し、保存工数を重複加算しない。
- strict DTO、cancel、stale、tamper回帰は各機能の受入条件として扱い、「多言語・設定・配布・QA」へ別加算しない。
- 実際のGitHub Release公開、署名鍵、promotion authorityは今回の監査範囲外であり、QA/配布75%を据え置いた。

## `docs/progress.md`反映gate

次の全条件を満たした場合だけ、正本へ提案値を反映する。

1. 反映対象commitを含む同一remote headを特定する。
2. Windows、macOS、frontend、Rust、format、Clippy、bundle等、repositoryが必須とする全CI jobがterminal状態になる。
3. required jobがすべてgreenで、cancelled、skipped相当の未検証必須job、古いheadの成功を混在させない。
4. worktreeの監査対象codeとremote headが一致し、未pushの機能差分を完成根拠に含めない。
5. `docs/progress.md`の領域値、各寄与、合計、説明、未完一覧を同一commitで更新し、加重式を再検算する。
6. 更新後の正本をCIで再検証し、そのdocumentation-onlyまたは後続headもterminal greenにする。

一つでも満たさない場合は本書をpending案のまま保持し、`docs/progress.md`の79.32%（表示79.3%）を維持する。

## 81.96%時点でも残る未完

- 任意の非tree・dense・multi-cycle topologyに対する一般経路探索と安全なcycle mutation。
- 任意角度・分岐・self-contactを含む一般正厚continuous motion、衝突回避、層順証明。
- 摩擦、弾性、塑性、圧縮、手指把持を含む一般物理motion。
- 花弁等の未証明技法を連続3D certificate付きcompilerへ昇格すること。
- 任意の一般画像・一般GLBから意味部位・surfaceを認識し、一般的な一枚紙展開図と折り手順を生成すること。
- expanded folderのWindowsオーナー実機E2E、権限・容量枯渇・同期softwareを含む障害matrix。
- 複数世代schema migrationの正式compatibility policy。
- 実際の署名済みGitHub Release公開、外部配布authority、stable promotionの運用実績。
- 全CIが同一headでterminal greenになった後の正本反映と再検証。

このため81.96%は「残件が小さい」ことを意味せず、一般物理motion、一般自動設計、正式配布という高難度の終盤作業を明示的に残した保守的な工数概算である。
