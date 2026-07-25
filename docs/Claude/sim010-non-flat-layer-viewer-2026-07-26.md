# Claude 実装指示書: SIM-010 適用済み非平坦 layer order 専用 read-only viewer

作成日: 2026-07-26

対象リポジトリ: `ORIGAMI2`

対象要件: `SIM-010`

この指示書の担当範囲: 適用済みの非平坦姿勢に現在結合されている native layer evidence を、専用の読取専用ビューアーで安全に表示する

## 0. この作業の位置付け

この作業は、既存の平坦 layer-order viewer や persisted timeline proof の見た目を流用するだけの作業ではない。現在適用されている非平坦姿勢、現在の project instance、project ID、revision、fold-model fingerprint、および `CurrentLayerEvidence::NonFlat` を native 側で再結合し、その時点でのみ成立する読取専用 snapshot を新しい versioned DTO として返す。

このパッケージを完了しても、次の理由により `SIM-010` を `Complete` に変更してはならない。

- Undo 後に layer evidence は fail-closed で破棄される。
- 現状の Redo は non-flat evidence を再証明して復元しない。
- この作業では timeline proof を layer evidence の代替 authority にしない。
- canonical な全体完成度は `docs/progress.md` 記載の 79.32%（表示 79.3%）を維持する。

したがって、この作業中および完了時も `docs/requirements-status.md` の `SIM-010` は `Partial` のままにする。`docs/progress.md`、`docs/requirements-status.md`、`docs/stacked-fold-design.md` はこの担当では変更しない。

## 1. 必須の Git・worktree 規則

### 1.1 作業開始前の確認

リポジトリ root で次を実行し、結果を作業報告へ残す。

```powershell
git status --short
git branch --show-current
git rev-parse HEAD
git config --local --get user.name
git config --local --get user.email
```

local Git identity は必ず次でなければならない。

```text
user.name = yuya
user.email = oltotlo79@gmail.com
```

異なる場合は実装、stage、commit を開始せず、その事実だけを報告すること。`.git/config`、global Git config、remote URL、認証情報を Claude の判断で変更してはならない。

### 1.2 既存差分の保護

同じ worktree では Codex とユーザーの差分が並行して存在し得る。次を厳守する。

- Claude の担当ファイル以外を整形、stage、commit、restore、削除しない。
- 担当予定の既存ファイルが作業開始時点ですでに dirty なら、その差分を上書きしない。
- dirty な担当予定ファイルについては、所有者から明示的な引き渡しを受けるまで既存ファイルの編集を保留し、新規ファイルおよび競合しないテストだけを進める。
- `git reset --hard`、`git checkout --`、`git restore`、`git clean` を使用しない。
- formatter の結果に担当外ファイルが含まれた場合、そのファイルを stage しない。担当外差分を勝手に戻すこともしない。
- commit 前に `git diff --cached --name-only` を確認し、各段階で指定したファイル以外が 1 件でも含まれたら commit しない。
- remote へ直接 push しない。Codex が確認後にまとめて push する。

特に次はユーザー所有または別担当の成果物であり、絶対に触れない。

- `docs/plans/code-audit-2026-07-22.md`
- `docs/plans/code-audit-round3-2026-07-23.md`
- `origami2-collision-ab-verification.png`
- `origami2-global-flat-foldability-panel.png`
- root の `target-*`
- `docs/Claude` 内のこの指示書以外の既存ファイル

## 2. 実装前に必ず読み、`rg` で再確認する現行実装

以下の名前は、指示書作成時点で実在を確認済みである。実装前に同名 symbol と周辺コードをもう一度 `rg` で確認し、名前や意味が変わっていた場合は推測で置換しない。

### 2.1 core の非平坦 layer evidence

`crates/ori-core/src/stacked_fold.rs`

- `StackedFoldNonFlatLayerOrderV1`
- `StackedFoldNonFlatFoldedFaceV1`
- `StackedFoldNonFlatOverlapCellV1`
- `StackedFoldNonFlatFacePairOrderV1`
- `STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1`
- `DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS`
- `revalidate_current_non_flat_layer_order_v1`
- `revalidate_current_graph_non_flat_layer_order_v1`

`StackedFoldNonFlatLayerOrderV1` で使用できる既存 getter:

- `model_id`
- `identity_namespace`
- `target_revision`
- `target_fingerprint`
- `fixed_face`
- `hinge_angles`
- `folded_faces`
- `material_faces`
- `tested_face_pairs`
- `source_overlap_cells_authenticated`
- `overlap_cell_count`
- `face_pair_order_count`
- `overlap_cells`
- `face_pair_orders`
- `authorizes_apply_stacked_fold`

`StackedFoldNonFlatFoldedFaceV1` で使用できる既存 getter:

- `face`
- `dropped_world_axis`
- `source_to_plane`

`StackedFoldNonFlatOverlapCellV1` で使用できる既存 getter:

- `boundary`
- `exact_boundary`
- `lower_face`
- `upper_face`

`StackedFoldNonFlatFacePairOrderV1` で使用できる既存 getter:

- `lower_face`
- `upper_face`

重要事項:

- `StackedFoldNonFlatOverlapCellV1::boundary()` は `Point2` の丸め済み投影座標である。
- `StackedFoldNonFlatOverlapCellV1::exact_boundary()` は同じ投影境界の exact rational provenance である。
- どちらも world XYZ ではない。
- `StackedFoldNonFlatFoldedFaceV1::source_to_plane()` は source 2D から、その面が使用した world-axis projection plane への exact affine transform である。world XYZ transform ではない。
- `authorizes_apply_stacked_fold()` は `false` である。この性質を変更してはならない。

### 2.2 exact rational

`crates/ori-foldability/src/exact.rs`

- `ExactSign`
- `ExactRationalValue`
- `ExactPointValue`
- `ExactAffineTransform`

`ExactRationalValue::to_f64` を使用できる。exact value を文字列化した decimal や `f64` だけに縮退させてはならない。

### 2.3 non-flat cell transport の構造検証

`crates/ori-collision/src/non_flat_cell_transport.rs`

- `NonFlatCellTransportLimitsV1`
- `NonFlatCellTransportErrorV1`
- `preflight_non_flat_cell_transport_v1`
- private function `validate_complete`
- `certify_non_flat_cell_transport_v1`
- `certify_non_flat_cell_transport_with_limits_v1`

現行の `validate_complete` は少なくとも次を検証している。

- material face ID の一意性と非空性
- folded face が material face を過不足なく被覆すること
- dropped axis が 0、1、2 のいずれかであること
- exact affine transform の各成分が有限な `f64` に変換可能であること
- overlap cell 数と face-pair order 数が一致すること
- 各 cell boundary が 3 点以上であること
- rounded boundary と exact boundary の点数が一致すること
- exact point の `to_f64().to_bits()` と rounded point の `to_bits()` が一致すること
- lower/upper face が既知で相異なること
- cell と face-pair order の lower/upper が一致すること
- 同じ face pair の逆向き関係が混在しないこと

この検証を viewer 側へコピーして別実装にしてはならない。後述の新しい public read-only validator へ切り出し、transport certification と viewer の双方から同一実装を呼ぶ。

### 2.4 current applied pose と world geometry

`apps/desktop/src-tauri/src/applied_pose.rs`

- `capture_current_applied_pose_capability`
- `revalidate_current_applied_pose_capability`
- `CurrentAppliedPoseCapability::generation`
- `CurrentAppliedPoseView::semantic_pose`
- `CurrentAppliedPoseView::tree`
- `CurrentAppliedPoseView::graph`

`crates/ori-core/src/applied_pose.rs`

- `APPLIED_POSE_MODEL_ID_V1`
- `CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1`
- `AppliedPoseV1::model_id`
- `AppliedPoseV1::fixed_face`
- `AppliedPoseV1::hinge_angles`

現在の model ID の wire 値:

```text
tree_absolute_hinge_angles_v1
closed_graph_absolute_hinge_angles_v1
```

`crates/ori-kinematics/src/tree.rs`

