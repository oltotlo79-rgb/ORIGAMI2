# Claude実装指示: SIM-010 native viewer のgraph issuer正規positiveを完成する

作成日: 2026-07-26  
対象repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`  
対象branch: `main`  
前提報告: `docs/Codex/claude-sim010-viewer-negative-matrix-report-2026-07-26.md`  
前提commit: `ccc6026902513e987918cb80bf8fa08f640da2db`

## 1. 目的

前提commitで、native non-flat layer-order viewerのresource境界、structural negative matrix、tree issuer、dropped X/Y/Zの正規positiveは実装済みである。未達なのは、正規constructorだけで生成したnon-flat closed-graph poseとfreshなgraph layer evidenceをviewerへ渡し、responseが実issuerどおりgraph modelを公開するpositive回帰だけである。

今回の目的は次の一点に限定する。

- private proofの偽造、`unsafe`、transmute、raw pointer、test-only production分岐を一切使わず、正規のnative pose authorityとcore graph non-flat revalidationだけでgraph issuer positiveを1件追加する。

一般cycle Apply、一般continuous collision、一般layer transport、archive schema、production APIの拡張は今回の対象外である。graph positiveを追加してもSIM-010を完成扱いにせず、viewerはread-onlyかつproject mutation非認可のままにする。

## 2. 着手ゲート

以下をすべて確認できるまで編集を開始しないこと。

1. `git log -5 --oneline -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` に `ccc6026` が存在する。
2. `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` に自分以外の未commit差分がない。
3. `apps/desktop/src-tauri/src/applied_pose.rs` に、次のtest-only helperがcommit済みで存在する。

   ```rust
   #[cfg(test)]
   pub(crate) fn install_pose_authority_with_angles(
       project: &mut ProjectState,
       angles: Vec<(EdgeId, f64)>,
       fixed_face: FaceId,
   ) -> Result<(), PoseAuthorityError>
   ```

   戻り値のerror型または引数の借用形は、実際にcommitされた同等契約を正本とする。このhelperは入力順・角度bitsを変更せず、`capture_request -> prepare -> commit_prepared`だけを通る必要がある。
4. `git diff -- apps/desktop/src-tauri/src/applied_pose.rs` が空であり、上記helperを別担当が編集中でない。
5. `git diff --cached --name-only`を確認し、自分以外のstageを巻き込まない。

helperが未commit、signatureがpanic固定、または対象fileがdirtyなら、競合回避のため作業を開始せず、`docs/Codex`の新規reportへ待機理由と観測したHEADを記録すること。既存差分をstash、restore、reset、整形、stageしてはならない。

## 3. 編集範囲

原則として変更してよいproduction repository fileは次の1件だけ。

- `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs`

同file内の`#[cfg(test)]` fixture、test-support module宣言、positive testを追加してよい。

変更禁止:

- `apps/desktop/src-tauri/src/applied_pose.rs`
- `apps/desktop/src-tauri/src/global_flat_foldability.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/stacked_fold_read.rs`
- `apps/desktop/src-tauri/src/stacked_fold_transaction.rs`
- `crates/**`
- frontend source/test
- `docs/plans/**`
- `docs/Codex/**`既存report
- `target-*`
- PNG検証画像

共有test fixtureを使うため、対象fileのtop-level `#[cfg(test)]` に次と同等のprivate module宣言を置くことは許可する。

```rust
#[cfg(test)]
#[path = "../../../../test-support/dense_grid_cycle.rs"]
mod viewer_dense_grid_cycle_test_support;
```

既存`test-support/dense_grid_cycle.rs`自体は編集しない。

## 4. 正規graph fixtureの構築

### 4.1 geometry

既存共有fixtureの次のconstructorを使う。

```rust
viewer_dense_grid_cycle_test_support::miura_authority_pattern(3, 3)
```

返却値のうち、horizontal hinge列の先頭3本をmoving setとする。全hinge registry、各hingeのMountain/Valley assignment、face registry、fixed faceは、作成した`ProjectState`の

```rust
project.editor.topology_analysis_input(project.project_id).analyze()
```

から取得すること。FaceIdやEdgeIdを手書きしない。fixed faceはcanonicalな規則、少なくとも`(face.key, FaceId::canonical_bytes())`の最小値で決め、保存順へ依存させない。

### 4.2 closed endpoint候補

全hingeをcanonical `EdgeId`順に並べる。moving 3本以外の角度は`+0.0`、moving hingeの角度絶対値は既存cycle回帰と同じ

```rust
2.0 * (1.0_f64).atan2(100.0).to_degrees()
```

を使用する。

sign候補は`mask in 0..(1 << moving.len())`の有限8通りだけを決定順に試す。各moving hingeについて、既存`stacked_fold_read`回帰の次の規則とexactに一致させる。

```text
mountain = live topology assignmentがMountain
flip = maskの対応bitが1
mountain XOR flip がtrueなら負、falseなら正
```

候補ごとにfreshな`ProjectState`を共有fixtureから作り直すこと。失敗したcandidateのpending stateやprojectを次候補へ再利用しない。

各candidateは、前提helper

```rust
applied_pose::tests::install_pose_authority_with_angles(...)
```

へ完全angle vectorを渡し、`capture_request -> prepare -> commit_prepared`の正規経路だけで判定する。最初に成功したclosed graph candidateだけを採用する。全8通りが失敗した場合はtestをskip/ignoreせず明確にfailさせる。

角度vectorをhelperの内外で丸め、補完、並べ替え、符号正規化してはならない。canonical順はcaller側で一度だけ作る。`-0.0`を生成せず、inactiveは明示的に`+0.0`とする。

### 4.3 fresh graph layer evidence

