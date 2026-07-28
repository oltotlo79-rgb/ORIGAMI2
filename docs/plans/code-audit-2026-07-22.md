# ORIGAMI2 コード監査レポート（改善点・不具合・矛盾点）

- 監査基準コミット: `7fe7fd7`（2026-07-22 18:41）
- 実施: Claude（読み取り専用アドバイザー）。**本レポート以外のファイルは一切編集していない。**
- 方法: 5領域を並列で実ソース精読（衝突/平坦折り・折りエンジン/制約・frontend・native/IO・docs対コード）。全所見は file:line を開いて検証済み。ビルド/テストは未実行（Codexと`target/`共有のため）。
- 注意: リポジトリは分単位で更新中のため、行番号は基準コミット時点。修正前に現物を再確認すること。

重大度の目安 — **HIGH**: 実害のある不具合／利用者に見える誤り・データ損失。**MED**: 条件付き不具合・分かりにくい矛盾・保守性の重大な負債。**LOW**: 軽微・防御的多重化・死蔵コード・文書の些細な不整合。

---

## A. 確認された不具合（BUG）

### A-1. 【HIGH】静的メッシュ書き出しが閉路姿勢でパニックし、セッションがブリックする
`apps/desktop/src-tauri/src/mesh_export.rs:569-570`（パニック源 `applied_pose.rs:448/458`）
- 現在の適用姿勢が `CurrentNativeMaterialPose::Graph`（閉路/非tree モデル、`applied_pose.rs:1002-1006` で正規に生成される）のとき、`view.model()/pose()` が `tree().expect("tree pose view")` を呼び、`Graph` では `None` → **panic**。
- `capture_export_source`（`mesh_export.rs:479`）が **AppStateミューテックスを保持したまま**569行に達するため、パニックが**ミューテックスをpoison**。以降の全プロジェクトコマンドが `lock_project` で失敗し、**再起動まで操作不能**。
- 再現: 閉路モデルにnative 3D姿勢を適用 → 静的メッシュ書き出し。
- 修正: 569行の前で graph 姿勢をクリーンなエラーで拒否（「閉路姿勢は静的メッシュに書き出せません」）、または exporter を graph 対応にする。`.expect()` を graph 到達経路で使わない。
- 副次（同ファイル `:526/549`）: `.ori2` の `paper.*.texture_asset` が `texture_assets` に無い場合 `.expect("validated project texture reference")` がパニックしうる（読込検証は頂点/辺の未参照のみ確認、テクスチャ参照は未検証）。

### A-2. 【HIGH】制約ソルバーの角度が `vertex` を無視し、180°ずれた角度を「満足」と誤報告
`crates/ori-core/src/constraint_solver.rs:540-551`（`AngleBisector` も `:589-606`）
- `vector(edge)` は無条件に `end - start`（`:490-495`）。`FixedAngle{vertex, first_edge, second_edge, angle}` は `vertex` を破棄し、2辺の**生の向き**間の `atan2` を計算する。辺が頂点に向かって描かれていると、強制される角は**補角（180°ずれ・符号反転）**になる。
- 「制約を保つ移動」プレビュー/適用と `verify_geometric_constraint_solution_v1` を支える中核。ユーザーが「頂点Vで90°」と設定しても、辺の巻き（画面に出ない）次第で**誤った結果が「満足」と報告される**。
- 修正: 各辺ベクトルを `vertex` から外向きに揃えてから角度計算（`vertex == edge.end` のとき反転）。`vertex` がどちらの辺の端点でもなければ拒否。

