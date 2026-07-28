# Claude 完了報告: SIM-010 native viewer 残存 negative matrix

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
branch: `main`
指示書: `docs/Claude/sim010-viewer-native-negative-matrix-2026-07-26.md`

この report は stage / commit していない。remote push は行っていない。

**全項目完了ではない。** §7 の graph issuer positive が未到達である（§6 参照）。

> **2026-07-26 14:40 訂正**
> §7.2 の test 実測値と §8 の環境 blocker 記述を訂正した。詳細は末尾「11. 訂正」を参照。
> 要点は次の 2 点である。
> 1. 本 commit の module test は **44 passed / 0 failed** である。以前の「43 passed / 1 failed」は、
>    主 worktree に存在した他担当の未 commit 差分を巻き込んだ測定値であった。
> 2. ディスク枯渇は解消済みであり、Windows で cargo が動かない原因は
>    Application Control (`os error 4551`) である。

---

## 1. 着手前の競合確認（指示 §2）

実行時刻: `2026-07-26T14:07:27+09:00`

```text
HEAD                : e45498a4008a54354c4bf19b31b872cc9c2a0e87
git status --short  : （担当 file について出力なし = clean）
git log -5 (担当 file): 15f9793 / f74f74c / 358eeab
file SHA-256        : 79213CE7F427305EA2B687318D7C1746983D4013D98BDD297AA10B0BB5E03CFB
git diff --cached   : （出力なし = index は空）
git config user.name / user.email : yuya / oltotlo79@gmail.com
```

未 commit 差分なし、他担当の新 commit なし、自分以外の stage なし。着手条件を満たしたため作業を開始した。

---

## 2. commit

```text
hash      : ccc6026902513e987918cb80bf8fa08f640da2db
author    : yuya <oltotlo79@gmail.com>
committer : yuya <oltotlo79@gmail.com>
subject   : 非平坦層順ビューの否定境界行列を完成する
```

