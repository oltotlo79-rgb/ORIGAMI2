# Codex向け改善提案: 保守性2件と能力ギャップ1件

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
観測 HEAD: `9b33561`
作成者: Claude（読み取りのみで作成。code は 1 byte も変更していない）

この文書は提案であり、実施の可否と優先順位は Codex 側で判断されたい。
本文の数値はすべて当日 HEAD の実測値であり、推測値を含まない。

---

## 0. 提案の要旨

| # | 項目 | 種別 | 実測根拠 | 見積 |
|---|---|---|---|---|
| 1 | `lib.rs` / `App.tsx` の分割 | 保守性 | 28,704行 / 11,247行、112 command が 1 file に集中 | 中〜大 |
| 2 | `Unsupported` 分岐の縮小 | **能力** | 本番 fail-closed 分岐が特定済み。完成率に直接寄与 | 大 |
| 3 | `#![forbid(unsafe_code)]` の全 crate 適用 | 安全性 | 8 crate が未設定。**うち 8 crate すべて実 unsafe 使用 0** | 小 |

3 は本日中に完了できる。1 は段階実施が可能。2 は完成率の数字を動かせる唯一の項目である。

---

## 1. `lib.rs` / `App.tsx` の分割

### 1.1 実測値

```text
apps/desktop/src-tauri/src/lib.rs      28,704行
  内訳: 本番 1..16,355 行 / #[cfg(test)] mod tests 16,356..28,704 行（12,349行）
  #[tauri::command] の数: 112

apps/desktop/src/App.tsx               11,247行
  useState 37 / useEffect 23 / useCallback 34 / useMemo 17 / useRef 26
  定義している関数 component: 1（App のみ）
```

参考: repository 全体で 1,000 行超が 125 file、5,000 行超が 14 file。

```text
lib.rs                                  28,704
crates/ori-core/src/editor.rs           23,294
crates/ori-collision/src/continuous_path.rs  14,385
apps/desktop/src-tauri/src/stacked_fold_read.rs  13,142
crates/ori-collision/src/cayley/positive_thickness.rs  11,710
apps/desktop/src/App.tsx                11,247
crates/ori-core/src/constraints.rs      10,053
```

### 1.2 command 配置の偏り

`#[tauri::command]` は repository 全体で 178 個ある。うち **112 個（63%）が `lib.rs` 1 file に集中**している。

```text
lib.rs                        112
stacked_fold_read.rs            9
stacked_fold_transaction.rs     8
fold_3d_frames_import.rs        7
recovery.rs                     6
beginner_recognition.rs         6
instruction_export.rs           5
global_flat_foldability.rs      5
mesh_export.rs                  3
mesh_animation_export.rs        3
diagnostics.rs                  3
crease_export.rs                3
（以下略）
```

`lib.rs` は既に 21 個の `mod` 宣言を持ち、module 分割の方針自体は確立している。
つまり **分割の設計判断は済んでおり、残り 112 command が未移送のまま**という状態である。

### 1.3 lib.rs 内 112 command のドメイン分布（実測）

command 名から機械的に分類した。

```text
project        16    beginner       16    edge           10
constraint      8    geometric       7    layer           6
underlay        5    runtime         5    vertex          4
grid            4    cut             3    annotation      3
recent          2    snap            1    pattern         1
memo            1
```

`beginner` 16 個は既存 `beginner_recognition.rs` の隣に置ける。
`constraint` 8 + `geometric` 7 = 15 個は独立した 1 module になる。
`edge` 10 + `vertex` 4 + `face` 系は 2D 編集の pattern 操作として束ねられる。

### 1.4 churn の実測と、誇張しない評価

**正確を期すため明記する。直近 100 commit において `lib.rs` と `App.tsx` が変更されたのは各 1 回だけである。**

```text
直近100commit中の変更回数
  lib.rs    1/100
  App.tsx   1/100
  editor.rs 0/100

全期間の変更回数
  lib.rs    357回
  App.tsx   285回
```

したがって「今まさに編集競合が頻発している」という主張はしない。
現時点の competition risk は低い。

一方で本日、`apps/desktop/src-tauri/src/current_non_flat_layer_order_view.rs`（2,558行）で
Codex と Claude の編集が衝突し、Claude 側が着手ゲートで 30 分以上停止した実例がある。
2,558 行の file でこれが起きた以上、28,704 行の file で同じ状況になれば影響はより大きい。

