# 完成度再評価（正本発効条件付き、2026-07-22起案）

## 2026-07-30 次期候補（82.29%・同一head CI発効待ち）

発効済み正本81.96%を基準に、一般判定と初心者向け一般自動設計の利用者能力を再監査した。次期候補は **82.29%（表示82.3%）** であり、本節の計上対象を含むremote `main` head自身が後述の発効条件を満たすまでは、正本81.96%（表示82.0%）を維持する。

| 領域 | 全体比率 | 81.96%時点 | 次期候補 | 次期寄与 | 増分根拠 |
|---|---:|---:|---:|---:|---|
| 要件・基本設計・技術検証 | 5% | 85% | 85% | 4.25% | 据え置き |
| プロジェクト・保存・履歴 | 8% | 94% | 94% | 7.52% | 据え置き |
| 2D展開図エディター | 15% | 100% | 100% | 15.00% | 据え置き |
| 数式・幾何制約 | 9% | 85% | 86% | 7.74% | 互換singleton 1..=8件からdetachedな厳密配置を構成し、全production residualを再認証。current assignment証拠とnative DTO・strict TypeScript・日英UIで区別 |
| 3D折り・紙厚・衝突 | 17% | 75% | 75% | 12.75% | 4-block正厚作業は未完のため不計上 |
| 折り可能性・経路探索 | 18% | 78% | 78% | 14.04% | 据え置き |
| 折り手順・PDF | 10% | 92% | 92% | 9.20% | 据え置き |
| 入出力・互換性 | 5% | 100% | 100% | 5.00% | 据え置き |
| 多言語・設定・配布・QA | 5% | 75% | 75% | 3.75% | browser回帰を別加算しない |
| 初心者向け自動設計 | 8% | 35% | 38% | 3.04% | general semantic count 2..=14の生成・Apply・Undo/Redo・再読込、count 15 fail-closed、認識parser/apply 16件境界、production frontendのcount 14 browser lifecycleを一能力bundleとして計上 |
| **合計** | **100%** | — | — | **82.29%** | **表示82.3%** |

```text
4.25 + 7.52 + 15.00 + 7.74 + 12.75
+ 14.04 + 9.20 + 5.00 + 3.75 + 3.04 = 82.29%
```

二重計上を避けるため、manual count 2..=14、count 14 browser、認識cap同期、count 15負境界は初心者向け自動設計の+3点へ一括し、QAへ加算しない。constructive SATと証拠種別UI分離は数式・幾何制約の+1点へ一括する。bit-exact 135度の限定semantic family拡張は既計上familyの証拠密度向上、4-block正厚positive lifecycleは未完として、いずれも追加加算しない。

次期反映headとexactに同じ`head_sha`の単一CI run attemptで、必須7 job（`dependency-advisory-audit`、`frontend`、`slicer-acceptance`、`rust (macos-latest)`、`rust (windows-latest)`、`windows-bundle`、`macos-bundle`）がすべてterminalの`conclusion=success`となり、同じrunの必須4 artifact（`rustsec-warning-review`、`sample-viewer-runtime-log`、Windows NSIS、macOS app）が監査時に生成済みかつ`expired=false`であることを要求する。`frontend` jobはgeneral count 14、complete animal、complete insectの3 browser lifecycleを含む。別head・別attempt・skip・cancel・failure・artifact欠落・計上対象の未push差分が一つでもあれば82.29%を発効しない。

## 位置づけ

次の3監査を統合し、2026-07-26に反映前base CIまで完了した完成度再評価である。

- `beginner-custom-target-evidence-2026-07-22.md`
- `path-technique-evidence-2026-07-22.md`
- `project-design-evidence-2026-07-22.md`

本書の81.96%は`docs/progress.md`の発効規則に従う条件付き正本候補である。81.96%重み表と発効規則を含む反映head自身のCI条件が成立するまでは79.32%（表示79.3%）が正本であり、成立時に追加commitなしで81.96%（表示82.0%）が発効する。

2026-07-26の正本発効監査で、反映head `f9913149b69ad1bc83d89681aa9309b986063cc5`に対するCI #686が本書の全条件を満たしたため、81.96%（表示82.0%）は発効済みである。

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

表示値は小数第1位へ丸めて **82.0%** とする。ただし反映head自身のCI発効条件が成立するまでは正本79.3%を維持する。

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

## 反映前base CI証拠

