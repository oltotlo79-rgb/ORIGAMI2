# Claude作業報告: §7.1 否定regressionの解決（完了）

対象追補指示: `docs/Claude/sim010-viewer-blocker-resolution-2026-07-26.md`

結論: **追補で指示されたprivate structural adapter方式で§7.1を完了した。** 9件の否定regressionはすべて個別testとして固定済み。pushは行っていない。

## 1. commit

- commit: `ccce7bd0321207f4d930c13f0fccf98bc8ba3f28`
- author: `yuya <oltotlo79@gmail.com>`
- message: `非平坦層順の構造検証に敵対的回帰を追加する`
- 変更file: `crates/ori-collision/src/non_flat_cell_transport.rs` の1件のみ

`crates/ori-core/**`、`Cargo.toml`、公開re-export、error variantは変更していない（§2遵守）。

## 2. 実装内容

### 2.1 private structural adapter（§3）

module内のみに次を追加し、外部へre-exportしていない。

- `NonFlatFoldedFaceStructuralRefV1<'a>`（`face_id` / `dropped_world_axis` / `&ExactAffineTransform`）
- `NonFlatOverlapCellStructuralRefV1<'a>`（`&[Point2]` / `&[ExactPointValue]` / `lower_face` / `upper_face`）
- `NonFlatFacePairOrderStructuralV1`（`Clone, Copy`）
- `trait NonFlatLayerOrderStructuralSourceV1`（指示の8 accessor）
- `CoreNonFlatLayerOrderStructuralViewV1<'a>(&'a StackedFoldNonFlatLayerOrderV1)`

core viewは既存のimmutable getterだけで実装し、全index accessorは `.get(index)` を使う。宣言countと実データが一致しない場合はpanicせず `None` を返す。deep clone、大規模`Vec`確保、serialization round-tripは行っていない。

### 2.2 同一検証ロジックへの接続（§4）

- 検証本体を `fn validate_non_flat_layer_order_structural_source_v1<S: NonFlatLayerOrderStructuralSourceV1 + ?Sized>(&S)` へ意味変更なく移した。判定順序（material/folded集合 → axisとexact affineの有限性 → cell/pair count → cell走査 → crossing）と返すerror variantは移行前と同一。
- `None` を返す accessor は `IncompleteCoverage` へ閉じる。
- public wrapper `validate_non_flat_layer_order_structure_v1` はcore evidenceをprivate viewへ包んでhelperを呼ぶだけになった。signature、`ori-collision/src/lib.rs` のre-export、error variantの割当、proof identity、certification/capability/Apply authorityの発行条件、revision比較を行わないstructural-only契約はいずれも未変更。
- `certify_non_flat_cell_transport_with_limits_v1` は引き続きpublic wrapperを呼ぶ。

### 2.3 敵対的fixture（§5）

同fileの既存 `mod tests` 内にownedの `StructuralFixture` を置き、同じprivate traitを実装した。fixtureからpublic `StackedFoldNonFlatLayerOrderV1` は構築していない。exact値は `ExactRationalValue` の公開fieldで組み立てている（`ori-foldability` は既存の通常依存）。

base fixture: material faces `[a, b]`、folded faceは`a`(axis 0)と`b`(axis 2)、`source_to_plane`は有限なexact identity、overlapは3点triangle、rounded点とexact点は全座標`to_bits()`一致、cellとpairは同じ`(a, b)`、helperは`Ok(())`。

指示の9件を1条件ずつ独立testとして追加した。

| # | test名 | 変更 | 結果 |
|---|---|---|---|
| 1 | `a_missing_folded_face_is_incomplete` | folded faceを1件削除 | `IncompleteCoverage` |
| 2 | `a_duplicate_material_face_is_incomplete` | material facesを`[a, a]` | `IncompleteCoverage` |
| 3 | `an_out_of_range_dropped_world_axis_is_incomplete` | dropped axisを`3` | `IncompleteCoverage` |
| 4 | `negative_zero_rounded_provenance_is_incomplete` | rounded `x = -0.0`、exact `x = +0` | `IncompleteCoverage` |
| 5 | `a_pair_that_disagrees_with_its_cell_is_incomplete` | cell`(a,b)`のままpairを`(a,c)` | `IncompleteCoverage` |
| 6 | `an_unknown_face_in_a_cell_is_incomplete` | cell/pairを`(a,c)`、`c`はmaterial facesに無し | `IncompleteCoverage` |
| 7 | `a_self_paired_cell_is_incomplete` | cell/pairを`(a,a)` | `IncompleteCoverage` |
| 8 | `an_opposite_direction_cell_crosses` | valid`(a,b)`にvalid`(b,a)`を追加 | `Crossing` |
| 9 | `a_cell_and_pair_count_mismatch_is_incomplete` | cell countとpair countを1件ずらす | `IncompleteCoverage` |