分割の主たる便益は競合回避ではなく次の 3 点である。

1. **review 単位の縮小** — 現状 `lib.rs` の diff は変更箇所の特定に時間がかかる。
2. **test の局所実行** — `mod tests` 12,349 行が単一 module にあるため、
   command 1 個の回帰を確認するのに module 全体が対象になる。
3. **責務境界の明示** — 既存 21 module と同じ粒度へ揃えることで、
   新規 command の置き場所が自明になる。

### 1.5 提案する分割手順（native 側）

既存 module 分割と同じ方式を踏襲する。1 段階 = 1 commit とし、各段階で
`cargo check` / `cargo clippy -- -D warnings` / 該当 module test を通す。

**段階 A: `beginner_design_commands.rs`（16 command）**

```text
新規 file : apps/desktop/src-tauri/src/beginner_design_commands.rs
移送対象  : evaluate_beginner_candidates / cancel_reference_consensus /
            get_beginner_symmetric_parameter_estimate /
            evaluate_beginner_parameter_grid / get_beginner_parameter_grid_progress /
            cancel_beginner_parameter_grid / apply_beginner_symmetric_parameters /
            archive_beginner_reference_model_asset / apply_beginner_generated_plan /
            apply_beginner_parameter_grid_candidate / update_beginner_design_profile /
            update_beginner_reference_consensus / import_beginner_reference_model /
            activate_beginner_reference_model_asset /
            get_beginner_reference_model_geometry /
            suggest_beginner_reference_model_features
```

`beginner` 系は既に `beginner_recognition.rs` が存在し、
`ori-domain` 側も `beginner_candidates.rs` / `beginner_design.rs` /
`beginner_generation.rs` / `beginner_generator.rs` / `beginner_recognition.rs` に
分かれている。native 側だけが未分割である。

**段階 B: `geometric_constraint_commands.rs`（15 command）**

`constraint` 8 + `geometric` 7。`analyze_geometric_constraints` を含む。
現在 Codex が EDT-009 で `ori-core/src/constraints.rs` を集中的に触っているため、
**この段階は EDT-009 が一段落してから着手することを推奨する**。

**段階 C: `pattern_edit_commands.rs`（約 22 command）**

`edge` 10 + `vertex` 4 + `grid` 4 + `snap` 1 + `pattern` 1 + `cut` 3。
2D 展開図エディターの編集操作。この領域は progress.md で 100% とされており、
仕様変動が最も少ないため分割の安全性が高い。

**段階 D: `project_lifecycle_commands.rs`（16 command）**

`project` 16。`new_project` / `open_project` / `save_project` / `save_project_as` /
`validate_project` など。`project_persistence.rs` / `project_folder_io.rs` /
`save_path.rs` / `recovery.rs` が既にあるため、その隣に置く。

**移送時の注意**

- `AppState` / `ProjectState` / `lock_project` / `wire_id` などの共有 item は
  現在 `lib.rs` の private である。移送先から使うには
  `pub(crate)` または `pub(super)` へ最小限だけ広げる。
  **可視性を `pub` へ広げないこと。** 公開 API surface を増やさない。
- `#[cfg(test)] mod tests` 内の test は、移送した command に対応するものだけを
  同時に移す。test 名は変更しない。回帰の追跡性が失われる。
- `tauri::generate_handler!` の登録順は変えない。
  `apps/desktop/tests/tauriCapabilityContract.test.ts` が
  frontend の literal invoke と handler 登録の 1:1 対応を検証しているため、
  各段階でこの test を実行すること。

### 1.6 提案する分割手順（frontend 側）

`App.tsx` 11,247 行に対し、`apps/desktop/src/components/` には既に 52 component、
`apps/desktop/src/lib/use*.ts` には 4 個の custom hook がある。
`App.tsx` が定義する component は `App` 1 個だけであり、
**状態管理（useState 37 / useEffect 23）が App に集中している**構造である。

既存の 4 hook と同じ方式で状態を切り出す。本セッション中にも
`useFoldTechniqueTimelineProposal.ts` / 二次元計測状態 / グリッド分割設定が
同方式で hook 化されており、方針は確立済みである。