changed path（exact 1 file）:

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs   800 / 109
```

commit 後の file SHA-256:

```text
A8879975F90CF288DAB7A6881312F3F9903698846625C9200A6FBD5037B2882C
```

`git commit --only -- <path>` を使用し、他者の stage を巻き込んでいない。

---

## 3. 実装: proof を偽造しない値 projection（指示 §4）

`preflight_view_resources` から純粋な数値 projection と validator を切り出した。
production はこの projection を正規 proof の slice/count から一度だけ構築し、
test は同じ validator へ境界値を直接与える。**proof 自体は偽造していない。**

追加した private 項目:

| 項目 | 役割 |
|---|---|
| `ViewResourceCountsV1` | `Copy` な数値 projection。proof も authority も保持しない |
| `ViewResourceCountsV1::from_proof` | 正規 proof の slice 長・declared count から一度だけ構築 |
| `validate_view_resource_counts_v1` | count 群の唯一の判定点 |
| `validate_cell_boundary_counts_v1` | 1 cell の rounded/exact 点数と cap |
| `validate_world_polygon_count_v1` | 1 world polygon の点数と cap |
| `accumulate_bounded_total_v1` | checked add + 上限。aggregate 3 種と magnitude で共有 |
| `validate_safe_wire_integer_v1` | `u64` と JSON safe integer の両方 |
| `validate_serialized_json_bytes_v1` | serialized JSON 上限 |
| `validate_live_face_registry_v1` | live registry 完全一致（§6） |
| `resolve_cell_face_pair_v1` | 有向 face pair 解決（§6） |
| `validate_axis_derivation_v1` | dropped axis から plane axes を再導出（§6） |

projection が扱う field:

```text
material face count / folded face count / hinge count
declared overlap-cell count / actual overlap-cell slice length
declared pair count / actual pair-order slice length
declared tested-pair count / source overlap cells authenticated
```

per-cell の rounded/exact 点数、aggregate world/exact 点数、
aggregate exact magnitude bytes、serialized JSON bytes は
それぞれ上表の専用純関数が扱う。

production 経路は必ず同じ関数を通る。test だけが呼ぶ複製 validator は作っていない。
production 関数内に test-only branch は無い。projection 構築前に `Vec` を作らない。

category 分離は指示どおり維持した。

- 宣言値と実 slice 長の不一致、material/folded 不一致、rounded/exact 不一致、
  3 点未満 polygon → `invalid_evidence`
- 上限超過、checked add overflow、safe integer 超過、JSON 超過、
  `try_reserve_exact` 失敗 → `resource_limit`

---

## 4. resource 境界 test（指示 §5）

各上限を `max - 1` / `max` / `max + 1` で個別に固定した。

| 上限 | test 名 | 検証値 |
|---|---|---|
| `MAX_FACES_V1` | `the_material_face_ceiling_is_inclusive` | 1 / 511 / 512 accept、0・513 `resource_limit` |
| `MAX_HINGES_V1` | `the_hinge_ceiling_is_inclusive` | 1 / 4095 / 4096 accept、0・4097 `resource_limit` |
| `MAX_CELLS_V1` | `the_overlap_cell_ceiling_is_inclusive` | 0 / 4095 / 4096 accept、4097 `resource_limit` |
| `MAX_FACE_PAIR_ORDERS_V1` | `the_face_pair_order_ceiling_is_inclusive` | 4096 accept、declared/actual 4097 `resource_limit` |
| `MAX_WORLD_POLYGON_POINTS_V1` | `the_world_polygon_ceiling_is_inclusive` | 3 / 4095 / 4096 accept、4097 `resource_limit`、0・1・2 `invalid_evidence` |
| `MAX_CELL_POLYGON_POINTS_V1` | `the_cell_polygon_ceiling_is_inclusive` | 3 / 4095 / 4096 accept、4097 `resource_limit`、0・1・2 `invalid_evidence` |
| `MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1` | `the_aggregate_world_point_ceiling_is_inclusive` | cap-1 / cap accept、cap+1 `resource_limit` |
| `MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1` | `the_aggregate_exact_point_ceiling_is_inclusive` | 同上 |
| `MAX_EXACT_MAGNITUDE_BYTES_V1` | `the_aggregate_exact_magnitude_ceiling_is_inclusive` | 同上 |
| `MAX_EXACT_MAGNITUDE_BYTES_V1`（DTO 経路） | `the_exact_magnitude_budget_is_shared_and_capped` | cap 到達 accept、+1 `resource_limit` |
| `MAX_SERIALIZED_JSON_BYTES_V1` | `the_serialized_json_ceiling_is_inclusive` | cap-1 / cap accept、cap+1 `resource_limit` |
| JSON safe integer | `the_safe_wire_integer_ceiling_is_inclusive` | cap-1 / cap accept、cap+1 `resource_limit`、work 2 field も同様 |

追加必須項目:

| 項目 | test 名 |
|---|---|
| aggregate point の checked-add overflow | `a_bounded_accumulation_refuses_checked_add_overflow` |
| aggregate exact magnitude の checked-add overflow | 同上（同一 accumulator を共有） |
| zero faces / zero hinges | `a_zero_face_or_hinge_count_is_a_resource_limit` |
| material/folded count 不一致 | `a_material_and_folded_face_count_mismatch_is_invalid_evidence` |
| declared/actual cell 不一致 | `a_declared_and_actual_count_mismatch_is_invalid_evidence` |
| declared/actual pair 不一致 | 同上 |
| rounded/exact point count 不一致 | `a_rounded_and_exact_point_count_mismatch_is_invalid_evidence` |
| 0 / 1 / 2 点 polygon | `the_world_polygon_ceiling_is_inclusive`, `the_cell_polygon_ceiling_is_inclusive` |
| production projection と validator の同一性 | `the_production_preflight_accepts_the_canonical_fixture` |

**count から byte 数を求める checked-mul は現行実装に存在しない。**
magnitude は numerator/denominator の実 byte 長を checked add するだけであり、
乗算に由来する overflow 経路は無い。したがって該当 test は作っていない。

**allocation failure の実行注入は未実施。**
`validate_live_face_registry_v1` と `reserved_world_points` の
`try_reserve_exact` 失敗は `resource_limit` へ写る code path として保持しているが、
決定的に注入する production hook は追加していない（指示どおり）。OOM test も作っていない。

---

## 5. structural negative matrix（指示 §6）

| case | test 名 | 結果 |
|---|---|---|
| live face registry: 完全一致（順不同） | `the_live_face_registry_must_match_the_proof_exactly` | accept |
| live face registry: missing | 同上 | `invalid_evidence` |
| live face registry: extra | 同上 | `invalid_evidence` |
| live face registry: duplicate `[A,B,B]` vs `[A,B]` | 同上 | `invalid_evidence`（`dedup` で潰さない） |
| live face registry: 同数の foreign face | 同上 | `invalid_evidence` |
| material/folded coverage count mismatch | `a_material_and_folded_face_count_mismatch_is_invalid_evidence` | `invalid_evidence` |
| unknown lower face | `an_unknown_equal_or_disagreeing_face_pair_is_invalid_evidence` | `invalid_evidence` |
| unknown upper face | 同上 | `invalid_evidence` |
| equal face pair | 同上 | `invalid_evidence` |
| lower/upper の dropped axis 不一致 | 同上 | `invalid_evidence` |
| reversed pair | 同上 | 解決は成功。向きは proof 自身の主張であり cell digest に反映される（`the_exact_boundary_and_cell_digests_match_independently_fixed_preimages` が反転で digest が変わることを固定） |
| exact/rounded point count mismatch | `a_rounded_and_exact_point_count_mismatch_is_invalid_evidence` | `invalid_evidence` |
| dropped-axis tag と plane derivation の不一致 | `the_plane_axes_must_be_derived_from_the_dropped_axis` | `["y","x"]`、`["y","z"]`、`["x","y"]` 誤組合せ、未知 tag、空文字を `invalid_evidence` |
| non-finite world point | `a_non_finite_world_point_is_invalid_evidence` | NaN / +Inf / -Inf を `invalid_evidence`、`-0.0` は `+0.0` へ canonicalize |
| noncanonical exact zero denominator | `a_zero_rational_with_a_foreign_denominator_is_refused` | `[0x02]` / `[0x01,0x00]` / `[0x00]` / 空 を `invalid_evidence` |

切り出した純関数はすべて production 経路が通る。既存の正規 tree fixture 由来 test は
すべて維持されている（§7 の実測参照）。

---

## 6. issuer と dropped axis の positive（指示 §7）

### 6.1 到達したもの

| case | 結果 | test 名 | 使用した正規 constructor |
|---|---|---|---|
| tree issuer positive | **到達** | `applied_non_flat_evidence_yields_a_read_only_view`, `the_pose_model_id_follows_the_live_issuer_kind` | `ProjectState::new_with_paper` → `topology_analysis_input().analyze()` → `applied_pose::tests::install_tree_pose_authority_at_angle_on_face` → `global_flat_foldability::reanalyze_current_flat_layer_order` → `ori_core::revalidate_current_non_flat_layer_order_v1` |
| dropped X positive | **到達** | `the_vertical_crease_fixture_reaches_dropped_x_and_y` | 同上（縦クリース fixture） |
| dropped Y positive | **到達** | 同上 | 同上 |
| dropped Z positive | **到達** | `the_horizontal_crease_fixture_reaches_dropped_z` | 同上（横クリース fixture を新設） |
| 3 軸の網羅 | **到達** | `every_dropped_world_axis_is_reached_by_a_canonical_fixture` | 上記 2 fixture の和集合が exact に `{x, y, z}` |

各 fixture は次を満たす。

1. `ProjectState`、topology、flat layer order、pose authority、non-flat revalidation を
   通常の production 関数だけで生成している。
2. proof の private constructor を迂回していない（`unsafe`／transmute／raw pointer 不使用）。
3. response `poseModelId` が実 issuer の model ID と一致する
   （`the_pose_model_id_follows_the_live_issuer_kind`）。
4. dropped axis から導出した plane axes が正しい順序である
   （`validate_axis_derivation_v1` を全 face に適用）。
5. repeat で byte-identical
   （`the_same_snapshot_is_byte_identical_on_every_call`,
   `a_second_canonical_fixture_stays_read_only_and_deterministic`）。
6. `readOnly = true`、`authorizesProjectMutation = false` を維持。

**dropped Z について**: 縦クリース fixture は world XZ 平面と YZ 平面しか作らないため
Z 軸に到達できなかった。world X に平行なクリースを持つ
`horizontal_single_hinge_project` を新設し、折り後の面が world XY 平面へ入ることで
dropped Z を正規経路のみで得た。

### 6.2 未到達: graph issuer positive

**未到達である。** 担当 file 1 個の範囲では構成できない。

試行と根拠（実測）:

- `applied_pose::tests::four_vertex_cycle_project`（`pub(crate)`）と
  `applied_pose::tests::flat_foldable_cross_cycle_project`（`pub(crate)`）に対し、
  `install_tree_pose_authority_at_angle_on_face` で
  `5 / 15 / 30 / 45 / 60 / 90 / 120 / 150 / 170` 度を試したところ、
  **18 通りすべてが `PoseAuthorityError::KinematicsUnavailable` で失敗**した。
  degree-4 頂点の閉ループは全ヒンジ同一角度では閉じないためである。
  （検証用 probe test で確認し、commit には含めていない。）
- 個別角度を与えるには `NativePoseRequest` を構築する必要があるが、
  同型は `applied_pose` の `pub(super)` かつ **field が private** であり、
  `current_non_flat_layer_order_view` からは構築できない。
- `install_flat_graph_pose_authority_on_face` は angle が `0.0` 固定であり、
  完全 flat な pose からは非平坦 evidence が得られない
  （`build_pose_dto` が all-flat を拒否する正しい挙動）。
- 既存の正規 graph 非平坦 fixture は
  `stacked_fold_read.rs` の rank-16 cycle fixture
  （`propose_current_cycle_pose_inner_with_layers` →
  `apply_stacked_fold_transaction_inner` →
  `ori_core::revalidate_current_graph_non_flat_layer_order_v1`）にのみ存在する。
  当該関数は `stacked_fold_read` の private fn であり、
  かつ同 file は本指示 §9 で変更禁止である。

**必要な最小 API（core API を増やさない案）**:

`apps/desktop/src-tauri/src/applied_pose.rs` の `tests` module に、既存の
`install_tree_pose_authority_at_angle_on_face` と同型で
**ヒンジごとの角度を受け取る** `pub(crate)` helper を 1 個追加する。

```text
pub(crate) fn install_pose_authority_with_angles(
    project: &mut ProjectState,
    angles: Vec<(EdgeId, f64)>,
    fixed_face: FaceId,
)
```

これは `#[cfg(test)]` module 内であり production API surface を増やさない。
attack surface の増加は無い（既存 helper と同じ `capture_request` →
`prepare` → `commit_prepared` 経路をそのまま使う）。
これがあれば、閉ループが閉じる角度組を与えて graph issuer positive を
proof 偽造なしで構成できる。