- `MaterialTreeKinematicsModel::face_boundary`
- `MaterialTreePose::face_boundary`
- `MaterialTreePose::vertex_position`
- `MaterialTreePose::face_transform`
- `MaterialTreePose::face_ids`
- `MaterialFaceBoundary::vertices`
- `MaterialHingeGraphGeometry::face_ids`
- `MaterialHingeGraphGeometry::vertex_position`
- `MaterialHingeGraphGeometry::face_boundary_vertices`

`crates/ori-kinematics/src/graph.rs`

- `ClosedMaterialHingeGraphPose::fixed_face`
- `ClosedMaterialHingeGraphPose::hinge_angles`
- `ClosedMaterialHingeGraphPose::face_transform`

`crates/ori-kinematics/src/transform.rs`

- `RigidTransform::apply_point`

world outer boundary は、必ず現在の revalidated applied-pose capability に含まれる source vertex と face transform から求める。non-flat cell の UV 点を XYZ に持ち上げて作ってはならない。

### 2.5 native の layer evidence lifecycle

`apps/desktop/src-tauri/src/stacked_fold_transaction.rs`

- `CurrentLayerEvidence`
- `CurrentLayerEvidence::NonFlat`
- `CurrentLayerEvidence::CertifiedFlat`
- `PendingStackedFoldTransaction::layer_order`
- `apply_stacked_fold_transaction_inner`
- `reissue_target_pose_or_rollback`

現行の apply path は、non-flat proof の project ID、target revision、fingerprint、fixed face、hinge angles を target document と照合した後、document、timeline、layers、applied pose を atomic に反映する。

ただし現行の最後の代入は次の条件で project-owned evidence を保存している。

```rust
project.current_layer_evidence = target
    .is_none()
    .then(|| applied_layer_order.clone())
    .flatten();
```

このため target geometry を持つ tree/graph non-flat apply では transaction slot に evidence が残っても、`ProjectState::current_layer_evidence` へ evidence が入らない。この viewer の実欠落を直すため、rollback の可能性がある処理をすべて通過した後で、適用した `CurrentLayerEvidence::NonFlat` を project-owned field に保存する。`CertifiedFlat` の global flat slot 処理を拡張または意味変更してはならない。

`apps/desktop/src-tauri/src/lib.rs`

- `ProjectState::from_project_archive`
- `ProjectState::from_recovery_project_archive`
- `ProjectState::project_archive`
- `ProjectState::archived_layer_evidence`
- `revalidate_archived_layer_evidence`
- `execute_undo`
- `execute_redo`
- `wire_id`
- `lowercase_hex`

現行の重要な lifecycle:

- save 時、non-flat evidence は `LayerEvidenceArchiveKindV1::NonFlat` として archive 化される。
- open/recovery 時、archive の rounded boundary を authority として直接採用せず、tree revalidation、続いて graph fallback により fresh proof を再生成し、archive と比較する。
- `ProjectState::instance_id` は persist されず、open ごとに新しい instance になる。
- Undo と Redo は `project.current_layer_evidence = None` として fail-closed にする。

Undo/Redo のこの挙動は本パッケージでは変更しない。timeline proof から evidence を再構成してはならない。

### 2.6 既存 flat viewer と frontend

`apps/desktop/src-tauri/src/global_flat_foldability.rs`

- `CurrentLayerOrderViewRequest`
- `CurrentLayerOrderViewResponse`
- `CurrentLayerOrderCellDto`
- `get_current_layer_order_view`
- test `flat_evidence_reopens_at_canonical_revision_and_next_edit_invalidates_it`
- test `archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected`

既存の `get_current_layer_order_view` は certified-flat 専用であり、flat 2D の点を `[x, 0, -y]` に置く。非平坦 cell の UV に同じ変換を適用してはならない。既存 request、response、command を拡張せず、別 command、別 DTO、別 parser を作る。

`apps/desktop/src/lib/currentLayerOrderView.ts`

- `LayerOrderViewerCell`
- `CurrentLayerOrderView`
- `normalizeCurrentLayerOrderView`
- `getCurrentLayerOrderView`

`apps/desktop/src/components/StackedFoldPanel.tsx`

- 既存 `LayerOrderViewer`
- persisted timeline proof の表示

既存 `LayerOrderViewer` は `boundaryWorld` を flat XZ geometry として扱うため、non-flat viewer に再利用しない。

`apps/desktop/src/lib/foldPreviewAppliedPose.ts`

- `FoldPreviewAppliedPoseSnapshot`
- `createFoldPreviewAppliedPoseSnapshot`
- `foldPreviewAppliedPoseKey`

`FoldPreviewAppliedPoseSnapshot` の既存 field:

- `projectId`
- `revision`
- `fixedFaceId`
- `hingeAngles`
- `state`

`apps/desktop/src/App.tsx` には既存 state `appliedFoldPose` がある。新しい optional prop を `StackedFoldPanel` に渡すときはこれを使用する。

翻訳 catalog の実装規約は次を正本として読む。

- `apps/desktop/src/lib/effectiveCutDiagnosticPanelText.ts`
- `apps/desktop/tests/effectiveCutDiagnosticPanelText.test.ts`

## 3. 今回の完成条件

次がすべて成立したときだけ、この担当パッケージを完了と報告する。

1. non-flat 専用の version 1 native command が存在する。
2. command は current project と current applied pose と current non-flat evidence を毎回 native で再結合する。
3. world XYZ face geometry と、cell ごとの projection UV geometry が wire 上も UI 上も別の型・別の pane である。
4. exact boundary provenance を lossless DTO と SHA-256 digest で保持する。
5. response は `readOnly: true` かつ `authorizesProjectMutation: false` であり、それ以外の値を生成できない。
6. apply 後の project-owned `CurrentLayerEvidence::NonFlat` が viewer から取得できる。
7. save/open 後は archive の fresh native revalidation を通った evidence だけが、新しい project instance に結合されて取得できる。
8. Undo 後と Redo 後は evidence を復活させず、viewer は非表示のままである。
9. stale、ABA、foreign project、revision mismatch、fingerprint mismatch、pose mismatch、resource overflow は fail-closed になる。
10. TypeScript parser は exact-key、own-data-property、上限、相互整合性を検証し、detached deep-frozen value だけを返す。
11. UI は表示専用で、Apply、Undo、Redo、project mutation を起動する control を一切持たない。
12. 日本語と英語の表示文言、ARIA 名、empty/error state を catalog 化する。
13. 指定した focused test、全体 test、lint、build が通る。
14. commit は後述の 3 段階に分け、日本語 message を使用する。
15. direct push を行わない。

## 4. 明確な非目標・禁止事項

- existing `get_current_layer_order_view` の wire contract を変更しない。
- existing `currentLayerOrderView.ts` の型へ non-flat variant を追加しない。
- existing `LayerOrderViewer` を non-flat 用に条件分岐させない。
- cell UV を `[u, 0, -v]`、`[u, v, 0]` などへ変換して「world boundary」と呼ばない。
- `source_to_plane` を world transform として使用しない。
- archive の rounded boundary を exact provenance と呼ばない。
- persisted timeline proof を current layer evidence の代わりにしない。
- transaction slot の古い proof を viewer source にしない。
- response、hash、generation、read-only flag を apply authority に昇格させない。
- viewer response を `apply_stacked_fold_transaction_inner` その他の mutation path へ入力しない。
- cap 超過時に一部だけを返す、先頭 N 件へ truncate する、sampling する、silent fallback する実装にしない。
- native error に project ID、instance ID、revision、fingerprint、face ID、edge ID、座標、証明内容を含めない。
- UI error に raw native error、raw UUID の全長、exact numerator/denominator の全量を表示しない。
- Redo 時の自動 reproof を本パッケージへ追加しない。
- `SIM-010` を `Complete` にしない。
- 全体完成度を更新しない。
- 新しい npm package、Rust crate、hash library を追加しない。既存 dependency だけを使う。
- network access、remote branch 操作、direct push を行わない。

## 5. 新規 native wire contract