4番は`-0.0`と`+0.0`が数値比較では等しいことを利用し、`to_bits()`不一致でのみ拒否される既存境界を固定している。

既存のreal evidence testは残し、genuine `StackedFoldNonFlatLayerOrderV1` をpublic validatorが受理すること、certification pathも同じpublic validatorを通ること、proof identityとauthorityが変わらないことを同時に固定している。

## 3. 検証（§6）

repository root、`CARGO_TARGET_DIR`未設定＝共有`target/`。新しい`target-*`は作成・stage・commitしていない。

- `cargo test -p ori-collision --lib non_flat_cell_transport` → **13 pass / 0 fail**（新規12件＋既存1件、9件の否定regressionを個別に確認）
- `cargo fmt --all -- --check` → exit 0
- `cargo clippy -p ori-collision --lib --all-features -- -D warnings` → exit 0
- `git diff --check` → exit 0

### 環境要因による1点の未確認

指示の `cargo test -p ori-collision non_flat_cell_transport`（target絞り込みなし）は、この環境では**Smart App Controlにより完了しない**。lib変更で再リンクされた他のintegration test binary（`flat_endpoint_layer_order`、`effective_cut_static` など）が `os error 4551` でブロックされるため。5回再試行しても解除されなかった。

- ブロックされるのは**`non_flat_cell_transport` のtestを0件しか含まないtarget**であり（`0 passed; ... filtered out` と表示される）、本件の検証内容には寄与しない。
- `effective_cut_static` は単独実行（`cargo test -p ori-collision --test effective_cut_static`）では **1 pass** することを確認済み。したがってコード起因の失敗ではない。
- 私のtestが存在するlib targetは毎回実行でき、13 passしている。

Codex側でSACの影響を受けない環境があれば、target絞り込みなしのコマンドで最終確認をしてほしい。

## 4. 続行順序（§7）と現状

追補は完了したが、元指示書のCommit 1全体はまだ未完である。残りは次の順で続行する必要がある。

1. Commit 1残件: `current_non_flat_layer_order_view.rs`（command `get_current_non_flat_layer_order_view_v1`、request/response/error型、`ExactRationalDtoV1`、§5.6のdomain separator付きSHA-256 framing 3種、§5.5のaxis対応、§6のviewer cap、§7.2のlock/rejoin順序、§7.3の全照合、§7.4/7.5のworld outer boundary構築）、`src-tauri/src/lib.rs` へのcommand登録、§8のapply/persistence、§12 native test matrix
2. Commit 2: `currentNonFlatLayerOrderView.ts` と§13の否定matrix
3. Commit 3: viewer UI、world/UV pane分離、live locale、§14 DOM/UI matrix

## 5. Claudeのcommit一覧（この作業分）

- `92bd78c52aeb87d61787ed88670b84387bc3d0ca` 非平坦層順の構造検証を共通化する（Codex取り込み済み）
- `ccce7bd0321207f4d930c13f0fccf98bc8ba3f28` 非平坦層順の構造検証に敵対的回帰を追加する（本報告）

## 6. 触れていないもの

`SIM-010` は `Partial` のまま。`docs/progress.md` の79.32%（表示79.3%）、`docs/requirements-status.md`、`docs/stacked-fold-design.md`、既存 `get_current_layer_order_view` のwire contract、`currentLayerOrderView.ts`、`LayerOrderViewer` はいずれも未変更。`authorizes_apply_stacked_fold()` の境界も広げていない。

Codex側の差分、`docs/Codex/**` の他file、`docs/plans/**`、`origami2-*.png`、`target-*` はstage・commit・restoreしていない。
