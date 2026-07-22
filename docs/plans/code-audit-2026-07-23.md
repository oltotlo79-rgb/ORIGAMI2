# ORIGAMI2 コード監査レポート 第2回（矛盾・不具合・改善点・実装可能機能）

- 監査基準コミット: `684337a`（2026-07-22 23:55）
- 実施: Claude（読み取り専用アドバイザー）。**本レポート以外のファイルは一切編集していない。**
- 方法: 5領域を並列で実ソース精読（最新コード／衝突・native IO／2D編集／3D・手順／docs対コード）。全所見は file:line を開いて検証済み。前回監査（`docs/plans/code-audit-2026-07-22.md`）で修正済みの項目は再報告しない。
- 前提: リポジトリは分単位で更新中。行番号は基準コミット時点。修正前に現物を再確認すること。

---

## 0. 前回監査の是正状況（検証済み）

**確実に修正された（テスト付き）**：A-1（メッシュ書出パニック）、A-3（共面/貫通取り違え）、A-4（`-0.0`厚）、A-5（フォルダー層証拠破棄）、A-6（親フォルダー4098件）、A-7（非有限座標）、A-10（新規フォルダー同期失敗）、B-5（症状3=180°で層順序診断を配線、`stacked_fold_read.rs:4982-5038`）。
**健全と再確認**：A-3の潜在false-safe懸念（層順序フリップ）は安全 — フリップは利用者向け表示のみを変え、安全権威は層順序を見ない独立の厳密証明器からのみ発行（`applied_pose/static_collision.rs:833`）。
**未是正（正本progress.mdに残存）**：B-1（自動設計）、B-2（3D 99%）、B-3（制約100%）、B-4（一部）。§Bに詳細。

---

## A. 新規の不具合（BUG）

### A-1【HIGH・健全性】ブロック分解の折り重ねが、ブロックをまたぐ重なりセルを無検証で破棄し、ブロック局所の証明を全体権威として適用する
`apps/desktop/src-tauri/src/stacked_fold_read.rs:2989-2999, 3005-3010`（`restrict_snapshot`）／`crates/ori-collision/src/block_composition.rs:62-131`
- 閉路が単一で閉じないとき2ブロックに分解し**ブロックごと**に正厚層輸送を証明する。各ブロックのsnapshotを作る際、`overlap_cells` を「被覆面と`bottom_to_top_faces`が全てそのブロック内」の場合のみ保持し、**ブロックをまたぐ重なりセルと `face_pair_orders` を破棄している。破棄集合が空かのチェックが無い。**
- 閉路の**閉包**はカット面で健全に分解できるが、**層順序・衝突の安全性は分解できない** — 別ブロックの面同士も折り畳み後に重なり順序が必要。あるブロックの面が相手ブロックの面としか重ならない場合、**全セルが捨てられ空虚な層証明で通過**する。ブロック局所の成功が全モデルのapply/persist権威として使われる（`install_pending_blockwise_current_cycle_pose_v1`→`apply_stacked_fold_transaction`、本番配線済み）。
- 修正: 復元後、全体のoverlap cell・face_pair_orderが各ちょうど1ブロックへ割当済みか検証し、cross-blockセルが1つでもあれば `CYCLE_NONCLOSING` でfail-closed。

### A-2【HIGH】保存される `CycleLayerOrderProofV1` が、権威が証明していない全体ペアを記載（過大表示）
`apps/desktop/src-tauri/src/stacked_fold_read.rs:3133-3138`；`stacked_fold_transaction.rs:274-301, 1845`
- 保存proofの `pairs` を**制約前の全snapshot**の `face_pair_orders` から取るが、`target_order_sha256`/`transition_count` はブロック権威（ブロック内ペアのみ証明）由来。cross-block順序を「証明済み」と広告する。これは手順タイムライン・書き出し（`ori-formats`/`ori-domain`）へ折り層順の証拠として記録される。
- 修正: `layer_order_pairs` を両ブロック権威の証明済みペアの和集合から生成、またはcross-blockペアがあれば `CycleLayerOrderProofV1` を発行しない。