成功したprojectのcurrent semantic poseから、complete hinge vectorを`CanonicalHingeAngles`へbit-exactに再構成する。固定面もcurrent semantic poseと一致させる。

同じcurrent projectについて、既存の正規関数でfresh flat snapshotを解く。

```rust
global_flat_foldability::reanalyze_current_flat_layer_order(&project)
```

その後、次のcore graph入口だけを使ってfresh non-flat proofを発行する。

```rust
ori_core::revalidate_current_graph_non_flat_layer_order_v1(
    ori_core::RevalidateCurrentGraphNonFlatLayerOrderRequestV1 {
        identity_namespace: project.project_id,
        revision: project.editor.revision(),
        pattern: project.editor.pattern(),
        paper: project.editor.paper(),
        fixed_face,
        hinge_angles: &angles,
        current_flat: &flat,
        expected_archive: None,
        max_face_pairs:
            ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    },
)
```

tree revalidationへfallbackしてgraphに見せてはならない。proof private fieldの直接構築、serialization round-tripによる偽造、既存proofのfield変更は禁止する。

返ったproofだけをtest fixtureの`project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(proof))`へ設置し、production viewer read関数を呼ぶ。

## 5. 必須assertion

新しいgraph positive testは、最低限次をすべてassertすること。

1. current applied pose capabilityの`graph()`が`Some`である。
2. semantic pose model IDがclosed graph modelである。
3. viewer responseの`pose.model_id`が`GRAPH_POSE_MODEL_ID_V1`とexactに一致する。
4. response model IDがlive semantic poseのmodel IDと一致する。
5. `read_only == true`。
6. `authorizes_project_mutation == false`。
7. responseのface registryがlive proof registryと完全一致する。
8. responseのhinge count、cell count、pair countがproofのbounded countと一致する。
9. dropped axisからplane axesを既存`validate_axis_derivation_v1`で全face再検証する。
10.同一projectに対する連続2回のviewer readが`PartialEq`で一致し、`serde_json::to_vec`もbyte-identicalである。
11.別project instance、revision変更、pose angle 1 ULP変更、fixed face変更の少なくとも既存共通stale testがgraph positive追加後も通る。
12.responseはcollision clearance、continuous motion、Apply、project mutation authorityを新たに主張しない。

graph fixtureのcell数が0の場合でも、graph issuer positiveとして有効である。ただし0 cellを一般layer transport完成の根拠にしてはならない。cellが非空なら既存cell digest/invariantを通すだけにし、fixture固有digestをproduction契約へ追加しない。

## 6. 既存negative/resource行列の維持

前提commitの新規25件と既存19件を削除、ignore、条件緩和、test名変更してはならない。特に次を維持する。

- 全resource capの`max-1 / max / max+1`
- checked-add overflow
- live face registry missing/extra/duplicate/foreign
- unknown/equal/disagreeing face pair
- exact/rounded count mismatch
- dropped X/Y/Z positive
- exact rational canonical zero
- JSON safe integerとserialized byte上限
- deterministic digest preimage
- archive reopen fresh-instance test

現在並行中のarchive revalidation実装が完了するまでは、`a_reopened_project_needs_a_fresh_instance`の一時失敗をviewer側で迂回しない。対象外fileの修正、test弱化、`#[ignore]`は禁止する。root側のpersistence commit後に再実行して緑を確認する。

## 7. 検証

着手時と終了時に、HEAD、対象file status、対象file hash、Git identityをreportへ記録する。

最低限、次を実行する。

```powershell
rustfmt --edition 2024 --check apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
cargo fmt --all -- --check
cargo check --locked -p origami2-desktop --lib
cargo check --locked -p origami2-desktop --lib --tests
cargo clippy --locked --no-deps -p origami2-desktop --lib --all-targets --all-features -- -D warnings
cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1
git diff --check -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

Windows Application Control等でtest binaryを実行できない場合は、WSLの同一worktreeで専用の`CARGO_TARGET_DIR=/tmp/origami2-viewer-graph-positive`を使って実testを完走する。compileだけをtest成功と書かない。filter 0件も成功扱いにしない。

並行中のpersistence差分が原因で外部fileのcompile/testが失敗した場合は、対象file由来かを切り分けてreportする。外部fileを直して緑に見せない。persistence commit後に必ずviewer module全件を再実行する。

## 8. commitとpush

変更が対象file 1件だけで、module test全件、format、diff-checkが成功した場合に限り、日本語commitを作る。

推奨subject:

```text
非平坦層順ビューのグラフ発行者回帰を完成する
```

stageはexact pathだけを指定する。

```powershell
git add -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "非平坦層順ビューのグラフ発行者回帰を完成する"
```

他者のstage、dirty file、`docs/Codex`、`docs/plans`、画像、`target-*`をcommitへ含めない。amend、rebase、squash、reset、restore、stashは禁止する。pushは行わない。rootが他変更とまとめてpushする。

## 9. 完了報告

`docs/Codex/claude-sim010-viewer-graph-issuer-positive-report-2026-07-26.md`を新規作成し、stage/commitしない。

必須記載:

- 着手時/終了時HEAD
- commit hash、author、committer、subject
- exact changed path
- fixture constructorとcandidate mask探索規則
- 成功maskと完全hinge数、moving hinge数
- pose authorityがgraphである直接assertion
- graph revalidation入口
- viewer response model ID
- read-only/mutation非認可
- deterministic 2回read
- module testの実行件数、pass/fail/filtered
- format/check/clippy結果
- 外部並行差分によるblockerの有無
- `git status --short`
- 保護対象を触っていない確認
- pushしていない確認

graph positiveが正規経路で構成できなかった場合、proofを偽造して完了扱いにしない。試した全candidate、最初の失敗型、必要な最小追加境界を正確に報告して停止する。
