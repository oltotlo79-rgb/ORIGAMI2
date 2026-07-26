# Claude 追加訂正指示: SIM-010 native viewer の残存 blocker

作成日: 2026-07-26  
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`  
対象 branch: `main`  
remote push: 禁止

## 1. この指示を追加した理由

`docs/Claude/sim010-viewer-native-audit-addendum-2026-07-26.md` に基づく未 commit 修正を再監査した。監査時点の
`apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` は SHA-256
`12A388843A99AA3657D759412EACB3B961B7A1494EC118D5F767322ED52F96FD` であり、
次は解消済みである。

- typed `ProjectId` / `FaceId` / `EdgeId`
- request `-0.0` 拒否と raw `to_bits()` 比較
- zero rational denominator `[0x01]`
- viewer cheap preflight の shared validator より前への移動
- live face registry の duplicate 拒否
- world polygon の allocation 前 cap と fallible reservation
- raw domain separator
- nested digest の raw 32 byte framing
- response 全体で共有する exact magnitude budget

`cargo check --locked -p origami2-desktop --lib` は成功した。ただし以下は commit blocker として残る。

### 1.1 追試で native unit 6件失敗を確認

この指示作成後に追加された native unit test を、Windows Application Controlを迂回する
WSLで実行した。

```bash
CARGO_TARGET_DIR=/tmp/origami2-audit-desktop \
  cargo test -p origami2-desktop --lib current_non_flat_layer_order_view -- --nocapture
```

実測は **8件中2件成功、6件失敗** である。次の6件が失敗した。

```text
a_wrong_fixed_face_or_hinge_vector_is_stale
a_reopened_project_needs_a_fresh_instance_and_fresh_proof
applied_tree_evidence_yields_a_read_only_view
the_exact_zero_rational_uses_the_canonical_denominator
the_same_snapshot_is_byte_identical_on_every_call
undo_and_redo_leave_no_viewable_evidence
```

主な実測category:

- positive view、zero rational、repeat、Undo/Redo fixture: `internal_failure`
- reopened fresh instance fixture: `stale_authority`
- forged fixed-face/hinge fixture: testが期待したcategory集合外

2件だけ通った状態を完了扱いしない。上記exact commandを **8/8以上** へ直し、さらに本書の
不足matrixを追加した後にだけcommitする。

### 1.2 追加された四hinge helperは元のtransaction回帰ではない

未 commit の `stacked_fold_read::tests::four_hinge_tree_non_flat_evidence_state` は
`apply_dyadic_pose_path_preview_inner_v1` を通る。一方、本書 §4 のproduction修正は
`apply_stacked_fold_transaction_inner` の `project.current_layer_evidence` install境界である。
このhelperだけではtarget geometry付きstacked-fold transactionの回帰にならない。

`stacked_fold_read.rs` の既存
`assert_two_hinge_projective_schedule_round_trip` は、
`propose_current_stacked_fold_read_inner` から
`apply_stacked_fold_transaction_inner` までを実際に通す。target geometry付きNonFlat Applyの
回帰にはこの実経路を使い、Apply成功直後のproof binding、Apply直後archiveのfresh revalidation、
wrapper Undo/Redo後のNone、Redo後archiveのNoneを固定する。

dyadic applyにもfresh evidenceを保存する変更を加える場合、それは別契約である。
best-effort re-solve失敗をApply成功+Noneへ落とすのか、evidenceをatomic apply条件にするのかを
明記し、stacked-fold transactionの回帰を代用しない。

## 2. hash preimage から plane-axis tag を削除しない

現行未 commit 差分は raw domain separator へ直す過程で、face hash と
exact-boundary hash から次を削除している。

```rust
for tag in plane {
    frame(&mut hasher, tag.as_bytes());
}
```

plane axes は dropped axis から導出可能でも、V1 の byte contract に含まれる。
「意味的に冗長」を理由に preimage から省略してはならない。

必須:

1. face hash は raw `FACE_DOMAIN_V1` で開始する。
2. 可変長 ID は `u64` big-endian length + raw bytes で frame する。
3. world point count、canonical binary64 XYZ、dropped-axis tag の後に、
   2 個の plane-axis tag を順番どおり length-frame する。
4. その後に 6 個の exact affine rational を、sign tag、raw numerator
   magnitude length/bytes、raw denominator magnitude length/bytesで frame する。
5. exact-boundary hash も raw domain、dropped-axis tag、2 個の plane-axis
   tag、point count、各 exact UV rational の順を原指示 §5.6 と完全一致させる。
6. cell hash は raw domain、lower face ID frame、upper face ID frame、
   fixed 32 byte exact-boundary digest の順とし、nested digestにlength prefixを付けない。

production helper と同じ helper を expected 側で再利用するだけの自己一致 test は不可。
独立に固定した hard-coded expected SHA-256 を face、exact-boundary、cell の3種類で置く。
さらにrepeat byte-identical、world f64 1 bit、exact magnitude 1 bit、axis/plane tag、
lower/upper IDの各変化で該当digestだけが変わることを固定する。

## 3. cheap preflight の category を混同しない

cheap preflight を先へ移したこと自体は正しい。しかし、現在は次を一律
`resource_limit` にしている。

- `folded_faces.len() != material_faces.len()`
- declared overlap/pair count と実slice lengthの不一致
- exact boundary と rounded boundary のpoint count不一致
- polygonが3点未満

これは資源超過ではなく evidence の構造不整合である。次へ分離する。

### `invalid_evidence`

- material/folded coverage count不一致
- declared countと実slice length不一致
- face pairとcell count不一致
- exact/rounded boundary point count不一致
- 3点未満polygon
- duplicate/missing/extra/out-of-order structural content

### `resource_limit`

- faces、hinges、cells、pairs、per-polygon points、aggregate points の上限超過
- checked add/mul overflow
- exact magnitude、serialized JSON、safe integer上限の超過
- `try_reserve_exact` failure
- shared validatorを呼ぶ前に判明するviewer cap超過

### `stale_authority`

- current project/instance/revision/fingerprint、pose generation/model、
  fixed face、hinge ID/order/angle bits の再結合不一致

各categoryのserialized errorは引き続き `version` と `category` の2 fieldだけとする。
同じfixtureに stale と structural tamper が共存する場合を含め、原指示 §7.2 の
優先順をtestで固定する。

## 4. persistence file は現在 Codex 担当なので触らない

次の2 fileはCodexが並行して §8 を修正中である。競合防止のためClaudeは変更、
stage、commitしない。

```text
apps/desktop/src-tauri/src/stacked_fold_transaction.rs
apps/desktop/src-tauri/src/stacked_fold_read.rs
```

Claudeの担当は次に限定する。

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

他fileが本当に必要になった場合は変更せず、`docs/Codex` の報告へ exact blocker と
必要pathを書く。

## 5. native module test を実装する

現行moduleは production codeだけで `#[test]` が0件である。少なくとも次を同moduleの
unit testとして実装する。

