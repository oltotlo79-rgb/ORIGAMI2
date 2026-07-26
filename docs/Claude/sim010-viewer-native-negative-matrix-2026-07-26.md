# Claude 追加作業指示: SIM-010 native viewer 残存 negative matrix

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
対象 branch: `main`
remote push: 禁止

## 1. 目的

`docs/Codex/claude-sim010-viewer-native-followup-2026-07-26.md` を再精査した。
次の実装と検証は既に完了しているため、作り直さないこと。

- native viewer の正規tree fixture
- issuer model IDの選択
- raw domain separator、plane-axis tag、exact rational framing
- face / exact-boundary / cell の独立固定digest
- requestのtyped ID、raw angle bit比較、`-0.0`拒否
- data-free error 4 category
- WSL上のnative module test 19件
- frontend hostile-input、lifecycle、binding、DOM matrix

一方、同報告 §7.2 と §7.3 は未達と明記されている。本作業は、そのうち
production proofを偽造せずに検証可能なnative negative matrixと、正規fixtureから
到達可能なissuer/axis positive matrixを完成させる。

この作業は完成率を直接上げるための根拠にはしない。一般multi-block Applyや
一般continuous collision authorityを追加してはならない。

## 2. 着手前の競合確認

担当pathは原則として次の1 fileだけである。

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

着手前に必ず次を実行する。

```powershell
git status --short -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
git log -5 --oneline -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
Get-FileHash -Algorithm SHA256 apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

次のいずれかなら編集を開始しない。

- 担当fileに未commit差分がある。
- 本指示受領後に別担当の新しいcommitが同fileへ入った。
- indexに自分がstageしていない変更がある。

その場合は、内容を上書き、restore、stash、resetせず、
`docs/Codex/claude-sim010-viewer-negative-matrix-report-2026-07-26.md`
へ時刻、exact status、HEAD、file hashを書いて終了する。

## 3. 設計制約

次は禁止する。

- `StackedFoldNonFlatLayerOrderV1` のprivate fieldを迂回する偽造
- `unsafe`、transmute、raw pointerによるproof生成
- production crateへのtest-only public constructor追加
- `test-fixtures` featureや本番API surfaceの追加
- cap testのためだけに巨大な正規proofを実際に割り当てること
- viewerからproject mutation、Apply、token mint、authority発行を可能にすること
- categoryを緩めること
- 既存testの削除、ignore化、assertion弱化

viewerの現在の入口、wire DTO、hash V1 contract、定数値を維持する。
refactorは「既存production値を同じ順序で検査する純粋な内部projection」までに限定する。

## 4. proofを偽造しないresource preflightの分離

現行の `preflight_view_resources` は
`&StackedFoldNonFlatLayerOrderV1` を直接受け取るため、正規proofでは到達不能な
cap + 1をunit testできない。

同file内にprivateな値projectionを導入する。名称は実装に合わせてよいが、
次の性質を満たすこと。

1. projectionは `Copy` またはborrowだけで、所有proofやauthorityを保持しない。
2. production経路では、正規proofのslice/countからprojectionを一度だけ構築する。
3. projection構築前に巨大な `Vec` を作らない。
4. validatorはprojectionの数値と既存sliceの整合性を検査し、
   現行と同じ `invalid_evidence` / `resource_limit` を返す。
5. unit testはprojectionへ境界値を直接与える。proof自体は偽造しない。
6. test-only branchをproduction関数内へ追加しない。

少なくとも次のfieldを明示的に扱うこと。

```text
material face count
folded face count
hinge count
declared overlap-cell count
actual overlap-cell slice length
declared tested-pair count
actual pair-order slice length
per-cell rounded world point count
per-cell exact point count
aggregate rounded world point count
aggregate exact point count
aggregate exact magnitude bytes
serialized JSON bytes
```

checked addition/multiplicationを使い、overflowは `resource_limit` とする。
「declared値と実slice長の不一致」「material/folded count不一致」
「rounded/exact point count不一致」「3点未満polygon」は
`invalid_evidence` のままにする。

## 5. 必須resource境界test

各上限について、少なくとも `max - 1`、`max`、`max + 1` を独立に検証する。
`max + 1`だけを置くのではなくinclusive境界を固定すること。

対象:

- `MAX_FACES_V1`
- `MAX_HINGES_V1`
- `MAX_CELLS_V1`
- `MAX_FACE_PAIR_ORDERS_V1`
- `MAX_WORLD_POLYGON_POINTS_V1`
- `MAX_CELL_POLYGON_POINTS_V1`
- `MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1`
- `MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1`
- `MAX_EXACT_MAGNITUDE_BYTES_V1`
- `MAX_SERIALIZED_JSON_BYTES_V1`
- JSON safe integer上限

追加必須:

- aggregate pointのchecked-add overflow
- aggregate exact magnitudeのchecked-add overflow
- countからbyte数を求めるchecked-mul overflowが存在する場合、そのoverflow
- zero faces、zero hinges
- material/folded count不一致
- declared/actual cell count不一致
- declared/actual pair count不一致
- rounded/exact point count不一致
- 0 / 1 / 2点polygon

allocation failureを決定的に注入するproduction hookは追加しない。
現在のRust標準allocatorでは安全かつ決定的に再現できない場合は、
`try_reserve_exact` failureが `resource_limit` へ写るcode pathを
review可能な形で保持し、報告書に「実行注入未実施」と正直に記載する。
OOMを発生させるtestは禁止する。

## 6. structural negative matrix

production proofのprivate fieldを偽造せず、既存validation処理から
入力projectionまたは小さなprivate純関数へ安全に分離できるものだけを実装する。

必須候補:

- live face registry: missing / extra / duplicate / foreign
- material/folded coverage count mismatch
- unknown face pair
- equal face pair
- reversedまたは非canonical pair order
- exact/rounded point count mismatch
- dropped-axis tagとplane-axis derivationの不一致
- non-finite world point
- noncanonical exact zero denominator

分離した純関数はproduction経路でも必ず同じものを通すこと。
testだけが呼ぶ複製validatorを作らない。既存shared validatorより厳しくなる場合は
既存の正規tree 19 testとfrontend/native contractがすべて維持されることを確認する。

private proofを偽造しない限り到達不能な項目が残る場合は、無理に達成したと書かず、
exact blocker、必要な最小API、追加した場合のproduction attack surfaceを報告する。

## 7. graph issuerとdropped X/Y/Z positive

正規のproduction解析・revalidation経路だけを使い、次を個別fixtureで固定する。

- tree issuer positive
- graph issuer positive
- dropped X positive
- dropped Y positive
- dropped Z positive

1 fixtureで全caseを無理に満たす必要はない。各fixtureは次を満たすこと。

1. `ProjectState`、topology、flat layer order、pose authority、
   non-flat revalidationを通常のproduction関数だけで生成する。
2. proofのprivate constructorを迂回しない。
3. response `poseModelId` が実issuerのmodel IDと一致する。
4. dropped axisから導出したplane axesが正しい順序である。
5. face/exact-boundary/cell hashがrepeatでbyte-identicalである。
6. viewerがread-onlyであり、mutation/apply authorityを返さない。

正規graph fixtureを現行公開APIだけで構成できない場合は、core APIを増やさず、
使用可能なproduction constructorと不足している境界を報告する。

## 8. regressionと検証

Windows Application Controlでtest binaryが遮断される場合はcompile成功だけで
完了扱いにせず、WSLで同一worktree・同一HEADを検証する。

最低限:

```powershell
cargo fmt --all -- --check
cargo check --locked -p origami2-desktop --lib
cargo check --locked -p origami2-desktop --lib --tests
cargo clippy --locked -p origami2-desktop --lib --all-targets --all-features -- -D warnings
git diff --check
git diff --cached --check
git status --short
git config user.name
git config user.email
```

WSL:

```bash
CARGO_TARGET_DIR=/tmp/origami2-viewer-negative-matrix \
  cargo test --locked -p origami2-desktop --lib \
  current_non_flat_layer_order_view::tests -- --test-threads=1