### A-3【MED】頂点ドラッグで折り目/交点上に置いても分割・接続されず、幾何一致だが位相未接続の状態を作る
`apps/desktop/src/components/CreaseCanvas.tsx:883-941`（`accept`は他頂点との一致と紙外のみ拒否、辺内部への吸着を拒否しない）→ `move_vertex`（`lib.rs:8823-8855`）→ `Command::MoveVertex`（`editor.rs:2760-2771`、座標更新のみ・交点調整なし）
- クリック配置は同じ辺スナップで `split-edge` を生成する（`vertexPlacement.ts:80-85`）のに、ドラッグは分割しない。頂点を折り目や交点に正確に重ねられるが位相的に切れたまま。
- 修正: ドラッグ中は辺/中点/交点スナップを除外、または辺/交点上へのドロップを「移動＋分割/併合」調整へ回す。

### A-4【MED】角二等分制約が反対向き（reflex側）の辺でも成立する
`crates/ori-core/src/constraint_solver.rs:624-640`
- 残差は `û₁+û₂`（内部二等分方向）と二等分辺の外向きベクトルの正規化外積。外積0は二等分方向**と反平行 `-sum`** の両方で成立するため、角の内部でなくreflex側を向く辺が誤って受理される。向きガードなし（前回修正済みの `fixed_angle` 符号バグとは別）。
- 修正: 同方向ドット積が正、`sum_x*bisector.x + sum_y*bisector.y > 0` を追加要求。

### A-5【MED】`.ori2` 書き出しが reader の強制する下絵(underlay)検証を省き、開けないファイルを作りうる
`crates/ori-formats/src/lib.rs:559-574`（writer）vs `:598`（reader）；`ori2.rs:419`
- reader は `validate_project_underlays`（`lib.rs:612-627`、欠落layer参照・`content_kind != Underlay` を拒否）を実行するが、writer は残り8検証は走らせるのにこれを省く。`.ori2` writerは再読込しないため、不整合な下絵を持つ文書が**保存成功・再オープン不能**（A-7と同型）。フォルダーwriterは再読込で守られている（writer/writerの不整合）。
- 修正: writer内で `validate_project_underlays` を呼ぶ、または `.ori2` writerもフォルダー同様に再読込する。

### A-6【MED】正厚の180°折り提案が「ゼロ厚専用」の層順序診断に渡され、汎用エラーで失敗（症状3修正の副作用）
`apps/desktop/src-tauri/src/stacked_fold_read.rs:4982-5038` + `crates/ori-collision/src/static_collision.rs:1279-1284`
- 180°分岐は `requested_angle==180.0` だけで入り**厚みガードが無い**。中で `diagnose_static_collision_geometry_with_flat_layer_order_v1(..., paper.thickness_mm, ...)` を呼ぶが、この関数は非ゼロ厚を無条件拒否（`InconsistentMaterialPose`→汎用 `ANALYSIS_FAILED_MESSAGE`）。ユーザーが紙厚を正に設定した180°折り（正厚連続証明を得た単一/二ヒンジ）が不透明な「解析失敗」になる。非180°経路には厚み対応版（`:5075`）があるのに180°には無い。
- 修正: 分岐をゼロ厚で条件化し、正厚180°は厚み対応の層順序診断へ回す（または「正厚平坦折りは未対応」の明示保留）。

### A-7【MED】名前付き技法の適用で、`reverse`/`layer_selective` の専用トランザクションが死蔵し、汎用経路へ誤ルート
`apps/desktop/src/components/StackedFoldPanel.tsx:566, 578` は `kind==='layer'`/`'reverse'` で分岐するが、生産者 `App.tsx:12207-12213` は `'layer_selective'`/`'inside_reverse'`/`'outside_reverse'` しか出さない。専用の `applyNamedLayerSelectiveTransaction`/`applyNamedReverseFoldTransaction` は死蔵、実際は基本折りタイムラインを要求する汎用経路へ落ちる。
- 修正: `'layer_selective'` と `'inside_reverse'||'outside_reverse'` で分岐する。

### A-8【MED】技法選択を切替えても `confirmed`/`view` がリセットされず、技法Aで確認した証明を技法Bへ適用できる
`StackedFoldPanel.tsx:234-268`（`setConfirmed(false)`/`setView(idle)` するが依存配列に `namedBookFold` が無い）。同一snapshot/線で技法を切替えると `view==='ready'`・`confirmed===true` が残り、Applyボタン（`:1392`）が古いレビューに対し有効なまま。
- 修正: リセットeffectの依存に `namedBookFold?.techniqueId`/`kind` を追加。