### A-3. 【HIGH・潜在】平坦スタック判定が「共面重なり」と「貫通」を取り違え、本物の貫通を判定保留へ格下げ（false-safe経路あり）
`crates/ori-collision/src/static_collision.rs:1141-1154`（関連 `:1789-1811`, `:1289-1332`）
- 平坦スタック判定が `pair.proves_zero_thickness_penetration` を条件にするが、このフラグ（`legacy_dispatch_proves_zero_thickness_penetration`, `:1804-1810`）は **4頂点以上の面では `CoplanarAreaOverlap` と `TransversalCrossing` の両方で真**。
- そのため**ヒンジ隣接クアッド面の本物の貫通（自己交差）が `SharedFeatureFlatStack → RequiresHingeModel → Indeterminate` に格下げ**され、周辺コメント（`:1143-1147`「coincident positive **area** overlap」）と矛盾。
- さらに同じフラグが `whole_face_overlap_proven` を経て共有頂点の層順序フリップ（`:1289-1332`）を駆動し、層セルが被れば **`Allowed`（false-safe）に化けうる**。防いでいるのは `is_for_pose_authority`（`flat_endpoint_layer_order.rs:325-331`）が姿勢を実際に平坦と検証していない「規律」に過ぎない。
- 修正: 平坦スタック分岐と共有頂点フリップを `matches!(pair.evidence, CoplanarAreaOverlap)` に限定（`TransversalCrossing` を除外）。

### A-4. 【MED】`-0.0` の紙厚が「ゼロでも正厚でもない」扱いになり、貫通判定が判定保留へ潰れる
`crates/ori-collision/src/static_collision.rs:658`, `:974-975`, `:1037-1038`
- 検証は `!is_finite() || thickness < 0.0`（`:658`）のみ。IEEE で `-0.0 < 0.0` は偽なので **`-0.0` が通過**。`is_positive_zero`（bit比較）も `is_positive_thickness`（`> 0.0`）も偽になり、貫通スキャンがスキップされ全 `Penetrating` セルが `Indeterminate` へ潰れる。`+0.0` と `-0.0`（例: `0.0 * x`）で分類が変わる。
- 修正: `thickness == 0.0`（±0を吸収）を使う、または入力検証で `-0.0` を正規化/拒否。

### A-5. 【MED-HIGH】フォルダー保存が層証拠（layer evidence）を無音破棄する（.ori2 は拒否するのに）
`crates/ori-formats/src/project_folder.rs:334`（writer）/ `:565`（reader）
- `write_project_folder_v1_with_limits` は `archive.layer_evidence` を**一切読まず**、reader は `layer_evidence: None` を固定。フォルダー書出→読込で**層証拠が黙って消える**。
- 兄弟の `.ori2` は `read_project_ori2_with_limits` が `FormatError::LayerEvidenceRequiresArchiveApi`（`ori2.rs:569-571`、コメント「silently dropping it would make a subsequent save destructive」）で**拒否**しており、直接矛盾。フォルダー保存に切り替えると計算済み平坦折り層証拠を失い、次の保存が破壊的になりうる。
- 修正: フォルダーにも層証拠を永続化する、または writer 冒頭で `layer_evidence.is_some()` なら fail-closed。

### A-6. 【MED】親フォルダーが約4098件超だと既存フォルダーの上書き保存が失敗し復旧不能
`apps/desktop/src-tauri/src/project_folder_io/replacement.rs:35, 576, 1417-1419`
- `ensure_transaction_namespace_clear` が `parent.list_names(MAX_RECOVERY_PARENT_ENTRIES + 1)`（=4096）を呼び、超過で `InvalidTree`→`RecoveryRequired`。デスクトップ/ドキュメント/同期フォルダーがエントリ過多だと上書き保存がハード失敗し、保留トランザクションも復旧不能（データ破損はなし）。
- 修正: `.origami2-folder-*` 名前空間だけに絞り、ディレクトリ全体の総数上限を撤廃。

### A-7. 【MED-LOW】非有限座標の頂点が `null` として書かれ、ファイルを二度と開けなくなる
`crates/ori-formats/src/lib.rs:568`（write）/ `:586`（read）
- `write_project_json_with_size_limit` の7検証に頂点座標の有限性チェックがない（制約が無いと早期return, `:809-811`）。`serde_json` は非有限 `f64` を `null` として書き、reader は `f64` に戻せず**再オープン不能**。native JSON は `.ori2` とフォルダー両形式の土台。
- 他の全writerは境界で有限性を守っている（`fold.rs:928`, DXF, `mesh_export.rs:842-848`, `svg.rs`）ため、ここだけの防御の穴。コマンド入力側は有限性検証済みなので実到達性は低くMED-LOW。
- 修正: writer/reader 双方が呼ぶ `validate_project_geometry_finiteness` を追加。

