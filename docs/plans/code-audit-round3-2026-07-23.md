# ORIGAMI2 コード監査レポート 第3回（不具合・矛盾・改善・振る舞い保存リファクタ）

- 監査基準コミット: `958e804`（2026-07-23 16:51、Codex停止中に実施＝torn readなし）
- 実施: Claude（読み取り専用アドバイザー）。**本レポート以外のファイルは一切編集していない。**
- 方法: 5領域を並列で実ソース精読（Rust中核バグ／Rust冗長性／frontend／native・formats／横断）。全所見は file:line を開いて検証済み。前回まで（`code-audit-2026-07-22.md`, `code-audit-2026-07-23.md`）で修正済みの項目は再報告しない。
- **最重要方針**: 全提案は**振る舞い保存**で、**fail-closed安全性・厳密演算・資源上限・テスト網羅・技術的深さを一切下げない**。冗長削減は「意味を変えず重複を畳む」形に限定。各リファクタに安全性注記を付す。
- 各修正は before→after コードを添付。Codexがそのまま適用できる粒度にしてある。

---

## 0. 健全性の再確認（重要）

**前回修正は全て堅持**：A-1〜A-11・A-5（writer/reader検証対称）・B-5（症状3の層順序配線）は現行コードで維持。
**新しい難所コードは健全と確認**（精読の結果、局所成功を全体と誤認・無検証破棄・有界を一般と誤標示のいずれも無し）：
- `block_composition.rs`：`block_intersection_is_tree_v1` が単一面関節＋`edges==n-1`＋DFS連結を強制、multi-block権威は `authorizes_*`=false で正直にスコープ、辺の互いに素・固定面共有を証明しSHA封印＋改ざんテスト。
- `effective_cut_static.rs`：整列ヒンジ上の貪欲マージが正しく、被覆チェックで閉じ、fail-closed。
- `shared_hinge_solid_classification.rs`：正体積脱出証明は exact-E と direct-F の**両カーネル**がrank-3正体積を証明し、**両方の頂点全走査**で回廊外頂点を示す時だけ許容。
- `constraints.rs` のMUS/一般比率グラフ矛盾：厳密 `ExactPositiveRatioV1` 演算（浮動小数の丸めなし）、witness最小、work-metered。
- 新しい衝突コードに到達可能な `unwrap/expect/index` panic なし。

したがって本レポートの指摘は、**旧来の共有ヒンジ/頂点回廊分類器の精度・一貫性**と、**コメント/文書の矛盾**、および**振る舞い保存のリファクタ**が中心。安全性を脅かす新規の穴は見つからなかった。

---

## A. 不具合（BUG）— 修正コード付き

### A-1【MED】日本語ラベル「二面角」だが実際は面法線角（その補角）を表示
`apps/desktop/src/components/FoldPreview.tsx:4303`（`lib/foldPreviewMeasurement.ts:63`）
`measureWorldFaceNormalAngleDegrees` は2面の**法線**間角 `acos(n̂₁·n̂₂)` を返す（英語ラベルは正しく "Face-normal angle"）。法線同一向きなら `法線角 = 180° − 二面角`。JAユーザーは平坦な面対を「二面角 ~0°」と誤読（実二面角は180°）。
```tsx
// before (FoldPreview.tsx:4303)
? foldPreviewText(locale, '2面の二面角', 'Face-normal angle')
// after — 計算量と一致させる（ラベルのみ）
? foldPreviewText(locale, '2面の法線角', 'Face-normal angle')
```
安全性: 表示ラベルのみ。幾何・IPC不変。

### A-2【LOW-MED】state updater 内で `setResult(null)` を呼ぶ不純な更新
`apps/desktop/src/components/EffectiveCutDiagnosticPanel.tsx:92-98`
updaterは純粋でなければならず、StrictMode/並行で再実行され描画中に副作用が発火。今は冪等ゆえ無害だがReactルール違反。
```tsx
// after — updater外でリセット
setResult(null)
setSelected((previous) => {
  const next = new Set(previous)
  if (checked) next.add(key); else next.delete(key)
  return next
})
```
安全性: 観測結果は同一。ルール違反を除去。

