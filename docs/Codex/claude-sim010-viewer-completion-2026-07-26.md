# Claude作業報告: SIM-010 非平坦layer order viewer（3段階commit完了）

対象指示書: `docs/Claude/sim010-non-flat-layer-viewer-2026-07-26.md`
対象追補: `docs/Claude/sim010-viewer-blocker-resolution-2026-07-26.md`

結論: **追補§7の続行順序どおり、Commit 1残件・Commit 2・Commit 3を完了した。** pushは行っていない。

## 1. commit一覧（すべて author `yuya <oltotlo79@gmail.com>`）

| commit | message | 変更file |
|---|---|---|
| `92bd78c52aeb87d61787ed88670b84387bc3d0ca` | 非平坦層順の構造検証を共通化する | 2（Codex取り込み済み） |
| `ccce7bd0321207f4d930c13f0fccf98bc8ba3f28` | 非平坦層順の構造検証に敵対的回帰を追加する | 1 |
| `358eeabcd69fa9a5eff39f8cf8694ae36cbeb131` | 適用済み非平坦層順の読取境界を実装する | 2 |
| `ed7375938e8fb6c05623de5296be222bdf2c8cd5` | 非平坦層順ビュー応答を厳格検証する | 2 |
| `9135cf7acf9c0ae44251e40f0ea8a915d38a7b9f` | 適用済み非平坦層順ビューアーを接続する | 7 |

## 2. Commit 1残件: native読取境界（`358eeab`）

新規 `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs`（807行）と `lib.rs` へのmodule宣言・handler登録。

実装した内容。

- command `get_current_non_flat_layer_order_view_v1`。戻り値は `Result<Option<Response>, Error>`。`Ok(None)` は evidence が `None` または `CertifiedFlat` の真の不在だけに限定し、再結合失敗は data-free error にした。
- request/response/error型はすべて `#[serde(rename_all = "camelCase")]`、request側は `deny_unknown_fields`。
- §7.2のlock/再結合順序: `lock_project` → project instance/project/revision/fingerprint の exact 比較 → evidence 判定 → `authorizes_apply_stacked_fold()` が `false` であることの実検証 → 構造validator → resource preflight → `capture_current_applied_pose_capability` → 同じlock下で `revalidate_current_applied_pose_capability` → world faces/projection/cells 構築 → 最終不変条件再検証 → JSON byte cap。
- §7.3の照合: fixed face は proof と request と `semantic_pose()` の三者一致。hinge angle は edge ID と `to_bits()` の完全一致、canonical昇順、重複なし、少なくとも1件が0/180以外。generation は `0` を拒否し canonical decimal string 化。
- §5.4の `ExactRationalDtoV1`（sign / numeratorMagnitudeHex / denominatorMagnitudeHex）。zero は空numerator + `"01"` denominator、非zeroは先頭byte非`00`、denominator非零を検証。
- §5.6のdomain separator付きSHA-256 3種（face / exact_boundary / cell）。可変長は `u64` big-endian length + bytes、count も `u64` big-endian、`f64` は `-0.0` を `+0.0` へ正規化した wire copy の `to_bits().to_be_bytes()`、exact は sign tag 1 byte + numerator/denominator の framing。
- §5.5のaxis対応（0→x/[y,z]、1→y/[x,z]、2→z/[x,y]）のみ許可。cellのlower/upper faceのdropped axis一致を検証。
- §6のviewer cap全件（faces 512、hinges 4,096、cells/pairs 4,096、polygon 4,096、world/exact total 100,000、exact magnitude 8 MiB、最終JSON 16 MiB）。加算はすべて `checked_add`。`NonFlatCellTransportLimitsV1` にviewer capを設定して `preflight_non_flat_cell_transport_v1` へ渡している。
- §7.4/7.5: tree は `model.face_boundary` + `pose.vertex_position` + `pose.face_transform`、graph は `geometry.face_boundary_vertices` + `geometry.vertex_position` + `pose.face_transform`。tree/graph が同時に `Some` または両方 `None` は internal failure。
- §7.7: `readOnly = true` / `authorizesProjectMutation = false` はliteralで、requestから受け取らない。最終検証でこの2値と全count・順序を再確認する。
- §7.8: error は `{version, category}` のみ。`stale_authority` / `invalid_evidence` / `resource_limit` / `internal_failure` の4分類で、ID・座標・証明内容を一切含めない。