担当範囲外のため実装せず、提案に留める。

---

## 7. 検証結果（command 別・実測）

### 7.1 Windows

| command | 結果 |
|---|---|
| `rustfmt --edition 2024 --check <担当 file>` | exit 0、差分なし |
| `cargo fmt --all -- --check` | **fail**。差分は `crates/ori-core/src/constraint_solver.rs`、`crates/ori-core/src/constraints.rs` のみ（Codex の並行未 commit 差分。担当 file は差分 0） |
| `cargo check` / `cargo clippy` / `cargo test` | **実行不能**。`C:` の空き容量 0 byte（§8.3）。Application Control による `os error 4551` も継続 |
| `git diff --check` | 出力なし |
| `git diff --cached --check` | 出力なし |
| `git status --short` | §8.1 |
| `git config user.name` / `user.email` | `yuya` / `oltotlo79@gmail.com` |

### 7.2 WSL

環境: `Linux 6.6.87.2-microsoft-standard-WSL2 aarch64`、
同一 worktree `/mnt/c/.../ORIGAMI2`、同一 HEAD `ccc6026902513e987918cb80bf8fa08f640da2db`、
`CARGO_TARGET_DIR=/tmp/origami2-viewer-negative-matrix`

| command | 結果 |
|---|---|
| `cargo check --locked -p origami2-desktop --lib` | exit 0 |
| `cargo check --locked -p origami2-desktop --lib --tests` | exit 0 |
| `cargo clippy --locked --no-deps -p origami2-desktop --lib --all-targets --all-features -- -D warnings` | exit 0、**warning 0** |
| `cargo clippy --locked -p origami2-desktop --lib --all-targets --all-features -- -D warnings` | **fail**。`crates/ori-foldability/src/facewise.rs:1993` の `type_complexity`（Codex の並行未 commit 差分。担当 file 由来ではない） |
| `cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1` | **43 passed / 1 failed / 0 ignored / 0 measured / 626 filtered out** |