### A-3【LOW】面0の平面モデルで `model.faces[0].id` が例外
`apps/desktop/src/lib/instructionOnionSkin.ts:97-100`（非平面枝は `faces.some(...)` で守るが平面枝だけ `faces[0]` を信頼）
```ts
// after
if (model.kind === 'planar') {
  const face = model.faces[0]
  if (!face || request.pose.fixed_face !== null) return null
  return new Map([[face.id, new Matrix4()]])
}
```
安全性: `null` は既存の「利用不可」契約。例外を意図した優雅な失敗へ。

### A-4【LOW-MED・潜在panic】`applied_pose` のアクセサが `tree().expect()`（`Graph`姿勢で潜在panic）
`apps/desktop/src-tauri/src/applied_pose.rs:401,410,453,463`
`CurrentNativeMaterialPose` は `Graph` 変種あり（`:1008`生成）で `tree()` は `None`。今日は `spawn_blocking`＋`panic=unwind` で `ANALYSIS_FAILED` に化けて封じ込め。A-1と同型の潜在panicで原因を消す。
```rust
// after — expectを除き、呼び出し側でOptionを束縛
let (model, pose) = capability.tree()
    .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
```
安全性: fail-closed維持、`catch_unwind`依存を除去。

### A-5【LOW・地雷】`from_document` が `restore_beginner_design_profile(...).expect()`
`apps/desktop/src-tauri/src/lib.rs:621-622`（二つ隣の `restore_archive_editor:1186` は `.map_err` で優雅処理）。今はテストのみだがopen経路配線で読込時panic。
```rust
// after — from_document を Result 化し :1187 と揃える
.restore_beginner_design_profile(document.beginner_design_profile)
    .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
```
安全性: fail-closed規約に整合。

### A-6【LOW-MED】共有ヒンジを頂点数だけで推定し、AABB高速路が厳密路と食い違う
`crates/ori-collision/src/cayley/positive_thickness.rs:2050-2194`（`NativeStaticCollisionGeometryProof` へ供給）
(a) `shared_vertex_count == 2` だけで `SharedHingeCorridorAllowed` を出し、**実ヒンジ辺が両面を結ぶかを検査しない**（新分類器 `effective_cut_static.rs:867-889` は実ヒンジ端点一致を要求）。(b) AABB高速路（`:2119-2154`）が**厳密交差種別を確認せず**回廊内overlapと判定 → 同一幾何が高速路の発火有無で `Separated` と `…Allowed` に割れ、診断件数を水増し。**両方fail-safe**（AABBは真交差の上位集合、非ヒンジ対は下流 `InconsistentMaterialPose` で拒否）だが精度・一貫性の欠陥。
```rust
// (a) after — 実ヒンジ同一性を確認してから共有ヒンジ回廊を使う（fail-closed維持）
} else if shared_vertex_count == 2
    && exact.hinges.iter().any(|hinge| {
        let ep = hinge.endpoint_vertices;
        ep.contains(&shared_vertices[0].expect("counted").0)
            && ep.contains(&shared_vertices[1].expect("counted").0)
        // かつ hinge が試験中の2面を結ぶこと（parent/child == {first,second}）
    })
{ /* …既存のAABB回廊テスト… */ }
// (b) after — 回廊acceptを exact intersection.kind() が判った後（:2195 の match 内）へ移し、
//     diagnose_source_flat_prism_pair_v1（:1906-1928）と同じく高速路は純粋なaccept/reject述語に留める
```
安全性: いずれも分類を**厳格化**するのみ（ヒンジ同一性述語を追加／厳密カーネルへ委譲）。貫通検出は不変。

