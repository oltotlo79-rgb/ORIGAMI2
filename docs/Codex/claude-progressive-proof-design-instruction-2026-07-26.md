# Codex向け設計追加指示: 投機的Applyと漸進証明による厳密性スケール拡張（2026-07-26）

## 0. この文書の位置づけ

Claudeは本文書の作成時点でソースを一切編集していない。以下はすべて実コード・実git履歴の読み取りから導いた設計指示であり、実装はCodexが行う。

本文書は要件を新設しない。既存のSIM-010（部分実装）と「3D折り・紙厚・衝突 75%」「折り可能性・経路探索 78%」の未完境界に対する**実装方針の追加**である。`docs/requirements-definition.md`のMUST 87件は変更しない。

オーナー決定事項（本文書の前提、変更不可）:

1. 近似層に**投機的Apply権限を与える**。
2. 証明層の**スケール自体を上げる**。
3. 事後証明が失敗した場合、**自動で巻き戻さない**。未証明flagを付けて保存を許可し、巻戻しは利用者が判断する。
4. 実施範囲は**フェーズ1からフェーズ3まで全部**。

## 1. 目的

`docs/progress.md`の未完境界のうち、次の乖離を解消する。

| 層 | 現在の到達スケール（実測） | 根拠 |
|---|---|---|
| 近似層（frontend） | 面10,000 / 頂点1,000,000 / 三角形テスト1,000,000 | `apps/desktop/src/lib/foldPreviewCollision.ts:3-6`、`foldPreviewNarrowCollision.ts:37,45` |
| 近似層（native sampling） | 経路sample 64 | `MAX_STACKED_FOLD_PATH_SAMPLES_V1` |
| **証明層（正厚連続）** | **実証15〜17面 / hard cap 64面** | `MAX_POSITIVE_ENDPOINT_TREE_FACES_V1 = 64`、progress.md記載の15面実証 |
| 証明層（block合成） | 2〜8 block / composition 32 | `MULTI_BLOCK_MIN_BLOCKS_V1 = 2`、`MULTI_BLOCK_MAX_BLOCKS_V1 = 8`、`BLOCK_COMPOSITION_LIMIT_V1 = 32` |
| 証明層（subset最小化） | 16制約以下 | EDT-009 |

近似層は既に実作品規模で設計されているのに、証明層が15〜64面で止まっている。実際の作家作品の展開図は折り線数百〜数千を持つため、現状は「動かせるが証明できない」状態にある。

**この乖離を、近似の権限拡大（フェーズ1）と証明スケールの引き上げ（フェーズ2・3）の両面から詰める。**

## 2. 作業開始前の必須確認

次を実行し、結果を作業報告へ残すこと。

```powershell
git status --short
git branch --show-current
git rev-parse HEAD
git config --local --get user.name
git config --local --get user.email
```

Gitのlocal identityは次の値でなければならない。

```text
user.name = yuya
user.email = oltotlo79@gmail.com
```

値が異なる場合は実装を始めず、その事実を報告すること。`.git/config`、global Git設定、remote URLを変更してはならない。

同じworktreeに未commit差分が存在し得る。担当ファイル以外を整形、stage、restore、削除してはならない。特に次はユーザー所有または生成物なので、絶対にstage・commit・削除しないこと。

- `docs/plans/code-audit-2026-07-22.md`
- `docs/plans/code-audit-round3-2026-07-23.md`
- `origami2-*.png`
- `target-*`
- `docs/Claude/`、`docs/Codex/`配下の既存レポート

## 3. 全フェーズに共通する不変条件

以下は**1つでも破ったら実装を採用しない**。フェーズ横断の絶対条件である。

### 3.1 fail-closedの維持

`crates/ori-collision/src/continuous_path.rs:1-6`の既存契約を弱めてはならない。

> Sampling is deliberately not presented as CCD proof. The result can find a blocking sampled pose and can recommend the authenticated initial pose as a fail-closed hold, but it never certifies the open intervals between samples or authorizes mutation.

投機的Applyを追加しても、**サンプリング結果が「安全の証明」になってはならない**。投機的Applyは「証明がまだ無いことを明示したうえでproject変更を許す」ものであり、「近似で安全を肯定する」ものではない。この区別をコード上の型で表現すること。具体的には、既存の証明済みauthority型と投機的token型を**別の型**にし、投機的token型から証明済みauthority型へのFrom/Into変換を実装してはならない。

### 3.2 「無断」の禁止

`docs/requirements-definition.md`§2.3は「計算結果の正しさを損なう近似を**無断で**確定結果として扱わない」と定める。投機的Applyはこの条文に違反しないが、その条件は次の全てを満たす場合だけである。

- Apply前に、証明がまだ無いことを日英で明示し、利用者の明示確認を取る。
- 保存されたファイルに未証明であることが記録され、次回読込時も明示される。
- 未証明の折りが含まれるprojectを、証明済みのものと同じ表示で見せない。
- 診断JSON（OPS-004/005/006）にも未証明件数が反映される。ただし固定schemaの粗い件数区分だけとし、作品形状・座標・IDを含めない。

### 3.3 決定性

並列化・キャッシュ・段階実行を追加しても、次は不変でなければならない。

- 同じ入力に対する証明結果（肯定・否定・不明）が同一。
- 計上されるwork値が同一。**スレッド数、コア数、キャッシュヒット状況、実行順序に依存してはならない。**
- 入力順・保存順を変えても同一。
- 取消（cancel）の有無で肯定結果が変わってはならない（取消は`Unknown`へ閉じるだけ）。

### 3.4 資源計量の完全性

`crates/ori-collision/src/static_collision.rs:63`の`StaticCollisionLimits`は27個超の単調累積カウンタを持つ。並列化・キャッシュ導入で**この計量を緩めてはならない**。

既存コードに並列安全な計量の前例がある。`docs/progress.md`記載の private kernel が採る方式である。

> 呼出側の同一累積budgetへA局所hard envelopeを事前予約し、全additive/max counterをlocal meterで強制した実測deltaだけをresetなしでmergeする