### A-8. 【LOW】制約ソルバーの収束判定がoff-by-one（最終反復で収束しても NonConvergent）
`crates/ori-core/src/constraint_solver.rs:146-228`
- 残差チェックがループ先頭のみ（`:148-167`）。最終反復でステップ適用後の残差を検査せずループ終了 → `Err(NonConvergent)`（`:228`）。実質 `max_iterations-1` 手しか使えず、予算ちょうどで収束する解が「安全な制約解を作成できませんでした」となり Apply が無効化。
- 修正: ループ後に残差を1回再計算し、許容内なら `Ok`。

### A-9. 【LOW】全driver（完全確定）系が over_constrained と誤判定される
`crates/ori-core/src/constraint_solver.rs:126-139` + `apps/desktop/src-tauri/src/lib.rs:9773-9781`
- `variables.is_empty()` のとき `rank: 0` を固定（`:134`）する一方 `equation_count` は残差行数。`solve_system_classification` が `equation_count > rank(0)` → `over_constrained` を返す。パネルのバッジ（`GeometricConstraintPanel.tsx:251-263`）に誤診が出る。
- 修正: その早期returnで真のランクを計算、または残差許容内の全driver系は well-constrained とする。

### A-10. 【LOW】新規フォルダー保存が `sync_directory()` 失敗を握り潰す
`apps/desktop/src-tauri/src/project_folder_io.rs:750`
- rename コミット後の `let _ = self.parent.sync_directory();` が耐久性失敗を無視。新規フォルダー経路はジャーナル/レジストリを持たないため、メタデータflush失敗＋電源断で「成功と告げたのに存在しない保存」となり復旧手段なし（既存データ破損はなし）。
- 修正: 新規フォルダー経路でも `sync_directory()` の結果を伝播/リトライ。

---

## B. 矛盾点（CONTRADICTION）— docs対コード／コード対コメント

### B-1. 【HIGH】「初心者向け自動設計」は28個の固定テンプレート転写であり、生成器ではない
- 主張: `docs/requirements-definition.md:24-25,263`（AUT-101「目標形状から一枚紙の展開図と折り手順を自動生成」）、`docs/progress.md:24`「初心者向け自動設計 55%」「端末内限定生成を接続」。
- 実体: `crates/ori-domain/src/beginner_generator.rs` — `BeginnerGeneratedPlanKindV1` は**28個のハードコードbase**の閉じたenum（`:150-179`）。`generate_beginner_plans_v1`（`:854`）はパーツ数を数えて1テンプレを選ぶ巨大 match。一般tree経路 `append_bounded_radial_tree_graph`（`:2087`）は骨格を用紙の5-15%帯へ線形写像し、**山谷を `index % 2` で振るだけ**（`:2211-2215`）。**平坦折り可解性チェックも分子/円パッキングもtree法解法もない**。
- なぜ重要か: 「目標形状から折れる展開図を合成」という要件に対し、実体はスケールした骨格トレース＋交互MVで、折れる保証も目標形状の再現もない。これを自動設計の「55%」と計上し「general tree候補」と名付けるのは実体の過大表示。

### B-2. 【HIGH】「3D折り・紙厚・衝突 99%」は SIM-010 が唯一の部分実装で、正厚・多面は安全証明できず遮断
- 主張: `docs/progress.md:19`（3D領域99%）、`requirements-status.md:244`（SIM-010 が唯一の部分実装）。
- 実体: `crates/ori-collision/src/static_collision.rs` の公開分類器 `prove_static_collision_geometry`（`:720`）は正厚/多面で `PairEvidenceUnavailable`（`:882,903-904,995`）、未証明は `Indeterminate`（`:197`、遮断であって安全でない）。安全証明は「プロジェクト変更を認可しない」（`:553-555`）。一般正厚衝突は安全証明されず遮断へ落ちる。
- なぜ重要か: Apply がラボ用fixture以外で無効な未完MUSTを含む領域を「99%」とするのは、7/18監査が警告した「UI未接続基盤の厚い計上」の再現。

