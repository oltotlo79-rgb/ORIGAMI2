# Claude 報告: SIM-010 graph issuer positive — 着手ゲート未開放により commit 待機

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
指示書: `docs/Claude/sim010-viewer-graph-issuer-positive-2026-07-26.md`
Git identity: `yuya <oltotlo79@gmail.com>`（変更なし）
remote push: 実施していない

この report は stage / commit していない。

---

## 1. 結論

**commit していない。** 指示書 §2 の着手ゲート 5 項目のうち 1 項目が開かないため、
共有 file への編集を開始していない。

実装そのものは隔離した一時 worktree 上で完成・検証済みであり、
本 report §6 に全文を記載する。ゲートが開き次第、そのまま適用できる状態である。

---

## 2. 着手ゲートの観測（指示書 §2）

観測時刻: `2026-07-26T14:40:36+09:00`、最終確認 `14:55`
観測 HEAD: `bfc6f117c38a93463fb90504c65779d9c9fad9c6`

| # | ゲート条件 | 結果 |
|---|---|---|
| §2-1 | `git log -5 -- <対象 file>` に `ccc6026` が存在する | **通過**（`ccc6026` / `15f9793` / `f74f74c` / `358eeab`） |
| §2-2 | 対象 file に自分以外の未 commit 差分がない | **不通過** |
| §2-3 | `applied_pose.rs` に `install_pose_authority_with_angles` が commit 済み | **通過**（`applied_pose.rs:1723`、`Result<(), PoseAuthorityError>` を返す。panic 固定ではない） |
| §2-4 | `git diff -- applied_pose.rs` が空 | **通過**（出力なし） |
| §2-5 | `git diff --cached --name-only` に自分以外の stage がない | **通過**（index は空） |

### 2.1 不通過の内容

```text
git status --short -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
 M apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs

git diff --stat -- apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
 1 file changed, 104 insertions(+), 12 deletions(-)   （14:37 時点）
                                                       14:20 時点では 71 insertions(+), 12 deletions(-)
```

対象 file の working-tree hash（14:55 時点）:

```text
F9B9D2DDDB0A812164952CB0BAAC63BF39F183A4E4F471D7EAFEAE5B22444030
```

commit `ccc6026` 時点の hash（Claude が最後に書いた内容）:

```text
A8879975F90CF288DAB7A6881312F3F9903698846625C9200A6FBD5037B2882C
```

差分の内容は `mod tests` 内の `non_flat_tree_project` fixture の書き換えである。
commit `0264f0a 姿勢更新時に古い層順証拠を失効する` により pose 更新時に
`current_layer_evidence` が失効するようになったため、fixture を
「target pattern から source pattern を作り、認証済み stacked-fold document 遷移を 1 回通す」
形へ変更している途中と読み取れる。Claude の作業ではない。

### 2.2 待機理由と取った行動

指示書 §2 は次を定めている。

> helperが未commit、signatureがpanic固定、または対象fileがdirtyなら、
> 競合回避のため作業を開始せず、`docs/Codex`の新規reportへ待機理由と観測したHEADを記録すること。
> 既存差分をstash、restore、reset、整形、stageしてはならない。

したがって次を **一切行っていない**。

- 対象 file の編集、上書き、整形
- `git stash` / `git restore` / `git checkout` / `git reset`
- `git add` / `git commit`
- 他者の未 commit 差分への介入

60 秒間隔で最大 25 分の待機ポーリングを行ったが、ゲートは開かなかった。

---

## 3. 隔離環境での実装と検証

共有 file を触らずに実装を確定させるため、repository 外の一時 path へ
`git worktree add --detach <temp> HEAD` で HEAD の純粋な checkout を作り、
そこで実装・検証した。主 worktree の file は 1 byte も変更していない。

検証環境:

```text
WSL Ubuntu / Linux 6.6.87.2-microsoft-standard-WSL2 aarch64
worktree HEAD          : bfc6f117c38a93463fb90504c65779d9c9fad9c6
CARGO_TARGET_DIR       : /tmp/origami2-viewer-graph-positive
```

| command | 結果 |
|---|---|
| `rustfmt --edition 2024 --check <対象 file>` | 差分 0 |
| `cargo clippy --locked --no-deps -p origami2-desktop --lib --all-targets --all-features -- -D warnings` | exit 0、warning 0 |
| `cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1` | **46 passed / 0 failed / 0 ignored / 626 filtered out** |