### A-9【MED】「現在姿勢から証明」が偶数cycle候補のみのとき、押せるが何も起きない
`StackedFoldPanel.tsx:911-923`（`authoredCycleSchedule || evenCycleCandidates.length>0` で有効化）だが、ハンドラ `previewCurrentCyclePose` は authored schedule が無いと即return（`:687`）。候補ありでテキストエリア空だと、ボタンは有効なのに無反応（対話行き止まり）。
- 修正: `!authoredCycleSchedule` をボタンの `disabled` に加える、または「先にscheduleを入力」ヒントを出す。

### A-10【MED】分割/併合(Split/Merge)ボタンが説明のみ(declarativeOnly)ステップでも有効
`InstructionTimelinePanel.tsx:1105-1112`（Splitは `durationMs<200`、Mergeは末尾のみで無効化するが `declarativeOnly` を見ない）。同じ行の「3D表示」「姿勢更新」は declarativeOnly を見るのに不整合。画面の但し書き（`:1127-1131`）とも矛盾。
- 修正: 両方に `|| selectedStep.declarativeOnly` を追加。

### A-11【LOW-MED】A-1型のミューテックス汚染パニックが別経路に残存
`apps/desktop/src-tauri/src/lib.rs:11923`（`connect_t_junction` がロック保持中に `.expect("...must create its requested edge")`）、`stacked_fold_read.rs:4862`（ロック保持中の `unreachable!("material map emits only mountain or valley")`）。内部不変条件の防御的assertだが、上流バグをクリーンな `Err` でなく**セッションブリック**に変える（A-1と同型、A-1修正が局所的だった）。
- 修正: ロック保持中の `.expect()`/`unreachable!()` を fallible error へ。

---

## B. 矛盾点（CONTRADICTION）

### B-1【HIGH】自動設計「55%」は未是正で、しかも再計上案が60%へ引き上げ（過大と認めながら加点）
`docs/progress.md:24`（55%「端末内限定生成を接続」）。コードは前回同様 `crates/ori-domain/src/beginner_generator.rs:150-179`（28変種enum）、一般tree経路は骨格を5-15%帯へ線形写像（`:2159-2166`）＋山谷を `index%2`（`:2211-2215`）。平坦折り可解性チェックも合成もなし。Codexは応答docでB-1を「正しい」と認めた（`code-audit-response:26`）のに、再計上案（`progress-reassessment-pending-ci:26`）は**60%へ引き上げ**。自己矛盾。

### B-2 / B-3【HIGH】正本progress.mdに「3D 99%」「制約100%」が残存（応答docでは75%/85%に下方修正済み）
- 3D: `progress.md:19` は99%だが、公開分類器 `prove_static_collision_geometry`（`static_collision.rs:724`）は一般正厚・多面を `PairEvidenceUnavailable`（`:886,908,999`）＝遮断 `Indeterminate`（`:197`）へ閉じる。
- 制約: `progress.md:18` は100%だが、`constraints.rs:310-338` は矛盾検出7種のみ、11制約種のうち8種は `unchecked`→`Unknown`（`:1479-1534, 1686`）。`requirements-status.md:216` は「7種の直接矛盾」と正直化されたが `progress.md` は100%のまま。

### B-4【MED】requirements-status の矛盾集計が一部残存
57/25/5はコメント化されたが、`requirements-status.md:146`「実装済み32/27/28…のまま」、`:178`「33/27/27へ更新」、旧「36.9%/37.2%」（`:66,95,145`）が表示のまま。複数の「現在」集計が併存。