この節の型名、command 名、module 名は「今回新規作成する名前」である。現行 symbol として存在すると誤記しないこと。

### 5.1 新規 module と command

新規ファイル:

```text
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
```

新規 command:

```text
get_current_non_flat_layer_order_view_v1
```

新規 request 型:

```text
CurrentNonFlatLayerOrderViewRequestV1
```

新規 response 型:

```text
CurrentNonFlatLayerOrderViewResponseV1
```

新規 error 型:

```text
CurrentNonFlatLayerOrderViewErrorV1
```

command の意味上の戻り値:

```text
Result<Option<CurrentNonFlatLayerOrderViewResponseV1>,
       CurrentNonFlatLayerOrderViewErrorV1>
```

`Ok(None)` は次の真の不在だけに限定する。

- `ProjectState::current_layer_evidence` が `None`
- current evidence が `CurrentLayerEvidence::CertifiedFlat`

non-flat evidence があるのに project、pose、proof、resource の再結合に失敗した場合は `Ok(None)` に落とさず、data-free error を返す。

### 5.2 request JSON

Rust の全 request/nested request struct に `#[serde(rename_all = "camelCase", deny_unknown_fields)]` を付ける。

```json
{
  "version": 1,
  "expectedProjectInstanceId": "canonical-project-instance-id",
  "expectedProjectId": "canonical-project-id",
  "expectedRevision": 12,
  "expectedFoldModelFingerprintSha256": "64 lowercase hex chars",
  "expectedAppliedPose": {
    "fixedFaceId": "canonical-face-id",
    "hingeAngles": [
      {
        "edgeId": "canonical-edge-id",
        "angleDegrees": 73.5
      }
    ]
  }
}
```

request の必須規則:

- `version` は整数 `1` のみ。
- project instance ID、project ID、face ID、edge ID は既存 domain ID 型で deserialize し、既存 `wire_id` と同じ canonical wire form であること。
- `expectedRevision` は現在の exact revision と一致すること。
- fingerprint はちょうど 64 文字の lowercase hexadecimal。
- `fixedFaceId` は non-null。proof、editor current pose、revalidated applied-pose capability の fixed face と一致すること。
- `hingeAngles` は 1 件以上 4,096 件以下。
- edge ID は一意で、canonical wire ID の code-unit 昇順。
- angle は finite、`0.0 <= angleDegrees <= 180.0`。
- negative zero は許可しない。
- 少なくとも 1 件は `0.0` と `180.0` のどちらでもない。完全 flat endpoint の request を non-flat viewer へ送らない。
- TypeScript client は `FoldPreviewAppliedPoseSnapshot` から detached copy を作り、edge ID 順へ canonical sort して送る。
- native は request の angle と proof/current pose の angle を edge ID と `f64::to_bits()` で比較する。epsilon 比較、decimal 文字列比較、並び順だけの比較は禁止する。

### 5.3 response JSON

次を version 1 の exact key set とする。ここに field を追加する場合は version を上げる。parser 側だけ optional にすることは禁止する。

```json
{
  "version": 1,
  "modelId": "native_stacked_fold_non_flat_planar_order_v1",
  "projectInstanceId": "canonical-project-instance-id",
  "projectId": "canonical-project-id",
  "revision": 12,
  "foldModelFingerprintSha256": "64 lowercase hex chars",
  "pose": {
    "modelId": "tree_absolute_hinge_angles_v1",
    "generation": "7",
    "fixedFaceId": "canonical-face-id",
    "hingeAngles": [
      {
        "edgeId": "canonical-edge-id",
        "angleDegrees": 73.5
      }
    ]
  },
  "faces": [
    {
      "faceId": "canonical-face-id",
      "faceKeySha256": "64 lowercase hex chars",
      "worldOuterBoundaryXyzMm": [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 5.0, 2.0]
      ],
      "projection": {
        "droppedWorldAxis": "z",
        "planeAxes": ["x", "y"],
        "sourceToPlaneProjectionExact": {
          "m00": {
            "sign": "positive",
            "numeratorMagnitudeHex": "01",
            "denominatorMagnitudeHex": "01"
          },
          "m01": {
            "sign": "zero",
            "numeratorMagnitudeHex": "",
            "denominatorMagnitudeHex": "01"
          },
          "m10": {
            "sign": "zero",
            "numeratorMagnitudeHex": "",
            "denominatorMagnitudeHex": "01"
          },
          "m11": {
            "sign": "positive",
            "numeratorMagnitudeHex": "01",
            "denominatorMagnitudeHex": "01"
          },
          "tx": {
            "sign": "zero",
            "numeratorMagnitudeHex": "",
            "denominatorMagnitudeHex": "01"
          },
          "ty": {
            "sign": "zero",
            "numeratorMagnitudeHex": "",
            "denominatorMagnitudeHex": "01"
          }
        }
      }
    }
  ],
  "cells": [
    {
      "cellKeySha256": "64 lowercase hex chars",
      "exactBoundarySha256": "64 lowercase hex chars",
      "lowerFaceId": "canonical-face-id",
      "upperFaceId": "canonical-face-id",
      "projection": {
        "droppedWorldAxis": "z",
        "planeAxes": ["x", "y"],
        "roundedBoundaryUvMm": [
          [1.25, 3.5],
          [5.0, 3.5],
          [1.25, 8.0]
        ],
        "exactBoundaryUv": [
          {
            "u": {
              "sign": "positive",
              "numeratorMagnitudeHex": "05",
              "denominatorMagnitudeHex": "04"
            },
            "v": {
              "sign": "positive",
              "numeratorMagnitudeHex": "07",
              "denominatorMagnitudeHex": "02"
            }
          }
        ]
      }
    }
  ],
  "work": {
    "testedFacePairs": 1,
    "materialFaceCount": 2,
    "sourceOverlapCellsAuthenticated": 0,
    "overlapCellCount": 1,
    "facePairOrderCount": 1,
    "worldBoundaryPointCount": 6,
    "exactBoundaryPointCount": 3
  },
  "readOnly": true,
  "authorizesProjectMutation": false
}
```

上の JSON は shape の例であり、sample の `exactBoundaryUv` 点数と `roundedBoundaryUvMm` 点数を実 fixture として使ってはならない。実 response では両者の点数は必ず完全一致させる。

### 5.4 exact rational DTO

新規 DTO の意味上の型:

```text
ExactRationalDtoV1 {
  sign: "negative" | "zero" | "positive",
  numeratorMagnitudeHex: string,
  denominatorMagnitudeHex: string
}
```

規則:

- numerator と denominator は `ExactRationalValue` が保持する canonical big-endian magnitude bytes を lowercase even-length hex にしたもの。
- zero は `sign == "zero"`、`numeratorMagnitudeHex == ""`、`denominatorMagnitudeHex == "01"`。
- non-zero numerator は空でなく、偶数長で、先頭 byte が `00` でなく、全体が lowercase hex。
- denominator は必ず空でなく、偶数長、0 ではなく、先頭 byte が `00` でない。
- non-zero rational の numerator magnitude は 0 でない。
- sign と numerator zero/non-zero の関係が一致しない値を拒否する。
- decimal string、scientific notation、base64、signed two's complement へ置換しない。
- UI の描画には `roundedBoundaryUvMm` を使用する。exact rational は provenance、digest、検証用であり、JS で巨大整数除算して描画しない。

### 5.5 world axis と projection plane の対応

core の `dropped_world_axis` は wire では文字列へ変換する。

| core 値 | `droppedWorldAxis` | `planeAxes` | UV の意味 |
|---:|---|---|---|
| `0` | `"x"` | `["y", "z"]` | `u = world Y`, `v = world Z` |
| `1` | `"y"` | `["x", "z"]` | `u = world X`, `v = world Z` |
| `2` | `"z"` | `["x", "y"]` | `u = world X`, `v = world Y` |

`planeAxes` を自由な string array として扱わず、上の 3 組だけを許可する。cell の lower face と upper face に対応する `StackedFoldNonFlatFoldedFaceV1` は同じ dropped axis を持たなければならない。異なる場合は evidence 不整合として fail-closed にする。