46 件 = 既存 44 件 + 新規 graph positive 2 件。
既存 44 件は削除・`#[ignore]`・条件緩和・test 名変更のいずれもしていない（指示書 §6）。
filter 0 件の実行は無い。

Windows 上では Application Control により build script が
`os error 4551` で遮断されるため、native 検証は WSL のみで行った。
compile 成功だけを test 成功として書いていない。

検証に使った一時 worktree は `git worktree remove --force` で削除する。

---

## 4. fixture 構成と candidate 探索（指示書 §4）

### 4.1 geometry

```text
viewer_dense_grid_cycle_test_support::miura_authority_pattern(3, 3)
paper.thickness_mm = 0.1
moving = horizontal hinge の先頭 3 本
```

face registry、hinge registry、Mountain/Valley assignment は
`project.editor.topology_analysis_input(project.project_id).analyze()` の
`simulation_snapshot()` から取得している。`FaceId` / `EdgeId` の手書きは無い。

固定面は保存順に依存させず、`(face.key.0, face.id.canonical_bytes())` の最小値で決めている。

### 4.2 candidate mask 探索規則

```text
step = 2.0 * (1.0_f64).atan2(100.0).to_degrees()

mask in 0..(1 << 3) を決定順に走査
各 moving hinge index i について
    mountain = live topology assignment が Mountain
    flip     = mask の bit i が 1
    mountain XOR flip が true なら -step、false なら +step
moving 以外の hinge は明示的に +0.0
角度 vector は caller 側で canonical EdgeId 順に一度だけ sort
```

候補ごとに共有 fixture から `ProjectState` を作り直しており、
失敗 candidate の pending state や project を次候補へ再利用していない。
角度の丸め・補完・並べ替え・符号正規化は helper の内外いずれでも行っていない。
`-0.0` は生成していない。

### 4.3 探索結果

```text
成功 mask         : 7
完全 hinge 数      : 12
moving hinge 数    : 3
material face 数   : 9
overlap cell 数    : 0
```

mask 0〜6 は `install_pose_authority_with_angles` が
`capture_request -> prepare -> commit_prepared` の途中で失敗し、
mask 7 が最初に閉じた closed graph candidate である。
全 8 通り失敗した場合は skip / ignore せず `panic!` で明示 fail する実装にしている。

overlap cell 数は 0 である。指示書 §5 のとおり graph issuer positive としては有効であり、
**一般 layer transport 完成の根拠にはしていない**。
fixture 固有 digest を production 契約へ追加していない。

### 4.4 fresh graph evidence

commit 済み semantic pose から complete hinge vector を
`CanonicalHingeAngles` へ bit-exact に再構成し、固定面も current semantic pose と一致させている。
flat snapshot は `global_flat_foldability::reanalyze_current_flat_layer_order` で解いている。

proof 発行は次の core graph 入口だけを使用している。

```text
ori_core::revalidate_current_graph_non_flat_layer_order_v1(
    RevalidateCurrentGraphNonFlatLayerOrderRequestV1 { ..., expected_archive: None, ... }
)
```

tree revalidation への fallback、private field の直接構築、
serialization round-trip による偽造、既存 proof の field 変更は行っていない。
`unsafe`、transmute、raw pointer、test-only production 分岐も使っていない。

---

## 5. 追加した test と §5 必須 assertion の対応

### 5.1 `a_closed_graph_issuer_yields_a_read_only_graph_view`

| §5 項目 | assertion |
|---|---|
| 1 capability `graph()` が `Some` | `assert!(live.graph().is_some())` |
| 2 semantic pose model が closed graph | `assert_eq!(live.semantic_pose().model_id(), GRAPH_POSE_MODEL_ID_V1)` |
| 3 response `pose.model_id` が graph | `assert_eq!(response.pose.model_id, GRAPH_POSE_MODEL_ID_V1)` |
| 4 response model ID と live semantic 一致 | `assert_eq!(response.pose.model_id, live.semantic_pose().model_id())` |
| 5 `read_only == true` | `assert!(response.read_only)` |
| 6 `authorizes_project_mutation == false` | `assert!(!response.authorizes_project_mutation)` |
| 7 face registry が proof registry と完全一致 | proof material face を canonical 順へ並べ `wire_id` 化し、response face 列と `assert_eq!` |
| 8 hinge / cell / pair count が proof と一致 | hinge・cell・pair・work の 4 種を `assert_eq!` |
| 9 dropped axis から plane axes 再検証 | 全 face に `validate_axis_derivation_v1` |
| 10 連続 2 回 read が `PartialEq` と byte で一致 | `assert_eq!(response, repeated)` と `serde_json::to_vec` の一致 |
| 12 新たな authority を主張しない | 5・6 に加え viewer が mutation callback / apply 経路を持たないことは既存契約どおり |