### A-7【LOW-MED】`face_count > 2` の衝突無し証明が緩いAABB回廊、`==2` は厳密円柱回廊（剛性の非対称）
`crates/ori-collision/src/static_collision.rs:972-1011`
`==2` は `diagnose_bound_shared_hinge_solid_for_edge_v1`（厳密円柱回廊）で admit するが、`>2` は同じ証明を `diagnose_bound_positive_thickness_prism_pairs_v1` の**AABB回廊**（ヒンジ線分±8×厚）で発行。証明のdocstring（`:824-833`）は「独立有限回廊分類器が admit した時だけ」と主張するが**2面時のみ真**。
```rust
// after — 多面も2面と同じ厳密分類器へ通す（:972の face_count>2 特別扱いを削除）
let classified = diagnose_bound_shared_hinge_solid_for_edge_v1(
    bound, paper_thickness_mm, Some(hinge.edge()),
).map_err(map_shared_hinge_solid_diagnostic_error)?;
// 以降 Allowed/Penetrating/回廊fallback を :1012-1048 の既存 ==2 アームと同様に match
```
安全性: 緩いAABBを厳密円柱へ置換＝admitを**減らす**方向のみ（証明を強化、決して弱めない）。性能上AABB高速路を残すなら docstring を「多面はAABB回廊で admit」と訂正する（どちらか必須）。

### A-8【LOW・契約/死蔵ガード】有界MUSのオラクル呼び出し上限が到達不能
`crates/ori-core/src/constraints.rs:444,460-505`
`MAX_..._CONSTRAINTS_V1=16`、`MAX_..._ORACLE_CALLS_V1=65_535`。全非空部分集合を各1回＝最大 `2^16−1=65_535` なので `oracle_calls > 65_535` は決して発火せず、早期中断枝は死蔵。MUS結果自体は正しい（増加サイズ走査で最小基数）。
```rust
// after — 上限を件数上限から導出し「限界==最悪部分集合数」を明示不変条件に
pub const MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1: usize =
    (1 << MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1) - 1;
```
安全性: 現状の振る舞いを厳密に保存しつつ、暗黙のタイトさを検査可能な不変条件に。

---

## B. 矛盾点（CONTRADICTION）

### B-1〜B-3【正本progress.md未是正】3D 99%・制約100%・自動設計55%
実コードは不変（3D=`prove_static_collision_geometry` が一般正厚を `PairEvidenceUnavailable`/遮断`Indeterminate`；制約=`Unknown{SolverRequiredConstraintKinds}` の限定family；自動設計=28変種enum＋`index%2`）。オーナー決定のCIゲートで正本は79.3%維持中だが、受理済み再計上案は**82.0%**（3D 99→75・制約100→85・**自動設計55→35**・経路45→78）。**自動設計は前回「過大と認めつつ60%へ加点」だったのが35%へ是正済み**。
```
// progress.md 該当セルの最小是正（凍結維持なら pending 82.0% への注記でも可）
| 3D折り・紙厚・衝突 | 17% | 75% | 12.75% | …一般正厚・任意self-contactは未証明で遮断（Indeterminate）
| 数式・幾何制約     |  9% | 85% |  7.65% | …直接矛盾certificateは限定family、一般充足可能性と非同値
| 初心者向け自動設計 |  8% | 35% |  2.80% | …一般treeは骨格の線形写像＋交互M/Vで平坦可解性を合成しない
```

### B-4【新規】soundな矛盾種別数が文書間で不一致（実enumと合わない）
`progress.md:32`「13種」・`requirements-status.md:19,222`「15種」・`code-audit-response-2026-07-23.md:46,90`「9種(+2)」だが、実enum `DirectConstraintConflictKindV1`（`constraints.rs:315-389`）は**17変種**。数え方の基準が未定義。
```
// requirements-status.md:222 before「15種のsoundな矛盾」
// after「17種の直接矛盾variant（13 pairwise + 4 general-graph、DirectConstraintConflictKindV1）」
// progress.md:32 の「13種すべて」も同基準へ統一
```