**この方式を一般化して使うこと。**新方式を発明しないこと。具体的手順は§4.1.3に示す。

### 3.5 完成度・要件文書の扱い

- `docs/progress.md`の完成率、`docs/requirements-status.md`のMUST集計（現在85/2/0）を、**内部品質の改善だけで上げてはならない**。
- 上げる場合は、利用者がUIから実行できる経路が増えたことを根拠として、該当ID行と重み表を同じcommitで更新する。
- 81.96%（表示82.0%）の正本発効規則（`docs/progress.md:9`）を書き換えてはならない。
- 各フェーズの完了時に、そのフェーズで**何を証明していないか**を必ず明記すること。過大申告は本プロジェクトで繰り返し監査訂正の対象になっている（EDT-009は「13種→5種→6種→7種→8種→9種」と5回訂正された）。同じ失敗を繰り返さないこと。

### 3.6 テスト規約

- Rust: 新規の定理・境界には必ず**肯定fixtureと否定fixtureの両方**を置く。否定側は「なぜ肯定してはいけないか」をコメントで説明する。
- 各パッケージで`#[test]`を追加し、既存テストを削除・弱化しない。
- TypeScript: `node --test`（静的契約）と`vitest`（DOM）の両方を追加する。
- 敵対的fixtureを必ず含める: field欠落、余分field、非canonical UUID、prototype上の値、enumerable getter、境界値の1 ULP差。

---

# フェーズ1: 差分証明基盤と投機的Apply

**目標: 数百面規模を数分で証明完了させ、投機的Applyの未証明flagが徐々に解消される状態を作る。**

このフェーズは研究要素を含まない。既存機構の一般化と並列化のみで構成する。フェーズ2の前提基盤でもあるため、**必ず最初に完成させること。**

## 4.1 P1-1: 証明層の並列化

### 4.1.1 現状（実測）

- workspaceに`rayon`依存は**存在しない**。
- `std::thread`のヒットは`crates/ori-collision/src/cayley/positive_thickness.rs`、`crates/ori-core/src/constraints.rs`、`crates/ori-numeric/src/lib.rs`の3ファイルのみ。
- `spawn_blocking`は`apps/desktop/src-tauri/`側のジョブ管理層にあり、証明そのものは単一スレッドで走る。
- 一方 canonical unordered face pair 単位の証明は**本質的に独立**である。

つまり並列化は研究要素ゼロで取れる伸びしろであり、progress.md記載の「15面が112.52秒→5.52秒」という既存最適化と直交する。

### 4.1.2 担当ファイル

- `Cargo.toml`（workspace dependencies に `rayon` 追加）
- `crates/ori-collision/Cargo.toml`
- `crates/ori-collision/src/static_collision.rs`
- `crates/ori-collision/src/cayley.rs`（`WorkMeter`、`checked_work_sum`の並列対応）
- 新規 `crates/ori-collision/src/parallel_meter.rs`

### 4.1.3 実装要件

**分割単位**は canonical unordered face pair とする。face単位・triangle単位に細分化しないこと（共有memoの局所性を壊す）。

計量は次の手順で行う。**この順序を変えないこと。**

1. **事前計算**: 全 canonical unordered face pair を列挙し、canonical順（既存の`FaceId`順序規約）で安定ソートする。
2. **事前予約**: pair数から各workerの局所hard envelopeを算出し、呼出側の`StaticCollisionLimits`から**先に**差し引く。予約に失敗したら並列実行を開始せず、既存のpreflight拒否へ閉じる。
3. **局所実行**: 各workerは自分の局所meterだけを使う。共有カウンタへ実行中にアクセスしてはならない。
4. **決定的merge**: 全workerの完了後、**canonical pair順**で実測deltaをmergeする。完了順・スレッド順でmergeしてはならない。`reset`は行わない。
5. **照合**: mergeした合計が事前予約の範囲内であることを再検証する。超えていれば結果を破棄して資源上限errorへ閉じる。

### 4.1.4 絶対に行ってはいけないこと

- 並列化のために`StaticCollisionLimits`のいずれかのフィールドを緩める、または既定値を上げる。
- 完了順にmergeする（`max`系カウンタは順序に依存しないが、`additive`系の中間overflow検出が順序依存になる）。
- `rayon`の既定スレッドプールをグローバルに設定する（Tauriの既存ジョブ管理と競合する）。専用プールを作り、スレッド数をワーカ設定として明示すること。
- 部分完了の結果で肯定を発行する。1つでもworkerが資源上限・取消・非有限で閉じたら、全体を`Unknown`へ閉じる。
- スレッド数を結果や計上work値に影響させる。

### 4.1.5 必須回帰

```text
parallel_face_pair_proof_matches_sequential_result_bit_exact
parallel_face_pair_proof_work_total_independent_of_thread_count
parallel_face_pair_proof_merge_order_is_canonical_not_completion_order
parallel_face_pair_proof_reservation_failure_rejects_before_spawn
parallel_face_pair_proof_single_worker_resource_exhaustion_closes_whole_result
parallel_face_pair_proof_cancel_during_partial_completion_yields_unknown
parallel_face_pair_proof_reversed_input_order_identical_result
```

`parallel_face_pair_proof_work_total_independent_of_thread_count`は、スレッド数1・2・4・8で同一の全カウンタ値になることを固定する。これが通らない実装は採用しない。

### 4.1.6 P1-1 受入条件

- 15面正厚treeの実測時間が、スレッド数4で逐次実行比**2.5倍以上**高速化する。数値を報告に記載すること。
- 上記7 testが成功する。
- `cargo clippy -p ori-collision --all-targets -- -D warnings` が exit 0。
- 既存の`ori-collision --lib`全件が成功する（progress.md記載の74件を下回らないこと）。

### 4.1.7 P1-1 検証コマンド

```powershell
cargo fmt --all -- --check
cargo test -p ori-collision --lib
cargo test -p ori-collision parallel_face_pair
cargo clippy -p ori-collision --all-targets -- -D warnings
```

---

## 4.2 P1-2: 証明済みpairの永続キャッシュと差分無効化