**優先して切り出せる候補（凝集度が高く、App 全体へ波及しないもの）**

1. `underlay` 系状態（下絵 object の表示・透明度・lock）
2. `annotation` 系状態（注釈 object）
3. `recent projects` 系状態
4. `runtime update` 系状態（更新確認 UI）

いずれも対応する native command が 2〜5 個と少なく、
他の状態との相互依存が薄い。

**注意**

- `App.css` に対応 style がある component は、hook 化しても class 名を変えない。
- `apps/desktop/tests/*.dom.test.tsx` は 61 file / 491 test が通っている。
  hook 抽出のたびに `npm run test:dom` を全件実行し、件数が減っていないことを確認する。

### 1.7 完了条件

- `lib.rs` の本番部が 10,000 行未満。
- `App.tsx` が 6,000 行未満。
- `cargo clippy --locked --all-targets --all-features -- -D warnings` が緑。
- `npm run test:snap`（現状 2,006 pass）と `npm run test:dom`（現状 491 pass）が
  **件数を減らさずに**緑。
- `tauriCapabilityContract.test.ts` が緑。

---

## 2. `Unsupported` 分岐の縮小（能力ギャップ）

### 2.1 なぜこれが最優先か

`docs/progress.md` の完成率は 2026-07-26 11:43 の `f991314` で 81.96% になって以降、
15:14 の `9b33561` まで **3 時間半変化していない**。

その間の 20 commit の追加行を実測すると次のとおりである。

```text
本番コード   3,721行
テストコード 3,708行   → 追加分の 49% がテスト
```

作業は止まっていないが、**堅牢化と証拠整備に配分されており、
どの領域進捗にも寄与していない**。完成率を動かすには
fail-closed 分岐そのものを減らす必要がある。

本番 crate 内の fail-closed marker の実測値は次のとおり。

```text
Indeterminate   249箇所
Unknown         207箇所
Unsupported      18箇所
```

`Unsupported` は 18 箇所と最も少なく、かつ条件が明示的である。
**最小の変更で最大の可視効果が得られる。**

### 2.2 候補 A（最優先）: 複数共有ヒンジ対の連続衝突

**場所**: `crates/ori-collision/src/continuous_path.rs:2434-2456`

現在の実装は次のとおりである。

```rust
fn classify_continuous_pair_v1(
    shared_hinges: usize,
    shared_vertex: Option<bool>,
    group_membership: Option<(Option<usize>, Option<usize>)>,
) -> ContinuousPairCoverageKindV1 {
    if shared_hinges == 1 {
        ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
    } else if shared_hinges > 1 || shared_vertex.is_none() {
        ContinuousPairCoverageKindV1::Unsupported
    } else if shared_vertex == Some(true) {
        ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor
    } else if group_membership.is_none()
        || group_membership.is_some_and(|(first, second)| first.is_none() || second.is_none())
    {
        ContinuousPairCoverageKindV1::MetadataMissing
    } else if group_membership.is_some_and(|(first, second)| first == second) {
        ...
```

**意味**: 2 面が共有するヒンジが 1 本のときだけ連続衝突を証明できる。
**2 本以上は無条件に無証明で閉じている。**

`docs/progress.md` の残件「一般tree・**複数pair**・正厚の解析的CCD」が
この 1 分岐に対応する。

**この候補を推す理由**

1. 変更対象が 1 分岐に特定済みで、影響範囲が閉じている。
2. `SharedHingeNeedsCorridor` の corridor 構築経路が既に存在するため、
   ゼロから証明を組む必要がない。多ヒンジへの一般化で済む。
3. wire 契約である `ContinuousPairCoverageKindV1` に variant を追加せずに実装できる。
   `shared_hinges > 1` を `SharedHingeNeedsCorridor` 相当へ寄せる形になる。
4. 「3D折り・紙厚・衝突」領域（全体比率 17% × 現在 75%）に直接効く。

**実装方針の提案**

- `shared_hinges == 1` の corridor 構築が、共有ヒンジ 1 本を軸とした
  区間包囲であることを前提に、共有ヒンジ集合 `H` に対して
  各ヒンジ由来の corridor の**共通部分**を取る形へ一般化する。
