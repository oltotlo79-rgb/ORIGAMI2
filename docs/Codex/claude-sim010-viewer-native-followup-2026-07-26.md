# Claude 完了報告: SIM-010 非平坦 layer-order viewer 追補

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
branch: `main`
Git identity: `yuya <oltotlo79@gmail.com>`（変更なし）
remote push: 実施していない

対応した指示書:

- `docs/Claude/sim010-viewer-completion-followup-2026-07-26.md`
- `docs/Claude/sim010-viewer-native-audit-addendum-2026-07-26.md`
- `docs/Claude/sim010-viewer-native-audit-correction-2026-07-26.md`
- `docs/Claude/sim010-viewer-frontend-audit-addendum-2026-07-26.md`

この report は stage / commit していない。

---

## 1. 追加した commit

| # | hash | subject | 変更 file |
|---|---|---|---|
| 1 | `f74f74c4d7b66a0559fe424d3e42a1abcc2fb0b1` | 非平坦層順ビューの再取得境界を修正する | 下記 7 path + 誤混入 1 件（§9.1） |
| 2 | `15f9793b6e5e9d30943a36a175aa636cf7cba405` | 非平坦層順ビューのネイティブ境界を完成する | `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` |
| 3 | `b4c884f9ae58a59cafabe011f07eb2a6de0458cc` | 非平坦層順ビューの反射境界回帰を完成する | `apps/desktop/src/lib/currentNonFlatLayerOrderView.ts`, `apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx` |

author はすべて `yuya <oltotlo79@gmail.com>`。既存 3 commit
（`358eeab` / `ed73759` / `9135cf7`）は amend / rebase / squash / drop していない。