### 4.2.1 設計

証明を「1回で全部」から「継続的に少しずつ」へ変える。証明済みの canonical unordered face pair を永続キャッシュに保持し、編集・折り操作で影響を受けた pair だけを無効化して再証明する。

### 4.2.2 キャッシュキー（全項目必須）

次の全項目の完全一致でのみヒットとする。**1つでも欠けたらキャッシュを使ってはならない。**

```text
project instance ID
project ID
revision
geometry fingerprint
pose generation
紙厚 bit列（f64 to_bits、+0.0 と -0.0 を区別する）
canonical unordered face pair（FaceId 昇順）
certificate model ID（8種のいずれか）
issuer context
```

`certificate model ID`は`crates/ori-collision/src/continuous_path.rs:120-134`の8定数を使う。新しいmodel IDを作る場合は別packageとして扱い、本packageでは既存8種のみ対象とする。

### 4.2.3 無効化の証明責務

**ここが本packageで最も難しい部分である。**「影響を受けない pair は再証明しなくてよい」という主張は、それ自体が証明を要する。

差分無効化は、次を満たす場合だけ既存証明を保持できる。

- 編集commandが触れた頂点・辺・面の集合を`AppliedEditImpactSetV1`として算出する。
- pair の両面が impact set と**共有要素を持たない**ことを、`FaceId`・`VertexId`・`EdgeId`の集合演算で示す。
- かつ、両面の world 変換（`ExactFacePose`）が bit-exact に不変であることを示す。
- かつ、紙厚 bit列が不変であることを示す。
- かつ、その pair の証明が依存した**共有memoのエントリ**が無効化されていないことを示す。

**上記のいずれか1つでも示せない場合、pair の証明を破棄して再証明すること。**「たぶん影響しない」で保持してはならない。

### 4.2.4 絶対に行ってはいけないこと

- geometry fingerprint の一致だけでヒットとする（pose generation と紙厚 bit列も必須）。
- 破棄された古い pose generation の証明を、同じ角度値になったからといって再利用する（ABA）。
- 紙厚 `+0.0` と `-0.0` を同一視する。既存コードは`+0.0`だけを許可する経路があり、この区別は意味を持つ。
- 1 ULP差の角度・厚さを同一視する。
- キャッシュ容量が尽きたときに、古いエントリを黙って捨てて「証明済み」を主張する。上限に達したら明示的に一部を未証明へ戻し、その件数を報告可能にすること。
- キャッシュをファイルに書く場合、path・raw bytesをWebViewへ渡す（既存のIO-003契約に反する）。

### 4.2.5 上限

新しいhard capを次の名前で定義し、既存の資源上限と同じくone-shot・cooperative cancel・absolute deadlineへ束縛する。

```rust
pub const MAX_PROOF_CACHE_ENTRIES_V1: usize = 65_536;
pub const MAX_PROOF_CACHE_STORAGE_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_PROOF_CACHE_INVALIDATION_WORK_V1: usize = 2_000_000;
```

数値は初期値であり、実測後に調整してよい。ただし**無制限にしてはならない。**

### 4.2.6 必須回帰

```text
proof_cache_hit_requires_all_key_components
proof_cache_miss_on_pose_generation_change_only
proof_cache_miss_on_thickness_one_ulp_drift
proof_cache_rejects_signed_zero_thickness_conflation
proof_cache_aba_same_angle_different_generation_is_miss
proof_cache_invalidation_discards_pair_touching_impact_set
proof_cache_invalidation_retains_pair_with_proven_disjointness
proof_cache_invalidation_retains_nothing_when_shared_memo_invalidated
proof_cache_capacity_exhaustion_reverts_entries_to_unproven_explicitly
proof_cache_result_identical_to_cold_run_bit_exact
```

`proof_cache_result_identical_to_cold_run_bit_exact`は、キャッシュあり・なしで最終結果と全work計上値が完全一致することを固定する。これが本packageの正しさの核である。

### 4.2.7 P1-2 受入条件

- 1頂点だけを動かした後の再証明で、再証明対象 pair 数が全 pair 数の**20%未満**になること。実測値を報告に記載する。
- 上記10 testが成功する。
- キャッシュあり・なしで結果が bit-exact 一致する。

---

## 4.3 P1-3: 投機的Applyと未証明flagの永続化

### 4.3.1 既存機構の活用

`.ori2`には既に`required_features: Vec<String>`があり、8個のfeature flagを立てている（`crates/ori-formats/src/ori2.rs:23-30`）。

```text
instruction_timeline_v1
declarative_instruction_steps_v1
numeric_expressions_v1
geometric_constraints_v1
layers_v1
reference_model_assets_v1
editor_history_v1
layer_evidence_v1
```

**未証明状態には新しいfeature flagを立てる。**これにより、このflagを知らない旧版リーダーは開くことを拒否する。投機的状態に対して欲しいfail-closed挙動が、スキーマ移行なしで自動的に成立する。

```rust
pub const ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1: &str = "speculative_unproven_fold_v1";
```

命名は既存8個と同じ`<snake_case>_v1`規約に従っている。この規約から外れないこと。

### 4.3.2 担当ファイル

- `crates/ori-formats/src/ori2.rs`（feature定数と required_features 生成）
- `crates/ori-formats/src/project_folder.rs`（展開folder形式にも同じ契約を適用）
- `crates/ori-core/src/editor/history_persistence.rs`（history entryの未証明マーク）
- `crates/ori-core/src/stacked_fold.rs`
- `apps/desktop/src-tauri/src/stacked_fold_transaction.rs`
- `apps/desktop/src/lib/coreClient.ts`

### 4.3.3 型設計（§3.1の遵守）

投機的tokenと証明済みauthorityは**別の型**にする。

```rust
/// 証明を伴わない投機的Apply権限。近似層の観測結果に束縛されるが、
/// 安全性の証明ではない。
pub struct SpeculativeUnprovenFoldTokenV1 { /* 非公開field */ }
```

