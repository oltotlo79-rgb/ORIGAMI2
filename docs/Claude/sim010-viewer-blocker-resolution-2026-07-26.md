# SIM-010 viewer 実装追補: §7.1 否定 regression の安全な実装方法

対象指示書: [`sim010-non-flat-layer-viewer-2026-07-26.md`](sim010-non-flat-layer-viewer-2026-07-26.md)

対象報告: [`../Codex/claude-sim010-viewer-progress-2026-07-26.md`](../Codex/claude-sim010-viewer-progress-2026-07-26.md)

## 0. Codex 側の判断

- `92bd78c52aeb87d61787ed88670b84387bc3d0ca`「非平坦層順の構造検証を共通化する」は、その狭い変更内容に合う commit message を使った独立 commit として受理する。
- 同 commit は Codex 側で `main` へ取り込み、remote へ push 済みである。amend、rebase、squash、再作成を行わない。
- `authorizes_apply_stacked_fold()`、proof、capability、mutation authority の境界は一切広げない。
- 下記の private adapter 方式で §7.1 の否定 regression を完了し、その後は元指示書の Commit 1 残件、Commit 2、Commit 3 を中断せず順番に続行する。

## 1. 禁止する解決方法

`ori-core` は変更しない。`StackedFoldNonFlatLayerOrderV1`、`StackedFoldNonFlatOverlapCellV1`、folded face、face-pair order の各型へ、次のいずれも追加しない。

- test constructor、setter、builder
- feature-gated constructor
- `Serialize` / `Deserialize`
- layout 依存の `unsafe` / `transmute`
- 不正 evidence を production build で構築できる公開 API

`#[cfg(test)]` の constructor は、依存 crate として build される `ori-core` では `ori-collision` の test から利用できない。feature-gated constructor は、feature 有効 build や `--all-features` で不正 evidence 構築 API を production library に公開するため採用しない。

`ori-core` から `ori-collision` を dev-dependency にして検証を呼ぶ案も採用しない。通常依存側と unit-test 側で別 copy の `ori_core` が link され得るため、本件の型を安全に共有する境界にならない。

## 2. 実装範囲

この追補で変更してよい production file は次の1件だけである。

- `crates/ori-collision/src/non_flat_cell_transport.rs`

必要な test は同 file の既存 `mod tests` 内へ追加する。`crates/ori-core/**`、`Cargo.toml`、公開 re-export、error variant は変更しない。

## 3. private structural adapter

`non_flat_cell_transport.rs` の module 内だけに、外部へ re-export しない private structural view を追加する。production adapter は core evidence を borrow し、大規模 `Vec`、deep clone、serialization round-trip を作らない。

衝突がなければ、型名と accessor 名は次に統一する。

```rust
struct NonFlatFoldedFaceStructuralRefV1<'a> {
    face_id: FaceId,
    dropped_world_axis: u8,
    source_to_plane: &'a ExactAffineTransform,
}

struct NonFlatOverlapCellStructuralRefV1<'a> {
    boundary: &'a [Point2],
    exact_boundary: &'a [ExactPointValue],
    lower_face: FaceId,
    upper_face: FaceId,
}

#[derive(Clone, Copy)]
struct NonFlatFacePairOrderStructuralV1 {
    lower_face: FaceId,
    upper_face: FaceId,
}

trait NonFlatLayerOrderStructuralSourceV1 {
    fn material_face_count(&self) -> usize;
    fn material_face_id(&self, index: usize) -> Option<FaceId>;
    fn folded_face_count(&self) -> usize;
    fn folded_face(
        &self,
        index: usize,
    ) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>>;
    fn overlap_cell_count(&self) -> usize;
    fn overlap_cell(
        &self,
        index: usize,
    ) -> Option<NonFlatOverlapCellStructuralRefV1<'_>>;
    fn face_pair_order_count(&self) -> usize;
    fn face_pair_order(
        &self,
        index: usize,
    ) -> Option<NonFlatFacePairOrderStructuralV1>;
}

struct CoreNonFlatLayerOrderStructuralViewV1<'a>(
    &'a StackedFoldNonFlatLayerOrderV1,
);
```

`CoreNonFlatLayerOrderStructuralViewV1` は既存の immutable getter だけで trait を実装する。全 index accessor は `.get(index)` を使い、宣言 count と実データが一致しない場合は panic せず `None` を返して fail closed にする。

## 4. 同一検証ロジックへの接続