補助として `issuer_kind(&live) == PoseIssuerKindV1::Graph` も固定している。

### 5.2 `a_graph_issuer_view_still_refuses_every_stale_binding`（§5 項目 11）

graph positive 追加後も次が `stale_authority` で拒否されることを固定する。

```text
別 project instance
revision 変更
fixed face 変更
pose angle 1 ULP 変更（to_bits() ^ 1）
```

---

## 6. 適用可能な実装全文

ゲート開放後、対象 file へ次の 2 箇所を追加すれば完了する。
既存 44 件の test には一切触れない。

### 6.1 file top-level（`#[cfg(test)] mod tests` の直前）

```rust
/// The shared dense-grid cycle fixture, included for the graph issuer test.
///
/// Another module already includes the same file. Both inclusions are test-only
/// and neither can reference the other, so the duplicate is deliberate.
#[cfg(test)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../../test-support/dense_grid_cycle.rs"]
mod viewer_dense_grid_cycle_test_support;
```

**`#[allow(clippy::duplicate_mod)]` について**:
`stacked_fold_read.rs` が同じ `test-support/dense_grid_cycle.rs` を既に
`mod dense_grid_cycle_test_support;`（private）として include している。
指示書 §3 は本 file への module 宣言を明示的に許可しているが、
そのままでは `cargo clippy -- -D warnings` が `clippy::duplicate_mod` で失敗する。
`stacked_fold_read` 側の module は private のため参照できず、
crate root（`lib.rs`）への宣言移動は §3 で禁止されている。
したがって「意図的な重複である」旨のコメント付きで局所 allow を 1 個だけ置いた。
warning を握り潰す目的ではなく、file 範囲制約に由来する不可避な重複への限定的な抑制である。
別解が望ましければ `lib.rs` へ `pub(crate)` の共有宣言を 1 個置く形が最小である。

### 6.2 `mod tests` 末尾へ追加