### 5.6 deterministic SHA-256

新しい hash helper は `current_non_flat_layer_order_view.rs` 内の private helper として作る。既存 helper で同じ framing を行うものが見つかった場合だけ再利用し、名前から意味を推測して流用しない。

すべての hash preimage は次を満たす。

- domain separator を先頭に置く。
- 可変長 byte 列は `u64` big-endian length と bytes で frame する。
- count も `u64` big-endian。
- ID は既存 `wire_id` が返す canonical UTF-8 bytes を frame する。
- finite `f64` は response 構築時に `-0.0` を `+0.0` へ canonicalize した後、`to_bits().to_be_bytes()` を使用する。計算元や proof を書き換えず、wire/hash 用 copy だけを正規化する。
- exact rational は sign tag 1 byte、numerator length/bytes、denominator length/bytes で frame する。
- hash は lowercase 64-character hex とする。

domain separator:

```text
origami2.non_flat_layer_view.v1.face
origami2.non_flat_layer_view.v1.exact_boundary
origami2.non_flat_layer_view.v1.cell
```

`faceKeySha256` の preimage:

1. face domain
2. face ID
3. world boundary point count
4. 各 XYZ の `f64` bits
5. dropped-axis tag
6. plane-axis tags
7. `sourceToPlaneProjectionExact` の 6 rational を `m00,m01,m10,m11,tx,ty` 順

`exactBoundarySha256` の preimage:

1. exact-boundary domain
2. dropped-axis tag
3. plane-axis tags
4. exact point count
5. 各点の `u`、`v` rational

`cellKeySha256` の preimage:

1. cell domain
2. lower face ID
3. upper face ID
4. `exactBoundarySha256` の raw 32 bytes

native test で同じ fixture の再実行が byte-identical であること、face/cell input の 1 bit 変更で digest が変わることを確認する。TypeScript parser は SHA-256 を再計算したと主張せず、形式、一意性、canonical order を検証する。

### 5.7 canonical ordering

- response `pose.hingeAngles`: `edgeId` code-unit 昇順、重複なし。
- response `faces`: `faceId` code-unit 昇順、重複なし。
- response `cells`: `cellKeySha256` code-unit 昇順、重複なし。
- face pair は方向付きであり、lower/upper を ID 順に並べ替えてはならない。
- polygon の点順は core/live model の canonical walk を保持し、表示都合で reverse、rotate、deduplicate しない。
- exact boundary と rounded boundary は同じ index の点を表す。

## 6. resource caps

次を viewer v1 の固定上限とし、native と TypeScript parser で同じ値を使用する。変更する場合は両側と test fixture を同一 commit で更新する。

```text
max faces                         = 512
max hinges                        = 4,096
max overlap cells                 = 4,096
max face-pair orders              = 4,096
max points in one world polygon   = 4,096
max points in one cell polygon    = 4,096
max total world boundary points   = 100,000
max total exact boundary points   = 100,000
max exact magnitude bytes total   = 8 MiB
max final serialized JSON bytes   = 16 MiB
```

追加規則:

- face 数は 1 以上。
- 各 world face boundary は 3 点以上。
- 各 cell boundary は 3 点以上。
- cell 数と face-pair order 数は一致。
- material face 数、folded face 数、response face 数は一致。
- `work.materialFaceCount == faces.length`
- `work.overlapCellCount == cells.length`
- `work.facePairOrderCount == cells.length`
- `work.worldBoundaryPointCount` は response faces の全 world point 数と一致。
- `work.exactBoundaryPointCount` は response cells の全 exact point 数と一致。
- `sourceOverlapCellsAuthenticated` と `testedFacePairs` は non-negative safe integer であり、core の既存 work bound を超えない。
- すべての加算と byte-size 計算に checked arithmetic を使う。
- 大きな `Vec` を確保する前に既知 count を preflight する。
- exact magnitude byte 総量は hex 化前の source bytes で checked accumulation する。
- response 構築後、Tauri へ返す前に `serde_json::to_vec` で最終 JSON byte 数を確認する。16 MiB 超過なら全体を拒否する。
- cap 超過を空配列や `Ok(None)` に変換しない。

`NonFlatCellTransportLimitsV1` の default は viewer cap より大きい。viewer は上記 cap を設定した `NonFlatCellTransportLimitsV1` を `preflight_non_flat_cell_transport_v1` に渡し、さらに world point、exact byte、最終 JSON の viewer 固有 cap を検証する。

## 7. native 実装手順

### 7.1 共通 structural validator の切り出し

`crates/ori-collision/src/non_flat_cell_transport.rs` へ次の新規 public function を作る。

```text
validate_non_flat_layer_order_structure_v1
```

意味上の signature:

```rust
pub fn validate_non_flat_layer_order_structure_v1(
    value: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), NonFlatCellTransportErrorV1>
```

実装要件:

- 現在の private `validate_complete` の検証をそのまま 1 箇所へ移す。
- `certify_non_flat_cell_transport_with_limits_v1` は新しい public function を呼ぶ。
- viewer も新しい public function を呼ぶ。
- transport certification の binding/transition authority は viewer へ持ち込まない。
- public function 自体は validation のみで proof や mutation capability を発行しない。
- error variant と既存 test の意味を変えない。
- `crates/ori-collision/src/lib.rs` から明示的に re-export する。

最低限の regression:

- complete evidence を受理。
- folded face の欠落を拒否。
- duplicate material face を拒否。
- dropped axis `3` を拒否。
- exact/rounded point の bit mismatch を拒否。
- cell/pair mismatch を拒否。
- unknown face を拒否。
- lower == upper を拒否。
- reverse-direction crossing を拒否。
- certification path が以前と同じ validator を通る。

### 7.2 command の lock と再結合順序

新しい command は次の順序を守る。

1. `lock_project` で project を lock。
2. request の project instance ID、project ID、revision、fold-model fingerprint を current project と exact 比較。
3. `project.current_layer_evidence` を確認。
4. evidence が `None` または `CertifiedFlat` なら `Ok(None)`。
5. evidence が `NonFlat` なら、cap の範囲内で read-only clone/capture に必要な予約を行う。
6. `capture_current_applied_pose_capability(&project)` を呼ぶ。
7. 同じ project lock を保持したまま `revalidate_current_applied_pose_capability(&project, &capability)` を呼ぶ。
8. capability generation、semantic pose、tree/graph issuer pair、proof をすべて検証。
9. structural validator と全 resource preflight を実行。
10. world faces、projection metadata、cells を構築。
11. response binding、count、hash、flags を最終再検証。
12. final JSON byte cap を検証。
13. project lock を解放して response を返す。

fixed lock order は project first、project-owned applied-pose authority second とする。逆順に lock しない。global flat layer slot はこの command では lock しない。

### 7.3 project/evidence/pose の必須照合

以下のすべてが一致しなければ data-free stale/invalid error とする。

- request `expectedProjectInstanceId` と `ProjectState::instance_id`
- request `expectedProjectId` と `ProjectState::project_id`
- request `expectedRevision` と `project.editor.revision()`
- request fingerprint と `project.editor.fold_model_fingerprint_v1()` の現在値
- proof `model_id()` と `STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1`
- proof `identity_namespace()` と current project ID
- proof `target_revision()` と current revision
- proof `target_fingerprint()` と current fold-model fingerprint
- proof `fixed_face()` と request fixed face
- proof fixed face と `AppliedPoseV1::fixed_face()`
- proof fixed face と tree/graph live pose の fixed face
- proof hinge-angle vector と request hinge-angle vector
- proof hinge-angle vector と `AppliedPoseV1::hinge_angles()`
- proof hinge-angle vector と tree/graph live pose の complete canonical hinge angles
- `project.editor.current_applied_pose()` と revalidated `CurrentAppliedPoseView::semantic_pose()`
- live model face ID set と proof material face ID set
- proof folded-face ID set と proof material face ID set
- request/current proof が実際に非平坦であること