### B-3. 【MED-HIGH】「数式・幾何制約 100%」だが EDT-009 の矛盾検出は7個の固定パターンのみ
- 主張: `requirements-definition.md:138`（EDT-009「矛盾した制約の原因を特定表示」）、`requirements-status.md:212` 実装済み、`docs/progress.md:18`「数式・幾何制約 100%」。
- 実体: `crates/ori-core/src/constraints.rs` の `DirectConstraintConflictKindV1` は**7変種**（`:310-338`）で全て2項の構文的パターン。11制約種のうち PointOnLine/対称/回転対称/角二等分/等長/平行/長さ比 は `unchecked`（`:1479-1535`）。`NoDirectConflict` は unchecked が空、すなわち **FixedLength/水平/垂直 のみの集合でしか返らない**。それ以外は `Unknown{SolverRequiredConstraintKinds}`。一般の過拘束/矛盾は原因特定されず「判定保留」。コード自身がコメントで「global satisfiability certificate ではない」（`:370`）と明言。
- なぜ重要か: 一般的な制約作業で「矛盾なし」すら返せない検出器を含む領域を「100%」とするのは過大。

### B-4. 【MED-HIGH】requirements-status.md が自身の集計と矛盾／複数の「現在値」が併存
`docs/requirements-status.md`
- `:5` と `:15` は「実装済み86 / 部分実装1 / 未着手0」だが、同じく「現在の行単位集計」と題した `:13` は「実装済み57 / 部分実装25 / 未着手5」。さらに旧記述「完成率36.9%…」「実装済み32/27/28」（`:62,141-142`）が現行の86/1/0 と並存。同一文書が**4つの異なる「現在」**を提示。
- 加えて「86/87実装済み（98.9%）」と `progress.md` の加重79.3%・経路探索45% が不整合。原因は「実装済み」が各MUSTを**実装済みの縮小スコープに再定義**して測っているため（`requirements-status.md:7`）。VAL-003は凸面のみ（`facewise.rs:715-791`）、VAL-004は「任意の無衝突経路探索は行わない」等。
- なぜ重要か: 「86/87完了」を読むと初版ほぼ完成に見えるが、実体は79.3%で経路探索は半分未満。

### B-5. 【MED】症状3が利用者経路に残る — 層順序版の診断がユーザー経路に接続されていない
`apps/desktop/src-tauri/src/stacked_fold_read.rs:3963`（層順序なしの `diagnose_static_collision_geometry` を呼ぶ）
- 層順序で共面重なりを `Allowed` にできる版 `diagnose_static_collision_geometry_with_flat_layer_order_v1`（`static_collision.rs:1268-1339`）は存在するが、それを呼ぶのは `applied_pose/static_collision.rs` のみ。デスクトップの折り重ね端点DTOは**層順序なしの平の診断**を呼ぶため、座布団折り等の正当な平坦折りが実画面で今も「判定保留」表示（本会話の症状3）。症状4（貫通誤標示）は `96a131d` で `RequiresHingeModel→判定保留` に是正済みだが、症状3はこの配線差で残存。
- 修正: ユーザー経路の折り重ね診断を層順序版へ切替。

### B-6. 【MED】静的メッシュ書き出しの `ProvenTransversalPenetration` が共面重なりを含む（種別名が意味的に誤り）
`crates/ori-collision/src/static_collision.rs:962-972`, `:164-171`
- `+0.0` 厚で `proven_zero_thickness_penetrating_pairs > 0` のとき `ProvenTransversalPenetration` を返すが、その件数は `CoplanarAreaOverlap` を含む（`:1805`）。共面重なりは transversal crossing ではないため enum 名/フィールド名が一部のペアに対し意味的に誤り（Display文言は中立で、enum が自身のメッセージと矛盾）。
- 修正: 厚み中立の種別名へ改称、または coplanar/transversal を分離集計。