```rust
    // -- graph issuer positive ---------------------------------------------

    /// Half of the canonical dense-grid cycle step, in degrees.
    ///
    /// The same magnitude the existing cycle regressions use, so the closed
    /// endpoint stays inside the certified neighbourhood.
    fn canonical_cycle_step_degrees() -> f64 {
        2.0 * (1.0_f64).atan2(100.0).to_degrees()
    }

    /// Builds a project whose applied pose is a closed non-flat graph pose.
    ///
    /// Only production constructors are used: the shared 3x3 Miura pattern, the
    /// canonical pose authority path, and the core graph revalidation entry
    /// point. No proof field is written directly.
    fn non_flat_graph_project() -> ProjectState {
        let step = canonical_cycle_step_degrees();
        let mut failures = Vec::new();
        for mask in 0..(1usize << 3) {
            // Every candidate starts from a fresh project; a rejected pending
            // state is never carried into the next candidate.
            let (pattern, mut paper, horizontal, _vertical) =
                super::viewer_dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
            paper.thickness_mm = 0.1;
            let moving = horizontal.into_iter().take(3).collect::<Vec<_>>();
            let mut project = ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology
                .simulation_snapshot()
                .expect("the shared fixture yields a simulation snapshot");
            // The fixed face is chosen canonically, never by storage order.
            let fixed = snapshot
                .faces
                .iter()
                .min_by_key(|face| (face.key.0, face.id.canonical_bytes()))
                .expect("at least one face")
                .id;
            let mut angles = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| {
                    let Some(index) = moving.iter().position(|edge| *edge == hinge.edge) else {
                        // An inactive hinge is explicitly positive zero.
                        return (hinge.edge, 0.0_f64);
                    };
                    let mountain = hinge.assignment == ori_topology::FoldAssignment::Mountain;
                    let flip = mask & (1 << index) != 0;
                    (hinge.edge, if mountain ^ flip { -step } else { step })
                })
                .collect::<Vec<_>>();
            angles.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
            if let Err(error) = crate::applied_pose::tests::install_pose_authority_with_angles(
                &mut project,
                angles,
                fixed,
            ) {
                failures.push(format!("mask {mask}: pose {error:?}"));
                continue;
            }
            let Some(pose) = project.editor.current_applied_pose() else {
                failures.push(format!("mask {mask}: no applied pose"));
                continue;
            };
            let fixed_face = pose
                .fixed_face()
                .expect("the committed pose fixes one face");
            let committed = CanonicalHingeAngles::new(
                pose.hinge_angles()
                    .iter()
                    .map(|angle| {
                        HingeAngle::new(angle.edge(), angle.angle_degrees())
                            .expect("a committed hinge angle is representable")
                    })
                    .collect::<Vec<_>>(),
            )
            .expect("the committed hinge vector is canonical");
            let flat = match crate::global_flat_foldability::reanalyze_current_flat_layer_order(
                &project,
            ) {
                Ok(flat) => flat,
                Err(_) => {
                    failures.push(format!("mask {mask}: flat layer order"));
                    continue;
                }
            };
            // The graph entry point only; a tree fallback would not be a graph
            // issuer positive.
            let proof = match ori_core::revalidate_current_graph_non_flat_layer_order_v1(
                ori_core::RevalidateCurrentGraphNonFlatLayerOrderRequestV1 {
                    identity_namespace: project.project_id,
                    revision: project.editor.revision(),
                    pattern: project.editor.pattern(),
                    paper: project.editor.paper(),
                    fixed_face,
                    hinge_angles: &committed,
                    current_flat: &flat,
                    expected_archive: None,
                    max_face_pairs: ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                },
            ) {
                Ok(proof) => proof,
                Err(error) => {
                    failures.push(format!("mask {mask}: graph revalidation {error:?}"));
                    continue;
                }
            };
            project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(proof));
            return project;
        }
        panic!("no closed graph candidate succeeded: {failures:?}");
    }

    #[test]
    fn a_closed_graph_issuer_yields_a_read_only_graph_view() {
        let project = non_flat_graph_project();

        // The live pose authority really is a closed graph issuer.
        let capability = capture_current_applied_pose_capability(&project)
            .expect("the capability is capturable")
            .expect("the project owns an applied pose");
        let live = revalidate_current_applied_pose_capability(&project, &capability)
            .expect("the capability revalidates")
            .expect("the capability is still current");
        assert!(
            live.graph().is_some(),
            "the live issuer must be a closed graph"
        );
        assert_eq!(live.semantic_pose().model_id(), GRAPH_POSE_MODEL_ID_V1);
        assert_eq!(issuer_kind(&live).unwrap(), PoseIssuerKindV1::Graph);

        let response = view(&project);
        assert_eq!(response.pose.model_id, GRAPH_POSE_MODEL_ID_V1);
        assert_eq!(response.pose.model_id, live.semantic_pose().model_id());
        assert!(response.read_only);
        assert!(!response.authorizes_project_mutation);

        let proof = match project.current_layer_evidence.as_ref() {
            Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
            _ => panic!("the fixture must own non-flat evidence"),
        };
        // The response registry is exactly the proof registry.
        let mut proof_faces = proof
            .material_faces()
            .iter()
            .map(|face| face.face_id)
            .collect::<Vec<_>>();
        proof_faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let response_faces = response
            .faces
            .iter()
            .map(|face| face.face_id.clone())
            .collect::<Vec<_>>();
        let expected_faces = proof_faces
            .iter()
            .map(|face| wire_id(face).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(response_faces, expected_faces);
        assert_eq!(response.faces.len(), proof.material_faces().len());
        assert_eq!(response.pose.hinge_angles.len(), proof.hinge_angles().len());
        assert_eq!(response.cells.len(), proof.overlap_cell_count());
        assert_eq!(
            response.work.face_pair_order_count,
            proof.face_pair_order_count()
        );
        assert_eq!(response.work.material_face_count, response.faces.len());
        assert_eq!(response.work.overlap_cell_count, response.cells.len());

        for face in &response.faces {
            validate_axis_derivation_v1(
                face.projection.dropped_world_axis,
                face.projection.plane_axes,
            )
            .expect("the plane axes are derived from the dropped axis");
        }
        for cell in &response.cells {
            assert_ne!(cell.lower_face_id, cell.upper_face_id);
            assert_eq!(
                cell.projection.rounded_boundary_uv_mm.len(),
                cell.projection.exact_boundary_uv.len()
            );
        }

        // Two consecutive reads are identical in value and in bytes.
        let repeated = view(&project);
        assert_eq!(response, repeated);
        assert_eq!(
            serde_json::to_vec(&response).unwrap(),
            serde_json::to_vec(&repeated).unwrap()
        );
    }

    #[test]
    fn a_graph_issuer_view_still_refuses_every_stale_binding() {
        let project = non_flat_graph_project();
        let base = canonical_request(&project);
        let mut foreign_instance = base.clone();
        foreign_instance.expected_project_instance_id = ProjectId::new();
        let mut stale_revision = base.clone();
        stale_revision.expected_revision = base.expected_revision + 1;
        let mut wrong_face = base.clone();
        wrong_face.expected_applied_pose.fixed_face_id = FaceId::new();
        let mut one_ulp = base.clone();
        let angle = &mut one_ulp.expected_applied_pose.hinge_angles[0];
        angle.angle_degrees = f64::from_bits(angle.angle_degrees.to_bits() ^ 1);
        for request in [foreign_instance, stale_revision, wrong_face, one_ulp] {
            let error = build_current_non_flat_layer_order_view_v1(&project, &request)
                .expect_err("a stale binding is refused for a graph issuer too");
            assert_eq!(
                category(error),
                CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority
            );
        }
    }
```