angle は ID と `to_bits()` の完全一致とする。set が同じでも duplicate、missing、extra、非 canonical order があれば拒否する。

`CurrentAppliedPoseCapability::generation()` は `0` を拒否し、response では JSON number ではなく canonical decimal string にする。

generation string の規則:

- ASCII decimal digit のみ。
- leading zero なし。
- `"0"` は不可。
- `u64` へ round-trip 可能。

これにより JavaScript の safe-integer 上限を超える native generation でも同一性を失わない。

### 7.4 tree の world outer boundary

`CurrentAppliedPoseView::tree()` が `Some((model, pose))` の場合:

1. proof material face を canonical face ID 順に走査。
2. `model.face_boundary(face_id)` または同一 issuer の `pose.face_boundary(face_id)` から `MaterialFaceBoundary` を得る。
3. `MaterialFaceBoundary::vertices()` の順序を保持。
4. 各 vertex について `pose.vertex_position(vertex_id)` から source `Point3` を得る。
5. `pose.face_transform(face_id)` を得る。
6. `RigidTransform::apply_point` で source point を world point へ変換。
7. XYZ が finite で point cap 内であることを確認し、wire/hash 用 copy の negative zero だけを positive zero へ canonicalize。

model と pose が同じ issuer に結合されていることは `CurrentAppliedPoseView` の revalidation 境界に依存し、別に再 solve した pose や preview の Three.js geometry を混ぜない。

### 7.5 graph の world outer boundary

`CurrentAppliedPoseView::graph()` が `Some((geometry, pose))` の場合:

1. proof material face を canonical face ID 順に走査。
2. `geometry.face_boundary_vertices(face_id)` から canonical outer vertex walk を得る。
3. 各 vertex について `geometry.vertex_position(vertex_id)` から source `Point3` を得る。
4. `pose.face_transform(face_id)` を得る。
5. `RigidTransform::apply_point` で world point を求める。
6. XYZ が finite で point cap 内であることを確認し、wire/hash 用 copy の negative zero だけを positive zero へ canonicalize。

tree と graph が同時に `Some`、または両方 `None` になる場合は internal invariant failure として拒否する。

### 7.6 projection metadata と cell

- face の projection metadata は proof の `folded_faces()` から直接得る。
- exact affine の 6 成分を exact rational DTO に変換する。
- cell の rounded UV は `StackedFoldNonFlatOverlapCellV1::boundary()` から得る。
- cell の exact UV は `StackedFoldNonFlatOverlapCellV1::exact_boundary()` から得る。
- exact UV の `to_f64().to_bits()` と rounded UV の `to_bits()` が各点で一致することを shared validator と response builder の境界で確認する。
- lower/upper face を folded-face map で引き、dropped axis が等しいことを確認する。
- cell の projection axes はその dropped axis から節 5.5 の表だけで導出する。
- world face polygon と cell UV polygon を交差変換しない。
- cell が 0 件でも valid response である。ただし UI で「衝突がないことの証明」とは表示しない。

### 7.7 response authority の固定

native constructor は次を literal に設定する。

```text
readOnly = true
authorizesProjectMutation = false
```

request からこれらを受け取らない。response builder の test で false/true 以外を生成する path がないことを確認する。core proof の `authorizes_apply_stacked_fold()` が `false` であることも debug assertion だけでなく実検証する。

### 7.8 data-free error

新規 error category は最低限次に閉じる。

```text
stale_authority
invalid_evidence
resource_limit
internal_failure
```

error serialization は version と category の固定 field だけにする。`Display` や debug source を Tauri payload に渡さない。

分類:

- foreign instance/project/revision/fingerprint、pose generation mismatch、pose/proof mismatch: `stale_authority`
- structural coverage、axis、exact/rounded、pair contradiction: `invalid_evidence`
- viewer cap、checked arithmetic、JSON byte cap: `resource_limit`
- lock poisoning、serialization failure、issuer invariant failure: `internal_failure`

frontend は category ごとの localized summary を表示してよいが、native data を補間しない。

### 7.9 command 登録

`apps/desktop/src-tauri/src/lib.rs` で:

- new module を宣言。
- `tauri::generate_handler!` に `get_current_non_flat_layer_order_view_v1` を 1 回だけ追加。
- existing `get_current_layer_order_view` を残す。
- test-only shortcut や unregistered duplicate command を作らない。

`apps/desktop/tests/tauriCapabilityContract.test.ts` が handler 登録と frontend invoke 名の一致を検証できる状態にする。既存 test が自動検出するなら test source を不要に変更しない。

## 8. apply、Undo/Redo、save/open の挙動

### 8.1 apply 後の project-owned evidence

`apply_stacked_fold_transaction_inner` の末尾を狭く修正する。

要件:

- document commit、target pose reissue、flat layer install など rollback の可能性がある処理が成功した後にだけ evidence を install。
- `applied_layer_order` が `Some(CurrentLayerEvidence::NonFlat(proof))` なら、target geometry の有無にかかわらず `project.current_layer_evidence` へ同じ proof を保存。
- `target.is_none()` の既存 path が必要とする evidence 保存を壊さない。
- `CertifiedFlat` は既存 global flat layer capability の install 成否を正本とし、non-flat 用修正で二重 authority を作らない。
- transaction slot の `applied_layer_order` は既存の diagnostic/persistence semantics を維持する。
- install failure を partial success にしない。
- response viewer の型を transaction に渡さない。

修正前に `applied_layer_order`、`target`、`layer_guard` の全 match arm を読み、単純に代入行だけを無条件 clone に置換して flat semantics を変えない。

意味上は次の狭い分岐にする。実際の borrow/clone は現行 ownership に合わせる。

```rust
project.current_layer_evidence = match applied_layer_order.as_ref() {
    Some(CurrentLayerEvidence::NonFlat(_)) => applied_layer_order.clone(),
    _ if target.is_none() => applied_layer_order.clone(),
    _ => None,
};
```

この分岐を採用する前後で、`CertifiedFlat` の既存 test と non-flat target geometry の新規 test を両方通す。

### 8.2 Undo/Redo

既存 `execute_undo` と `execute_redo` が `project.current_layer_evidence = None` にする挙動を維持する。

受入挙動:

- Apply 成功直後: viewer response が存在。
- Undo 成功直後: `Ok(None)`、UI 非表示。
- その後 Redo 成功直後: evidence を timeline から復活させず `Ok(None)`、UI 非表示。
- Redo 後に fresh native reproof が別の既存正規操作で得られた場合だけ、以後の viewer response を許可。

この制約を UI の cache でも守る。Undo 前の response を Redo 後に再表示しない。

### 8.3 save/open/recovery

`ProjectState::archived_layer_evidence` と `revalidate_archived_layer_evidence` の現行設計を維持する。

受入挙動:

1. non-flat Apply 後に project archive を作る。
2. archive を `ProjectState::from_project_archive` で開く。
3. 新しい `instance_id` と canonical reopened revision に対する fresh snapshot を得る。
4. archive の rounded cell dataだけではなく、`revalidate_current_non_flat_layer_order_v1` または `revalidate_current_graph_non_flat_layer_order_v1` で fresh exact proof が生成済みであることを確認。
5. old instance の viewer request は `stale_authority`。
6. reopened instance/current revision/current fingerprint/current stable applied pose の request は response を得る。
7. archive の face、cell、pair、pose、fingerprint の tamper は reopen または viewer 前の revalidation で拒否。
8. recovery open も同じ fresh-instance/fresh-proof 原則。

archive schema に exact rational を新規永続化する必要はない。viewer exact data は reopened live proof から得る。

## 9. TypeScript strict parser/client

### 9.1 新規ファイル

```text
apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
apps/desktop/tests/currentNonFlatLayerOrderView.test.ts
```

新規 export の推奨名:

```text
CurrentNonFlatLayerOrderViewV1
normalizeCurrentNonFlatLayerOrderViewV1
getCurrentNonFlatLayerOrderViewV1
```

これらも今回新規作成する名前である。既存 `currentLayerOrderView.ts` へ混在させない。