### B-7. 【MED】切断で分離したパターンを「閉路制約のため」と誤表示
`apps/desktop/src/components/FoldPreview.tsx:4223-4228`, `:4323-4328`
- `foldPreviewModel.ts` は非tree運動学を2種生成: `static_cycle/cyclic_hinge_graph`（`:281-282`）と `static_components/cut_material_components`（`:251-259`、ヒンジ0、切断で分離）。FoldPreview は `tree` か否かでしか分岐せず、切断分離にも「…because of cycle constraints」（`:4226,4326`）を表示。閉路も折り線も無いのに誤診。
- 修正: `model.kinematics.reason` で分岐し、`cut_material_components` に固有メッセージ。

---

## C. 改善点（IMPROVEMENT）— 保守性・死蔵コード・過剰設計

### C-1. 【HIGH】App.tsx が極端なゴッドコンポーネント
`apps/desktop/src/App.tsx` — **12,177行、useState 126個**、useEffect 25、useCallback 34、useMemo 22。`coreClient` 全面をimport。1関数に126の独立state → 単体テスト不能・再レンダー追跡困難・全機能編集が同一12k行に集中（Codexが常時編集する中で衝突面が大）。次点で `coreClient.ts`（6,088行・144関数）、`FoldPreview.tsx`（4,987行）。3ファイルで約23k行。
- 修正: state群をカスタムフック/コンテキストへ抽出、JSXを機能パネルへ分割、coreClientをドメイン別に分割。

### C-2. 【HIGH】TS側とRust側で厳密幾何の衝突判定が二重実装され、両方が実行時に動く
- TS: `foldPreviewNarrowCollision.ts`（5,287行）+ witness/exact-triangle ≈ **6,518行**。Rust: `crates/ori-collision`（66,223行）。TS版は対話プレビュー、Rust版は `nativeStaticCollisionView.ts` 経由で、**両方が実行時に動く**。2つの独立した厳密幾何実装を手作業で数値整合させ続ける必要があり、correctnessの時限爆弾。
- 修正: Rust `ori-collision` を単一の真実源とし、TS側は薄い近似プレビューへ縮小、または厳密判定をIPCの背後へ。最低限、両立の理由を文書化し相互整合テストを追加。

### C-3. 【MED】症状2 — 閉路ヒンジグラフはプレビューで対話折り不可
`apps/desktop/src/lib/foldPreviewModel.ts:280-283`; `FoldPreview.tsx:1439-1468`
- `hinges.length >= faces.length`（=閉路）で `static_cycle` を返し、no-op の平坦 `updatePose`（`:1456-1468`）を設置 → ドラッグ/角度が無効。閉路折りの実経路は別パネル（`StackedFoldPanel.tsx` → `proposeCurrentCyclePoseV1/applyCurrentCyclePose`）にあり、バックエンドに要求角の閉路版（`stacked_fold_transaction.rs:81-99`）もあるが、対話プレビューは到達しない。水風船/座布団基本形がプレビューのドラッグでは行き止まり（可視＋ariaメッセージはあり）。
- 修正: 閉路グラフをプレビューから認証済み閉路姿勢経路へ配線、または静的メッセージで閉路パネルへ明示誘導。

### C-4. 【MED】過剰な防御的IPC境界（同一チームが書くコアに対して）
`apps/desktop/src/lib/coreClient.ts` ほか
- 全変更IPCに所有権ガード3つ組（`expectedProjectId` 247 / `expectedProjectInstanceId` 231 / `expectedRevision`）＝1ファイルで478ガード引数、全ソースで622。加えて非テストで **`Object.freeze` 1,243箇所**（coreClient 90、narrowCollision 71）、`WeakMap` trust-registry 15。native側も同ガード3つ組が100コマンドで重複（`lib.rs`、168コマンド）。**変更コマンドでガード漏れは無い**（correctnessは健全）＝重複であって不具合ではない。
- 修正: 3つのIDを1つの `ProjectFencingToken`/`ProjectExpectation` newtype にまとめ一度だけ渡す。内部DTOの `Object.freeze` は `readonly` 型に委ね撤去。