### B-5【MED】名前付き技法の同一性が、ハードコードされたローカライズ表示名で判定される（コメントと矛盾）
`App.tsx:12190-12193`（`hasCanonicalName(['山折り','Mountain fold'])` で分類、外れると `'book'`→`StackedFoldPanel.tsx:149-151` が `unsupported` として遮断）。Rust側も同様に `technique_motion.rs:107-117` が `"山折り"/"Mountain fold"` 完全一致を要求 — **自身のコメント `:74-76`「kindは明示で、翻訳タイトルから推論しない」と直接矛盾**。技法をリネーム/再ローカライズすると使えなくなる。accordion経路は実際のcrease assignment（`EdgeKind`）で判定（`stacked_fold_transaction.rs:1067-1076`）しており、そちらが正解。
- 修正: 表示テキストでなく `action.kind`／crease assignment で判定。

### B-6【MED】ブロック合成の関節チェックが非連結ブロックを許容（＋本番未使用）
`block_composition.rs:442-463`。`has_articulation |= shared==1` で `shared==0`（他と1面も共有しない孤立ブロック）を黙認。「連結した関節構造」という文書上の不変条件が未強制。加えて `BlockComposedPathAuthorityV1` は `lib.rs` からexportされるが `continuous_path.rs` テストのみで使用（本番死蔵）。
- 修正: ブロック交差グラフの連結性（ブロックの木）を要求、または未使用型を削除。

### B-7【LOW-MED】分割/併合の成功時に「手順の順番を変更しました」と誤表示
`InstructionTimelinePanel.tsx:704, 713` が Split(1→2)/Merge(2→1) 成功時に `{kind:'moved'}`（`instructionTimeline.ts:620-627`「順番を変更しました」）を出す。並べ替えではない。union に split/merge 種別が無い（`:148-165`）。
- 修正: split/merge 用のnotice種別と文言を追加。

### B-8【LOW】コンパス円が「作図に使える」と説明されるのに何もスナップしない
`App.tsx:7958-7962`（「交点を見ながら定規相当の線作図」）だが `SnapKind`（`snap.ts:9-18`）に円が無く、`resolveAdditionSnap` は `compassCircles` を受け取らない（`CreaseCanvas.tsx:824-881`）。目視するしかなく、説明の目的を果たさない。（§D-F2で機能化提案）

### B-9 正本progress.mdは、受理した再計上（84.0%）とも矛盾したまま
`progress.md:25` は79.32%で領域入力99%/100%/55%。応答doc・再計上案はこれらを過大と宣言し84.0%（75%/85%/60%）へ。CIゲート（`progress-reassessment-pending-ci:58-69`）で意図的に79.3%維持中だが、公式ファイルが他3docの誤りとする数字を今も掲げる状態。加えて `requirements-status.md:5`「実装済み86/87」（≈99%）と加重79.3%も、スコープ再定義で未整合のまま併存。

---

## C. 改善点（IMPROVEMENT）

- **C-1** スナップが距離でなくカテゴリ順で解決（`snap.ts:331-432`）→ 9.9px先の頂点が0.5px先のグリッドに勝つ。各カテゴリ最良候補をピクセル距離＋控えめなバイアスで選ぶべき。
- **C-2** 自動分割が境界辺を除外し、描いた辺上の孤立頂点を無視（`editor.rs:2381-2543`、`kind!=Boundary` フィルタ2409/2462）。新reg折り目が既存孤立頂点や境界内点を通っても接合登録されない。
- **C-3** 参照グリッドが10進nice値のみで、N等分（1/8・1/3等）にできない（`snap.ts:280-314, 488-497`）。折り紙の実用グリッドは分数。（§D-F3）
- **C-4** ステップ変更ゲート `editingDisabled` が `fileOperationActive` を含まない（`InstructionTimelinePanel.tsx:260`）→ ファイル保存/読込中もAdd/Update/Move/Split/Merge/Deleteが有効。`|| fileOperationActive` を追加。
- **C-5** コンパイル済み名前付き技法の手順が日本語ハードコードで、英語ユーザーにも日本語のみで届く（`technique_motion.rs:431,617-629,750,1066` 等）。locale を通すかキー化。
- **C-6** ブロック版 `continuous_certified()` が `CurrentCycle` 版の target-angle↔schedule 再照合を省く（`stacked_fold_transaction.rs:249-263` vs `:197-248`）。防御的多層化として揃えるべき。
- **C-7** ステップ編集のvisual JSONが構造検証（`parseInstructionVisual`, `instructionTimeline.ts:767`）を通らず `JSON.parse` のみ（`InstructionTimelinePanel.tsx:614-620`）→ 形不正が汎用 `update_failed` に。GLBアニメ書出が図書出の `certificateExportBlocked` ゲートを省く（`:870-877` vs `:844-852`）。

