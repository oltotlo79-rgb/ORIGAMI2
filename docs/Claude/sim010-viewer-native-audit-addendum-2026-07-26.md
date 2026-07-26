# Claude 実装追補: SIM-010 non-flat viewer native 境界の独立監査残件

対象指示書:

- `docs/Claude/sim010-non-flat-layer-viewer-2026-07-26.md`
- `docs/Claude/sim010-viewer-blocker-resolution-2026-07-26.md`

対象報告:

- `docs/Codex/claude-sim010-viewer-completion-2026-07-26.md`

この追補は、上記完了報告とその後の未 commit native 差分を Codex が独立監査した結果である。元指示書の要件を弱めず、ここに列挙する残件をすべて解消すること。

## 0. 評価基準と監査時点

監査対象の未 commit file:

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

監査時点の SHA-256:

```text
49CF3DB5F6734C950CB4C8BAD98FCE5318321A30FE6F01CCE628D2EB2BD9DEAA
```

監査時点の差分量:

```text
84 insertions / 24 deletions
```

同 hash は複数回の再取得で不変だった。次の compile check も成功した。

```powershell
cargo check --locked -p origami2-desktop --lib
```

ただし、compile 成功は意味上の受入条件を満たしたことを示さない。本追補に記載する semantic blocker と test 欠落を解消するまで、native viewer を完了と報告しないこと。

## 1. 監査で解消済みと確認した項目

次は監査 hash の差分で解消済みである。後続修正で後退させないこと。

1. proof の `model_id()`、project identity namespace、target revision、target fingerprint を current project と照合する。
2. proof の complete hinge vector と revalidated semantic pose を、件数、edge ID、`f64::to_bits()` で照合する。
3. semantic pose model と live issuer kind を照合し、tree と closed graph で異なる pose model ID を返す。
4. proof material face registry と live tree/graph face registry を比較する境界が追加された。
5. exact rational の hash input は DTO の ASCII hexadecimal ではなく raw magnitude bytes を使用する。
6. face affine と cell boundary の exact magnitude byte budget は一つの accumulator で集計する。
7. `readOnly = true`、`authorizesProjectMutation = false`、data-free error payload の既存設計は維持されている。

ただし、4 は duplicate を `dedup()` で消してしまう残件がある。5 は domain separator の framing が未修正である。以下の各節に従って完成させること。

## 2. Git、所有権、保護対象

既存 commit は amend、rebase、squash、作り直しをしない。

```text
358eeabcd69fa9a5eff39f8cf8694ae36cbeb131
ed7375938e8fb6c05623de5296be222bdf2c8cd5
9135cf7acf9c0ae44251e40f0ea8a915d38a7b9f
```

作業開始時と各 commit 前に次を確認する。

```powershell
git config --local user.name
git config --local user.email
```

期待値:

```text
yuya
oltotlo79@gmail.com
```

次を厳守する。

- direct push を行わない。
- 担当外の変更を restore、stage、commit しない。
- `docs/Codex/**` は外部報告であり、stage、commit、restore しない。
- `docs/plans/code-audit-2026-07-22.md` を変更しない。
- `docs/plans/code-audit-round3-2026-07-23.md` を変更しない。
- `origami2-collision-ab-verification.png` を変更しない。
- `origami2-global-flat-foldability-panel.png` を変更しない。
- `target-*` を作成、削除、stage、commit しない。
- `crates/ori-core/src/constraint_solver.rs` と `crates/ori-core/src/constraints.rs` の並行差分に触れない。
- frontend の並行差分へ変更を重ねない。
- `docs/progress.md`、`docs/requirements-status.md`、`docs/stacked-fold-design.md` を変更しない。
- `SIM-010` や全体完成度を独断で更新しない。