CARGO_TARGET_DIR=/tmp/origami2-viewer-negative-matrix \
  cargo clippy --locked -p origami2-desktop --lib \
  --all-targets --all-features -- -D warnings
```

既存19件を含む全module testのpass数を実測で報告する。
test filterで0件実行だった場合は成功扱いにしない。

## 9. commit、Git identity、禁止path

Git identityは必ず次を維持する。

```text
yuya
oltotlo79@gmail.com
```

対象fileだけをexact stageし、indexを再確認する。

```powershell
git add -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
git diff --cached --name-only
git diff --cached --check
git commit --only -m "非平坦層順ビューの否定境界行列を完成する" -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

禁止:

- push
- amend / rebase / squash / reset / restore / stash
- Git identity変更
- `apps/desktop/src-tauri/src/stacked_fold_read.rs`
- `apps/desktop/src-tauri/src/stacked_fold_transaction.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `crates/**`
- frontend source/test
- `docs/Codex/**` のstage/commit
- `docs/plans/**`、`origami2-*.png`、`target-*` の変更・削除・stage
- 他者の未commit差分の整形、stage、commit

## 10. 完了報告

新規に次を作成する。

```text
docs/Codex/claude-sim010-viewer-negative-matrix-report-2026-07-26.md
```

報告書には必ず次を書く。

- 着手時HEAD、担当file hash、競合確認結果
- commit hash、author/committer、exact changed path
- resource上限ごとの `max-1 / max / max+1` test名
- structural negative caseごとのtest名
- tree/graph、dropped X/Y/Zの到達結果と使用した正規constructor
- 未到達caseと、その理由
- command別の実行環境、pass/fail/ignored/filtered件数、warning数
- Windows遮断の有無、WSL kernel、同一HEAD確認
- `git status --short` と保護対象を触っていない確認

報告書自体はstage/commitしない。未達項目がある場合に「全項目完了」と書かないこと。