---

## D. 実装可能なユーザー利便性機能（工数 S/M/L・実コード上の根拠つき）

**まず即効の小工数（S）：**
- **F-D（Sステップ）：手順ステップの複製（Duplicate step）** — 近似的な連続折りに便利。`addInstructionStep` は姿勢+メタを丸ごと取る（`coreClient.ts:4357-4367`）。追加姿勢アクション（`InstructionTimelinePanel.tsx:552-577`）の隣に `duplicateSelectedStep()` を置くだけ。declarativeステップにも有効。**S**
- **F-E：先頭/末尾へ移動＋ドラッグ並べ替え** — 現状±1のみ。`moveInstructionStep` は任意index可（`coreClient.ts:4493`）。ジャンプボタン（0/末尾）とDnD。**S/M**
- **F-A：2要素間の距離/角度表示（2D）** — 現状は1線の長さのみ（`App.tsx:6748`）。頂点間距離・辺間角度を追加。計測文法を再利用。**S**

**中工数（M）：**
- **F-1：角度→対象作図「Ray to target」（C-9を解消）** — 選択頂点から角度指定で射線を伸ばし、対象の辺/折り目/頂点に当たった位置を終点に自動生成＋分割。射線交差は `plan_add_edge_with_intersections`（`editor.rs:2381`）の `segment_intersection` を再利用。**折り紙で最頻の参照作図**（22.5°分子・参照線）。今は長さを手計算するしかない。**M**
- **F-2：コンパス円の交点スナップ（B-8を機能化）** — 既に描かれる円（`App.tsx:7930`）を実作図幾何に。`SnapKind` に `'circle-intersection'` を追加し `compassCircles` を `resolveAdditionSnap` へ渡す。**Huzita/Haga作図（辺の三等分等）は円×線の交点そのもの**。**M**
- **F-3：分数/N分割グリッド＋対角** — 用紙の分割数（8×8・三等分）と対角を指定し `SnapGrid` へ厳密座標を供給（`snap.ts:280-314`）。**箱折り・ミウラは 2ⁿ/3ⁿ グリッド前提**で、現行10進グリッドでは表現不能。**S/M**
- **F-6：ステップごとのカメラ・ブックマーク取得** — `InstructionVisual.camera` は検証済み（`instructionTimeline.ts:895-903`）で適用も済み（`FoldPreview.tsx:1037-1044`）。取得だけ欠落。`onCameraChange` コールバック＋「現在カメラを保存」ボタン。**M**
- **F-10：技法パレット（プレビュー→確認→適用を1操作に）** — 現状は名前付き技法適用に4クリック（安全検証→タイムラインプレビュー→レビュー確認→適用）。`document.techniques[]` 上のタイル一覧を作り、トークン一致時に一連をチェーン。**M**
- **F-11：3Dプレビュー内の計測（頂点間距離/二面角）** — `FoldPreview.tsx` は頂点/面を選択済み（`:2565-2592`）だが単一選択・計測なし。第2選択と読み取りを追加。**M**
- **EDT-009 一般矛盾原因の特定** — 既存の有界Gauss-Newtonソルバーが rank/over-constrained分類を計算済み（`constraint_solver.rs:126-139`）。そこから**最小不能部分集合**を抽出。現実サイズでNP困難でない。MUST要件（現状7パターンのみ）の実質達成。**M**
- **交差の一括自動分割** — 検出済みの `UnsplitIntersection`（`lib.rs:14409`）を反復し、既存の `ConnectEdgeIntersection`/`ConnectIntersectionCluster`（`:11756,11804`）を適用。取込などで既に交差した展開図を一括修復。**S/M**

**やや大きい（M/L）：**
- **F-4：選択の配列/放射複製（Array / radial repeat）** — `MirrorSelection`（`lib.rs:9040-9109`、ID列を事前確保）と `rotate_edge_about_point`（`:9110`）を一般化。星型・フラッシャー・テッセレーションに必須。**M/L**
- **F-9：隣接ステップのオニオンスキン（ghost）** — 姿勢補間ヘルパ（`instructionTimeline.ts:349`）とアニメ再生（`:462,506`）が既存。`ghostPose` propで低不透明度の第2面群を描画。何が変わったか一目で分かる。**M/L**