反映前base commit`45dfae5d2e66ee19f04d405bb7e9642c1237a950`の[CI #685](https://github.com/oltotlo79-rgb/ORIGAMI2/actions/runs/30183421620)は、run ID `30183421620`、attempt 1、2026-07-26T01:45:37Z開始・02:31:20Z完了で、次の7 jobがすべて`completed / success`となった。

- `dependency-advisory-audit`（job `89743858311`）
- `frontend`（job `89743858306`）
- `slicer-acceptance`（job `89743858296`）
- `rust (macos-latest)`（job `89743858307`）
- `rust (windows-latest)`（job `89743858338`）
- `macos-bundle`（job `89746348028`）
- `windows-bundle`（job `89746348022`）

同じrunでは、発効監査時に次の4 artifactが生成済みで`expired=false`だった。retention期限は2026-08-02であり、この監査記録は期限後の自動削除によって失効しない。

- `ORIGAMI2-macos-app-30183421620`（artifact `8626588537`、23,155,167 bytes、GitHub artifact archive digest SHA-256 `9007fa0cdd97002464e8cf27271295710795b67a20bcfdf8dcc6f1704cd8a5a8`）
- `ORIGAMI2-windows-nsis-30183421620`（artifact `8626643864`、17,007,506 bytes、GitHub artifact archive digest SHA-256 `ba5e9f47229fb7a73aec9857f82fb73d1814ee1b5123b1b3aea5a806956ba54b`）
- `rustsec-warning-review`（artifact `8626320166`、1,265 bytes、GitHub artifact archive digest SHA-256 `a1fd782353a216f524c0de6de09f229d913cdc460d43244bf7082e9421bdf79d`）
- `sample-viewer-runtime-log`（artifact `8626360815`、300 bytes、GitHub artifact archive digest SHA-256 `c9be8d6c63391d25e34381ccfd16ba6d7366f770a56add8e1f1e989f1c9e823a`）

このrunは反映前codeのbase証拠であり、81.96%自体を発効しない。

## 反映headの正本発効条件

81.96%重み表と発効規則を含むremote `main` headを反映headとする。次の全条件を満たした場合だけ、81.96%を正本として発効する。

1. 反映headとexactに同じ`head_sha`の単一CI run attemptを特定する。
2. 上記と同名の必須7 jobがすべてterminalの`conclusion=success`となる。formatとClippyは両Rust job内の必須stepとしてjob successに包含する。
3. `rustsec-warning-review`、`sample-viewer-runtime-log`、`ORIGAMI2-windows-nsis-${run_id}`、`ORIGAMI2-macos-app-${run_id}`の4 artifactが同じrunで生成済みかつ発効監査時に`expired=false`である。
4. worktreeの監査対象codeとremote headが一致し、未pushの機能差分を完成根拠に含めない。
5. `docs/progress.md`の領域値、各寄与、合計、説明、未完一覧を同一commitで更新し、加重式を再検算する。
6. cancelled、skipped相当の未検証必須job、別attempt、別head、古い成功を混在させない。

queued / in-progress、job未生成、success以外、artifact欠落のいずれかがある間は79.32%（表示79.3%）を維持する。失敗したheadは発効せず、後続headはそのexact `head_sha`で全条件を満たす必要がある。一度記録した発効はartifact retention期限後も失効せず、green後の文書追記commitを要求しない。

## 反映headの正本発効証拠

remote `main`の反映head `f9913149b69ad1bc83d89681aa9309b986063cc5`に対する[CI #686](https://github.com/oltotlo79-rgb/ORIGAMI2/actions/runs/30185019151)は、run ID `30185019151`、attempt 1、2026-07-26T02:43:43Z開始・03:27:47Z完了で、次の7 jobがすべて`completed / success`となった。

- `dependency-advisory-audit`（job `89748013459`）
- `frontend`（job `89748013457`）
- `slicer-acceptance`（job `89748013419`）
- `rust (macos-latest)`（job `89748013444`）
- `rust (windows-latest)`（job `89748013442`）
- `macos-bundle`（job `89750495557`）
- `windows-bundle`（job `89750495528`）

同じrunの次の4 artifactは発効監査時にすべて`expired=false`だった。

- `ORIGAMI2-macos-app-30185019151`（artifact `8627063844`、23,155,438 bytes、GitHub artifact archive digest SHA-256 `dc1dbd7b061b9ff654f3491e7f2b36c4551b5a05fd67ce925cb3543502709ca3`）
- `ORIGAMI2-windows-nsis-30185019151`（artifact `8627108329`、16,998,864 bytes、GitHub artifact archive digest SHA-256 `195ddd7f7a1b795315bff01a193de9c3c0f6e3c4bd9f309e349408c26ca2d609`）
- `rustsec-warning-review`（artifact `8626759991`、1,265 bytes、GitHub artifact archive digest SHA-256 `fdd38544fae8617fa5c95b1efc71659d6841d92a1c69f2314956c9c06965abc5`）
- `sample-viewer-runtime-log`（artifact `8626796135`、300 bytes、GitHub artifact archive digest SHA-256 `d1057fb16c46a12d4389bc1bb39fd089cf32a95ead5d74408d184bd89a354557`）

run head、remote head、attempt、7 job、4 artifactをexactに照合し、別head・別attempt・失敗・取消の混在はなかった。したがって本書の発効条件は成立し、81.96%（表示82.0%）を正本として採用した。

## 81.96%時点でも残る未完

- EDT-009の全制約種に対するsoundな一般充足可能性判定、一般矛盾原因、一般最小不能部分集合。
- 任意の非tree・dense・multi-cycle topologyに対する一般経路探索と安全なcycle mutation。
- 任意角度・分岐・self-contactを含む一般正厚continuous motion、衝突回避、層順証明。
- 摩擦、弾性、塑性、圧縮、手指把持を含む一般物理motion。
- 花弁等の未証明技法を連続3D certificate付きcompilerへ昇格すること。
- 任意の一般画像・一般GLBから意味部位・surfaceを認識し、一般的な一枚紙展開図と折り手順を生成すること。
- expanded folderのWindowsオーナー実機E2E、権限・容量枯渇・同期softwareを含む障害matrix。
- 複数世代schema migrationの正式compatibility policy。
- 実際の署名済みGitHub Release公開、外部配布authority、stable promotionの運用実績。

このため81.96%は「残件が小さい」ことを意味せず、一般物理motion、一般自動設計、正式配布という高難度の終盤作業を明示的に残した保守的な工数概算である。