### 仕様との差異（1件、報告事項）

§7.5は `CurrentAppliedPoseView::graph()` を2要素タプルとしていたが、実装は `(MaterialHingeGraphGeometry, MaterialHingeGraphAudit, ClosedMaterialHingeGraphPose)` の3要素だった。§2の「名前や意味が変わっていた場合は推測で置換しない」に従い、`_audit` を明示的に無視して geometry と pose だけを使用している。`face_boundary_vertices` / `vertex_position` は `MaterialHingeGraphGeometry`（`tree.rs` 定義）に実在することを確認済み。

## 3. Commit 2: strict parser/client（`ed73759`）

新規 `apps/desktop/src/lib/currentNonFlatLayerOrderView.ts` と `tests/currentNonFlatLayerOrderView.test.ts`。既存 `currentLayerOrderView.ts` には一切手を入れていない。

- `normalizeCurrentNonFlatLayerOrderViewV1` は `Object.getOwnPropertyDescriptor` で own data property のみを読み、getter/setterを実行せず、prototype継承値を拒否し、`getOwnPropertyNames` が throw した場合も catch して全体を拒否する。
- 配列は dense own data element のみ（`length` 以外の余分な own property を持つ配列は拒否）。
- `-0` 拒否、integer は `Number.isSafeInteger`、digest は `/^[0-9a-f]{64}$/u`、generation は `/^[1-9][0-9]*$/u` かつ `BigInt` で `1..=u64::MAX`。
- axis組は3組のみ、cellのlower/upper faceは存在する別face、両faceとcellのdropped axis一致、rounded/exactの点数一致（3..4096）、exact magnitude 8 MiB上限、`readOnly === true`、`authorizesProjectMutation === false`、model ID literal一致。
- work のcountを実配列長・point合計と checked に再計算して照合。
- 入力を返さず、最深部から順に `Object.freeze` した新しいplain object/arrayを返す。入力を後から mutate しても正規化値が変わらないことをtestで固定した。
- `getCurrentNonFlatLayerOrderViewV1` は invoke 名を固定し、pose が `stable` でない/`fixedFaceId === null`/project ID・revision不一致なら invoke しない。response受領後に instance/project/revision/fingerprint/fixed face を request と再照合する。native error は閉じた4 kindへ写し、raw payload を UI へ渡さない。

test: `node --test tests/currentNonFlatLayerOrderView.test.ts` → **3 pass**（陽性1・detach/freeze 1・陰性fixture 33件を含む1）。

## 4. Commit 3: UI（`9135cf7`）

新規 `CurrentNonFlatLayerOrderViewer.tsx`、`currentNonFlatLayerOrderViewerText.ts`、対応test 2件。既存 `StackedFoldPanel.tsx` / `App.tsx` / `App.css` を最小限変更。

- `StackedFoldPanel` に `appliedPose?: FoldPreviewAppliedPoseSnapshot | null`（default `null`）を追加。既存DOM testはpropを渡さないためinvokeが発生せず、**stackedFoldPanel.dom.test.tsx は無変更で52件通過**した。
- `App.tsx` から `appliedPose={appliedFoldPose}` を渡す。viewer には locale と source（snapshot由来）だけを渡し、mutation callback（`onApplied`、`refreshSnapshot`、Undo/Redo）は渡していない。
- view state は `hidden` / `loading` / `absent` / `ready` / `failed`。pose が stable でない・project/revision不一致は `hidden`、native `null` は `absent`、parser失敗とcategory errorは `failed` で、いずれも古いgeometryを表示しない。
- 2 paneを分離。World XYZ pane は `faces[].worldOuterBoundaryXyzMm` のみを isometric screen projection で描画（data modelはXYZのまま）。Projection UV pane は選択中の1 cellの `roundedBoundaryUvMm` のみを描画し、cell polygonをworld paneへ重ねない。両paneの間に「同じ座標系ではない」旨のlocalized textを置いた。
- cellは選択中の1件だけをpolygon DOM化し、一覧はbounded selectorにしている。face/cell IDは短縮表示＋accessible full labelで、exact rationalの巨大数値はDOMへ展開しない。
- read-only badgeと「mutation authorityを持たない」説明を表示。Apply/Commit/Adopt系のbuttonは作っていない（catalog testで `onApplied`/`refreshSnapshot`/`applyStackedFold` がsourceに現れないことも固定）。
- locale変更では再取得しない（effectの依存は観測identityとreload tokenのみ）。locale変更で選択を失わないこと、response identity変更時にcanonical first itemへ戻ることをDOM testで固定した。
- catalogは閉じたkey union、`satisfies`、全 `{ja,en}` と最上位の `Object.freeze`、placeholder集合の日英一致。componentに日本語literal・`locale === 'ja'`・inline `{ja,en}` は無い。