### B-5【B-4残存】複数の「現在」総計とMUST集計が併存
総計3つ（79.3%正本／82.0%再計上案／84.0%旧07-22応答）、MUST集計も86/1/0 と 85/2/0 が併存、`requirements-status.md:3` header日付「2026-07-20」だが本文は07-23。→ 旧84.0%/86-1-0を07-23の82.0%/85-2-0へ一本化、header日付を「2026-07-23」へ。

### B-6【コード対コメント】`static_collision.rs:1136-1146` のコメントが `:1104` の多面証明構築子と矛盾
コメントは「多面診断は公開幾何証明を発行できない」と言うが、`:1104` は正厚・全三角・回廊対応の多面姿勢（`face_count>2` 含む）に `NativeStaticCollisionGeometryProof` を発行する。安全境界の記述が実装と食い違う。→ コメントを「多面公開証明は `:1104` の正厚三角枝のみが、共有ヒンジ回廊完全被覆かつ無貫通で発行。本fall-throughは証明を作らずゼロ厚診断のみで `PairEvidenceUnavailable` へfail-close」と訂正（コードは不変）。

### B-7【品質】`ProvenTransversalPenetration` の名前が自身のコメントと矛盾
`crates/ori-collision/src/static_collision.rs:156-165`。コメントは「共面正面積重なりと非三角whole faceの厳密貫通も admit する（＝transversal専用でない）」と言うのに、フィールドは `proven_transversal_pairs`/`first_proven_transversal_pair`。wireは既に `proven_zero_thickness_penetration` へ一般化済み（`progress.md:135-136`）でRust内部名だけ遅れ。
```rust
// after（振る舞い保存の改名）
ProvenZeroThicknessPenetration { …, proven_penetrating_pairs, first_proven_penetrating_pair }
// #[error(...)] 文言は既に中立なので識別子のみ変更
```

---

## C. 死蔵コード（**削除でなくgate／配線を推奨。深さを下げない**）

### C-1【高価値】健全な有界MUSオラクルが実装済みだが本番未接続 → EDT-009へ配線
`crates/ori-core/src/constraints.rs:460` `find_bounded_direct_mus_v1`＋`BoundedDirectMusV1`（指数2ⁿ部分集合MUS、≤16制約/≤65535呼び）。全呼び出しが `#[cfg(test)]`（`:3302`〜）内、desktopは `prepared.preflight()`（`lib.rs:8253`）経由でMUSに到達しない。`lib.rs:28,38` でexportのみ。
**推奨**：`#[cfg(test)]` gate、または**より高価値：EDT-009（`analyze_geometric_constraint_document`, `lib.rs:8228`）へ配線**。これで要件が「残」とする「一般最小不能部分集合」が**健全に**実現する（07-23応答が正しく退けたGauss-Newtonランク法と違い、これは健全）。B-3の実質解消にも寄与。

### C-2 `cycle_fold_transaction` モジュール（本番未到達の重複）
`crates/ori-core/src/cycle_fold_transaction.rs:48,95`。全呼び出しがテスト、`lib.rs:44` export、本番は src-tauri の `stacked_fold_transaction.rs` が担う（C-6既知）。→ 単一の閉路適用primitiveとして配線しsrc-tauri重複を除去、または「研究先行・本番外」のmod doc＋gateで silent divergence を防ぐ。

### C-3 `BlockComposedPathAuthorityV1`＋`issue_..._v1`（本番未到達・export）
`crates/ori-collision/src/block_composition.rs:926,1008`。呼び出しは `continuous_path.rs:7245,7259`（`#[cfg(test)]:5767`〜）のみ、`lib.rs:53,58` export。→ `direct_f_affine_corridor` と同じ `#[cfg(test)]` gate、または本番経路へ配線。削除しない（テスト有）。

