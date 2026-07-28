# Claude作業報告: SIM-010 非平坦layer order viewer（部分完了）

対象指示書: `docs/Claude/sim010-non-flat-layer-viewer-2026-07-26.md`

結論: **Commit 1のうち§7.1のみ完了。Commit 1全体、Commit 2、Commit 3は未着手。** pushは行っていない。

## 1. 作業開始前確認（§1.1）

```text
branch = main
HEAD   = d2135569c91cdb2a762de1ea00c02936050666b8
user.name  = yuya
user.email = oltotlo79@gmail.com
```

Commit 1の担当予定ファイル（`non_flat_cell_transport.rs`、`ori-collision/src/lib.rs`、`src-tauri/src/{lib.rs,stacked_fold_transaction.rs,stacked_fold_read.rs,global_flat_foldability.rs}`）は開始時点で全てcleanだった。§1.2の保護対象には触れていない。

## 2. 完了した範囲: §7.1 共通structural validatorの切り出し

- commit: `92bd78c52aeb87d61787ed88670b84387bc3d0ca`
- author: `yuya <oltotlo79@gmail.com>`
- message: `非平坦層順の構造検証を共通化する`
- 変更file（2件、いずれもCommit 1の担当file内）
  - `crates/ori-collision/src/non_flat_cell_transport.rs`
  - `crates/ori-collision/src/lib.rs`

内容。

- private `validate_complete` を `pub fn validate_non_flat_layer_order_structure_v1(&StackedFoldNonFlatLayerOrderV1) -> Result<(), NonFlatCellTransportErrorV1>` へ改名して公開した。**検証本体は1行も変更していない**（error variantも意味も同一）。
- `certify_non_flat_cell_transport_with_limits_v1` は新しいpublic functionを呼ぶ形に変更した。
- `crates/ori-collision/src/lib.rs` から明示的にre-exportした。
- doc commentで「validationのみでproof・capability・mutation authorityを発行しない」「2 revisionを比較しない」ことを明記した。
- 既存fixtureに、certification pathとpublic validatorが同じ完全性定義を共有することのassertionを追加した。

検証（repository root、`CARGO_TARGET_DIR`未設定＝共有`target/`）。

- `cargo test -p ori-collision non_flat_cell_transport` → 3 pass / 0 fail
- `cargo fmt --all -- --check` → exit 0
- `cargo clippy -p ori-collision --all-targets -- -D warnings` → exit 0

commit messageについて。§15はCommit 1へ`適用済み非平坦層順の読取境界を実装する`を指定しているが、commandもDTOも未実装の段階でそのmessageを使うと実態を過大表示するため、切り出しだけを表す別messageにした。**これは指示からの意図的な逸脱であり、Codex側で扱いを判断してほしい。** Commit 1の残りを実装する際、このcommitを含めてsquashするか、指定messageで積み増すかは任せる。

## 3. 未解決のブロッカー: §7.1の否定regressionが現構成では書けない

§7.1は次の否定regressionを要求している。

- folded faceの欠落
- duplicate material face
- dropped axis `3`
- exact/rounded pointのbit mismatch
- cell/pair mismatch
- unknown face
- lower == upper
- reverse-direction crossing

しかし `StackedFoldNonFlatLayerOrderV1`、`StackedFoldNonFlatOverlapCellV1` などは `crates/ori-core/src/stacked_fold.rs:274-294` で**全fieldがprivate**であり、公開構築経路は `revalidate_current_non_flat_layer_order_v1`（正当なevidenceしか返さない）だけである。したがって `ori-collision` のtest moduleからは不正なevidenceを構築できず、上記8件を書けない。

選択肢は次のいずれかで、どちらも `crates/ori-core/src/stacked_fold.rs` の変更を伴う。同fileはCommit 1の担当file listに含まれないため、こちらの判断では変更しなかった。

1. `ori-core` に `#[cfg(test)]` またはfeature gateしたtest専用constructorを追加し、`ori-collision` から不正値を組み立てる。
2. 否定regressionを `ori-core` 側のtest moduleへ置き、`validate_non_flat_layer_order_structure_v1` を呼ぶ。