commit 1 の changed path:

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs   84 / 24
apps/desktop/src/App.css                                          15 / 0
apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx     - / -（旧 parent が binary）
apps/desktop/src/lib/currentNonFlatLayerOrderView.ts               - / -（旧 parent が binary）
apps/desktop/tests/currentNonFlatLayerOrderView.test.ts          589 / 189
apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx   398 / 21
apps/desktop/tests/stackedFoldPanel.dom.test.tsx                  88 / 0
```

commit 2 の最終 source SHA-256:

```text
79213ce7f427305ea2b687318d7c1746983d4013d98bdd297aa10b0bb5e03cfb
```

（監査 baseline は `12A38884...`。commit 2 以降にも §2〜§8 の修正を重ねたため一致しない。）

---

## 2. native 監査追補・訂正への対応

### 2.1 raw domain separator と framing（addendum §3 / correction §2）

- face、exact-boundary、cell の 3 hash はいずれも raw domain bytes から始まる。
  `frame(&mut hasher, DOMAIN)` は全廃した。
- nested `exactBoundarySha256` は length prefix なしの固定 32 byte として cell hash へ入れる。
- correction §2 に従い、face hash と exact-boundary hash の **plane-axis tag 2 個を復活**させた。
  一度削除したが、V1 byte contract の一部であるため元へ戻している。
- sign tag は `negative=0x00` / `zero=0x01` / `positive=0x02`、
  dropped-axis tag は `x=0x00` / `y=0x01` / `z=0x02` で固定。
- exact rational は sign tag + raw numerator magnitude length/bytes +
  raw denominator magnitude length/bytes。hex ASCII は hash に入らない。

hard-coded expected digest は production helper から生成していない。
Python で byte contract を再実装して独立算出した literal である。

| 種別 | expected SHA-256 |
|---|---|
| face | `309cb86e2e3f08c119aa03fcc6f237c701afdcd2ffe1e51eba2799fb7241d0d5` |
| exact boundary | `818f5643b6e3078f0b902bbfc328ab77f4dc47902637e6bf2d85776a4ae7567c` |
| cell | `25376bc0687be46f491d0d368a1c3a36e20b21d1cfcc60fa480e68fe2b64739f` |

対応 test:

- `the_face_digest_matches_an_independently_fixed_preimage`
- `the_exact_boundary_and_cell_digests_match_independently_fixed_preimages`
  （lower/upper 反転、nested digest への length prefix 付与が別 digest になることも固定）
- `the_domain_separator_is_not_length_framed`
- `the_exact_hash_uses_raw_magnitude_bytes_not_ascii_hex`
- `one_bit_of_world_geometry_or_exact_magnitude_changes_its_digest`
- `the_sign_and_axis_tags_are_frozen`
- `the_same_snapshot_is_byte_identical_on_every_call`

### 2.2 cheap preflight の順序と category（addendum §4 / correction §3）

順序を次に変更した。

1. request と current project / instance / revision / fingerprint の照合
2. current evidence 取得、`None` / `CertifiedFlat` のみ `Ok(None)`
3. proof model ID / identity namespace / target revision / target fingerprint 照合
4. **allocation を伴わない cheap preflight**（`preflight_view_resources`）
5. applied pose capability の capture / revalidate
6. proof / request / semantic pose の bit-exact 再結合
7. shared structural validator（`validate_non_flat_layer_order_structure_v1`）
8. bounded response 構築
9. response invariant と serialized JSON cap

correction §3 に従い category を分離した。

- `invalid_evidence`: material/folded coverage 不一致、declared count と実 slice 長の不一致、
  face pair と cell count 不一致、exact/rounded point count 不一致、3 点未満 polygon、
  live registry duplicate、zero rational の非正準 denominator
- `resource_limit`: faces / hinges / cells / pairs / per-polygon / aggregate points 上限超過、
  checked add overflow、exact magnitude 上限、serialized JSON 上限、
  safe-integer 上限、`try_reserve_exact` 失敗
- `stale_authority`: project / instance / revision / fingerprint / pose / hinge 再結合の不一致

`testedFacePairs` と `sourceOverlapCellsAuthenticated` は `u64` 変換と
`Number.MAX_SAFE_INTEGER` の両方で検査する。

### 2.3 world polygon の事前 cap と fallible allocation（addendum §5）

`reserved_world_points` を追加し、tree / graph の両分岐で

1. 既知 vertex count を取得
2. 3 点未満は `invalid_evidence`、4,096 超過は `resource_limit`
3. `try_reserve_exact` で確保（失敗は `resource_limit`）
4. canonical walk 順のまま world point を push

の順にした。cap 判定前の `collect::<Vec<_>>()` は無い。reverse / rotate / dedup もしない。
`-0.0` の wire-copy canonicalization と finite check は維持している。

### 2.4 exact zero の canonical denominator（addendum §6）

`exact_rational_dto` は `sign = zero` のとき denominator が `[0x01]` 以外を拒否する。
test `a_zero_rational_with_a_foreign_denominator_is_refused` が
`[0x02]` / `[0x01,0x00]` / `[0x00]` / 空 を個別に拒否し、
`sign=zero` かつ numerator 非空、`sign=positive` かつ numerator 空も拒否する。

### 2.5 request の typed ID・negative zero・raw bit 比較（addendum §7）

- request DTO を `ProjectId` / `FaceId` / `EdgeId` へ変更した。
- `canonical_finite(requested.angle_degrees)` を比較前に呼ぶ処理を削除し、
  proof / request / semantic を `to_bits()` の生値で比較する。
- `validate_request_hinge_vector` を追加し、hinge count `1..=4096`、
  canonical code-unit 順、duplicate 拒否、finite、`-0.0` 拒否、`0..=180`、
  少なくとも 1 個が非 flat を invoke 経路上で検査する。

対応 test: `a_wrong_fixed_face_or_hinge_vector_is_refused`,
`an_empty_request_hinge_vector_is_a_resource_limit`。

### 2.6 live face registry（addendum §8）

`dedup()` を削除した。現在は

1. proof material face を canonical bytes 順に sort し `windows(2)` で duplicate を拒否
2. live slice を borrow し、`live.len() == proof_face_count` を allocation 前に検査
3. `try_reserve_exact` で確保
4. sort 後に `windows(2)` で live duplicate を拒否
5. proof vector と exact 比較

### 2.7 issuer kind の判定（監査で判明した追加事実）

`CurrentAppliedPoseView` は tree と graph の **両方**の projection を同時に公開する。
そのため `(Some, None)` / `(None, Some)` による判定は常に `internal_failure` になる。
正しい authority は `semantic_pose().model_id()` であり、`PoseIssuerKindV1` を導入して

- pose model ID
- live face registry の選択
- world boundary の walk

の 3 箇所を同じ issuer kind から導出するようにした。
test `the_pose_model_id_follows_the_live_issuer_kind` で固定している。

---

## 3. frontend 追補への対応

`docs/Claude/sim010-viewer-frontend-audit-addendum-2026-07-26.md` のうち、
§3（1 snapshot reflection）と §4.2 の一部は commit `658d516`（別担当）で既に入っていたため
二重実装していない。commit 3 では未達分だけを実装した。

### 3.1 malformed stable request と absence の分離（§4.1）

監査時点の client は、hostile / malformed な stable request も
`null`（evidence absence）へ軟化していた。これを次へ変更した。

- `absent`（invoke 0、absence 表示）: `state !== 'stable'`、`fixedFaceId === null`、
  project ID mismatch、revision mismatch、hinge 0 件、完全 flat な hinge vector
- `malformed`（invoke 0、data-free `invalid_evidence`）: shape 不正、
  非 canonical UUID（instance / project / fixed face / edge）、fingerprint 不正、
  unsafe revision、hinge entry の own field 不一致、非 finite、`-0`、範囲外、
  duplicate edge、hinge cap + 1、accessor / throwing trap / revoked Proxy

**仕様との差異（明記）**: 追補 §4.1 は hinge count を `1..=4096` と規定しているが、
hinge 0 件は「まだ折られていない model」という正常状態であり、
これを `invalid_evidence` にすると既存の公開挙動を壊す。したがって
hinge 0 件と完全 flat vector は absence のままにし、cap + 1 のみ malformed とした。
追補 §1 の「より厳しく fail-closed かつ既存挙動を壊さない条件」に従った判断である。

### 3.2 DOM matrix の case ID（§9）

`apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx` は 68 test。

| case ID | test 名 |
|---|---|
| UI-LIFE-01 | `UI-LIFE-01 shows the loading status before the response settles` |
| UI-LIFE-02 | `UI-LIFE-02 reports absence when the project owns no non-flat evidence` |
| UI-LIFE-03 | `UI-LIFE-03 renders both panes read-only without mutation controls` |
| UI-LIFE-04..07 | `UI-LIFE-0{4,5,6,7} maps the <category> category to a closed failure message` |
| UI-LIFE-08 | `UI-LIFE-08 never shows a raw native error string` |
| UI-LIFE-09 | `UI-LIFE-09 refuses an undefined native response as invalid evidence` |
| UI-SRC-01..09 | `UI-SRC-0N ...`（locale 不再取得 / 同値 new source / 1-bit hinge / old pixel 同期消去 / late response / A-B-A / null 同期非表示 / unmount resolve / unmount reject） |
| UI-GATE-01..06 | absence gate（running / blocked / indeterminate / project / revision / null fixed face） |
| UI-GATE-07..12 | malformed（duplicate / 不正 edge UUID / NaN / +Inf / -Inf / `-0` / 範囲外 / 負 / cap + 1） |
| UI-GATE-14,15 | absence gate（hinge 0 件 / 完全 flat） |
| UI-GATE-16..20 | malformed root（instance UUID / project UUID / unsafe revision / 大文字 fingerprint / 短い fingerprint） |
| UI-BIND-01..09 | `UI-BIND-0N ...`（fixed face / missing / extra / edge ID / 1-bit angle / instance / project / revision / fingerprint） |

reflection counter 0 を assertion する test:

- `never executes a source accessor`
- `never executes a request array index accessor`
- `never executes a request hinge field accessor`
- `fails closed for a revoked request source Proxy`

いずれも `reads === 0`、`invoke` 0 回、`invalid_evidence` 表示を同時に確認する。

### 3.3 literal NUL と text numstat（§7）

```text
node -e "...": NUL bytes: 0