- `SpeculativeUnprovenFoldTokenV1`から既存の証明済みauthority型への`From`/`Into`/`as_*`変換を実装してはならない。
- Serialize/Deserializeを実装してはならない（既存の非直列化one-shot token規約に従う）。
- `Clone`を実装する場合、既存の`Arc` identity保持規約と同じ意味にすること。
- one-shot消費とし、再使用は無変更へ閉じる。

### 4.3.4 束縛項目（既存Apply経路と同一水準）

投機的tokenは次の全項目へ束縛し、commit直前に再認証する。**既存Apply経路より緩めてはならない。**

```text
project instance
project ID
revision
geometry fingerprint
pose generation
request generation
紙厚 bit列
近似層が観測した blocking sample の有無と最初のblocking角度
```

最後の項目が重要である。**近似層が blocking sample を検出した場合、投機的Applyを発行してはならない。**投機的Applyが許されるのは「近似層で衝突が見つからなかったが、証明もまだ無い」状態だけである。「近似で衝突が見つかったが強行する」は許可しない。

### 4.3.5 history entryの扱い

Apply は既存どおり単一の atomic history entry を書く。これに未証明マークを持たせる。

- `docs/Codex/`既存記録のとおり、履歴は27コマンド・20 inverseで bit-exact 照合されている。未証明マークを追加しても、既存の inverse 生成と照合を壊さないこと。
- 未証明マークは entry の**メタデータ**であり、幾何そのものではない。Undo/Redo で幾何が bit-exact に復元される既存性質を変えない。
- 未証明マークが付いた entry を Undo した場合、Redo で未証明マークも復元される。
- 既定128件の履歴上限、trim動作を変えない。

### 4.3.6 撤回セマンティクス（オーナー決定）

**自動で巻き戻さない。**

- 事後証明が失敗（`Blocked`）または不明（`Unknown`）で終わった場合、該当 history entry の未証明マークを「証明失敗」または「証明不能」へ更新するだけとする。
- **`Undo`を自動実行してはならない。**利用者はその後さらに編集している可能性があり、無断で作業を失わせることは§3.2に違反する。
- 利用者へ提示するのは次の3点であり、選択は利用者が行う。
  1. 失敗した折り操作の識別（何手目のどの操作か）
  2. 理由（衝突証明あり / 証拠不足 / 資源上限 / 取消 / deadline）
  3. その entry まで Undo した場合に失われる後続編集の件数
- 「戻す」を選んだ場合のみ、既存の Undo を必要回数実行する。1回の操作として履歴に見えるようまとめてよいが、既存の inverse 照合を迂回してはならない。

### 4.3.7 絶対に行ってはいけないこと

- 未証明のprojectを、証明済みのものと同じ表示にする。
- `required_features`に未証明flagを立てずに未証明状態を保存する。
- 旧版リーダーが未証明ファイルを「証明済みとして」開けてしまう経路を作る。
- 近似層が blocking sample を検出した状態で投機的Applyを許す。
- 自動Undo。
- 未証明flagを、証明が完了していないのに消す。
- 復旧checkpoint（HIS-004）に未証明マークを保存しない（保存すること）。

### 4.3.8 必須回帰

Rust:

```text
speculative_token_has_no_conversion_into_proven_authority
speculative_token_is_not_serializable
speculative_apply_rejected_when_approximate_layer_found_blocking_sample
speculative_apply_rejected_on_revision_change
speculative_apply_rejected_on_pose_generation_change
speculative_apply_rejected_on_thickness_bit_drift
speculative_token_single_use_second_apply_is_no_change
unproven_mark_survives_undo_redo_round_trip
unproven_mark_persists_to_ori2_and_project_folder_and_recovery
ori2_without_speculative_feature_flag_rejects_unproven_document
legacy_reader_rejects_unknown_required_feature
proof_failure_updates_mark_without_automatic_undo
proof_failure_reports_subsequent_edit_count_for_user_decision
history_inverse_bit_exact_verification_unchanged_with_unproven_mark
```

TypeScript:

```text
speculativeApplyRequiresExplicitConfirmation
speculativeApplyRejectsWhenBlockingSampleObserved
unprovenBadgeDistinctFromProvenBadge
unprovenDocumentOnLoadShowsExplicitWarning
proofFailureOffersRevertButDoesNotAutoRevert
```

### 4.3.9 P1-3 受入条件

- 上記14 Rust test と5 TypeScript test が成功する。
- 未証明flagを立てた`.ori2`が、flagを削除した旧readerで**拒否される**ことを実測で示す。
- 診断JSON（OPS-004/005）に未証明件数の粗い区分が追加され、座標・ID・形状を含まないことをtestで固定する。

---

## 4.4 P1-4: 証明進捗UIと撤回UI

### 4.4.1 担当ファイル

- 新規 `apps/desktop/src/components/ProofProgressPanel.tsx`
- 新規 `apps/desktop/src/lib/proofProgressPanelText.ts`
- 新規 `apps/desktop/tests/proofProgressPanelText.test.ts`
- 新規 `apps/desktop/tests/proofProgressPanel.dom.test.tsx`
- `apps/desktop/src/lib/coreClient.ts`

### 4.4.2 表示要件

- 日英。既存の型付き翻訳カタログ規約に従う（`docs/Codex/claude-implementation-handoff-2026-07-26.md`§1参照）。位置依存label配列と`locale === 'ja' ? 0 : 1`を作らないこと。
- 証明進捗は「証明済みpair数 / 全pair数」と、未証明のhistory entry件数を表示する。
- 状態は次を独立した終端状態として区別する。VAL-009の既存規約と同じ粒度にすること。

```text
証明中 / 証明済み / 証明失敗 / 証明不能（証拠不足）/ 資源上限 / 取消 / deadline / 古い結果
```

### 4.4.3 ARIA

既存慣行に従う。

- 進捗・確認中: `role="status"` / `aria-live="polite"`
- 終端のblocking（証明失敗確定）: `role="alert"` / `aria-live="assertive"`

### 4.4.4 絶対に行ってはいけないこと