現状は「complete evidenceを受理」「certification pathが同じvalidatorを通る」の2点のみ実測固定できている。**残る8件は未カバーである。**

## 4. 未着手の範囲

### Commit 1の残り

- `apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs`（新規）
  - `CurrentNonFlatLayerOrderViewRequestV1` / `...ResponseV1` / `...ErrorV1`
  - command `get_current_non_flat_layer_order_view_v1`
  - `ExactRationalDtoV1`（sign / numeratorMagnitudeHex / denominatorMagnitudeHex）
  - §5.6のdomain separator付きSHA-256 framing 3種（face / exact_boundary / cell）
  - §5.5のdropped axis ↔ planeAxes 対応表
  - §6のviewer固有cap（faces 512、hinges 4,096、cells 4,096、world/exact points 100,000、exact magnitude 8 MiB、最終JSON 16 MiB）とchecked arithmetic
  - §7.2のlock順序と再結合順序、§7.3の全照合（`to_bits()` 一致、generationのcanonical decimal string化）
  - §7.4 tree / §7.5 graph のworld outer boundary構築
- `apps/desktop/src-tauri/src/lib.rs` へのcommand登録
- `stacked_fold_transaction.rs` / `stacked_fold_read.rs` のapply/persistence連携（§8）
- §12 native test matrix（陽性・陰性・persistence regression）

### Commit 2

- `apps/desktop/src/lib/currentNonFlatLayerOrderView.ts`（strict parser/client）
- `apps/desktop/tests/currentNonFlatLayerOrderView.test.ts`（§13の陽性・陰性matrix）

### Commit 3

- `CurrentNonFlatLayerOrderViewer.tsx`、`currentNonFlatLayerOrderViewerText.ts`、対応test
- `StackedFoldPanel.tsx` 接続（§11.2）、world paneとUV paneの分離（§11.3）、state（§11.4）
- `App.tsx` / `App.css`
- §14 DOM/UI test matrix

## 5. 実装時に注意が必要な点（調査済み事項）

- `StackedFoldNonFlatOverlapCellV1::boundary()` は投影後UVの丸め済み `Point2`、`exact_boundary()` は同じ投影境界のexact provenance。**どちらもworld XYZではない**。§4の禁止事項どおり、`[u, 0, -v]` 等へ変換して「world boundary」と呼んではならない。
- `source_to_plane()` はsource 2Dから投影平面へのexact affineであり、world transformではない。
- `authorizes_apply_stacked_fold()` は `false` のまま維持すること。
- `NonFlatCellTransportLimitsV1::default()` は `max_faces: 2_048`、`max_cells: 2_000_000`、`max_pairs: 2_000_000`、`max_boundary_points: 8_000_000` で、§6のviewer capより大きい。viewer側でcapを設定した `NonFlatCellTransportLimitsV1` を渡す必要がある。
- 環境固有の制約として、新規 `CARGO_TARGET_DIR` ではSmart App Controlが `proc-macro2`/`serde_json` のbuild scriptを `os error 4551` でブロックする。共有 `target/` か既に暖まったtarget dirでのみcargoを実行できる。

## 6. 残存差分と担当外

このcommit時点で、私の担当差分は残っていない（`92bd78c` に全て入っている）。

`git status --short` に残る他の差分（`geometricConstraint*`、`globalFlatFoldability*`、`constraints.rs`、`docs/progress.md`、`docs/requirements-status.md` ほか）はCodex側の作業であり、stage・commit・restoreしていない。`docs/plans/*`、`origami2-*.png`、`target-*` にも触れていない。

## 7. 変更していないもの（§0・§4の遵守）

- `docs/requirements-status.md` の `SIM-010` は `Partial` のまま。
- `docs/progress.md` の完成度 79.32%（表示 79.3%）は変更していない。
- `docs/stacked-fold-design.md` は変更していない。
- 既存 `get_current_layer_order_view` のwire contract、`currentLayerOrderView.ts` の型、`LayerOrderViewer` はいずれも未変更。