### 9.2 parser の防御要件

parser input は `unknown` とし、次を再帰的に検証する。

- object は `null` でなく、array でない。
- exact own key set。欠落 field と余分 field を拒否。
- `Object.getOwnPropertyDescriptors` を使い、必要 field が own data property であることを確認。
- getter/setter/accessor property を拒否し、実行しない。
- prototype から継承した field を拒否。
- Proxy/reflection trap が throw した場合は catch して全体を拒否。
- array は dense own data elements のみ。hole、extra enumerable/non-enumerable named property、accessor index を拒否。
- string、boolean、number は期待型と exact value を検証。
- number は finite。integer field は `Number.isSafeInteger`。
- `-0` を拒否。
- ID は既存 canonical ID policy と一致。
- digest/fingerprint は `/^[0-9a-f]{64}$/u`。
- generation は `/^[1-9][0-9]*$/u` かつ `BigInt` で `1..=u64::MAX`。
- cap を配列走査前に確認。
- count と実配列長・point 合計を checked safe-integer arithmetic で再計算。
- ordering と uniqueness を検証。
- lower/upper face は存在する別 face。
- cell lower/upper face の projection axis は一致。
- `droppedWorldAxis` と `planeAxes` の組は節 5.5 の 3 組のみ。
- exact/rounded boundary の点数は同じで 3..4096。
- exact rational hex の規則は節 5.4 と同じ。
- exact magnitude byte 総量は 8 MiB 以下。
- `readOnly === true`。
- `authorizesProjectMutation === false`。
- model ID は指定した 3 種の literal だけ。

parser は入力 object や nested array/object をそのまま返さない。

1. primitive を検証。
2. 新しい plain object/array へ copy。
3. 最深部から `Object.freeze`。
4. top-level まで全階層を freeze。

入力を後から mutate しても正規化済み value が変化しないことを test する。

### 9.3 client

`getCurrentNonFlatLayerOrderViewV1` は:

- invoke 名を `get_current_non_flat_layer_order_view_v1` に固定。
- request を exact shape で生成。
- `FoldPreviewAppliedPoseSnapshot` が `state === "stable"` でなければ invoke しない。
- snapshot project ID/revision と applied pose project ID/revision が一致しなければ invoke しない。
- `fixedFaceId === null` なら invoke しない。
- fold-model fingerprint は `ProjectSnapshot.fold_model_fingerprint` を使用。
- project instance ID は `ProjectSnapshot.project_instance_id` を使用。
- project ID は `ProjectSnapshot.project_id` を使用。
- revision は `ProjectSnapshot.revision` を使用。
- response `null` は真の不在として返す。
- non-null raw response は parser を通し、失敗した raw value を UI へ渡さない。
- native error object をそのまま投げ直して UI に表示しない。category を閉じた frontend error kind へ写す。

response 受領後、request と次を完全照合する。

- project instance ID
- project ID
- revision
- fold-model fingerprint
- fixed face
- hinge count、edge ID、angle bits 相当の `Object.is` 一致

JavaScript の number では native `f64` の全 NaN payload などを扱えないが、この contract は finite 0..180 に限定される。`Object.is` を使い、`-0` は事前に拒否する。

## 10. frontend ABA と非同期 stale 防止

新 viewer component は、load を開始した時点の次を closure に保持する。

- `ProjectSnapshot` の object reference
- `FoldPreviewAppliedPoseSnapshot` の object reference
- project instance ID
- project ID
- revision
- fingerprint
- fixed face
- canonical hinge vector
- request sequence number

response を state へ採用する直前に、すべてが現在値と一致することを確認する。

追加要件:

- component 内に source applied-pose object reference を保存する。
- prop の applied-pose object reference が変わったら、同じ ID/revision/angles に見えても以前の pixels を同期的に隠す。
- sequence counter と disposed flag で late response を破棄する。
- unmount 後に state を更新しない。
- stable から running/blocked/indeterminate へ変わった時点で response を隠す。
- project instance、project ID、revision、fingerprint のどれかが変わった時点で response を隠す。
- `null` response を受け取ったら古い ready state を保持しない。
- error response を受け取ったら古い ready state を保持しない。
- 同じ semantic pose が一度失われてから再度現れても、古い response を再利用しない。
- native generation は表示 snapshot の provenance であり mutation authority ではない。

これにより、close/open で同じ project ID と revision が再登場する ABA、Undo/Redo、同値 pose の再発行、遅延 Tauri response に対して fail-closed にする。

## 11. UI 実装

### 11.1 新規ファイル

```text
apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
apps/desktop/src/lib/currentNonFlatLayerOrderViewerText.ts
apps/desktop/tests/currentNonFlatLayerOrderViewerText.test.ts
apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx
```

既存 integration 変更:

```text
apps/desktop/src/components/StackedFoldPanel.tsx
apps/desktop/tests/stackedFoldPanel.dom.test.tsx
apps/desktop/src/App.tsx
apps/desktop/src/App.css
```

### 11.2 `StackedFoldPanel` 接続

`StackedFoldPanel` の `Props` に次の optional prop を追加する。

```text
appliedPose?: FoldPreviewAppliedPoseSnapshot | null
```

default は `null`。これにより既存 DOM test が prop を渡さない場合、Tauri invoke を発生させない。

`App.tsx` の既存 `<StackedFoldPanel>` から:

```text
appliedPose={appliedFoldPose}
```

を渡す。

viewer component には少なくとも次を渡す。

- locale
- current `ProjectSnapshot`
- current applied pose

viewer は mutation callback を受け取らない。`onApplied`、`refreshSnapshot`、Undo/Redo callback を渡さない。

### 11.3 二つの pane を分離

同一 card 内に、意味が明確に異なる二つの pane を設ける。

#### A. World XYZ / mm pane

- heading と aria-label に「World XYZ」「mm」を明記。
- source は `faces[].worldOuterBoundaryXyzMm` のみ。
- SVG の isometric screen projection は表示変換として許可するが、data model は XYZ のまま保持。
- X、Y、Z axis legend を表示。
- face outer boundary を face ごとに描画。
- selected face を視覚だけでなく `aria-selected` または同等の semantic state で示す。
- face 選択 control は keyboard 操作可能。
- selected cell の lower/upper face を world pane で強調してよい。
- raw face UUID 全長を主表示にせず、短縮 label と accessible full label を使う。

#### B. Selected cell projection UV / mm pane

- heading と aria-label に「Projection UV」「mm」を明記。
- source は選択した cell の `roundedBoundaryUvMm` のみ。
- `droppedWorldAxis` と `planeAxes` を表示。
- U と V がどの world axis に対応するか localized text で明記。
- lower face、upper face、exact boundary digest を表示。
- exact rational の巨大 numerator/denominator 全量を DOM に展開しない。
- cell selector は最大 4,096 件を一度に polygon DOM 化しない。選択中の 1 cell だけを描画し、一覧は bounded selector/list とする。
- cell polygon を world pane に重ねない。

二つの pane の間に「world face outline と per-plane cell projection は同じ座標系ではない」ことを明示する localized explanatory text を置く。

### 11.4 state

最低限の view state:

```text
hidden
loading
absent
ready
failed
```

規則:

- applied pose が stable でないときは `hidden`。
- applied pose と project snapshot が不一致なら `hidden`。
- native `null` は `absent`。古い geometry は表示しない。
- valid response は `ready`。
- parser failure、native category error は `failed`。古い geometry は表示しない。
- response `cells.length === 0` は `ready`。world faces と「表示可能な overlap cell はない。ただし衝突なしの証明ではない」という warning を表示。
- locale 変更だけでは refetch しない。
- locale 変更で selected face/cell を失わない。
- response identity が変わった場合、同じ face/cell ID が新 response に存在するときだけ選択を維持し、それ以外は canonical first item へ戻す。

### 11.5 read-only 表現