git diff --numstat f9913149b69ad1bc83d89681aa9309b986063cc5..HEAD -- <2 path>
336  0  apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
824  0  apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
```

両 path とも text diff として表示される。`--text` による強制表示はしていない。
commit 1 自体の numstat が `- -` になるのは、旧 parent blob 側に NUL があるためで、
新 blob には NUL が 1 byte も無い。

---

## 4. 検証結果（command 別実測）

### 4.1 Windows

| command | 結果 |
|---|---|
| `cargo check --locked -p origami2-desktop --lib` | exit 0 |
| `cargo check --locked -p origami2-desktop --lib --tests` | exit 0 |
| `rustfmt --edition 2024 --check <viewer file>` | exit 0（差分なし） |
| `cargo fmt --all -- --check` | **fail**（§9.2 参照。担当外 file のみ） |
| `cargo clippy --locked -p origami2-desktop --lib -- -D warnings` | exit 0、warning 0 |
| `cargo test -p origami2-desktop --lib` | **実行不能**（§9.3 参照） |
| `npx tsc -b --pretty false` | exit 0 |
| `npm run lint` | exit 0、warning 23（すべて担当外 file。scope 内 0） |
| `npx oxlint --max-warnings=0 <scope 4 file>` | exit 0、warning 0 |
| `npm run build` | exit 0（chunk size の情報 warning 1） |
| `node --test tests/currentNonFlatLayerOrderView.test.ts tests/currentNonFlatLayerOrderViewerText.test.ts tests/tauriCapabilityContract.test.ts` | 120 pass / 0 fail |
| `npx vitest run tests/currentNonFlatLayerOrderViewer.dom.test.tsx` | 68 pass / 0 fail |
| `npx vitest run tests/stackedFoldPanel.dom.test.tsx` | 50 pass / 0 fail |
| `npm run test:snap` | 2,006 pass / 0 fail |
| `npm run test:dom` | 61 file / 491 pass / 0 fail（`--localstorage-file` warning 3） |
| `git diff --check` | 出力なし |
| `git diff --cached --check` | 出力なし |

viewer 単独 68 件と panel 単独 50 件は別集計である。合計を単独件数として報告していない。

### 4.2 WSL（Ubuntu / aarch64、`CARGO_TARGET_DIR=/tmp/ori-wsl-target`）

Windows Application Control が新規 link された test binary を `os error 4551` で遮断するため、
native test は WSL 上で同一 worktree・同一 commit を実行した。

| command | 結果 |
|---|---|
| `cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1` | **19 pass / 0 fail** |
| `cargo test --locked -p ori-collision --lib non_flat_cell_transport::tests -- --test-threads=1` | **13 pass / 0 fail** |
| `cargo clippy --locked -p origami2-desktop --lib --all-targets --all-features -- -D warnings` | exit 0、warning 0 |

WSL 実行時の kernel: `6.6.87.2-microsoft-standard-WSL2 aarch64`。
GitHub Actions の run URL は本環境から取得していない（push 禁止のため CI job を起動していない）。

### 4.3 native unit test 一覧（19 件）

```text
applied_non_flat_evidence_yields_a_read_only_view
the_pose_model_id_follows_the_live_issuer_kind
the_same_snapshot_is_byte_identical_on_every_call
the_exact_rational_wire_form_is_canonical
a_zero_rational_with_a_foreign_denominator_is_refused
the_exact_magnitude_budget_is_shared_and_capped
the_face_digest_matches_an_independently_fixed_preimage
the_exact_boundary_and_cell_digests_match_independently_fixed_preimages
the_domain_separator_is_not_length_framed
the_exact_hash_uses_raw_magnitude_bytes_not_ascii_hex
one_bit_of_world_geometry_or_exact_magnitude_changes_its_digest
the_sign_and_axis_tags_are_frozen
a_foreign_instance_project_revision_or_fingerprint_is_stale
a_wrong_fixed_face_or_hinge_vector_is_refused
an_empty_request_hinge_vector_is_a_resource_limit
a_zero_cell_response_is_valid_and_never_claims_a_clearance_proof
a_project_without_non_flat_evidence_reports_absence
every_error_category_serializes_two_data_free_keys
a_reopened_project_needs_a_fresh_instance
```

fixture は module 内の `centered_single_hinge_project` と
`applied_pose::tests::install_tree_pose_authority_at_angle_on_face`、
`global_flat_foldability::reanalyze_current_flat_layer_order`、
`ori_core::revalidate_current_non_flat_layer_order_v1` だけを使用している。
`unsafe` による private constructor の迂回、test 専用の production capability 追加、
serialization round-trip による proof 偽造は行っていない。

---

## 5. data-free error matrix

`every_error_category_serializes_two_data_free_keys` が 4 category すべてで
serialized own key が `version` と `category` の 2 個だけであることを固定する。

```json
{"version":1,"category":"stale_authority"}
{"version":1,"category":"invalid_evidence"}
{"version":1,"category":"resource_limit"}
{"version":1,"category":"internal_failure"}
```

frontend 側も `UI-LIFE-08` と `UI-GATE-07..20` で、raw native error 文字列、
edge ID、exact magnitude が DOM に出ないことを確認している。

---

## 6. `git status --short`（`target-*` を除く）

```text
 M apps/desktop/src-tauri/src/stacked_fold_read.rs
 M apps/desktop/src-tauri/src/stacked_fold_transaction.rs