test filter で 0 件実行になった command は無い。

### 7.3 module test 44 件の内訳

新規 25 件を追加し、既存 19 件は削除・ignore 化・assertion 弱化のいずれも行っていない。

pass 43 件:

```text
a_bounded_accumulation_refuses_checked_add_overflow
a_declared_and_actual_count_mismatch_is_invalid_evidence
a_foreign_instance_project_revision_or_fingerprint_is_stale
a_material_and_folded_face_count_mismatch_is_invalid_evidence
a_non_finite_world_point_is_invalid_evidence
a_project_without_non_flat_evidence_reports_absence
a_rounded_and_exact_point_count_mismatch_is_invalid_evidence
a_second_canonical_fixture_stays_read_only_and_deterministic
a_wrong_fixed_face_or_hinge_vector_is_refused
a_zero_cell_response_is_valid_and_never_claims_a_clearance_proof
a_zero_face_or_hinge_count_is_a_resource_limit
a_zero_rational_with_a_foreign_denominator_is_refused
an_empty_request_hinge_vector_is_a_resource_limit
an_unknown_equal_or_disagreeing_face_pair_is_invalid_evidence
applied_non_flat_evidence_yields_a_read_only_view
every_dropped_world_axis_is_reached_by_a_canonical_fixture
every_error_category_serializes_two_data_free_keys
one_bit_of_world_geometry_or_exact_magnitude_changes_its_digest
the_aggregate_exact_magnitude_ceiling_is_inclusive
the_aggregate_exact_point_ceiling_is_inclusive
the_aggregate_world_point_ceiling_is_inclusive
the_cell_polygon_ceiling_is_inclusive
the_domain_separator_is_not_length_framed
the_exact_boundary_and_cell_digests_match_independently_fixed_preimages
the_exact_hash_uses_raw_magnitude_bytes_not_ascii_hex
the_exact_magnitude_budget_is_shared_and_capped
the_exact_rational_wire_form_is_canonical
the_face_digest_matches_an_independently_fixed_preimage
the_face_pair_order_ceiling_is_inclusive
the_hinge_ceiling_is_inclusive
the_horizontal_crease_fixture_reaches_dropped_z
the_live_face_registry_must_match_the_proof_exactly
the_material_face_ceiling_is_inclusive
the_overlap_cell_ceiling_is_inclusive
the_plane_axes_must_be_derived_from_the_dropped_axis
the_pose_model_id_follows_the_live_issuer_kind
the_production_preflight_accepts_the_canonical_fixture
the_safe_wire_integer_ceiling_is_inclusive
the_same_snapshot_is_byte_identical_on_every_call
the_serialized_json_ceiling_is_inclusive
the_sign_and_axis_tags_are_frozen
the_vertical_crease_fixture_reaches_dropped_x_and_y
the_world_polygon_ceiling_is_inclusive
```