**参考（実装困難＝短期非推奨）**：AUT-101一般目標からの展開図生成、AUT-005一般形状/骨格認識、SIM-010一般正厚多層折り、一般経路探索。いずれもNP困難/研究レベル（前回・本監査で確認）。

---

## E. 優先度（対応順）

**即修正すべき実バグ（HIGH）**
1. A-1 ブロック分解のcross-blockセル破棄（健全性・本番配線）— fail-closed追加。
2. A-2 ブロック層順序証明の過大表示（証拠の信頼性）。

**次に（MED、利用者に見える／データ健全性）**
3. A-5 `.ori2` 下絵検証欠落（開けないファイル）、A-6 正厚180°の汎用失敗、A-3 頂点ドラッグの無音不整合、A-4 角二等分reflex、A-11 残存ミューテックスパニック。
4. 技法/手順UIバグ群：A-7 死蔵分岐ルート、A-8 古い証明の適用、A-9 無反応ボタン、A-10 declarativeでSplit/Merge有効、B-5 表示名判定、B-7 誤notice。

**文書の是正（矛盾解消・信頼回復）**
5. B-1〜B-4/B-9：正本progress.mdの99%/100%/55%を実スコープへ。特に**B-1の「過大と認めつつ60%へ加点」は要再考**。requirements-status の残存集計（32/27/28等）と旧完成率を除去。

**利便性機能（着手推奨順）**
6. 小工数から：F-D 複製 / F-E 並べ替え / F-A 2D計測。
7. 中工数の高価値：F-1 角度→対象作図、F-2 コンパス円スナップ、F-3 分数グリッド（いずれも折り紙作図の基本を埋める）、EDT-009 最小不能部分集合、交差一括分割。
8. UX強化：F-6 カメラ保存、F-10 技法パレット、F-11 3D計測、F-9 オニオンスキン、F-4 配列複製。

---

（本レポートは監査所見であり実装指示ではない。採否・優先順位はオーナー判断。前回同様、Codexが本doc（docs/plans/）を読んで対応する運用が観測されている。）

---

## Codex追補（2026-07-23、監査後の追加証拠）

- コミット`920d2fb`で、単一`.ori2`既存保存先の認証済みjournalを`Prepared`、`OldMoved`、`NewPublished`の各永続化直後にtest限定で停止し、停止processとは別の新規processが通常read/recoveryを実行するUnix回帰を追加した。
- fixtureは非空Undo/Redo各1件、履歴上限7、project IDを保持する。各phaseでoldまたはnewの完全なarchive一方へ収束し、履歴認証とproject binding、親processによる二度目のstrict read、private残骸0を検証する。focused test 1件とdesktop lib Clippy `-D warnings`は成功した。
- failpointと補助processは`cfg(test)`かつUnix限定で製品binaryへ入らない。この証拠はin-process filesystem adapter試験を実process中断まで拡張するが、Windows正式bundleのnative file dialog、実ACL、同期software・virus対策softwareによるfile lockの実機受入を代替しない。したがって「プロジェクト・保存・履歴」78%と全体79.32%（表示79.3%）は変更しない。
- 後続コミット`b094053`ではWindowsの保存実装にもtest限定failpointを追加した。Windows固有契約に合わせ、認証済み`Prepared` journalの永続化直後と、atomic replacement完了後のnew-published状態（journal phaseは`Prepared`のまま）の2境界で停止し、別process recovery、非空履歴とproject binding、二度目のstrict read、private残骸0をnative focused 2 caseで確認した。Unixの`Prepared / OldMoved / NewPublished` 3-phase証拠は置換せず併存する。
- Windows native subprocess試験、desktop lib Clippy `-D warnings`、format、diff-checkは成功した。ただしこれはtest harness内のprocess中断自動回帰であり、正式bundleのnative file dialog、実ACL、同期software・virus対策softwareによるfile lockの実機受入ではない。「プロジェクト・保存・履歴」78%と全体79.32%（表示79.3%）は引き続き変更しない。
