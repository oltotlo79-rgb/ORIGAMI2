# Claude 作業指示: SIM-010 非平坦 layer-order viewer 完了追補

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
基準 branch: `main`
作業種別: 既存 3 commit の追補修正、未実施要件の完了、受入回帰の完成
remote push: 禁止

## 1. この指示の正本

次の原指示と報告を最初から最後まで UTF-8 で読み直すこと。

- 原指示: `docs/Claude/sim010-non-flat-layer-viewer-2026-07-26.md`
- blocker 追補: `docs/Claude/sim010-viewer-blocker-resolution-2026-07-26.md`
- 完了報告: `docs/Codex/claude-sim010-viewer-completion-2026-07-26.md`
- 完了報告 SHA-256:
  `aef07a164b9005e03ae5e57ac0d45db819cd24f9233e4cd96d787cf0f64f60db`

既存 commit は次の 3 件である。

- `358eeabcd69fa9a5eff39f8cf8694ae36cbeb131`
  `適用済み非平坦層順の読取境界を実装する`
- `ed7375938e8fb6c05623de5296be222bdf2c8cd5`
  `非平坦層順ビュー応答を厳格検証する`
- `9135cf7acf9c0ae44251e40f0ea8a915d38a7b9f`
  `適用済み非平坦層順ビューアーを接続する`

この 3 commit は amend、rebase、squash、drop しない。既に共有 worktree の履歴にあるため、追補は新しい commit として積むこと。

## 2. 監査結論

上記報告は「3 段階 commit 完了」としているが、原指示の完了条件は満たしていない。報告自身が §8、§12、§13 の未実施を明記しており、さらに実装監査で次の blocking defect が確認された。

以下は任意改善ではなく、すべて完了条件である。難しいことや fixture が大きいことを理由に省略しない。production visibility を test のためだけに広げず、既存 fixture/helper を同一 module の test から再利用すること。

## 3. 最優先 blocking defect

### 3.1 source file 内の literal NUL を全廃する

現在、次の通常の TypeScript/TSX source に literal `U+0000` が合計 3 byte 入っている。

- `apps/desktop/src/lib/currentNonFlatLayerOrderView.ts`: 2 個
- `apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx`: 1 個

このため Git が両 file を binary と判定し、`git diff --numstat` が `- -` になっている。review、blame、patch、監査を壊すため許容しない。

修正要件:

1. `exactRecord` の key 比較に sentinel join を使わない。
2. actual/expected をそれぞれ code-unit 順に sort し、長さと各 index の完全一致で比較する。
3. viewer の identity sentinel 自体を廃止する。後述の `[source, reloadToken]` を使用する。
4. repository の対象 source に literal NUL が 0 個であることを Node test で固定する。
5. 修正後の `git diff --numstat` で両 file が通常の追加/削除行数として表示されることを確認する。

escape sequence `"\0"`、`"\u0000"` へ置換して join を残す修正は不可。衝突しない sentinel を仮定しないこと。

### 3.2 viewer の再取得境界を `source` object にする

現在の effect identity は project instance、project ID、revision、fingerprint、pose state、fixed face しか含まず、hinge ID/angle を含まない。同じ revision/fingerprint/fixed face のまま hinge angle が変化すると native を再取得せず、古い geometry を表示する。

`StackedFoldPanel` は `nonFlatViewerSource` を `useMemo([appliedPose, snapshot])` で作っている。同じ source reference の locale-only rerender では再取得されないため、effect は次にする。

```text
[source, reloadToken]
```

要件:

- locale `ja -> en -> ja`、同じ source reference: refetch 0、selection/geometry 維持。
- 新しい applied-pose object または snapshot object: semantic 値が同じでも新規取得。
- hinge ID または angle が 1 bit でも変わった新 source: 直ちに old pixels を消し、新規取得。
- source が `null`、running、blocked、indeterminate、不一致 project/revision/null fixed face へ変わった直後: old pixels を消し、旧 response を再利用しない。
- A -> B -> A の同一 semantic pose 再発行でも、古い A response/cache を再利用しない。
- late A response は B または再発行 A を上書きしない。
- unmount 後の resolve/reject は state を変更しない。
- response identity が変わった取得では face/cell selection を canonical first へ戻す。locale-only rerenderでは戻さない。
- effect の `react-hooks/exhaustive-deps` warning を 0 にする。warning を「意図的」として残さない。
- loading/ready state には取得時の `source` reference を保持する。render 時点で `state.source !== source` なら、その commit では geometry を描画せず同期的に hidden/null とする。passive effect が走るまで旧 pixels を残してはならない。
- source が ready A から `null`、running、blocked、indeterminate、binding mismatch へ変わる遷移を、rerender 直後かつ effect flush 前の DOM assertion で固定する。最初から source が `null` の test だけで代用しない。