- card に read-only badge を表示。
- `authorizesProjectMutation: false` の意味を localized help として表示。
- Apply、Commit、Adopt、Use as proof、Retry apply の button を作らない。
- retry button を設ける場合も同じ current request の再読込だけにし、project mutation を起動しない。
- viewer response を component 外の mutation state へ渡さない。
- CSS class は既存 `.stacked-fold-layer-viewer` を意味変更せず、non-flat 専用 prefix を新設する。

### 11.6 翻訳 catalog

`currentNonFlatLayerOrderViewerText.ts` は既存 catalog 規約へ合わせる。

- closed key union。
- `Readonly<Record<..., LocalizedText>>`。
- 各 `{ ja, en }` と top-level object を `Object.freeze`。
- `satisfies` で key 完備性を compile-time 検証。
- 固定文言は `selectLocalizedText`。
- placeholder を持つ文言は `formatLocalizedText`。
- ja/en の placeholder 集合を完全一致。
- component 内の `locale === "ja"`、inline `{ ja, en }`、日本語 literal による表示分岐を禁止。
- wire model ID、axis code、digest、ID 自体は翻訳しない。

必要な文言群:

- card title
- loading、absent、failed 4 category
- read-only badge
- no mutation authority explanation
- world pane heading/ARIA
- projection pane heading/ARIA
- X/Y/Z/U/V axis labels
- dropped-axis explanation 3 種
- lower/upper face
- selected face/cell
- exact boundary digest
- face/cell count
- zero-cell warning
- coordinate systems are distinct warning
- retry read-only snapshot

## 12. native test matrix

新規 module の unit test と既存 lifecycle test を組み合わせ、最低限次を網羅する。

### 12.1 陽性

| Case | 期待 |
|---|---|
| tree non-flat apply 後 | current evidence が install され response を取得 |
| graph non-flat apply 後 | graph world transform から response を取得 |
| dropped X face/cell | axes が YZ |
| dropped Y face/cell | axes が XZ |
| dropped Z face/cell | axes が XY |
| exact rational positive/zero/negative | canonical lossless DTO |
| multiple faces | canonical face order |
| multiple cells | canonical cell-key order |
| zero cells | valid ready response、counts 0 |
| save/open | fresh instance/fresh proof で response |
| recovery open | fresh instance/fresh proof で response |
| repeated same snapshot | byte-equivalent JSON/hash |
| max accepted cap boundary | truncationなしで成功 |
| `readOnly`/`authorizesProjectMutation` | `true`/`false` 固定 |

### 12.2 陰性

| Case | 期待 |
|---|---|
| no evidence | `Ok(None)` |
| certified-flat evidence | `Ok(None)` |
| foreign project instance | `stale_authority` |
| foreign project ID | `stale_authority` |
| stale revision | `stale_authority` |
| stale fingerprint | `stale_authority` |
| request fixed face mismatch | `stale_authority` |
| request missing/extra/duplicate hinge | request reject または `stale_authority` |
| angle 1-bit mismatch | `stale_authority` |
| current applied pose missing | `stale_authority` |
| captured capability invalidated before revalidation | `stale_authority` |
| same project ID/revision after reopen, old instance | `stale_authority` |
| proof material/folded face coverage mismatch | `invalid_evidence` |
| live face registry mismatch | `invalid_evidence` |
| lower/upper unknown or equal | `invalid_evidence` |
| reverse pair crossing | `invalid_evidence` |
| exact/rounded point bit mismatch | `invalid_evidence` |
| lower/upper dropped axis mismatch | `invalid_evidence` |
| non-finite world point | `invalid_evidence` または `internal_failure`、data-free |
| per-polygon cap + 1 | `resource_limit` |
| face/hinge/cell/point cap + 1 | `resource_limit` |
| exact byte cap + 1 | `resource_limit` |
| final JSON cap + 1 | `resource_limit` |
| checked arithmetic overflow | `resource_limit` |
| Undo 後 | `Ok(None)` |
| Undo then Redo 後 | `Ok(None)`、old response 不使用 |
| archive tamper | reopen/revalidation reject |

### 12.3 persistence regression

既存 test 名を維持し、必要な assertion を追加する。

- `stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically`
- `global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected`

`global_flat_foldability.rs` を変更せず新規 module test から既存 fixture を再利用できるなら、それを優先する。test のためだけに production visibility を過度に広げない。

## 13. TypeScript parser test matrix

### 13.1 陽性

- tree response
- graph response
- dropped X/Y/Z の 3 組
- zero-cell response
- negative/zero/positive rational
- exact max cap
- detached deep freeze
- input mutation後も output 不変
- response/request binding 完全一致
- native `null` の保持

### 13.2 陰性

各 nested level について少なくとも次を設ける。

- missing field
- extra field
- inherited field
- accessor/getter field
- throwing getter
- Proxy reflection trap
- array hole
- array index accessor
- named extra array property
- wrong primitive type
- `NaN`、`Infinity`、`-Infinity`
- `-0`
- unsafe integer
- invalid UUID/ID wire form
- uppercase/short/long digest
- invalid generation、leading zero、`u64::MAX + 1`
- unknown layer model ID
- unknown pose model ID
- wrong version
- `readOnly: false`
- `authorizesProjectMutation: true`
- duplicate/out-of-order hinge
- duplicate/out-of-order face
- duplicate/out-of-order cell key
- lower/upper same face
- unknown lower/upper face
- axis/plane mismatch
- lower/upper face axis mismatch
- world polygon 0、1、2 点
- exact/rounded point-count mismatch
- work count mismatch
- cap + 1
- malformed rational sign
- odd-length、uppercase、leading-zero hex
- zero denominator
- zero sign with nonzero numerator
- nonzero sign with zero numerator
- exact byte aggregate cap + 1
- response project binding mismatch
- response pose binding mismatch

getter test は「getter が 1 回呼ばれて reject」では不十分である。getter が一度も実行されず reject されることを counter で確認する。

## 14. DOM/UI test matrix

`currentNonFlatLayerOrderViewer.dom.test.tsx` で最低限次を確認する。

- stable applied pose と一致 snapshot のときだけ invoke。
- optional applied pose 省略時は invoke 0 回。
- running、blocked、indeterminate は invoke 0 回または現在 load を破棄。
- project ID mismatch、revision mismatch、null fixed face は invoke 0 回。
- loading、absent、ready、failed の表示。
- world XYZ pane と projection UV pane が別 heading/ARIA を持つ。
- world pane は `worldOuterBoundaryXyzMm` だけを使用。
- projection pane は `roundedBoundaryUvMm` だけを使用。
- dropped X/Y/Z の axis label。
- lower/upper face selection highlight。
- keyboard による face/cell 選択。
- zero-cell warning が「衝突なし」と断定しない。
- read-only badge と no-mutation explanation。
- mutation button/callback が存在しない。
- late response は新しい pose/snapshot を上書きしない。
- applied-pose object reference の変更直後に old pixels が消える。
- 同じ semantic pose が再度渡されても old response を再利用しない。
- unmount 後の resolve で state update しない。
- locale `ja -> en -> ja` で refetch せず、selection と geometry を保持。
- locale switch で visible text と ARIA が即時に切り替わる。
- raw native error、raw exact big integer を DOM へ表示しない。

`stackedFoldPanel.dom.test.tsx` では:

- 新 optional prop を省略した既存 test が変わらず通る。
- prop を渡した integration case で viewer が 1 個だけ描画される。
- existing flat `LayerOrderViewer` と persisted proof viewer の意味・selector が変わらない。
- apply form の callback/count が viewer の mount や locale switch で変わらない。

## 15. 段階 commit と担当ファイル

各段階を独立 commit にする。commit 前に focused test、`git diff --check`、stage 対象確認を行う。remote push はしない。

### Commit 1: native boundary、apply/persistence

担当可能ファイル:

```text
crates/ori-collision/src/non_flat_cell_transport.rs
crates/ori-collision/src/lib.rs
apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs
apps/desktop/src-tauri/src/lib.rs
apps/desktop/src-tauri/src/stacked_fold_transaction.rs
apps/desktop/src-tauri/src/stacked_fold_read.rs
```

必要な既存 archive test を狭く拡張する場合だけ:

```text
apps/desktop/src-tauri/src/global_flat_foldability.rs
```

日本語 commit message:

```text
適用済み非平坦層順の読取境界を実装する
```

### Commit 2: strict TypeScript parser/client

担当ファイル:

```text
apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
apps/desktop/tests/currentNonFlatLayerOrderView.test.ts
```

handler/invoke contract test の明示更新が必要な場合だけ:

```text
apps/desktop/tests/tauriCapabilityContract.test.ts
```

日本語 commit message:

```text
非平坦層順ビュー応答を厳格検証する
```

### Commit 3: UI、catalog、integration

担当ファイル:

```text
apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
apps/desktop/src/lib/currentNonFlatLayerOrderViewerText.ts
apps/desktop/tests/currentNonFlatLayerOrderViewerText.test.ts
apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx
apps/desktop/src/components/StackedFoldPanel.tsx
apps/desktop/tests/stackedFoldPanel.dom.test.tsx
apps/desktop/src/App.tsx
apps/desktop/src/App.css
```

日本語 commit message:

```text
適用済み非平坦層順ビューアーを接続する
```

各 commit の stage 確認:

```powershell
git diff --check
git diff --cached --check
git diff --cached --name-only
git status --short
```

上記担当リスト外の file が staged なら commit しない。

## 16. 正確な検証 command

### 16.1 Commit 1 focused native

repository root:

```powershell
cargo test --locked -p ori-collision --lib non_flat_cell_transport::tests -- --test-threads=1
cargo test --locked -p origami2-desktop --lib current_non_flat_layer_order_view::tests -- --test-threads=1
cargo test --locked -p origami2-desktop --lib stacked_fold_read::tests::four_hinge_tree_level_three_proof_applies_and_persists_atomically -- --exact --test-threads=1
cargo test --locked -p origami2-desktop --lib global_flat_foldability::tests::archived_non_flat_evidence_is_freshly_solved_and_tamper_rejected -- --exact --test-threads=1
cargo check --locked -p origami2-desktop --all-targets
cargo fmt --all -- --check
cargo clippy --locked -p origami2-desktop --all-targets --all-features -- -D warnings
```

### 16.2 Commit 2 focused parser

working directory `apps/desktop`:

```powershell
node --test tests/currentNonFlatLayerOrderView.test.ts tests/tauriCapabilityContract.test.ts
npx tsc -b
npx oxlint src/lib/currentNonFlatLayerOrderView.ts tests/currentNonFlatLayerOrderView.test.ts
```

### 16.3 Commit 3 focused UI

working directory `apps/desktop`:

```powershell
node --test tests/currentNonFlatLayerOrderViewerText.test.ts
npx vitest run --config vitest.config.ts tests/currentNonFlatLayerOrderViewer.dom.test.tsx tests/stackedFoldPanel.dom.test.tsx
npx tsc -b
npx oxlint src/lib/currentNonFlatLayerOrderView.ts src/lib/currentNonFlatLayerOrderViewerText.ts src/components/CurrentNonFlatLayerOrderViewer.tsx src/components/StackedFoldPanel.tsx tests/currentNonFlatLayerOrderView.test.ts tests/currentNonFlatLayerOrderViewerText.test.ts tests/currentNonFlatLayerOrderViewer.dom.test.tsx tests/stackedFoldPanel.dom.test.tsx
```

### 16.4 frontend 全回帰

working directory `apps/desktop`:

```powershell
npm run test:snap
npm run test:dom
npm run build
npm run lint
```

### 16.5 workspace 全回帰

repository root:

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked --all-targets --no-fail-fast
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
```

環境起因または担当外の既存 failure が出た場合:

1. command、exit code、最初の根本 error を記録。
2. 同じ failure が Claude の commit 前 HEAD でも再現するか、read-only の範囲で確認。
3. 担当外 file を修正して test を緑に見せない。
4. focused test が通った事実と、全回帰の既存 blocker を分けて報告。

## 17. 実装後の source/diff 監査

完了前に次を `rg` で確認する。

```powershell
rg -n "get_current_non_flat_layer_order_view_v1|CurrentNonFlatLayerOrderViewRequestV1|CurrentNonFlatLayerOrderViewResponseV1" apps/desktop/src-tauri/src
rg -n "validate_non_flat_layer_order_structure_v1" crates/ori-collision
rg -n "getCurrentNonFlatLayerOrderViewV1|normalizeCurrentNonFlatLayerOrderViewV1" apps/desktop/src apps/desktop/tests
rg -n "CurrentNonFlatLayerOrderViewer" apps/desktop/src apps/desktop/tests
rg -n "get_current_layer_order_view" apps/desktop/src-tauri/src apps/desktop/src apps/desktop/tests
rg -n "authorizesProjectMutation|readOnly" apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs apps/desktop/src/lib/currentNonFlatLayerOrderView.ts apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
```

次の禁止 pattern も確認する。

```powershell
rg -n "boundaryWorld|\\[u,\\s*0|\\[.*0.*-.*v|sourceToPlane.*world" apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
rg -n "locale\\s*===\\s*['\\\"]ja['\\\"]|locale\\s*!==\\s*['\\\"]ja['\\\"]" apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
rg -n "invoke\\([^\\r\\n]*apply|onApplied|refreshSnapshot" apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
```

禁止 pattern の `rg` は false positive を目視確認する。comment や test 名に match しただけで機械的に削除しない。

各 commit 後に:

```powershell
git show --stat --oneline HEAD
git show --check HEAD
git status --short
```

最終報告に含めるもの:

- 3 commit の hash と日本語 subject
- 各 commit の変更ファイル
- focused/full test の command と結果
- `git status --short`
- direct push をしていないこと
- `SIM-010` を `Partial`、全体完成度を 79.3% のまま維持したこと
- Undo/Redo 後は viewer evidence が復活しないという残課題

## 18. 最終受入チェックリスト

- [ ] 現行 symbol/path を `rg` で再確認した。
- [ ] shared structural validator は 1 実装だけである。
- [ ] flat command/DTO/parser を変更していない。
- [ ] non-flat command は project instance、ID、revision、fingerprint を再結合する。
- [ ] current applied pose generation、model、fixed face、complete hinge vector を再結合する。
- [ ] proof material/folded/live face registry が完全一致する。
- [ ] world XYZ は live pose transform からだけ作る。
- [ ] cell UV は proof projection からだけ作る。
- [ ] UV を偽の XYZ に変換していない。
- [ ] exact rational provenance を lossless に運ぶ。
- [ ] rounded/exact point は bit-exact に対応する。
- [ ] axis mapping は X→YZ、Y→XZ、Z→XY のみ。
- [ ] caps は native/parser で一致し、truncate がない。
- [ ] response は `readOnly: true`。
- [ ] response は `authorizesProjectMutation: false`。
- [ ] error は data-free。
- [ ] apply 後に project-owned non-flat evidence が存在する。
- [ ] Undo 後は viewer が消える。
- [ ] Redo 後も古い viewer が復活しない。
- [ ] save/open は fresh instance/fresh native proof を要求する。
- [ ] old instance request は拒否される。
- [ ] parser は exact key、own data property、deep freeze を検証する。
- [ ] frontend の late response/ABA guard がある。
- [ ] world pane と UV pane が明確に分離されている。
- [ ] zero cell を「衝突なし」と断定していない。
- [ ] locale switch で refetch/selection reset がない。
- [ ] viewer に mutation control/callback がない。
- [ ] 日本語/英語 text と ARIA が catalog 化されている。
- [ ] 陽性・陰性・cap boundary test が通る。
- [ ] workspace 全回帰を実行した。
- [ ] 担当外差分を stage/commit していない。
- [ ] Git identity は `yuya <oltotlo79@gmail.com>`。
- [ ] commit message は日本語。
- [ ] direct push をしていない。
- [ ] `docs/progress.md` と `docs/requirements-status.md` を変更していない。
- [ ] 全体完成度 79.3%、`SIM-010 Partial` を維持した。