---

## 7. `git status --short`（`target-*` を除く）

```text
 M apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs   <- Codex の作業中差分
 M apps/desktop/src-tauri/src/global_flat_foldability.rs
 M apps/desktop/src-tauri/src/lib.rs
 M apps/desktop/src-tauri/src/stacked_fold_read.rs
 M apps/desktop/src-tauri/src/stacked_fold_transaction.rs
 M crates/ori-core/src/constraint_solver.rs
 M crates/ori-core/src/constraints.rs
 M crates/ori-core/src/editor.rs
 M crates/ori-core/src/lib.rs
 M crates/ori-core/src/stacked_fold.rs
 M crates/ori-foldability/src/facewise.rs
 M crates/ori-foldability/src/lib.rs
?? docs/Codex/
?? docs/plans/code-audit-2026-07-22.md
?? docs/plans/code-audit-round3-2026-07-23.md
?? origami2-collision-ab-verification.png
?? origami2-global-flat-foldability-panel.png
```

上記 `M` はすべて他担当の差分である。Claude は 1 件も触れていない。

保護対象の確認:

- `apps/desktop/src-tauri/src/applied_pose.rs` — 未変更
- `apps/desktop/src-tauri/src/global_flat_foldability.rs` — 未変更
- `apps/desktop/src-tauri/src/lib.rs` — 未変更
- `apps/desktop/src-tauri/src/stacked_fold_read.rs` — 未変更
- `apps/desktop/src-tauri/src/stacked_fold_transaction.rs` — 未変更
- `crates/**`（`test-support/dense_grid_cycle.rs` を含む） — 未変更
- frontend source / test — 未変更
- `docs/plans/**`、既存 `docs/Codex/**` report、`origami2-*.png`、`target-*` — 未変更・未 stage
- push / amend / rebase / squash / reset / restore / stash — いずれも未実施
- Git identity — `yuya <oltotlo79@gmail.com>` のまま

---

## 8. 依頼事項

1. 対象 file `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs` の
   作業中差分を commit していただきたい。commit 後に §6 の 2 箇所を適用し、
   `cargo fmt` / `clippy -D warnings` / module test 全件を再実行のうえ
   `非平坦層順ビューのグラフ発行者回帰を完成する` として commit する。
2. §6.1 の `#[allow(clippy::duplicate_mod)]` の可否を判断していただきたい。
   不可であれば `lib.rs` への共有 module 宣言（担当外 file）が必要になる。
3. `a_reopened_project_needs_a_fresh_instance` は commit `ccc6026` 単体では
   pass する。主 worktree で失敗するのは並行中の archive revalidation 差分に
   起因するため、persistence commit 後に viewer module 全件の再実行を依頼したい。