- raw path、raw error文字列、内部IDをUIへ出す。
- 未知の状態を「証明済み」側へ丸める。未知は必ず未証明側へ閉じる。
- 撤回ボタンを既定で目立たせ、誤操作で作業を失わせる。破壊的操作なので明示確認を要求する。
- locale切替でcontrolled値・selection・callback発火回数が変わる。

### 4.4.5 P1-4 検証コマンド

```powershell
cd apps/desktop
node --test tests/proofProgressPanelText.test.ts
npx vitest run --config vitest.config.ts tests/proofProgressPanel.dom.test.tsx
npx oxlint
npx tsc -b
npm run build
```

---

## 4.5 フェーズ1 完了条件

- P1-1〜P1-4 の全受入条件を満たす。
- 全体回帰（§7）が成功する。
- **報告に次を必ず明記すること。**
  - 15面の並列化後実測時間とスレッド数
  - 1頂点移動後の再証明pair比率
  - このフェーズで証明していないもの: 一般正厚連続運動、cross-block clearance、一般cycle、一般物理motion
- `docs/progress.md`「3D折り・紙厚・衝突」「折り可能性・経路探索」の進捗値は、**利用者経路が増えた分だけ**更新する。並列化とキャッシュは内部品質なので、それ単独では上げない（§3.5）。

---

# フェーズ2: 階層的証明合成によるスケール拡張

**目標: block合成を一般化し、数百〜数千面規模へ届かせる。**

このフェーズは研究要素を含む。前回のコスト見積りで「一般正厚連続運動・任意self-contact・一般層transport: 100〜200人月」と算定した領域そのものである。**フェーズ1完成後に着手すること。**

## 5.1 現状の足場（実測）

`crates/ori-collision/src/block_composition.rs`に既に次がある。

| 定数・関数 | 値・役割 |
|---|---|
| `MULTI_BLOCK_MIN_BLOCKS_V1` | 2 |
| `MULTI_BLOCK_MAX_BLOCKS_V1` | 8 |
| `BLOCK_COMPOSITION_LIMIT_V1` | 32 |
| `BLOCKWISE_POSITIVE_LAYER_ARITY_V1` | **2** |
| `BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1` | 4,096 |
| `diagnose_block_union_completeness_v1` | gap report発行 |
| `BlockUnionCompletenessGapReportV1` | gap report型 |
| `issue_complete_multi_block_positive_layer_authority_v1` | 完全被覆のsealed authority |
| `issue_block_composed_path_authority_v1` | 合成経路authority |
| `BlockComposedPathAuthorityV1` | 合成経路authority型 |

`docs/requirements-status.md`SIM-010行が明記する現在の限界:

> 内部proofではbounded 2..=8 blockのcanonical face/hinge unionを同一live geometry、hinge一意性、block共有tree、親authorityへsealed再結合できるが、**この完全被覆証拠単独ではApply・project mutation・viewerを認可しない**

> ただしこれはsubmitted setがlive geometryを完全被覆する境界だけを閉じ、**cross-block continuous clearance、共通articulation pose、cross-block layer transport**、Apply・project mutation・viewerを一切認可しない

**つまり残っている難所は正確に3つに特定されている。**この3つを順に閉じる。

## 5.2 P2-1: cross-block continuous clearance

### 5.2.1 証明すべき命題

現在は各 block 内部の連続clearanceだけが証明されている。block をまたぐ face pair について、遷移区間全体で strict 分離が保たれることを証明する必要がある。

block A の face `f` と block B の face `g` について、schedule 上の各遷移区間 `[θ_i, θ_{i+1}]` で次を示す。

```text
∀ θ ∈ [θ_i, θ_{i+1}] : dist(prism(f, θ), prism(g, θ)) > 0
```

既存の interval 手法（`MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 = 128`、`MAX_STACKED_FOLD_INTERVAL_DEPTH_V1 = 7`、`MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1 = 2_048`）を block 間へ拡張する。

### 5.2.2 実装方針

1. block 間の face pair を canonical に列挙する。両 block の face 数の積になるため、**broad phase を先に通すこと**。既存の`sweep-and-prune`（x-min・FaceId 決定sort）を再利用する。
2. 各 pair について、両 block の ancestry depth に応じた Lipschitz 変位上限で AABB を膨張し、strict 分離を試みる。
3. 未証明 pair を最小margin・lower endpoint・depth順で二分する既存 adaptive subdivision を使う。
4. 資源上限を超えたら`Unknown`へ閉じる。**block を減らして肯定してはならない。**

### 5.2.3 絶対に行ってはいけないこと

- sample 点だけで肯定する。開区間の証明が必須である。
- block 内の clearance から block 間の clearance を推論する。
- broad phase で除外した pair を「分離済み」として扱う。x strict gap を coverage margin へ含める既存規約に従い、**除外した pair も margin 計算へ寄与させること**。
- 片方の block だけの pose で判定する。両 block が同一 articulation pose を共有していることが前提であり、それは P2-2 の責務である。P2-2 未完のまま P2-1 で肯定を出してはならない。

### 5.2.4 必須回帰

```text
cross_block_clearance_proves_three_block_positive_thickness
cross_block_clearance_proves_eight_block_positive_thickness
cross_block_clearance_rejects_sample_only_evidence
cross_block_clearance_rejects_without_shared_articulation_pose
cross_block_clearance_broad_phase_excluded_pair_contributes_margin
cross_block_clearance_resource_exhaustion_yields_unknown_not_reduced_blocks
cross_block_clearance_reversed_block_order_identical_result
cross_block_clearance_near_collision_is_unknown_not_safe
```

## 5.3 P2-2: 共通articulation pose

### 5.3.1 証明すべき命題

複数 block が**同一の articulation pose**から発行されていることを、issuer・pose instance・hinge角bit列・紙厚bitまで束縛して証明する。

現在の`issue_complete_multi_block_positive_layer_authority_v1`は「source・厚さ・issuer context・層fingerprint・target角を再検証する」とあるが、**共通 articulation pose 自体は認可対象外**と明記されている。ここを閉じる。

### 5.3.2 実装方針