fail 1 件:

```text
a_reopened_project_needs_a_fresh_instance
```

---

## 8. 失敗 1 件の原因（担当外の並行差分）

### 8.1 症状

```text
panicked at current_non_flat_layer_order_view.rs:
  the archive reopens: "選択されたプロジェクトファイルが破損しているか、対応していない形式です。"
```

`ProjectState::from_project_archive` が `PROJECT_ARCHIVE_INVALID_MESSAGE` を返す。

### 8.2 担当変更が原因でないことの根拠

1. 同 test は本作業の直前（HEAD `b4c884f` 時点、`lib.rs` が clean だった時刻）に
   **19/19 の一部として pass していた**。
2. 現在 `apps/desktop/src-tauri/src/lib.rs` に Codex の未 commit 差分
   （+50 / -30）があり、archive 復元経路の
   `revalidate_current_non_flat_layer_order_v1` 呼出しが
   `reanalyze_editor_flat_layer_order_with_required_pairs` を使う形へ書き換え途中である。
3. **Claude が一切触れていない既存 test**
   `global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected`
   も、同じ再オープン経路で同時に FAILED になっている。
4. 本 commit の変更は resource/structural validator の抽出と test 追加のみで、
   archive 復元経路には一切触れていない。

以上より、この 1 件は **担当外の in-flight 差分による一時的な破損**であり、
Claude 側の regression ではない。
指示 §9 により `lib.rs` は変更禁止のため、緑に見せる修正は行っていない。
`lib.rs` の書き換えが完了した時点で再実行を依頼したい。

