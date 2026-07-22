# 経路探索・名前付き折り技法 実装監査（2026-07-22）

## 範囲と評価原則

commit `290ce4b` 以降のdyadic path、Tree/cycle proof、層順・正厚補助証明、名前付き技法compiler、preview/apply、保存・PDF/SVG出力を、`docs/requirements-definition.md` のSIM-002〜006、SIM-010、INS-001〜010およびproduction codeへ照合した。

fixtureだけの成功、read-only診断、必要条件だけの判定は完成として数えない。issuer-bound certificateをnativeがapply直前に再検証し、利用者がUIから確認できる経路だけを進捗根拠とする。本書は監査資料であり、CI全緑まで`docs/progress.md`正本を変更しない。

## production利用者経路

### Dyadic / Tree / cycle

1. `StackedFoldPanel`からlevel 3/5/9の有界graphを選択する。hinge数、state数、transition数、allocation前hard capをnativeとstrict clientの双方で制限する。
2. nativeはproject instance、project ID、revision、fold-model fingerprint、fixed face、canonical hinge order、source/target absolute poseへ要求を束縛する。
3. Treeはcycle closureを要求せず、連続経路、正厚、共有頂点層順輸送を独立issuerから取得する。cycleはschedule、closure、collision、層順の対応証明を要求する。
4. graph edgeごとのcertificateを集約してpath binding hashを作り、private preview tokenに保持する。WebViewにはproof objectや編集authorityを渡さない。
5. UIはcertified/no-path/unsupported/resource-limitと有界診断を表示する。利用者確認後のapplyだけがprivate proofを再検証し、現在姿勢・層順・timelineを一つのnative transactionとして更新する。
6. cancel、別project、revision変化、fingerprint変化、fixed-face/pose/certificate改変、token再利用では無変更で拒否する。

`1d167c0`、`86fc9c8`、`1668933`、`32343b3`、`248f9a3`、`6362a35`、`e9b3798`、`1fde246`、`e09efb8`、`62d0153`がこの経路の主要な接続・上限証拠である。4〜8 hinge Tree fixtureはproductionと同じissuer lifecycleへ移され、成功fixtureを都合よくcycle権限へ流用しない。

### 名前付き技法compiler / preview / apply

1. 二つ折り、段折り、蛇腹折り、中割り、かぶせ、沈め、つまみ、花弁等を、対応するproduction compilerまたは明示的unsupported結果へ振り分ける。
2. compilerは選択edge、M/V、fixed face、segment、開始・終了pose、project/revision/fingerprintへ束縛したstructured timelineを生成する。推測したraw motionや未証明premiseを成功として表示しない。
3. `preview_named_basic_fold_timeline`とPanelはread-only timeline、step再生、compiler model/versionを表示する。single-flight、cancel、stale/late response破棄を持つ。
4. applyはpreview token、timeline digest、compiler output SHA-256、全bindingをnativeで再計算し、確認済みの同一timelineだけを一括history commandで追加する。
5. compiler metadataは通常保存、expanded folder、recovery、Undo/Redoを通過し、再読込後もstrict clientがunknown field、kind/segment/digest改変を拒否する。
6. instruction PDF/SVG ZIPはcompiler kind、segment、proof scopeを表示する。private proof object、filesystem path、raw certificateは出力しない。

`bd8c4c1`、`edd8055`、`35cad35`、`94bc8d5`、`f661e6d`、`b59bb6f`、`50a8622`、`ac13662`、`eb1e380`、`42c4ed4`が主要なproduction接続・永続化・改変拒否証拠である。

## issuer-bound proofとfail-close境界

| 境界 | 強制内容 |
|---|---|
| Identity | instance/project/revision/fingerprint、canonical fixed face、hinge/edge ID、absolute source/target poseを完全一致させる |
| Graph | levelは3/5/9、hinge・state・transition・overlay・探索workをallocation前に制限する |
| Tree | positive-thickness certificateとshared-vertex layer transport proofを別issuerから要求し、cycle closure証明で代用しない |
| Cycle | Kawasaki/assignment、schedule closure、continuous collision、layer authorityが揃う限定cycleだけcertifiedとする |
| Preview | private registry tokenはsingle-use。cancel、stale、ABA、digest不一致、期限切れはapply authorityを持たない |
| Compiler | compiler kind/model/version/segment/output digestとtimeline全内容を再計算し、unknown/tampered metadataを拒否する |
| Unsupported | cut、hole、concave/self-intersecting/zero-length/duplicate boundary、未対応topology、厚み未証明、resource超過を成功へ格上げしない |
| Mutation | read/diagnose/preview/exportは`authorizes_project_mutation=false`。applyだけを一回の原子的transactionとする |

## テスト・CI証拠

- `apps/desktop/tests`はdyadic level/hard cap、Panel、browser preview/apply、cancel/stale、even-cycle候補、named-technique strict client、compiler metadata、instruction PDF/SVGを固定契約として検査する。
- `apps/desktop/src-tauri/src/stacked_fold_read.rs`はTree/cycle issuer、graph count、no-path/resource-limit、private token、atomic applyをnative unitで回帰する。
- `ori-kinematics`は3/5/9 level、detour、3〜8 hinge bounded graph、closure候補を、`ori-collision`は正厚・接触・共有頂点層順を独立に検証する。
- `instruction_export.rs`はcompiler-authored timelineを保存・再読込後にPDF/SVG ZIPへ実生成し、structured proof表示を確認する。
- frontend snapshot 1,650件、DOM 328件、TypeScript build、WSL format/checkが直近で成功している。Windows native executableがApplication Controlに遮断される場合はWSL実行とremote CIを併用する。

## 保守的進捗提案（正本未反映）

- 「折り可能性・経路探索」: **45% → 78%** を提案する。有界Treeと限定cycleについて、探索・issuer証明・UI preview・原子的applyまで連続した。一方、任意topology、dense/high-rank graph、一般cycle、一般正厚、証明困難な障害物回避は未完なので80%台へは上げない。
- 「折り手順・PDF」: **75% → 92%** を提案する。主要な名前付き技法compiler、認証preview/apply、再生、保存、PDF/SVG出力まで接続した。一方、全技法の一般物理motion、手指・持ち替えの自動生成品質、未対応技法が残るため100%とはしない。

他領域を正本値のまま据え置く機械試算は、79.32% + 18% × (78%-45%) + 10% × (92%-75%) = **86.96%（表示87.0%）** となる。これはCI全緑後に正本所有者が採否を決める提案値であり、本書作成時点の公式値は79.3%のままである。

## 未完の一般境界

- 任意の非tree・多cycle・dense graphに対する完全または実用的な一般経路探索。
- 一般cycleを現在姿勢・層順・展開図M/V・timelineへ安全にmutationする証明。
- 任意ヒンジ数・角度・分岐・接触を含む一般正厚continuous motionと衝突回避。
- self-contact、摩擦、弾性、塑性、紙の圧縮を含む実物理motion。
- 花弁等の未証明技法を、視覚的説明ではなく連続3D certificate付きcompilerへ昇格すること。
- 一般的な持つ位置、押さえる位置、持ち替え、手指軌跡を物理的に実行可能として自動証明すること。
- remote CIの全jobが同一headでterminal greenになった後の`docs/progress.md`更新。