- 共有ヒンジが増えるほど corridor は狭くなるため、
  健全性（sound）は保たれる。完全性（complete）は落ちるが、
  現状は「無証明」なので厳密な改善になる。
- 資源上限として共有ヒンジ数の上限を設け、超過は
  `Unsupported` ではなく **`resource_limit` 相当**へ分類する。
  「能力がない」と「予算が尽きた」を混同しない。
- 既存 test `certified_cardinal_degree_four_remains_unsupported_without_vertex_relief`
  （`continuous_path.rs:9027`）の期待値は、実装後に
  `Unsupported` から証明成功へ変わる可能性がある。
  **期待値を弱める方向の変更は禁止**とし、
  成功へ変わる場合は test 名も含めて意味が合うよう改める。

### 2.3 候補 B: 単一面ポーズ制限

**場所**:
- `crates/ori-collision/src/cell_order_transport.rs:337`
- `crates/ori-collision/src/flat_endpoint_layer_order.rs:553`

現在の条件（`cell_order_transport.rs:337` 付近）:

```rust
if face_count != 1 || hinge_count != 0 {
    return Err(CellOrderTransportErrorV1::UnsupportedPoseClass {
        faces: face_count,
        hinges: hinge_count,
    });
}
```

**意味**: cell 順序の transport は **面が 1 枚・ヒンジ 0 本のときしか扱えない**。
つまり折られていない紙にしか適用できない。

`flat_endpoint_layer_order.rs:553` も同じ `UnsupportedPoseClass` 判定を持つ。

**評価**: 候補 A より制約が強く、一般化の難度は高い。
ただし 2 箇所が同一の判定条件を持つため、
共通の pose class 判定を切り出して両方を同時に広げられる可能性がある。
候補 A の後に着手することを推奨する。

### 2.4 候補 C: 正厚 prism feature

**場所**: `crates/ori-collision/src/cayley/positive_thickness.rs:1806, 1827, 1834, 1904, 1955`
および `crates/ori-collision/src/effective_cut_static.rs:868, 877, 888`

`SourceFlatPrismFeatureV1::Unsupported` が 8 箇所で参照されている。
`positive_thickness.rs:2228` には次のコメントがある。

```text
edge. Unsupported geometry returns `Ok(None)` and remains an explicit
```

**評価**: `positive_thickness.rs` は 11,710 行あり、影響範囲が最も広い。
先に候補 A で手順を確立してから着手すべきである。

### 2.5 やってはいけないこと

- `Unsupported` を `Ok` へ変えるだけで、証明を伴わない緩和をしないこと。
  健全性が失われる。fail-closed は現在の最大の資産である。
- 既存 test の assertion を弱めて緑にしないこと。
- wire 契約 enum の variant を安易に増やさないこと。
  frontend parser が closed union で受けている。

### 2.6 完了条件（候補 A）

- `shared_hinges > 1` が `Unsupported` を返さない。
- 資源上限超過は `Unsupported` ではなく資源分類で返る。
- 共有ヒンジ 2 本・3 本の positive 回帰が、
  正規の解析経路のみで（proof 偽造なしで）通る。
- 既存の `continuous_path.rs` の test が 1 件も削除・ignore・緩和されていない。
- `docs/progress.md` の「3D折り・紙厚・衝突」領域進捗を再評価できる根拠が揃う。

---

## 3. `#![forbid(unsafe_code)]` の全 crate 適用

### 3.1 現状（実測）

```text
ori-collision     forbid 有
ori-kinematics    forbid 有
ori-core          なし
ori-domain        なし
ori-foldability   なし
ori-formats       なし
ori-geometry      なし
ori-instructions  なし
ori-numeric       なし
ori-topology      なし
```

10 crate 中 2 crate のみ。

### 3.2 重要な発見: 8 crate すべて実 unsafe 使用ゼロ

未設定 8 crate に対し `unsafe ` を全文検索した結果は次のとおり。

```text
ori-core          2箇所
ori-domain        0箇所
ori-foldability   0箇所
ori-formats       1箇所
ori-geometry      0箇所
ori-instructions  0箇所
ori-numeric       0箇所
ori-topology      1箇所
```

そして **ヒットした 4 箇所はすべて文字列・コメント中の "unsafe" という単語であり、
`unsafe` block ではない**。実体は次のとおり。