この追補で変更可能な production/test file は次に限定する。

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
apps/desktop/src-tauri/src/stacked_fold_transaction.rs
apps/desktop/src-tauri/src/stacked_fold_read.rs
```

archive regression を既存 fixture へ追加するために本当に必要な場合だけ、次も変更してよい。

```text
apps/desktop/src-tauri/src/global_flat_foldability.rs
```

handler 登録は既に自動 contract test を通っている。次は変更しない。

```text
apps/desktop/src-tauri/src/lib.rs
apps/desktop/tests/tauriCapabilityContract.test.ts
```

新しい public constructor、test-only production capability、unsafe layout access、serialization round-trip による proof 偽造を追加しない。

## 3. deterministic hash の raw domain separator

### 3.1 残る不具合

exact rational magnitude は raw bytes へ修正済みだが、3 種の domain separator は依然として次の形で `frame` されている。

```rust
frame(&mut hasher, FACE_DOMAIN_V1);
frame(&mut boundary_hasher, EXACT_BOUNDARY_DOMAIN_V1);
frame(&mut cell_hasher, CELL_DOMAIN_V1);
```

`frame` は先に `u64` length を書くため、hash preimage の先頭 byte は domain separator ではない。元指示書 §5.6 の「domain separator を先頭に置く」と一致しない。

### 3.2 必須修正

各 hash の最初の update は raw domain bytes にする。

意味上は次の形である。

```rust
hasher.update(FACE_DOMAIN_V1);
boundary_hasher.update(EXACT_BOUNDARY_DOMAIN_V1);
cell_hasher.update(CELL_DOMAIN_V1);
```

その後の variable-length field だけを `u64` big-endian length と bytes で frame する。

- canonical wire ID: length + UTF-8 bytes
- exact numerator magnitude: length + raw magnitude bytes
- exact denominator magnitude: length + raw magnitude bytes
- count: `u64` big-endian
- finite `f64`: canonicalized wire copy の `to_bits().to_be_bytes()`
- `exactBoundarySha256` を cell hash へ入れるとき: raw 32 bytes

sign tag は次で固定し、test で凍結する。

```text
negative = 0x00
zero     = 0x01
positive = 0x02
```

dropped-axis tag も次で固定する。

```text
x = 0x00
y = 0x01
z = 0x02
```

hash helper が DTO の hex string を入力に取る形へ戻らないこと。

### 3.3 必須 regression

同じ production helper をそのまま呼んで expected 値を作る test は不十分である。独立に算出した lowercase 64-character hex を fixture の expected literal として hard-code する。

最低限:

- one tree face の `faceKeySha256`
- one exact triangle の `exactBoundarySha256`
- one directed pair の `cellKeySha256`
-同じ fixture を 2 回実行して byte-identical JSON/digest
- world `f64` の 1 bit 変更で face digest が変化
- exact numerator raw byte の 1 bit 変更で exact-boundary digest が変化
- lower/upper の向きを反転すると cell digest が変化
- ASCII hex `"01"` を hash した旧 preimage と raw byte `0x01` の新 preimageが同一でない
- domain の `u64` length prefix を残した旧 preimageと新 preimageが同一でない

## 4. cheap resource preflight を heavy structural validation より前へ移す

### 4.1 残る不具合

監査時点では次の順序である。

```text
validate_non_flat_layer_order_structure_v1(proof)
preflight_view_resources(proof)
```

shared structural validator は face/folded/cell/pair の `HashSet` を構築し、全 cell を走査する。core default が許す大きな evidence を viewer 上限 4,096 で拒否する前に、重い allocation と work を実行してしまう。

### 4.2 必須順序

project lock を保持したまま、少なくとも次の順序にする。

1. request の current project instance、project ID、revision、fingerprint を照合。
2. current evidence を取得。
3. `None` / `CertifiedFlat` だけを `Ok(None)` とする。
4. proof model、project namespace、target revision、target fingerprint を照合。
5. allocation を伴わない cheap viewer preflight。
6. current applied pose capability を capture/revalidate。
7. proof/request/semantic/native pose を照合。
8. shared structural validator。
9. bounded world/exact response 構築。
10. response invariant と final JSON byte cap。

cheap preflight は大きな clone、`HashSet`、response `Vec` を作らず、borrowed slice の長さと checked accumulation だけを使う。

最低限検証する count:

- material faces: `1..=512`
- folded faces: material face count と一致し `<= 512`
- hinges: `1..=4,096`
- overlap cells: `0..=4,096`
- face-pair orders: cell count と一致し `<= 4,096`
- 各 cell rounded/exact boundary: 点数一致かつ `3..=4,096`
- total exact points: checked sum で `<= 100,000`
- `testedFacePairs` と `sourceOverlapCellsAuthenticated`: 既存 core work bound の範囲

cap 超過と checked overflow は `resource_limit`。binding mismatch を `invalid_evidence` へ落とさない。

### 4.3 必須 regression

- face cap + 1
- folded face cap + 1
- hinge cap + 1
- cell cap + 1
- pair cap + 1
- one-cell polygon cap + 1
- total exact point cap + 1
- checked sum overflow fixture
- cap + 1 が shared structural validator の heavy pathへ進む前に `resource_limit`
- cap boundary exactly equal は truncate せず成功

## 5. world polygon の事前 cap と fallible allocation

### 5.1 残る不具合

tree boundary と graph vertex walk は、現在 `Vec` へ collect した後に `3..=4,096` を検証している。既知 slice length を allocation 前に検証すること。

### 5.2 必須修正

tree:

1. `model.face_boundary(face_id)` を取得。
2. `boundary.vertices().len()` を `3..=4,096` で検証。
3. `try_reserve_exact` で output を予約。
4. canonical walk 順に world point を追加。

graph:

1. `geometry.face_boundary_vertices(face_id)` を取得。
2. `vertices.len()` を `3..=4,096` で検証。
3. `try_reserve_exact` で output を予約。
4. canonical walk 順に world point を追加。

次を守る。

- cap 超過前に `collect::<Vec<_>>()` しない。
- allocation failure を panic、abort、partial polygon にしない。
- allocation failure は data-free `resource_limit` へ閉じる。
- point 順を reverse、rotate、deduplicate しない。
- finite check と `-0.0` の wire-copy canonicalization を維持する。
- total world point checked sum `<= 100,000` を維持する。

## 6. exact rational zero の canonical denominator

### 6.1 残る不具合

現在の validation は zero rational について、空 numerator と非零 denominator を確認するだけである。次の値を受理してはならない。

```text
sign = zero
numerator = []
denominator = [0x02]
```

### 6.2 必須修正

zero は厳密に次だけを許可する。

```text
sign = zero
numerator_magnitude_be = []
denominator_be = [0x01]
```

non-zero は既存規則を維持する。

- numerator は非空、先頭 `0x00` なし、全体が非零
- denominator は非空、先頭 `0x00` なし、全体が非零
- sign と numerator zero/non-zero が一致

zero denominator `[0x02]`、`[0x01, 0x00]`、空、leading zero を独立 test で拒否する。

## 7. request の negative zero、raw bit 比較、typed domain ID

### 7.1 残る不具合

request angle の比較で `canonical_finite(requested.angle_degrees)` を使用すると、request `-0.0` が `+0.0` へ変換される。proof に `+0.0` がある場合、明示的に禁止された request `-0.0` を受理してしまう。

request の project、face、edge ID も `String` のままであり、元指示書 §5.2 の既存 domain ID 型による deserialize を満たしていない。

### 7.2 必須修正

request 型は意味に応じて既存 domain ID 型を使う。

```text
expectedProjectInstanceId -> ProjectId
expectedProjectId         -> ProjectId
fixedFaceId               -> FaceId
edgeId                    -> EdgeId
```

response DTO は既存どおり canonical wire string でよい。

request semantic validation:

- hinge count `1..=4,096`
- edge ID は canonical code-unit 順
- duplicate、missing、extra を拒否
- angle は finite
- `0.0 <= angle <= 180.0`
- `Object.is` 相当の negative zero を拒否
- 少なくとも 1 angle は `0.0` / `180.0` 以外
- proof、request、semantic pose の angle は raw `to_bits()` で比較
- request comparison 前に `-0.0` を canonicalize しない

world/hash 用 response copy の `-0.0 -> +0.0` canonicalization と、request authority validation を混同しない。

最低限の request regression:

- canonical typed IDs を受理
- malformed、nil、foreign project/face/edge ID を拒否
- duplicate / out-of-order edge
- missing / extra edge
- angle `NaN`、`+Infinity`、`-Infinity`
- angle `-0.0`
- angle `< 0`、`> 180`
- all-flat endpoint
- angle 1-bit mismatch
- exact match

## 8. live face registry の duplicate と unbounded copy

### 8.1 残る不具合

監査差分は live face registry を `to_vec()` した後に sort と `dedup()` を行う。例えば live が `[A, B, B]`、proof が `[A, B]` のとき、extra duplicate を消して一致させてしまう。

また、live count を確認する前の `to_vec()` は unbounded copy である。

### 8.2 必須修正

1. tree/graph live face slice を borrow。
2. allocation 前に `live.len() == proof_face_count` を確認。
3. proof count は cheap preflight 済みで `<= 512` であること。
4. bounded `try_reserve_exact` を行う。
5. canonical bytes 順へ sort。
6. `windows(2)` で duplicate を明示拒否。
7. `dedup()` で不正入力を正規化しない。
8. proof material face vector と exact 比較。

最低限:

- exact same registry を受理
- missing live face を拒否
- extra live face を拒否
- duplicate live face を拒否
- same count だが one foreign face を拒否
- tree と graph の両方

## 9. §8 apply、Undo/Redo、save/open/recovery

この節は元指示書で必須だったが、完了報告でも未実施と明記されている。viewer boundary の完成条件であるため省略しない。

### 9.1 Apply

`apply_stacked_fold_transaction_inner` の rollback-prone operation がすべて成功した後にだけ current evidence を install する。

意味上の分岐:

```rust
project.current_layer_evidence = match applied_layer_order.as_ref() {
    Some(CurrentLayerEvidence::NonFlat(_)) => applied_layer_order.clone(),
    _ if target.is_none() => applied_layer_order.clone(),
    _ => None,
};
```

次を維持する。

- target geometry が `Some` でも NonFlat proof を保存。
- `target.is_none()` の既存保存 semantics を維持。
- `CertifiedFlat` は既存 global flat capability install の結果を正本とする。
- install failure を partial success にしない。
- transaction slot の diagnostic/persistence semantics を変えない。
- viewer response 型を mutation path へ渡さない。

### 9.2 Undo/Redo

既存の `execute_undo` / `execute_redo` が `current_layer_evidence = None` にする挙動を維持する。

- Apply 直後は viewer response が存在。
- Undo 直後は `Ok(None)`。
- Redo 直後も古い evidence を復活させず `Ok(None)`。
- Redo 後の自動 reproof を追加しない。

### 9.3 Save/open/recovery

- archive は current NonFlat evidence を保存。
- open/recovery は archive rounded dataをそのまま current proof にしない。
- live modelで fresh native revalidation を実行。
- reopened project は新しい instance ID を持つ。
- old instance request は `stale_authority`。
- new instance/current revision/current fingerprint/current stable pose だけが response を得る。
- face、cell、pair、pose、fingerprint tamper を拒否。

既存 regression を狭く拡張する。

```text
stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically
global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected
```

## 10. native unit test matrix

`current_non_flat_layer_order_view.rs` は監査時点で unit test が 0 件だった。次の matrix を production helper と lifecycle regression の組合せで実装する。

陽性:

- tree Apply 後の response
- graph Apply 後の response
- tree pose model ID
- closed graph pose model ID
- dropped X / Y / Z
- positive / zero / negative exact rational
- multiple faces canonical order
- multiple cells canonical digest order
- zero cells
- repeated identical snapshot
- cap boundary exactly equal
- `readOnly = true`
- `authorizesProjectMutation = false`

binding 陰性:

- foreign instance
- foreign project ID
- stale revision
- stale fingerprint
- proof namespace/revision/fingerprint mismatch
- fixed face mismatch
- proof/request/semantic missing、extra、duplicate hinge
- angle 1-bit mismatch
- current applied pose missing
- old instance after reopen
- live face missing、extra、duplicate、foreign
- tree/graph model mismatch

evidence 陰性:

- material/folded coverage mismatch
- unknown/equal lower-upper face
- reverse pair crossing
- exact/rounded bit mismatch
- lower/upper dropped axis mismatch
- non-finite world point
- malformed exact zero denominator

resource 陰性:

- face/folded/hinge/cell/pair cap + 1
- per-world polygon cap + 1
- per-cell polygon cap + 1
- total world/exact point cap + 1
- aggregate exact magnitude cap + 1
- final JSON cap + 1
- checked arithmetic overflow
- fallible allocation failure path

persistence:

- NonFlat target geometry Apply 後に current evidence
- Undo 後 `Ok(None)`
- Undo then Redo 後 `Ok(None)`
- save/open fresh response
- recovery fresh response
- archive tamper reject

test のためだけに production visibility を過度に広げない。module 内の private builder と既存 lifecycle fixture を使う。public mutation capability や forged proof constructor を追加しない。

## 11. data-free error 分類

error serialization は引き続き exact 2-key payload とする。

```json
{
  "version": 1,
  "category": "stale_authority"
}
```

許可 category:

```text
stale_authority
invalid_evidence
resource_limit
internal_failure
```

分類:

- request/current/proof/pose binding mismatch: `stale_authority`
- structural coverage、axis、exact/rounded、registry duplicate: `invalid_evidence`
- cap、checked arithmetic、fallible allocation、final JSON byte cap: `resource_limit`
- lock poisoning、serialization、issuer invariant failure: `internal_failure`

全 category test で serialized own key が `version` と `category` だけであることを確認する。次を error、panic、debug、assert messageへ含めない。

- project/instance ID
- revision
- fingerprint
- face/edge ID
- coordinate
- proof、exact numerator/denominator

cap 超過を `Ok(None)`、空配列、truncate、sampling に変換しない。

## 12. commit 分割

既存 commit を変更せず、後続 commit を次の意味で分ける。

native boundary、hash、caps、unit tests:

```text
非平坦層順ビューのネイティブ境界を完成する
```

Apply/persistence と lifecycle regression:

```text
非平坦層順の適用後証拠を永続境界へ接続する
```

実態上、test が同じ production file と不可分なら、test を対応 production commit に含める。空 commit や実態と異なる subject を作らない。

各 commit 前:

```powershell
git diff --check
git diff --cached --check
git diff --cached --name-only
git status --short
```

stage は exact path 指定にする。`git add -A`、`git add .`、directory 単位 stage を使わない。

## 13. exact validation command

repository root から実行する。

focused native:

```powershell
cargo test --locked -p ori-collision --lib non_flat_cell_transport::tests -- --test-threads=1
cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1
cargo test --locked -p origami2-desktop --lib stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically -- --exact --test-threads=1
cargo test --locked -p origami2-desktop --lib global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected -- --exact --test-threads=1
```

native static checks:

```powershell
cargo check --locked -p origami2-desktop --all-targets
cargo fmt --all -- --check
cargo clippy --locked -p origami2-desktop --all-targets --all-features -- -D warnings
```

handler/invoke contract:

```powershell
Set-Location apps/desktop
node --test tests/tauriCapabilityContract.test.ts
Set-Location ../..
```

workspace regression:

```powershell
cargo test --workspace --locked --all-targets --no-fail-fast
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
```

diff/source audit:

```powershell
git diff --check
git diff --cached --check
git show --check HEAD
rg -n "frame\\(&mut .*DOMAIN|dedup\\(\\)|canonical_finite\\(requested\\.angle_degrees\\)" apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
rg -n "current_layer_evidence\\s*=|applied_layer_order|target\\.is_none" apps/desktop/src-tauri/src/stacked_fold_transaction.rs
git status --short
```

最初の `rg` は、次が残っていないことを確認するための禁止 pattern である。

- domain separator を length-frame する呼出し
- live registry の duplicate を `dedup()` で消す処理
- request `-0.0` を comparison 前に canonicalize する処理

環境起因または担当外 failure が出た場合は、command、exit code、最初の根本 error、変更前 HEAD での再現有無を報告する。担当外 file を直して緑に見せない。

## 14. 完了報告

作業完了後、次を新規作成する。

```text
docs/Codex/claude-sim010-viewer-native-audit-addendum-report-2026-07-26.md
```

報告には次を含める。

- 監査 baseline hash
- 最終 source hash
- commit hash と日本語 subject
- commit ごとの exact changed files
- hash expected vectors
- focused/full validation の exact command、pass/fail、test count
- data-free error matrix
- `git status --short`
- Git identity
- direct push をしていないこと
- protected path と担当外差分に触れていないこと

この report は `docs/Codex/**` のため stage、commit しない。完了条件を満たさない残件がある場合は「完了」と書かず、具体的 blocker と再現手順を記載する。

## 15. 最終受入チェックリスト

- [ ] raw domain separator が hash preimage の先頭である。
- [ ] exact hash は raw magnitude bytes を使用する。
- [ ] hard-coded expected digest が 3 種ある。
- [ ] cheap cap preflight が heavy structural validator より前である。
- [ ] world polygon は allocation 前に per-polygon cap を検証する。
- [ ] response allocation は fallible である。
- [ ] exact zero denominator は `[0x01]` のみである。
- [ ] request `-0.0` を拒否する。
- [ ] request/proof/semantic angle は raw bits で一致する。
- [ ] request ID は domain ID 型で deserialize する。
- [ ] live face duplicate を拒否し、`dedup()` で消さない。
- [ ] live registry を count check 前に unbounded copy しない。
- [ ] proof/current project/current pose/live registry の全 binding が一致する。
- [ ] tree/graph の正しい pose model ID を返す。
- [ ] NonFlat target geometry Apply 後に current evidence が存在する。
- [ ] Undo と Redo の後は evidence が存在しない。
- [ ] save/open/recovery は fresh proof と fresh instance を要求する。
- [ ] old instance request と archive tamper を拒否する。
- [ ] error は data-free 4 category へ閉じる。
- [ ] cap 超過時に truncate、sampling、`Ok(None)` を行わない。
- [ ] native positive/negative/cap/persistence matrix が通る。
- [ ] workspace regression と clippy が通る。
- [ ] 担当外差分を stage、commit、restore していない。
- [ ] Git identity は `yuya <oltotlo79@gmail.com>`。
- [ ] commit message は日本語である。
- [ ] direct push を行っていない。