### 3.3 native の project/evidence/pose 再結合を原指示 §7.3 どおり完成する

現行 `current_non_flat_layer_order_view.rs` は request と current project の照合は行うが、少なくとも次を明示照合していない。

- proof `model_id()` と正本 model ID
- proof `identity_namespace()` と current project ID
- proof `target_revision()` と current revision
- proof `target_fingerprint()` と current fingerprint
- proof hinge vector と revalidated `AppliedPoseV1::hinge_angles()`
- proof material face ID set と live tree/graph model face ID set の完全一致

また fixed face だけを semantic pose と比較しており、proof/request/current semantic/native pose の hinge vector 三者以上の bit-exact 再結合が不足している。

修正要件:

1. 原指示 §7.2 の lock 順序と §7.3 の全 bullet を 1 件ずつ実装・test する。
2. angle は edge ID、vector length、canonical order、`to_bits()` をすべて比較する。
3. tree/graph の native pose と semantic pose の内部整合性を capability revalidation に依存してよいが、proof と semantic pose の照合は viewer 境界で必ず行う。
4. live model face set は「proof の各 face が存在」だけでなく、extra live face も拒否する完全一致にする。
5. mismatch は原指示の分類どおり data-free `stale_authority` または `invalid_evidence` とし、ID、revision、fingerprint、座標、raw error を error payload に入れない。
6. `project.editor.current_applied_pose()` と revalidated semantic pose の current 性を回帰で固定する。
7. response `pose.modelId` は issuer kind に対応させる。tree は `ori_core::APPLIED_POSE_MODEL_ID_V1`（`tree_absolute_hinge_angles_v1`）、graph は `ori_core::CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1`（`closed_graph_absolute_hinge_angles_v1`）を正本とする。graph に tree model ID を付けない。
8. TypeScript parser も上記 2 値だけの closed union とし、tree/graph positive、unknown model negative を設ける。

### 3.4 exact hash framing と aggregate cap を修正する

原指示 §5.6 は exact rational を sign tag 1 byte、raw numerator magnitude length/bytes、raw denominator magnitude length/bytes で frame するよう指定している。現行 `frame_exact` は lowercase hex の ASCII bytes を hash しており、preimage が仕様と異なる。

修正要件:

- hash は `ExactRationalValue` の canonical raw magnitude bytesを frameする。
- DTO は引き続き lowercase even-length hex とする。
- hash のために hex ASCII を frameしない。
- zero の exact DTO は必ず `sign=zero`、numerator `""`、denominator `"01"`。
- 同一 fixture は byte-identical JSON/digest。
- face/cell input の f64 1 bit または exact magnitude 1 bit の変更で該当 digest が変わる。
- `faceKeySha256`、`exactBoundarySha256`、`cellKeySha256` の domain/framing順を原指示 §5.6 と完全一致させる。

現行 native は face 構築と cell 構築で `magnitude_bytes` を別々に 0 から数えている。8 MiB は response 全体の aggregate cap である。1 個の checked accumulator を faces と cells で共有し、合計 8 MiB + 1 を `resource_limit` にすること。

`testedFacePairs` と `sourceOverlapCellsAuthenticated` も、core の正本 work bound 内かつ frontend の safe integer として lossless であることを native で確認する。

world face boundary は、live model から既知 vertex count を得た時点で 3..=4,096 と aggregate cap を検査し、`try_reserve_exact` または同等の fallible reservation が成功してから point を構築する。cap + 1 の `Vec` を先に collect してから拒否しない。faces、cells、hinges、exact points、serialized bufferについても、大きな確保前のpreflightとallocation failureのdata-free分類を回帰する。

viewer 固有の cheap count preflight は shared structural validator より先に行う。shared validator は core 上限の faces/folded/cells を `HashSet` 化するため、viewer 上限 4,096 を超える入力を validator に渡してから拒否してはならない。順序は、既知 count と checked aggregate の cheap preflight、current pose と project/evidence binding、shared structural validation、fallible reservation、response 構築とする。viewer cap + 1 が validator/大 allocation 前に data-free `resource_limit` となる回帰を設ける。また stale binding と structural tamper が同居する fixture で、原指示 §7.2/§7.3 の category 優先順を固定する。

### 3.5 TypeScript strict parser/client を完全にする

現行 test は陽性 1、detach/freeze 1、33 fixture を 1 test にまとめたものだけで、原指示 §13 の matrix を満たさない。

必須修正:

- object と array の symbol own property を extra field として拒否する。
- string own property だけを列挙する方法で symbol を見落とさない。
- getter/setter を 0 回のまま拒否する。counter で確認する。
- ownKeys/getOwnPropertyDescriptors/getOwnPropertyDescriptor 等の throwing Proxy trap を data-free に拒否する。
- nested object の missing、extra、inherited、accessor を各 level で確認する。
- array hole、index accessor、named extra property、symbol extra property を拒否する。
- zero rational は denominator `"01"` 以外を拒否する。
- native `null` だけを evidence absence として保持する。予期しない `undefined` や malformed response を absence に軟化しない。
- response/request binding は fixed face だけでなく、hinge ID/order/angle bits まで完全一致させる。
- duplicate request hinge、同一 ID、nonfinite、`-0`、out-of-range は native invoke 前または native 境界で fail-closed にする。
- exact magnitude aggregate cap は affine と cell exact points を合計する。
- face key/cell key/digest の重複と canonical order を個別回帰にする。

原指示 §13.1、§13.2 の全列挙項目を traceable な test 名または case ID へ対応させること。「33件に含まれるはず」という報告は禁止。

### 3.6 apply/persistence lifecycle を原指示 §8 どおり実装する

報告が未実施と明記した §8.1、§8.2、§8.3 は必須である。

`apply_stacked_fold_transaction_inner` の rollback-prone operation がすべて成功した後だけ、意味上次を満たす狭い分岐を入れる。

```rust
project.current_layer_evidence = match applied_layer_order.as_ref() {
    Some(CurrentLayerEvidence::NonFlat(_)) => applied_layer_order.clone(),
    _ if target.is_none() => applied_layer_order.clone(),
    _ => None,
};
```

実際の borrow/clone は現行 ownership に合わせる。

必須挙動:

- target geometry を持つ tree non-flat Apply 後も project-owned NonFlat evidence が入る。
- target geometry を持つ graph non-flat Apply 後も同じ。
- `target.is_none()` の既存 semantics を壊さない。
- `CertifiedFlat` の global flat authority/install semantics を変えない。
- install failure/pose reissue failure/transaction failure は partial success を残さない。
- Undo 後は evidence `None`、viewer `Ok(None)`。
- Redo 後も timeline/archive から旧 evidence を復活させず `Ok(None)`。
- fresh native reproof 後だけ再表示できる。
- save/open/recovery は archive rounded dataを直接 authority とせず、fresh revalidation proof と fresh instance を作る。
- old instance request は `stale_authority`。
- archive の face/cell/pair/pose/fingerprint tamper は reopen/revalidation で拒否。

既存 test 名を維持して assertion を追加する。

- `stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically`
- `global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected`

recovery の fresh-proof/fresh-instance regression も追加する。

### 3.7 native test matrix を省略しない

`current_non_flat_layer_order_view.rs` は現在 unit test 0 件である。原指示 §12.1、§12.2、§12.3 の全 case を実装する。

最低限:

- tree/graph positive
- dropped X/Y/Z
- exact rational negative/zero/positive
- multiple face/cell canonical order
- zero-cell valid response
- deterministic repeatとdigest 1-bit sensitivity
- save/open/recovery fresh proof
- max accepted cap と各 cap + 1
- no evidence / certified-flat
- foreign instance/project/revision/fingerprint
- proof identity/revision/fingerprint tamper
- fixed face mismatch
- missing/extra/duplicate/out-of-order hinge
- angle 1-bit mismatch
- semantic/native pose mismatch
- missing current applied pose
- invalidated capability
- material/folded/live face coverage mismatch
- unknown/equal/reversed face pair
- exact/rounded mismatch
- dropped axis mismatch
- nonfinite world point
- exact aggregate cap
- JSON cap
- checked arithmetic
- Undo、Redo、archive tamper

private constructorsを迂回する `unsafe` test fabricationは禁止。既存の正規 revalidation/apply fixture を使用し、必要なら同一 module 内の private helper を小さく切り出す。

### 3.8 DOM/UI と integration matrix を完成する

原指示 §14 の全 bullet を実装する。現行 dedicated viewer DOM test は 6 件だけである。

特に必須:

- loading/absent/ready/4 category failed
- stable 以外と全binding mismatchでinvoke 0またはcurrent load破棄
- world pane は XYZ field のみ、projection pane は rounded UV のみ
- dropped X/Y/Z labels
- face/cell selection と keyboard
- selected cell の lower/upper face を world pane 上の対応 face として明示 highlight し、選択変更時に追従する。単に cell list の行を選択表示するだけでは未達。
- zero-cell warning
- read-only/no-mutation
- late response、old pixels、same semantic reissue、A-B-A、unmount
- locale-only no refetch、selection/geometry保持、text/ARIA即時切替
- raw native error/raw exact big integer 非表示
- `StackedFoldPanel` に viewer 1 個だけ
- 既存 flat `LayerOrderViewer` semantics 不変
- apply callback/count は viewer mount/locale switch で不変