### C-4 `ConstraintSolveErrorV1::UnsupportedConstraintKind`（構築されず・テストが偽の安心）
`crates/ori-core/src/constraint_solver.rs:58`。唯一の参照 `:1039` が `Err(A) | Err(UnsupportedConstraintKind)` を許容するテストで、変種は生成されないためアームは決して行使されず assertion が自明成立。→ 変種を除きテストを `Err(InvalidConstraintDocumentOrGeometry)` のみへ厳格化（API安定が必須なら `// 予約・未emit` 注記＋テスト厳格化）。

### C-5【品質】`#[allow(dead_code)] mod cayley` の一括抑制が真の死蔵を隠す
`crates/ori-collision/src/lib.rs:23`。cayleyは本番到達（`continuous_path.rs:21` 等）なのにmod全体の許可で、到達コードと研究コードの警告を一律に潰す（2,892行の `direct_f_affine_corridor` が監査まで隠れた原因）。→ 一括allowを外し、真にテスト専用のサブモジュールは `#[cfg(test)]`、研究先行の個別itemにのみ理由付きの狭い `#[allow(dead_code)]`。これで C-1/C-3 がコンパイル時に露見する。

---

## D. 振る舞い保存リファクタ（冗長削減・保守性）— コード付き

**全て「意味を変えず重複を畳む」。資源上限・fail-closed・厳密演算・テストは不変。**

### D-1【Rust衝突・約550行削減】メータリング/クランプ定型のマクロ・関数化
- `projected()` のハード上限クランプ `.min(hard.field)` が~10メソッド計**176行** → `clamp_to_hard!` field-listマクロ（`positive_thickness.rs:103-131,580,641`, 各corridor/prism/ef_boundary…）。
- `charge_counter`（6サブモジュール**114行**同一）・`set_fixed_counter`（7モジュール、**ただし `direct_f_affine_corridor` は再初期化ガードを欠くので6つだけ集約**。安易統一はガード追加＝振る舞い変化なので不可）→ `positive_thickness.rs` へ `pub(super) fn` として集約。
- `check_limit`/`build_limit` 系7関数（77行、`actual>maximum` 判定）→ crate毎 `macro_rules!`。
- `cayley.rs` の既存 `checked_work_sum`（`:3870`）を**41箇所**のインライン `checked_add().ok_or(ResourceLimitExceeded{...})` に採用。
```rust
macro_rules! clamp_to_hard { ($s:ident,$h:ident,$t:ty,[ $($f:ident),+ ]$(,{ $($x:tt)* })?) =>
  { <$t> { $( $f: $s.$f.min($h.$f), )+ $( $($x)* )? } }; }
```
安全性: `min`クランプ・overflow→`ResourceLimitExceeded`・`STAGE`・ガード意味は byte単位で同一。DoSメータリング不変。

### D-2【frontend】App.tsx分割・OCC newtype・i18n外部化
- **App.tsx（12,675行・useState 136・useRef 48・useEffect 26）** → 自己完結クラスタをフック化。まず `useGridDivisionPreference`（localStorageのみ触る、`App.tsx:1064-1089,11321`）、次に `useCreasePairMeasurement`（計測状態＋retain/clear effect＋memo＋2ハンドラ）、`useFoldTechniqueTimelineProposal`（preview状態＋stale memo＋3ハンドラ、OCCゲート込み）。**状態/effect/依存配列/キーは丸ごと移動のみ**。
- **`coreClient.ts` OCCガードnewtype**（`expected*`三つ組が**726箇所**）：
```ts
export type ProjectOccGuard = Readonly<{ expectedProjectInstanceId:string; expectedProjectId:string; expectedRevision:number }>
export function matchesProjectOccGuard(rec:Readonly<{projectInstanceId:unknown;projectId:unknown;revision:unknown}>, g:ProjectOccGuard):boolean {
  return rec.projectInstanceId===g.expectedProjectInstanceId && rec.projectId===g.expectedProjectId && rec.revision===g.expectedRevision }
// request型は `ProjectOccGuard & Readonly<{…}>` に。検証行はこの関数へ置換。
```
安全性: **同じ`===`比較・同じ順序**。`Object.freeze`・全admission検査は不変。ガード/freezeを一切除去しない。
- **i18n外部化**（インライン`{ja,en}` **933組**、`text()`565＋`appMessage()`158呼び）→ 既存 `i18n.ts` 基盤のキー付きカタログ（同一文字列・同一 `formatMessage`）。1パネルずつ `text()` API据え置きで漸進移行。
- 重複ロジック：`advanceMeasurementPair`≡`advanceFoldPreviewMeasurementIds`（同一）を1関数化、未分割交差カウントの二重計算（`App.tsx:2494,10463`）を単一memo化。

