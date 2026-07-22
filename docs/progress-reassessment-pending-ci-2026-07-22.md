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
| 数式・幾何制約 | 9% | 100% | 100% | 9.00% | 据え置き |
| 3D折り・紙厚・衝突 | 17% | 99% | 99% | 16.83% | 据え置き。今回のTree/path成果を重複加算しない |
| 折り可能性・経路探索 | 18% | 45% | 78% | 14.04% | dyadic 3/5/9、Tree/cycle issuer proof、preview、atomic apply |
| 折り手順・PDF | 10% | 75% | 92% | 9.20% | named compiler、認証preview/apply、保存、PDF/SVG ZIP |
| 入出力・互換性 | 5% | 100% | 100% | 5.00% | 据え置き。安全summary exportを重複加算しない |
| 多言語・設定・配布・QA | 5% | 75% | 75% | 3.75% | 据え置き。外部release authorityは未完 |
| 初心者向け自動設計 | 8% | 55% | 72% | 5.76% | custom target、一般木候補、画像/GLB consensus、stale-safe apply |
| **合計** | **100%** | — | — | **90.35%** | **表示90.4%** |

## 加重計算

正本79.32%からの差分は次のとおり。

```text
要件・基本設計       5% × (85% - 70%) = 0.75%
プロジェクト・保存   8% × (94% - 78%) = 1.28%
経路探索            18% × (78% - 45%) = 5.94%
折り手順            10% × (92% - 75%) = 1.70%
初心者自動設計       8% × (72% - 55%) = 1.36%
差分合計                                  11.03%
79.32% + 11.03% =                         90.35%
```

領域別寄与の直接合計でも、4.25 + 7.52 + 15.00 + 9.00 + 16.83 + 14.04 + 9.20 + 5.00 + 3.75 + 5.76 = **90.35%** となり一致する。

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

## 90.35%時点でも残る未完

- 任意の非tree・dense・multi-cycle topologyに対する一般経路探索と安全なcycle mutation。
- 任意角度・分岐・self-contactを含む一般正厚continuous motion、衝突回避、層順証明。
- 摩擦、弾性、塑性、圧縮、手指把持を含む一般物理motion。
- 花弁等の未証明技法を連続3D certificate付きcompilerへ昇格すること。
- 任意の一般画像・一般GLBから意味部位・surfaceを認識し、一般的な一枚紙展開図と折り手順を生成すること。
- expanded folderのWindowsオーナー実機E2E、権限・容量枯渇・同期softwareを含む障害matrix。
- 複数世代schema migrationの正式compatibility policy。
- 実際の署名済みGitHub Release公開、外部配布authority、stable promotionの運用実績。
- 全CIが同一headでterminal greenになった後の正本反映と再検証。

このため90.35%は「残件が小さい」ことを意味せず、一般物理motion、一般自動設計、正式配布という高難度の終盤作業を明示的に残した保守的な工数概算である。