### C-5. 【MED】i18n テキストがインライン `{ja,en}` 対（約1,180組）で散在
`apps/desktop/src/lib/i18n.ts`, `appMessages.ts`（151行のみ）
- i18n基盤は存在するが、UI文字列は圧倒的にインラインリテラル（非テストで `ja:` 1,180 / `en:` 1,186）。中央カタログが151行しかなく、網羅監査・再利用・翻訳者への受け渡しができない。`en` が `ja` より6個多く、未対応文字列漏れの疑い（要確認、`DEFAULT_LOCALE='ja'`）。
- 修正: インライン対をキー付きカタログへ移行。

### C-6. 【MED/死蔵】`cycle_fold_transaction` モジュール全体が未到達（閉路経路の重複実装）
`crates/ori-core/src/cycle_fold_transaction.rs:48,95`（`lib.rs:42-43` でexport）
- `prepare/apply_..._cycle_fold_transaction_v1` はテスト外に呼び出し無し。実際の閉路適用は `apps/desktop/src-tauri/src/stacked_fold_transaction.rs:610` 側が担い、同じ閉包証明束縛を再実装。完成に見えてアプリでは何も提供しない死蔵＋乖離リスク。
- 修正: このモジュールを単一の閉路適用経路として配線（src-tauri重複を除去）、または削除。

### C-7. 【LOW】研究先行の死蔵コード
- `crates/ori-collision/src/cayley/positive_thickness/direct_f_affine_corridor.rs`（**2,892行**）はテスト専用、製品未到達（`cayley` モジュール全体が `#[allow(dead_code)]`, `lib.rs:23`）。本番の共有ヒンジ分類は非affineの `direct_f_corridor` を使用。→ `#[cfg(test)]` 化または配線。

### C-8. 【LOW】その他の死蔵/誤解を招く分岐
- `ConstraintSolveErrorV1::UnsupportedConstraintKind`（`constraint_solver.rs:57-58`）はどの経路も返さない死蔵変種（全11種サポート）。テスト `:771` が偽の安心を与える。
- `execute_command` Branch A（`lib.rs:12280-12287`）は `?` で必ず早期return するため後続簿記が到達不能な死蔵コード（挙動は正しくfail-closed）。
- Windows のreservation registry ルートが継承ACLで作成され world-readable（`replacement.rs:1967-1969`、unix は 0700）。絶対パス・SHA-256・識別子を共有Windows機で読める（機密性のみ）。

### C-9. 【MED・機能欠落】角度を数値指定して「対象（辺/折り目/点）まで伸ばす」折り目作成が無い
- **あるもの（除外対象・実装済み）**: EDT-003（`requirements-status.md:206`）で、選択頂点を始点に**長さ＋角度＋線種**から終点と線を1つのnative原子的commandで作成できる（`apps/desktop/src-tauri/src/lib.rs:9919-9941`、`終点 = 始点 + 長さ×(cos,sin)`）。**長さを数値指定**する方式。
- **無いもの（本指摘）**: 始点＋**角度だけ**を数値指定し、その方向へ射線を伸ばして**任意の辺/折り目/点と交わる位置**を終点にする作図（長さは交点で自動決定）。この経路は core・native・frontend のいずれにも存在しない（edge作成の "extend to target / until intersection" はgrepで皆無）。角度スナップ（`apps/desktop/src/lib/snap.ts:89`、`referenceKind` は `'global-horizontal' | 'edge'`）はドラッグ方向を刻むだけで、数値角度で対象まで自動延長する機能ではない。
- **要件状況**: EDT-003 は「座標・長さ・角度を指定」までで（`requirements-definition.md:132-133`）、「角度指定＋対象まで延長」は要件になく**実装予定も無い**。
- **回避策（不完全）**: 角度スナップ＋交点スナップでマウス作図するか、交点までの長さを手計算して長さ＋角度で作る。いずれも「数値角度→対象まで」を直接満たさない。
- **なぜ重要か**: 折り紙作図で「この点からN度で対辺（や別の折り目）まで折る」は基本操作。現状は交点距離を手計算させられ精度・操作性が落ちる。
- **提案**: 「始点＋角度＋対象（辺/折り目/点）」を受け、厳密に交点を解いて終点とする作図コマンドを追加（既存の厳密述語 `crates/ori-geometry` の segment 交差を再利用）。長さ指定版とはUIを分ける。