- tree positive / graph positive と正しいpose model ID
- dropped X / Y / Z
- face/exact-boundary/cell hard-coded digest
- repeat determinism と各1-bit mutation
- foreign instance/project/revision/fingerprint
- proof/request/semanticのmissing/extra/out-of-order/duplicate hinge
- angle 1 bit mismatchとrequest `-0.0`
- live face missing/extra/duplicate
- zero denominator `[0x02]`
- max inclusive / cap + 1: faces、hinges、cells、pairs、polygon、
  aggregate world/exact points、exact magnitude、serialized JSON
- `invalid_evidence` / `resource_limit` / `stale_authority` /
  `internal_failure` のdata-free serialized shape

private constructorを `unsafe` で迂回しない。既存の正規revalidation fixtureを使い、
必要なら同module内のproduction helperを小さく分離する。

Windows Application Control によりtest binary実行が `os error 4551` で遮断される場合も、
compileだけで成功扱いにしない。WSLまたはGitHub ActionsのLinux jobで同じexact commitを
実行し、run URL / job / commit SHAを報告する。

## 6. 必須検証

```powershell
cargo fmt --all -- --check
cargo check --locked -p origami2-desktop --lib
cargo clippy --locked -p origami2-desktop --lib --all-targets --all-features -- -D warnings
cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view
git diff --check
git status --short
git config user.name
git config user.email
```

Git identity:

```text
yuya
oltotlo79@gmail.com
```

## 7. commit と禁止事項

変更は `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` だけをexact stageし、
他者のstageを巻き込まないよう次の形式でcommitする。

```powershell
git commit --only -m "非平坦層順ビューのネイティブ境界を完成する" -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

禁止:

- push
- amend / rebase / squash / reset
- Git identity変更
- `docs/Codex/**` のstage/commit
- `docs/plans/**`、`origami2-*.png`、`target-*` の変更・削除・stage
- Codex担当のpersistence 2 fileへの変更
- test未実行を成功として報告
- hard-coded digest testのexpectedをproduction helperから生成

完了後は新規 `docs/Codex/claude-sim010-viewer-native-followup-2026-07-26.md`
へ、commit hash、変更path、各case-to-test対応、command別pass数、warning数、
Windows遮断の有無と代替Linux/WSL証拠、残件を正確に記録する。報告書自体はstageしない。