### 8.3 その他の環境 blocker

- `C:` の空き容量が **0 byte**。`target` 225.7 GB に加え `target-*` が 49 個・約 190 GB。
  `target-*` は削除禁止のため Claude では対処できない。
  Windows 上の `cargo` 実行はすべて不能である。
- Windows Application Control による test binary の `os error 4551` は継続。
  したがって native 検証はすべて WSL で行った。compile 成功のみを成功扱いにしていない。

---

## 9. `git status --short`

```text
 M apps/desktop/src-tauri/src/global_flat_foldability.rs
 M apps/desktop/src-tauri/src/lib.rs
 M apps/desktop/src-tauri/src/stacked_fold_read.rs
 M apps/desktop/src-tauri/src/stacked_fold_transaction.rs
 M crates/ori-collision/src/block_composition.rs
 M crates/ori-collision/src/lib.rs
 M crates/ori-core/src/constraint_solver.rs
 M crates/ori-core/src/constraints.rs
 M crates/ori-foldability/src/facewise.rs
 M crates/ori-foldability/src/lib.rs
?? docs/Codex/
?? docs/plans/code-audit-2026-07-22.md
?? docs/plans/code-audit-round3-2026-07-23.md
?? origami2-collision-ab-verification.png
?? origami2-global-flat-foldability-panel.png
?? target-*（49 ディレクトリ）
```

上記の `M` はすべて Codex の並行差分である。Claude は 1 件も触れていない。

保護対象の確認:

- `apps/desktop/src-tauri/src/stacked_fold_read.rs` — 未変更
- `apps/desktop/src-tauri/src/stacked_fold_transaction.rs` — 未変更
- `apps/desktop/src-tauri/src/lib.rs` — 未変更
- `crates/**` — 未変更
- frontend source / test — 未変更
- `docs/Codex/**` — stage / commit していない
- `docs/plans/**`、`origami2-*.png`、`target-*` — 変更・削除・stage いずれもしていない
- 他者の未 commit 差分の整形・stage・commit — していない
- push / amend / rebase / squash / reset / restore / stash — いずれもしていない
- Git identity — 変更していない

---