- 各 block の`ExactHingePose`が、同一の親 pose から決定的に導出されたことを示す。
- 共有 hinge については、両 block での occurrence が bit-exact に同一の`ExactPoint3`であることを示す。既存の`canonical_point_eq`を使う。
- 共有頂点については、既存の shared-vertex wedge 手法（`MAX_SHARED_VERTEX_WEDGE_VERTICES_V1 = 256`、`MAX_SHARED_VERTEX_WEDGE_WORK_V1 = 4_000_000`）を block 間へ拡張する。
- 導出のchainを非直列化 opaque authority として封印する。既存の sealed authority 規約に従う。

### 5.3.3 絶対に行ってはいけないこと

- 角度値が同じだから同一poseとみなす（ABA）。pose instance identity が必須である。
- 1 ULP差を許容する。
- 別 project instance / 別 revision の pose を結合する。
- 共有 hinge の occurrence が「ほぼ同じ座標」で許可する。bit-exact 一致が必須である。

### 5.3.4 必須回帰

```text
shared_articulation_pose_proves_eight_block_same_instance
shared_articulation_pose_rejects_equal_angle_different_instance_aba
shared_articulation_pose_rejects_one_ulp_hinge_angle_drift
shared_articulation_pose_rejects_one_ulp_thickness_drift
shared_articulation_pose_rejects_cross_project_instance
shared_articulation_pose_shared_hinge_occurrence_must_be_bit_exact
shared_articulation_pose_authority_is_not_serializable
```

## 5.4 P2-3: cross-block layer transport

### 5.4.1 証明すべき命題

`BLOCKWISE_POSITIVE_LAYER_ARITY_V1 = 2`が現在の arity 制限である。block 間で層順序（layer order）を輸送し、arity を一般化する。

`docs/requirements-status.md`SIM-010は「一般複数層transport」を未完と明記している。

### 5.4.2 実装方針

1. まず arity 2 → 3 へ拡張する。**一気に一般化しないこと。**本プロジェクトの既存の進め方（正厚treeを3面→4面→5面→…→15面と1段ずつ拡張し、各段で証明とfixtureを固定した）に従う。
2. 各段で、層順序が block 境界を越えて一貫することを証明する。既存の`facewise_layer_order_v1`と`flat_endpoint_layer_order.rs`を参照する。
3. 各段の到達点を`docs/progress.md`へ記録する。**到達していない arity を実装済みと書かないこと。**

### 5.4.3 段階目標

```text
arity 2（現状）→ 3 → 4 → 8 → 16
```

各段で必須回帰を追加し、次の段へ進む前に前段を固定する。

### 5.4.4 絶対に行ってはいけないこと

- arity を一般化したと主張して、実際には特定の形状クラスだけで動く実装を出す。対象クラスを必ず明示すること。
- cycle を含む topology で肯定する（フェーズ2の対象は tree と bounded cycle まで。一般 multi-cycle は対象外であり、その旨を明記すること）。
- 層順序の証明を欠いたまま viewer へ接続する。

## 5.5 P2-4: block自動分割

### 5.5.1 設計

現在は block 分割が呼出側から与えられる前提である。数千面規模を扱うには、**展開図から証明可能な block 分割を自動決定する**必要がある。

### 5.5.2 実装方針

- 分割は決定的でなければならない。同じ展開図から常に同じ分割を得ること。
- 分割の目標は「各 block が既存の正厚tree証明の到達範囲（現状15〜17面、hard cap 64面）に収まること」。
- `MULTI_BLOCK_MAX_BLOCKS_V1 = 8`が上限なので、8 block × 64 面 = 512 面が現状の理論上限である。数千面へ届かせるには`MULTI_BLOCK_MAX_BLOCKS_V1`の引き上げも必要になる。**引き上げは P2-1〜P2-3 が完成し、cross-block証明が block数に対してスケールすることを実測で示した後に行うこと。**
- 分割できない場合は`Unknown`へ閉じる。無理に分割して肯定してはならない。

### 5.5.3 絶対に行ってはいけないこと

- 分割を非決定的にする（HashMap のイテレーション順に依存する等）。
- 分割の都合で hinge を切断する。分割は face の grouping であり、topology を変えてはならない。
- 分割数を増やすことで cross-block pair 数が爆発することを無視する。block 数 n に対し cross-block pair は O(n²) で増える。上限計量に含めること。

## 5.6 フェーズ2 完了条件

- P2-1〜P2-4 の必須回帰が全て成功する。
- **実測で到達した面数・block数・arityを報告に明記すること。**目標値ではなく実測値を書く。
- このフェーズで証明していないものを明記する: 一般 multi-cycle、一般 self-contact、一般物理motion、一般非平坦target。
- `docs/requirements-status.md`SIM-010行と`docs/progress.md`を、**実際に到達した範囲だけ**で更新する。

---

# フェーズ3: 領域分離と証明到達度の可視化

**目標: 平坦折り判定と正厚連続運動で別の最適手法を使い、利用者に「何がどこまで証明されているか」を領域別に見せる。**

## 6.1 P3-1: 平坦折り判定への ply/treewidth FPT 導入

### 6.1.1 背景

全体平坦折り可能性の判定は NP完全である（Bern & Hayes 1996）。ただし **ply（重なり枚数）と、対応する平面グラフの treewidth をパラメータとすると fixed-parameter tractable** である（arXiv:2306.11939, "A Parameterized Algorithm for Flat Folding"）。

実作品の展開図は面数が数千でも ply が構造的に小さいことが多い。**面数ではなく ply でスケールする**アルゴリズムを入れれば、平坦折り判定は数千面へ届く可能性がある。

### 6.1.2 重要な制約

**この手法は正厚連続運動には使えない。**FPT 結果は平坦折り可能性の判定に関するものであり、フェーズ2で扱う正厚 prism の連続clearanceには適用できない。P3-1 の成果を SIM-010 の到達範囲として計上してはならない。対象は VAL-003 / VAL-005 側だけである。

### 6.1.3 現状