?? docs/Codex/
?? docs/plans/code-audit-2026-07-22.md
?? docs/plans/code-audit-round3-2026-07-23.md
?? origami2-collision-ab-verification.png
?? origami2-global-flat-foldability-panel.png
```

`stacked_fold_read.rs` と `stacked_fold_transaction.rs` の未 commit 差分は
Codex の並行作業であり、Claude は触っていない（§9.4 参照）。
`docs/plans/**`、`origami2-*.png`、`target-*` は変更・削除・stage していない。

---

## 7. 未実施項目（完了と書けない項目）

以下は **未達**である。

### 7.1 §8/§9 の apply / 永続境界

`docs/Claude/sim010-viewer-native-audit-correction-2026-07-26.md` §4 により
`stacked_fold_transaction.rs` と `stacked_fold_read.rs` は Codex 担当へ移管された。
したがって Claude は次を実施していない。

- `apply_stacked_fold_transaction_inner` の evidence install 分岐
- `apply_dyadic_pose_path_preview_inner_v1` 後の current evidence install
- `four_hinge_tree_level_three_proof_applies_and_persists_atomically` の緑化
- `archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected` の拡張

**確認済みの事実（Codex 向け情報）**:

`stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically`
は現在 **失敗している**。失敗地点は
`apps/desktop/src-tauri/src/stacked_fold_read.rs:7257` 付近の

```rust
assert!(matches!(
    project.current_layer_evidence,
    Some(...CurrentLayerEvidence::NonFlat(_))
));
```

原因は `apply_dyadic_pose_path_preview_inner_v1` が
`current_layer_evidence` を一切設定しないことである。
`stacked_fold_transaction.rs:1965` の分岐（`applied_layer_order` を見る match）は
stacked-fold transaction 経路のもので、dyadic path preview 経路には効かない。

移管前に検証した修正案（一度実装し、担当移管により revert 済み）:
`restore_persisted_current_pose` 成功後、`slot.take()` の直前に
`revalidate_current_non_flat_layer_order_v1` →
`revalidate_current_graph_non_flat_layer_order_v1` の順で fresh proof を solve し、
成功時のみ `CurrentLayerEvidence::NonFlat` を install する。
失敗時は evidence を `None` にして partial success を残さない。

### 7.2 forge が必要な cap / registry negative case

addendum §4.3、§8、§10 の次の case は **未実装**である。

```text
face cap + 1 / folded face cap + 1 / hinge cap + 1 / cell cap + 1 / pair cap + 1
per-cell polygon cap + 1 / total world point cap + 1 / total exact point cap + 1
serialized JSON cap + 1 / checked arithmetic overflow / allocation failure path
live face missing / extra / duplicate / foreign
material/folded coverage mismatch / unknown・equal・reversed face pair
exact/rounded bit mismatch / dropped axis mismatch / nonfinite world point
graph issuer positive / dropped X・Y の positive
```

理由: これらはいずれも `StackedFoldNonFlatLayerOrderV1` の内部を
正規の revalidation 経路では作れない値へ差し替える必要がある。
同型は全 field が private で、正規 constructor は core の証明経路のみである。
指示は `unsafe` による private constructor 迂回と、test 専用 production capability の
追加を明示的に禁じているため、現状の API では到達できない。

到達させるには次のいずれかが必要である（Codex 判断を要請）。

1. `ori-core` に `#[cfg(any(test, feature = "test-fixtures"))]` の
   proof builder を追加する。
2. viewer 側の cap 検査 helper（`preflight_view_resources` 等）を
   proof ではなく count/slice を引数に取る純関数へ切り出し、
   その純関数へ直接 cap + 1 を与える。

現在の `preflight_view_resources` は `&StackedFoldNonFlatLayerOrderV1` を取るため、
2 の切り出しを行えば cap 系 8 case は proof 偽造なしで到達できる。
担当範囲外の判断になるため、実装せず提案に留める。

### 7.3 dropped X / Y の positive、graph positive

使用した単一 hinge fixture では dropped axis が `y` と `x`（面ごと）になり、
`z` を含む 3 軸すべてと graph issuer の positive は同一 fixture で得られない。
`assert` は「3 軸のいずれか」と「axis から plane axes が正しく導出されること」を
固定しているが、3 軸個別の positive fixture は未実装である。

---

## 8. 保護対象の確認

- `docs/progress.md`、`docs/requirements-status.md`、`docs/plans/**` を変更していない。
- `docs/Codex/**` を stage / commit していない。
- `origami2-*.png` を変更・削除・stage していない。
- `target-*` を作成・削除・stage・commit していない。
- `crates/**`、`apps/desktop/src-tauri/src/lib.rs`、
  `apps/desktop/tests/tauriCapabilityContract.test.ts` を変更していない。
- `apps/desktop/src/components/StackedFoldPanel.tsx`、`App.tsx` を変更していない。
- remote push を行っていない。
- Git identity を変更していない。

---

## 9. 環境・運用上の blocker と事故報告

### 9.1 commit 1 への docs file 誤混入（要確認）

commit `f74f74c` に

```text
docs/Claude/sim010-viewer-native-audit-addendum-2026-07-26.md  673 / 0
```

が混入している。`git add` では 7 path のみを指定し、直前の
`git diff --cached --name-only` でも 7 path しか出ていなかったが、
commit 実行時点で index に当該 file が入っていた（並行 agent による stage と推定）。

指示の禁止 stage 対象（`docs/Codex/**`、`docs/plans/**`、`origami2-*.png`、`target-*`）
には該当しないが、意図した変更ではない。
amend / reset は禁止されているため訂正していない。処置が必要なら Codex 側で判断されたい。

### 9.2 `cargo fmt --all -- --check` の失敗

失敗しているのは次の担当外 file だけである。

```text
crates/ori-core/src/constraint_solver.rs:1458
crates/ori-core/src/constraints.rs:6567
crates/ori-core/src/constraints.rs:6950
```

いずれも Codex の並行未 commit 差分であり、Claude は触れていない。
担当 file (`current_non_flat_layer_order_view.rs`) は
`rustfmt --edition 2024 --check` で差分 0 である。

### 9.3 Windows Application Control による test binary 遮断

新規 link された `origami2_desktop_lib-*.exe` の実行が
`os error 4551`（アプリケーション制御ポリシーによってブロック）で失敗する。
別ディレクトリへ copy しても同じく遮断される。
このため native test は WSL で実行した（§4.2）。compile 成功のみを成功扱いにしていない。

### 9.4 ディスク枯渇（重要）

作業中に `C:` の空き容量が **0 byte** になり、
`cargo clippy` が `os error 112`（ディスク空き領域不足）で失敗した。

内訳（実測）:

```text
target             225.7 GB
target-* 合計       約 190 GB（49 ディレクトリ）
```

`target-*` は指示により削除禁止のため Claude では対処できない。
以後の Windows 上の Rust build を継続するには、
不要な `target-*` の削除など、ユーザーまたは Codex による容量確保が必要である。

### 9.5 共有 worktree の並行編集

作業中、担当 file が外部から commit された。

- `658d516bb2f51880583f9ead58dae29087c87501`
  「非平坦層順ビューの敵対入力境界を厳格化する」
  （`currentNonFlatLayerOrderView.ts` / 同 test / viewer DOM test）

追補 §2 に従い、reset / restore / checkout / amend は行わず、
既に同等以上の修正が入っていた項目（1 snapshot reflection、request detach の
descriptor 化）は二重実装せず、未達分のみを commit 3 で実装した。
既存 test を弱めた箇所は無い。hostile source 4 test の期待値のみ、
`absence` → `invalid_evidence` へ **強化**する方向で更新している。