報告の「`stackedFoldPanel.dom.test.tsx` は無変更で52件」は不正確である。standalone は 46 件、viewer 6 件との合計が 52 件である。今後は command ごとの実数を分けて報告する。

### 3.9 Tauri capability contract

`apps/desktop/tests/tauriCapabilityContract.test.ts` を実行し、新しい literal frontend invoke が handler に正確に 1 回登録され、unknown command が拒否されることを確認する。

既存自動検出で通る場合は production source を変えない。ただし test 実行を省略して「自動検出なら」と報告しない。

## 4. 担当可能 file

原則として次だけを変更してよい。

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
apps/desktop/src-tauri/src/lib.rs
apps/desktop/src-tauri/src/stacked_fold_transaction.rs
apps/desktop/src-tauri/src/stacked_fold_read.rs
apps/desktop/src-tauri/src/global_flat_foldability.rs
apps/desktop/src-tauri/src/recovery.rs
apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
apps/desktop/src/components/StackedFoldPanel.tsx
apps/desktop/src/App.tsx
apps/desktop/src/App.css
apps/desktop/src/lib/currentNonFlatLayerOrderViewerText.ts
apps/desktop/tests/currentNonFlatLayerOrderView.test.ts
apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx
apps/desktop/tests/currentNonFlatLayerOrderViewerText.test.ts
apps/desktop/tests/stackedFoldPanel.dom.test.tsx
apps/desktop/tests/tauriCapabilityContract.test.ts
```

`crates/ori-collision` の共通 validator は既に別 commit で導入済みである。新しい実証済み defect がなければ変更しない。担当外変更が本当に必要なら、編集前に `docs/Codex` の報告へ blocker と exact path/理由を書き、勝手に範囲を広げない。

## 5. commit 分割

既存 commit を変更せず、次の 3 commit を目安にする。

1. `非平坦層順ビューの再取得境界を修正する`
   - literal NUL 全廃
   - native 再結合/hash/cap 修正
   - parser/client strictness
   - effect/selection/late-response 修正
   - focused regression
2. `非平坦層順の適用後証拠を永続境界へ接続する`
   - apply、Undo/Redo、save/open/recovery
   - persistence regression
3. `非平坦層順ビューの受入回帰を完成する`
   - §12、§13、§14、Tauri contract の残り

意味の異なる変更を 1 巨大 commit にしない。各 commit 前に exact stage path を確認する。

## 6. 必須検証

repository root:

```powershell
git status --short
git diff --check
git diff --numstat f9913149b69ad1bc83d89681aa9309b986063cc5..HEAD
git config user.name
git config user.email
```

Git identity は次のまま維持する。

```text
yuya
oltotlo79@gmail.com
```

Rust:

```powershell
cargo fmt --all -- --check
cargo check -p origami2-desktop --lib
cargo clippy -p origami2-desktop --lib -- -D warnings
cargo test -p origami2-desktop --lib current_non_flat_layer_order_view
cargo test -p origami2-desktop --lib four_hinge_tree_level_three_proof_applies_and_persists_atomically
cargo test -p origami2-desktop --lib archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected
```

frontend:

```powershell
cd apps/desktop
node --test tests/currentNonFlatLayerOrderView.test.ts tests/currentNonFlatLayerOrderViewerText.test.ts tests/tauriCapabilityContract.test.ts
npx vitest run tests/currentNonFlatLayerOrderViewer.dom.test.tsx tests/stackedFoldPanel.dom.test.tsx
npx tsc -b --pretty false
npm run lint
npm run build
npm run test:snap
npm run test:dom
```

最後に repository root へ戻り、literal NUL が 0 であることと対象 file が text diff であることを再確認する。

## 7. 禁止事項

- remote push
- amend/rebase/squash/reset
- Git user identity の変更
- `docs/progress.md`、`docs/requirements-status.md`、`docs/plans/**` の変更
- `docs/Codex/**` の stage/commit
- `origami2-*.png` の変更、削除、stage
- `target-*` の変更、削除、stage
- test skip、warning の「意図的」扱い、実行していない test の成功報告
- raw native error、ID、座標、exact magnitude の DOM/error message への露出
- viewer への mutation callback/authority の追加

## 8. 完了報告

作業完了後、新規 report を次へ作る。

```text
docs/Codex/claude-sim010-viewer-followup-2026-07-26.md
```

この report は stage/commit しない。次を正確に書く。

- 新 commit hash、author、message、変更 file
- 本指示 §3.1〜§3.9 の各項目に対する実装箇所と test 名
- 原指示 §12/§13/§14 の case-to-test traceability
- 各 command の pass 数を個別に記載
- warning 数
- literal NUL 0 と text `numstat`
- `git status --short`
- 未実施項目がある場合は「完了」と書かず、exact blocker を明記