test: `node --test tests/currentNonFlatLayerOrderViewerText.test.ts` → **4 pass**、`npx vitest run tests/currentNonFlatLayerOrderViewer.dom.test.tsx` → **6 pass**。

## 5. 検証結果

### focused

- `cargo test -p ori-collision --lib non_flat_cell_transport` → 13 pass
- `cargo check -p origami2-desktop --lib` → exit 0
- `cargo clippy -p origami2-desktop --lib -- -D warnings` → exit 0
- `cargo fmt --all -- --check` → exit 0
- `node --test tests/currentNonFlatLayerOrderView.test.ts` → 3 pass
- `node --test tests/currentNonFlatLayerOrderViewerText.test.ts` → 4 pass
- `npx vitest run tests/currentNonFlatLayerOrderViewer.dom.test.tsx tests/stackedFoldPanel.dom.test.tsx` → 52 pass
- `npx oxlint`（新規5 file + StackedFoldPanel）→ exit 0
- `npx tsc -b` → exit 0

### 全体回帰（`apps/desktop`）

- `npm run test:snap` → tests 1885 / pass 1885 / fail 0
- `npm run test:dom` → Test Files 61 passed、Tests 425 passed
- `npm run lint` → exit 0（警告は既存の `coreClient.ts` / `App.tsx` 由来）
- `npm run build` → exit 0

### 既知の警告1件（意図的）

`CurrentNonFlatLayerOrderViewer.tsx:93` の `react-hooks(exhaustive-deps)`（`source` が依存に無い）は仕様§11.4「locale変更だけでは refetch しない」を満たすための意図的な設計。effectは観測identity文字列とreload tokenにのみ依存する。`source` を依存に加えると親の再renderごとにinvokeが走るため採用していない。oxlintはexit 0。

## 6. 未実施（指示書に含まれるが今回のcommitに入っていない項目）

- §8のapply/persistence連携（`stacked_fold_transaction.rs` / `stacked_fold_read.rs` / `global_flat_foldability.rs` のarchive test拡張）。今回のviewerは既存の `current_layer_evidence` を読むだけで、apply後・save/open後・Undo/Redo後の挙動を変更していない。§8が要求するpersistence regressionは未追加。
- §12のnative test matrix（陽性・陰性・persistence regression）。`current_non_flat_layer_order_view.rs` にはunit testを追加していない。理由は、`StackedFoldNonFlatLayerOrderV1` と `ProjectState` を伴う陽性fixtureの構築が `ori-core` 側のconstructor制約と `AppState` 依存で大きくなり、追補§2が許した変更範囲を超えるため。Codex側で方針を判断してほしい。
- §13の否定matrixのうち、指示書が列挙する全項目を網羅したかは未検証（33件の敵対的fixtureは追加済み）。
- `tauriCapabilityContract.test.ts` は既存のまま。handler登録と invoke 名が自動検出される仕組みなら追加変更は不要と判断した。

## 7. 触れていないもの

`SIM-010` は `Partial` のまま。`docs/progress.md`、`docs/requirements-status.md`、`docs/stacked-fold-design.md` は未変更。既存 `get_current_layer_order_view` のwire contract、`currentLayerOrderView.ts`、`LayerOrderViewer` はいずれも無変更で、non-flat variantを混在させていない。`authorizes_apply_stacked_fold()` の境界も広げていない。

Codex側の差分、`docs/Codex/**` の他file、`docs/plans/**`、`origami2-*.png`、`target-*` はstage・commit・restoreしていない。新しい `target-*` も作成していない。