```text
crates/ori-core/src/constraints.rs:7487
  .expect("the unsafe two-ratio counterexample prepares");

crates/ori-core/src/constraints.rs:7502
  "an unsafe two-ID ratio pair must remain unchecked without becoming a candidate"

crates/ori-formats/src/project_folder.rs:1707
  "unsafe path was accepted: {path:?}"

crates/ori-topology/src/lib.rs:411
  /// blocked; silently treating them as auxiliary would create unsafe 3D input.
```

**したがって 8 crate すべてに `#![forbid(unsafe_code)]` を追加しても、
code 変更は 1 行の attribute 追加だけで完結する。**

### 3.3 具体的な変更

各 crate の `src/lib.rs` 先頭へ次を追加する。
既存 2 crate と同じ位置・同じ書式に揃えること。

```rust
#![forbid(unsafe_code)]
```

対象 file:

```text
crates/ori-core/src/lib.rs
crates/ori-domain/src/lib.rs
crates/ori-foldability/src/lib.rs
crates/ori-formats/src/lib.rs
crates/ori-geometry/src/lib.rs
crates/ori-instructions/src/lib.rs
crates/ori-numeric/src/lib.rs
crates/ori-topology/src/lib.rs
```

### 3.4 apps/desktop/src-tauri は対象外

native app 側は platform IO のため実 `unsafe` を使用している。

```text
apps/desktop/src-tauri/src/lib.rs:15467          let renamed = unsafe {
apps/desktop/src-tauri/src/project_folder_io.rs:1530   libc::mkfifo(...)
apps/desktop/src-tauri/src/project_folder_io/unix.rs:84, 117, 128, 148, 164
```

いずれも Unix の `mkfifo` / fd 操作であり、正当な用途である。
ここへ `forbid` を付けるべきではない。
代わりに `#![deny(unsafe_op_in_unsafe_fn)]` の追加と、
各 `unsafe` block への安全性コメント（`// SAFETY:`）付与を推奨する。
現状 `// SAFETY:` コメントの有無は未確認である。

### 3.5 完了条件

- 8 crate に `#![forbid(unsafe_code)]` が入っている。
- `cargo check --workspace --locked` が緑。
- `cargo clippy --workspace --locked --all-targets --all-features -- -D warnings` が緑。
- 既存 test 件数が減っていない。

この項目は **8 行の追加と workspace ビルド確認だけで完了する**。
所要時間は build 時間が支配的である。

---

## 4. 推奨する実施順序

```text
1. 項目3（forbid）        — 本日中に完了可能。8行追加のみ
2. 項目2 候補A（複数共有ヒンジCCD） — 完成率に効く唯一の項目
3. 項目1 段階A/C/D（native分割）    — EDT-009と競合しない範囲
4. 項目1 frontend hook抽出          — 既存4hookと同方式
5. 項目1 段階B（constraint分割）    — EDT-009一段落後
6. 項目2 候補B/C                    — 候補Aの手順確立後
```

項目 3 → 2 の順にするのは、項目 3 が build 確認だけで終わり、
その過程で workspace 全体の緑が確認できるためである。

---

## 5. この文書の作成方法と制約

- 本文の数値はすべて HEAD `9b33561` に対する実測である。
- 引用した code は `sed` / `grep` による原文であり、要約や書き換えをしていない。
- Claude は本提案の作成にあたり **code を 1 byte も変更していない**。
- 実施の可否、優先順位、担当割り当ては Codex 側の判断に委ねる。
- Claude が実施を担当する場合は、`docs/Claude` へ対象 file と
  完了条件を明示した指示書を配置いただきたい。
  Claude は `docs/Claude` の指示書に明示された file 以外を編集しない運用である。

### 5.1 誇張を避けた点の明記

第 1 項について、「編集競合が頻発している」とは主張していない。
直近 100 commit で `lib.rs` と `App.tsx` が変更されたのは各 1 回であり、
現時点の競合リスクは低い。分割の便益は review 単位・test 局所性・責務境界であり、
競合回避は副次的な効果である。

本日発生した実際の競合は 2,558 行の
`current_non_flat_layer_order_view.rs` で起きたものであり、
`lib.rs` / `App.tsx` で起きたものではない。