現在の public validator 本体を、次の private pure helper へ意味変更なく移す。

```rust
fn validate_non_flat_layer_order_structural_source_v1<
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
>(
    source: &S,
) -> Result<(), NonFlatCellTransportErrorV1> {
    // 現在の HashSet、axis、exact bit、cell/pair、crossing 検証を移す。
}
```

public wrapper は core evidence を private view に包んで helper を呼ぶだけにする。

```rust
pub fn validate_non_flat_layer_order_structure_v1(
    value: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    validate_non_flat_layer_order_structural_source_v1(
        &CoreNonFlatLayerOrderStructuralViewV1(value),
    )
}
```

`certify_non_flat_cell_transport_with_limits_v1` は、引き続きこの public wrapper を呼ぶ。次を変えてはならない。

- public function signature と `ori-collision/src/lib.rs` の re-export
- `NonFlatCellTransportErrorV1` の variant と各条件の割当
- public proof identity
- certification、capability、Apply authority の発行条件
- revision 比較を行わない structural-only という既存契約

private helper の移行前後で、現在の各判定順序と返す error を維持する。trait accessor が `None` を返した場合は `IncompleteCoverage` へ閉じる。

## 5. test 専用 adversarial fixture

同 file の既存 `mod tests` 内だけに、owned の test fixture と folded face / cell / pair record を置き、同じ private trait を実装する。fixture から public `StackedFoldNonFlatLayerOrderV1` を構築してはならない。

base fixture は次を満たす。

- material faces は `[a, b]`
- folded faces は `a` と `b` の各1件
- dropped world axis は `0..=2`
- `source_to_plane` は有限な exact identity transform
- overlap boundary は3点 triangle
- rounded point と exact point は全座標が `to_bits()` 一致
- overlap cell と face-pair order は同じ `(a, b)`
- helper が `Ok(())` を返す

base fixture を clone し、1条件だけを変える独立 test を最低限次の9件追加する。

| # | 変更 | 期待結果 |
|---|---|---|
| 1 | folded face を1件削除 | `IncompleteCoverage` |
| 2 | material faces を `[a, a]` にする | `IncompleteCoverage` |
| 3 | dropped world axis を `3` にする | `IncompleteCoverage` |
| 4 | rounded `x = -0.0`、exact `x = +0` rational にする | `IncompleteCoverage` |
| 5 | cell `(a,b)` のまま pair を `(a,c)` にする | `IncompleteCoverage` |
| 6 | cell と pair を `(a,c)` にし、`c` を material faces に加えない | `IncompleteCoverage` |
| 7 | cell と pair を `(a,a)` にする | `IncompleteCoverage` |
| 8 | valid `(a,b)` に valid `(b,a)` の cell / pair を追加する | `Crossing` |
| 9 | cell count と pair count を1件ずらす | `IncompleteCoverage` |

4番は通常の数値比較では等しい `-0.0` と `+0.0` を使い、rounded / exact provenance が `to_bits()` 不一致なら拒否する既存境界を固定する。

既存 real evidence test は残し、次を同時に固定する。

- genuine `StackedFoldNonFlatLayerOrderV1` を public validator が受理する
- certification path も同じ public validator を通る
- proof identity と authority が変わらない

## 6. 検証

repository root から次を実行し、すべて成功させる。

```text
cargo test -p ori-collision non_flat_cell_transport
cargo fmt --all -- --check
cargo clippy -p ori-collision --all-targets -- -D warnings
git diff --check
```

Smart App Control の `os error 4551` を避けるため、共有 `target/` または既に build 済みの target directory を使う。新しい `target-*` を repository へ追加、stage、commit しない。

## 7. 完了後の続行順序

この追補を完了しても Commit 1 全体の完了ではない。次の順に止めずに続行する。

1. 元指示書 Commit 1 の command、DTO、digest、viewer cap、lock/rejoin、persistence、native test matrix
2. Commit 2 の strict frontend parser/client と否定 matrix
3. Commit 3 の read-only viewer UI、world / UV pane 分離、live locale、DOM/UI matrix

`92bd78c` を再利用し、残りは実態に一致する日本語 commit message で分割する。commit 前に次を確認する。

```text
git config --local user.name
git config --local user.email
```

期待値は `yuya` と `oltotlo79@gmail.com` である。push は行わない。Codex 側の差分、`docs/Codex/**`、`docs/plans/**`、`origami2-*.png`、`target-*` を stage、commit、restore しない。