## 10. 未達項目のまとめ

1. **graph issuer positive**（§6.2）。担当 file 1 個では構成不能。
   必要な最小 API と理由を §6.2 に記載した。
2. **allocation failure の実行注入**（§4 末尾）。決定的注入は行っていない。
   code path は保持し、OOM test は作っていない。
3. **count → byte の checked-mul overflow**（§4 末尾）。
   現行実装に該当する乗算が存在しないため test 対象が無い。
4. **`a_reopened_project_needs_a_fresh_instance` の 1 件 fail**（§8）。
   担当外の in-flight 差分が原因であり、修正は担当外。

したがって「全項目完了」とは書かない。

---

## 11. 訂正（2026-07-26 14:40）

### 11.1 module test の実測値

本 report §7.2 / §7.3 / §8 は「43 passed / 1 failed」と記載していたが、これは
**他担当の未 commit 差分を含む主 worktree での測定値**であり、本 commit 単体の評価としては
誤りであった。

commit `ccc6026902513e987918cb80bf8fa08f640da2db` を一時 worktree
（`git worktree add --detach <repo 外の一時 path> HEAD`）へ純粋に checkout し、
他担当の未 commit 差分を含まない状態で再実行した結果は次である。

| command（WSL、`CARGO_TARGET_DIR=/tmp/origami2-head-check`） | 結果 |
|---|---|
| `cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1` | **44 passed / 0 failed / 0 ignored / 623 filtered out** |
| `cargo clippy --locked -p origami2-desktop --lib --all-targets --all-features -- -D warnings`（依存込み） | exit 0、warning 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test --locked -p ori-collision --lib non_flat_cell_transport::tests -- --test-threads=1` | 13 passed / 0 failed |

したがって §8 で「担当外の in-flight 差分が原因」と推定した
`a_reopened_project_needs_a_fresh_instance` の失敗は、**推定ではなく確定**である。
本 commit 単体では同 test も pass する。

その後、主 worktree では失敗が 1 件から 8 件へ増えたが、これは Codex の commit
`0264f0a 姿勢更新時に古い層順証拠を失効する` により pose 更新時に
`current_layer_evidence` が失効するようになったためであり、
同差分に合わせた fixture 修正が Codex により進行中である（担当 file への外部差分）。

使用した一時 worktree は検証後に `git worktree remove --force` で削除済みであり、
repository には残していない。

### 11.2 ディスク容量に関する記述

§8.3 の「`C:` の空き容量が 0 byte」は 2026-07-26 13:47 時点の実測
（`Get-PSDrive C` の `Free = 0`、`Used` が全容量 951.6 GB と一致）であり、
その時点では事実であった。

しかしその後 `target` が 225.7 GB から 10.5 GB へ縮小し、14:35 時点では
**189.8 GiB（約 203 GB）が空いている**。したがって
「ディスク枯渇が継続中の blocker である」という記述は現状に合致しない。

一方、Windows 上で `cargo` が動作しない事実は変わっていない。原因は次に訂正する。

```text
cargo check --locked -p origami2-desktop --lib --tests
  -> could not execute process `...\build\windows_aarch64_msvc-...\build-script-build`
     Caused by: アプリケーション制御ポリシーによってこのファイルがブロックされました。 (os error 4551)
```

`target` が縮小した結果 build script が新規生成され、それを Application Control が
遮断している。すなわち Windows 側の blocker は **ディスクではなく Application Control**
である。ビルドを伴わない `cargo fmt --all -- --check` は Windows 上でも exit 0 で完走する。

### 11.3 §8.3 の置き換え

§8.3「その他の環境 blocker」は次の内容へ読み替えること。

- Windows 上の `cargo check` / `cargo clippy` / `cargo test` は
  Application Control (`os error 4551`) により実行不能。
  native 検証はすべて WSL で実施した。compile 成功のみを成功扱いにしていない。
- ディスク容量は 2026-07-26 14:35 時点で約 203 GB の空きがあり、blocker ではない。