`docs/technical-research.md`§2.8のとおり、現在は外部 SAT/SMT に依存せず`convex_faces_facewise_v1`専用の決定論的 solver を実装している。

- exact 平面反射と重なり cell から canonical 面対の二値変数を作る
- Mountain/Valley、推移、taco-taco、taco-tortilla 等の許容 tuple を BFS で伝播
- 未確定成分は明示的な`SearchFrame` stack と assignment trail を用いる DFS で探索

対象クラスは**凸 material face 限定**である。

### 6.1.4 実装方針

1. **既存 solver を置き換えないこと。**新しい model ID を追加し、既存`convex_faces_facewise_v1`は保持する。
2. ply と treewidth を先に測る診断を実装する。ply または treewidth が閾値を超える入力は既存 solver へ回す。
3. tree decomposition を決定的に構築する。同じ入力から常に同じ decomposition を得ること。
4. 動的計画法の各段で、既存の exact 平面反射・重なり cell の意味を変えないこと。
5. 可（possible）は independent に再検証できる certificate がある場合だけ返す。既存規約（`docs/technical-research.md`§2.8）を弱めない。
6. 時間制限つき3値判定（VAL-005、1〜300秒）の契約を維持する。時間切れは「不可」ではなく「不明」。

### 6.1.5 絶対に行ってはいけないこと

- 既存`convex_faces_facewise_v1`を削除・置換する。
- FPT の成果を正厚連続運動（SIM-010）の到達範囲として計上する。
- treewidth の近似値で肯定する。decomposition の width を実測し、上限内であることを示すこと。
- tree decomposition を非決定的に構築する。
- 対象クラス（凸 material face、ply上限、treewidth上限）を明示せずに「一般化した」と主張する。
- 時間切れを「不可」へ変換する。

### 6.1.6 必須回帰

```text
fpt_flat_folding_matches_existing_solver_on_small_instances
fpt_flat_folding_decomposition_is_deterministic
fpt_flat_folding_high_treewidth_falls_back_to_existing_solver
fpt_flat_folding_high_ply_falls_back_to_existing_solver
fpt_flat_folding_possible_result_has_independently_verifiable_certificate
fpt_flat_folding_timeout_yields_unknown_not_impossible
fpt_flat_folding_reversed_input_order_identical_result
fpt_flat_folding_does_not_claim_positive_thickness_authority
```

最後の test が重要である。FPT 経路が正厚 authority を発行できないことを型レベルで固定すること。

### 6.1.7 P3-1 受入条件

- 既存 solver と小規模インスタンスで**完全一致**する。
- ply が小さい大規模インスタンス（面数500以上）で判定が完了する。**実測面数と ply を報告に明記する。**
- 上記8 test が成功する。

## 6.2 P3-2: 領域別証明到達度UI

### 6.2.1 設計

利用者に「この作品の、どの領域が、どこまで証明されているか」を見せる。

領域は次で分ける。

| 領域 | 証明手法 | 到達スケールの表示 |
|---|---|---|
| 局所平坦折り（VAL-002） | 川崎・前川定理 | 頂点単位の可否 |
| 全体平坦折り（VAL-003/005） | 既存 solver または FPT | 面数・ply・treewidth・使用手法 |
| 正厚静的衝突 | face pair 分類 | 証明済みpair数 / 全pair数 |
| 正厚連続運動（SIM-010） | block合成 | 面数・block数・arity |
| 折り経路 | dyadic / interval | level・遷移数 |

### 6.2.2 表示要件

- 日英。型付き翻訳カタログ規約に従う。
- 各領域について、**証明済み / 未証明 / 証明不能 / 対象外**を区別する。「対象外」を「不合格」と混同しない（VAL-002の既存規約と同じ）。
- 使用した証明手法の model ID を利用者に見せる。ただし raw な内部識別子ではなく、日英の説明文へ写像すること。
- 未証明flag（P1-3）が立っている history entry 件数をここに集約する。

### 6.2.3 絶対に行ってはいけないこと

- 「証明できていない」を「安全」と読める表示にする。
- 対象外を不合格として表示する。
- 全領域を1つの％にまとめる。**領域ごとに独立して見せること。**単一の数値へ丸めると、どこが弱いか分からなくなる。
- raw path、raw error、内部UUIDを表示する。

### 6.2.4 担当ファイル

- 新規 `apps/desktop/src/components/ProofCoverageMatrixPanel.tsx`
- 新規 `apps/desktop/src/lib/proofCoverageMatrixPanelText.ts`
- 新規 `apps/desktop/tests/proofCoverageMatrixPanelText.test.ts`
- 新規 `apps/desktop/tests/proofCoverageMatrixPanel.dom.test.tsx`

### 6.2.5 必須回帰

```text
proofCoverageMatrixDistinguishesOutOfScopeFromFailure
proofCoverageMatrixShowsPerDomainStatusNotSinglePercentage
proofCoverageMatrixMapsModelIdToLocalizedDescription
proofCoverageMatrixAggregatesUnprovenHistoryEntryCount
proofCoverageMatrixLocaleSwitchPreservesSelection
proofCoverageMatrixRejectsUnknownDomainStatusToUnprovenSide
```

## 6.3 フェーズ3 完了条件

- P3-1・P3-2 の必須回帰が全て成功する。
- FPT 経路の到達面数・ply・treewidth を実測で報告する。
- P3-1 が正厚 authority を発行しないことを型で示す。
- `docs/progress.md`「折り可能性・経路探索」の進捗値を、平坦折り側で増えた利用者経路の分だけ更新する。

---

## 7. 全体回帰（各フェーズ完了時に必須）

repository root:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`apps/desktop`:

```powershell
npm run test:snap
npm run test:dom
npm run lint
npm run build
```

現在の基準値（`docs/Codex/claude-implementation-handoff-2026-07-26.md`§4記載）を下回らないこと。

```text
npm run test:snap → tests 1815 / pass 1815 / fail 0（以上）
npm run test:dom  → Test Files 60 passed、Tests 416 passed（以上）
```