### D-3【native】ProjectExpectation newtype・lock_and_expect・共有validator
- **`ProjectExpectation` newtype**：三つ組を**111コマンド**が運び、`ensure_expected_project` 54回・`execute_command` 126回に通す。**wire引数3つは維持しbody内で生成**（IPCペイロード不変）。
```rust
#[derive(Clone,Copy)] struct ProjectExpectation { instance_id:ProjectId, project_id:ProjectId, revision:u64 }
```
- **`lock_and_expect` ヘルパ**：`lock_project(&state)?; ensure_expected_project(&project,…)?` の前置きが**50箇所以上**。lockとverifyの間に処理が要るコマンドは従来どおり2関数を直接呼ぶ。
- **共有 `validate_project_document`**：writer(`ori-formats/src/lib.rs:575-583`)/reader(`:603-611`)の9検証列がバイト同一の手動同期（A-5非対称を生んだ脆さ）→ 同一fnを両側が呼ぶ構造に（**将来のA-5再発を構造的に封じる**）。
安全性: いずれも前置き/検証列を畳むのみ。全ガード・全検証・順序不変。

### 触らない load-bearing（明示）
`authorizes_*/observes_*` の fail-closed 権限拒否マーカー**85個**（型ごとの監査面）、4×11 `TOPOLOGY_CONTACT_POLICY_TABLE`（正規JSONとテスト照合＝仕様）、各証明フェーズで**異なる資源**を測る `*Limits` 群のフィールド集合 — verbatim維持。

---

## E. 優先度（対応順）

**実バグ（利用者影響／原因秘匿）**
1. A-1 二面角ラベル誤り（JAユーザーが補角を誤読）、A-4 applied_pose潜在panic、A-5 from_document地雷、A-2/A-3 frontend小バグ。

**精度・一貫性（fail-safeだが証明の精度を下げている）**
2. A-6 共有ヒンジ頂点数推定＋AABB食い違い、A-7 多面証明の剛性非対称（厳密分類器へ統一）、A-8 死蔵ガード。

**矛盾・正直さ（正本の是正）**
3. B-1〜B-3 progress.mdの99/100/55%→82.0%案（自動設計35%は是正済み）、B-4 種別数の基準統一、B-5 総計・集計・日付の一本化、B-6/B-7 コメント/名前の訂正。

**死蔵の整理（削除せずgate／配線）**
4. **C-1 健全MUSオラクルをEDT-009へ配線（B-3の実質解消・高価値）**、C-5 一括allow除去、C-2/C-3/C-4 gate。

**保守性（振る舞い保存の負債返済）**
5. D-1 Rustメータリング定型のマクロ化（~550行）、D-2 App.tsx分割/OCC newtype/i18n、D-3 ProjectExpectation/共有validator。

---

（本レポートは監査所見であり実装指示ではない。採否・優先順位はオーナー判断。全提案は品質・深さ・正しさを下げないことを最優先に設計した。前回同様、Codexが本doc（docs/plans/）を読んで対応する運用が観測されている。）