---

## D. 本会話で観測された症状1〜4の現状

| 症状 | 現状 | 根拠 |
|---|---|---|
| 1. 交差線の風船基本形→「幾何が不正」 | **仕様（未改善）**。交差の自動分割が無く、平面分割を要求 | `validation.rs:546` UnsplitIntersection。11制約種のUI接続は解消済み |
| 2. 中心頂点追加後に折れない | **仕様（閉路プレビュー未対応）**。別パネルに閉路経路あり | C-3、`foldPreviewModel.ts:282` |
| 3. 座布団折りが判定保留だらけ | **利用者経路に残存**。層順序版が未配線 | B-5、`stacked_fold_read.rs:3963` |
| 4. 平坦折りが「貫通」 | **是正済み**（`RequiresHingeModel→判定保留`）。ただし A-3/A-4 の潜在問題あり | `96a131d`、collision監査 |

---

## E. 確認された健全点／過去指摘の解消（誤解防止のため明記）

- **未完成スタブ無し**: reachable な `todo!()/unimplemented!()` はゼロ（`unreachable!()` は全てガード済み）。
- **7/18監査の「UI未接続6,600行」は解消**: 補正候補パイプライン（現≈7,989行）は `FoldPreview.tsx` に完全接続・実行時到達（frontend監査で全経路追跡）。
- **`console.*` は0**、エラーは診断リポーターへ集約。空catchは103→**66**へ減り、全て意図的コメント付き。
- **11制約種は全てUI作成に接続**、ソルバーは本物のGauss-Newton＋Tikhonov。
- **永続化は堅牢**: 復旧は復元前に再検証（`recovery.rs:1145/1180`、ABAエポックガード）、`.ori2` は原子的ジャーナル保存、FS置換は no-follow＋reparse拒否＋起動時復旧。
- **変更コマンドのOCCガード漏れは無し**。

---

## F. 優先度つき推奨（対応順）

**即修正すべき実バグ（HIGH）**
1. A-1 メッシュ書き出しの graph 姿勢パニック（セッションブリック）— `.expect()` を fallible 化。
2. A-2 制約ソルバーの角度180°ずれ — 辺ベクトルを頂点基準で外向きに。
3. A-3 平坦スタック判定の coplanar/transversal 取り違え（潜在false-safe）— 証拠を `CoplanarAreaOverlap` に限定。
4. A-5 フォルダー保存の層証拠無音破棄 — fail-closed か永続化。

**次に（MED、利用者に見える／データ健全性）**
5. A-4 `-0.0` 厚、A-7 非有限→再オープン不能、A-6 親4098件で保存失敗。
6. B-5 症状3の層順序版を利用者経路へ配線。B-7 切断分離の誤メッセージ。A-8 収束off-by-one。

**文書の是正（矛盾解消・信頼回復）**
7. B-1〜B-4: 「自動設計55%」「3D 99%」「制約100%」「86/87実装済み」を実スコープに整合。requirements-status の自己矛盾集計（86 vs 57）と旧記述を除去。再計上案（90.35% pending CI, `progress-reassessment-pending-ci-2026-07-22.md`）は上記の過大計上を増幅しうるので、ドメイン入力値の実体化を先行。

**保守性（MED-LOW、負債返済）**
8. C-1 App.tsx分割、C-2 衝突二重実装の一本化、C-4 IPCガード集約、C-5 i18n外部化、C-6/C-7/C-8 死蔵コード整理。

---

（本レポートは監査所見であり、実装指示ではない。対応の採否・優先順位はオーナー判断。）