既存のlint警告（`App.tsx`、`coreClient.ts`、`EffectiveCutDiagnosticPanel.tsx`のreact-hooks/optional-chaining）は担当差分由来ではないので解消不要。ただし**新規に警告を増やさないこと。**

### 7.1 既知の環境問題

`docs/Codex/claude-implementation-handoff-2026-07-26.md`§6記載の事象に注意すること。

> 新規`CARGO_TARGET_DIR`（例`target-claude-p3`）では`proc-macro2`/`serde_json`のbuild scriptが`os error 4551`でブロックされ、再試行でも解除されなかった。既存の暖まったtarget dirか共有`target/`なら実行できる。

隔離target dirでの検証を前提にした手順は、この環境では成立しないことがある。共有`target/`を使うこと。

---

## 8. 実施順序（変更不可）

```text
P1-1 並列化
  ↓
P1-2 証明キャッシュと差分無効化
  ↓
P1-3 投機的Applyと未証明flag
  ↓
P1-4 証明進捗UIと撤回UI
  ↓  ← フェーズ1完了。ここで一度オーナーへ報告し承認を得る
P2-2 共通articulation pose      ← P2-1の前提なので先に実施
  ↓
P2-1 cross-block continuous clearance
  ↓
P2-3 cross-block layer transport（arity 2→3→4→8→16 の各段で固定）
  ↓
P2-4 block自動分割
  ↓  ← フェーズ2完了。ここで一度オーナーへ報告し承認を得る
P3-1 平坦折りへのFPT導入
  ↓
P3-2 領域別証明到達度UI
```

**P2-2 を P2-1 より先に実施すること。**cross-block clearance は共通 articulation pose の証明を前提とするため、順序を逆にすると P2-1 が肯定を出せない（§5.2.3参照）。

フェーズ1完了時とフェーズ2完了時には、次へ進む前にオーナーへ報告し承認を得ること。フェーズ2以降は研究要素を含み、想定どおり進まない可能性がある。**進まない場合はその事実を報告すること。無理に肯定範囲を広げてはならない。**

---

## 9. commit・引渡し規約

- 1 package = 1 commit。パッケージを跨いだ commit を作らない。
- commit message は既存の日本語体裁に合わせる（例: `二進有理経路プレビュー境界を分離する`）。
- author は `yuya <oltotlo79@gmail.com>`。
- push は行わない。オーナーが差分を確認してからまとめて push する。
- `docs/requirements-status.md`と`docs/progress.md`を更新する場合は、該当 package の commit に含める。
- 担当外ファイルを stage しない。

---

## 10. 完了報告形式

各 package について次を記載すること。

```text
### P<n>-<m>: <名称>

- commit: <full SHA>
- author: <name <email>>
- 変更file（<件数>件）
  - <path>
  - ...
- focused検証
  - <command> → <結果>
  - ...

設計上の要点。
- <実装した定理・境界>
- <肯定しなかったcaseとその理由>
- <決定性の根拠>

実測値。
- <性能・スケールの実測値。目標値ではなく実測値>

このpackageで証明していないもの。
- <明示的に列挙>
```

最後に次を必ず記載すること。

```text
## 完成度への影響

- docs/progress.md の変更: <あり/なし。ありの場合は領域名と旧値→新値、根拠となる利用者経路>
- docs/requirements-status.md の変更: <あり/なし。ありの場合は該当IDと根拠>
- MUST集計（現在 85/2/0）の変更: <あり/なし>
- 81.96%（表示82.0%）正本発効規則: 変更していない

## 未解決事項

- <環境問題、想定外の障害、判断を仰ぎたい点>
```

---

## 11. この文書の作成方法と制約

本文書は次の実測に基づく。

- `crates/ori-collision/src/block_composition.rs`の公開API・定数の実測
- `crates/ori-collision/src/static_collision.rs:63`の`StaticCollisionLimits`フィールド27件超の実測
- `crates/ori-collision/src/continuous_path.rs:1-6, 120-134`のsampling契約とcertificate model ID 8件
- `crates/ori-formats/src/ori2.rs:23-30`の`ORI2_FEATURE_*`定数8件
- `apps/desktop/src/lib/foldPreviewCollision.ts:3-6`、`foldPreviewNarrowCollision.ts:37,43,45,47`の近似層上限
- workspace `Cargo.toml`と各crate `Cargo.toml`の依存実測（rayon 不在の確認）
- `crates/*.rs`に対する`std::thread`検索（3ファイルのみ）
- `docs/requirements-status.md`、`docs/progress.md`の未完境界記述

### 11.1 推測に基づく箇所（実装前に検証が必要）

次は実測ではなく、Claudeの判断である。Codex側で妥当性を再評価すること。

- §4.1.6 の「スレッド数4で2.5倍以上」という受入基準。face pair の粒度が細かすぎるとスレッド生成コストで達成できない可能性がある。
- §4.2.5 の3つのhard cap初期値（65,536 / 16 MiB / 2,000,000）。実測後に調整が必要。
- §4.2.7 の「再証明pair比率20%未満」。展開図の構造次第で達成できない可能性がある。
- §5.5.2 の「8 block × 64面 = 512面」という理論上限の算出。cross-block pair 数の増加を考慮していない粗い見積りである。
- §6.1.4 の ply / treewidth 閾値。実作品の展開図の ply 分布を測っていないため、閾値は実測後に決める必要がある。

### 11.2 誇張を避けた点

- フェーズ2は前回コスト見積りで100〜200人月と算定した研究課題であり、「実装指示を書いたから実装できる」とは主張しない。進まない可能性を§8に明記した。
- ply/treewidth FPT が**正厚連続運動には使えない**ことを§6.1.2で明記した。これを SIM-010 の到達範囲へ計上すると過大申告になる。
- 数千面という目標は、§5.5.2の算出でも現状の定数では512面が上限である。「数千面へ届く」は`MULTI_BLOCK_MAX_BLOCKS_V1`引き上げとcross-block証明のスケール実証が前提であり、達成が保証されているわけではない。
- 本文書は要件を新設せず、MUST 87件を変更しない。フェーズ1〜3の完了は「完成」を意味しない。
