# Claude実装分の申し送り（2026-07-26）

`docs/Claude/implementation-cooperation-2026-07-26.md`のP1/P2/P3を実装した。pushは行っていない。Codex側で同時作業との差分を再検証してまとめてpushすること。

## 1. P1: ProtrusionDimensionEditorの翻訳カタログ化

- commit: `1acffaedba7321a524acd476c5af70a88e8a213f`
- author: `yuya <oltotlo79@gmail.com>`
- 変更file（5件）
  - `apps/desktop/src/components/ProtrusionDimensionEditor.tsx`
  - `apps/desktop/src/lib/protrusionDimensionEditorText.ts`（新規、42 key）
  - `apps/desktop/tests/protrusionDimensionEditorText.test.ts`（新規）
  - `apps/desktop/tests/protrusionDimensionEditor.dom.test.tsx`
  - `apps/desktop/tests/beginnerDesignProfileIntegration.test.ts`
- focused検証
  - `node --test tests/protrusionDimensionEditorText.test.ts tests/beginnerDesignProfileIntegration.test.ts` → 29 pass / 0 fail
  - `npx vitest run --config vitest.config.ts tests/protrusionDimensionEditor.dom.test.tsx` → 8 pass / 0 fail
  - `npx oxlint`（5 file）→ exit 0、`npx tsc -b` → exit 0

設計上の要点。

- ARIA名は`{name} binding {id}`、`{name} binding {id} (mm)`、`{name} binding {id} (度|degrees)`の3 templateへ集約し、field名を`{name}`で差し込む。文字列連結でlocale別文を作っていない。placeholder集合は日英一致。
- `binding {id}・{symmetry}・数 {count}`と`Binding {id} · {symmetry} · count {count}`は1 keyで保持し、中点・空白・大文字小文字を一字維持した。
- 数値境界、`onChange`/`onRemove`/reorder callback、DOM要素の種類と順序、`partKinds`のwire値は未変更。
- `beginnerDesignProfileIntegration.test.ts`はcomponentと新catalogを連結して同じ機能契約を検査する形に更新した。assertionは削除していない（`'Curvature'`等のlabel検索はcatalog側で成立する）。
- DOM回帰に`en -> ja -> en`のrerenderを追加し、controlled値・target identity不変とcallback未発火を固定した。

## 2. P2: ProofScopeSummaryの翻訳カタログ化

- commit: `f5a8dfd18f521b2c2561fea3f08c5bc060cc9584`
- author: `yuya <oltotlo79@gmail.com>`
- 変更file（4件）
  - `apps/desktop/src/components/ProofScopeSummary.tsx`
  - `apps/desktop/src/lib/proofScopeSummaryText.ts`（新規、24 key）
  - `apps/desktop/tests/proofScopeSummaryText.test.ts`（新規）
  - `apps/desktop/tests/proofScopeSummary.dom.test.tsx`
- focused検証
  - `node --test tests/proofScopeSummaryText.test.ts` → 4 pass / 0 fail
  - `npx vitest run --config vitest.config.ts tests/proofScopeSummary.dom.test.tsx` → 3 pass / 0 fail
  - `npx oxlint`（4 file）→ exit 0、`npx tsc -b` → exit 0

設計上の要点。

- 位置依存label配列`['未判定', 'Not checked']`と`label[locale === 'ja' ? 0 : 1]`を廃し、status→`LocalizedText`のmapへ置換した。未知global statusは`unavailable`、未知local statusは`indeterminate`へ閉じる挙動は不変。
- global 6種とlocal 3種、`未取得`/`Unavailable`は別keyとして保持した。件数summaryの英語は`sufficiency proven`/`indeterminate`と小文字なので、status labelと共有せず専用templateにしている。
- `createProofScopePresentation`、redacted diagnostics JSON、model/version、`aria-pressed`、`onSelectVertex`は未変更。
- DOM回帰に同一`LocaleStore`での`setLocale('ja')`→`setLocale('en')`を追加し、再mountなし（同一DOM node）・diagnostics JSON同一・`onSelectVertex`未発火を固定した。

## 3. P3: 回転対称制約のsoundな直接矛盾

- commit: `4df4837e7d9035e8bb163533b7f3b473ebe7b0f3`
- author: `yuya <oltotlo79@gmail.com>`
- 変更file（8件）
  - `crates/ori-core/src/constraints.rs`
  - `apps/desktop/src/lib/geometricConstraints.ts`
  - `apps/desktop/src/lib/geometricConstraintPanelText.ts`
  - `apps/desktop/tests/geometricConstraints.test.ts`
  - `apps/desktop/tests/geometricConstraintPanelText.test.ts`
  - `apps/desktop/tests/requirementsDesignEvidence.test.ts`
  - `docs/requirements-status.md`
  - `docs/progress.md`

### 3.1 追加した定理

同一役割順の`center/source/target`に対し

```text
target - center = Rot(alpha) * (source - center)
target - center = Rot(beta)  * (source - center)
```

を課すと、`(Rot(alpha) - Rot(beta)) * (source - center) = 0`である。2次元回転行列の差は

```text
det(Rot(alpha) - Rot(beta)) = 4 sin²((alpha - beta) / 2)
```

をもち、prepared validationが保証する`0 < alpha, beta < 360`と`alpha != beta`のもとで非零になる。よって差の行列は正則で、唯一の同時解は`source == center`、回転が長さを保存するので`target == center`でもある。すなわち全役割が同一点へcollapseする解しか残らない。

そこで`{center, source}`または`{center, target}`をunordered endpointsにもつ実在edgeへ、consistentな正の`FixedLength`が指定されている場合だけ、rotation 2件とfixed length 1件の3制約を直接矛盾として発行する。半径が正なのでcollapseは排除され、他に解が無いため3制約は同時充足不能である。

### 3.2 肯定しないzero-radius境界

次はいずれも肯定しない。

- rotation 2件だけ（collapse解が残るため）。
- 現在座標が離れているだけ。座標はsolverで移動できるので非ゼロ証拠にしない。
- 役割順が異なる、`source`/`target`が反転している、centerが違う。`RotationRoleKey`は役割を並べ替えない。
- 角度がbit-identical（`ScalarGroupSummary`の`to_bits()`一致）。`% 360`、epsilon、solverの`sin/cos`は根拠にしない。
- 半径edgeでない別edgeの`FixedLength`。端点関係は`edge_id_lookup()`では証明できないため、`set.source_pattern.edges`を1回走査して`VertexPairKey`索引を作り、そこからのみ半径候補を得る。
- 同一edgeに異なる値の`FixedLength`が混在するgroup。`consistent_assignment()`が`None`を返す場合は候補にせず、`DifferentFixedLengths`へ委ねる。
- solverの非収束・rank・残差。

`collapsing_every_role_zeroes_both_rotation_residuals`で、全3頂点を同一点へdriveすると異なる角度2件の残差がともに`0.0`になることを実測固定した。preflightの肯定はこの数値に依存していない（同テスト内で、固定長が無ければ肯定しないことも併せて固定している）。

### 3.3 決定性

- witness選択は`{center,source}`と`{center,target}`両方の候補を集め、`(fixed constraint ID canonical bytes, edge ID canonical bytes)`の最小1件を採る。source側・target側を無条件に優先しない。
- 同一bit値の重複`FixedLength`はcanonical最小IDを採用する（`ScalarGroupSummary::observe`の既存規約）。
- `constraint_ids`はrotation 2件＋fixed length 1件の3 ID、canonical sort済み・重複なし。
- `conflict_sort_key`を`(u8, CanonicalId × 4)`へ拡張し、既存全armへ末尾zeroを追加した。新variantはdiscriminant 17で4 entity IDを含むため、同一triple・異なるradius edgeでも全順序になる。
- rotation走査は準備済みrecord数に対しO(N log N)、半径索引はpattern edge数に対し線形。rotation×edgeのcross productは作っていない。既存work ceilingは緩めていない。
- conflictが成立しないrotation IDは従来どおりunchecked IDとして残す。

### 3.4 TypeScript境界

- discriminated unionへ4 entity fieldの新kindを追加し、parserは`hasExactKeys`・canonical UUID・3 vertexのpairwise distinctを既存kindと同じ強度で検査する。`fixed_radius_edge`はentity型が違うのでvertex UUIDとの一致だけを理由に拒否していない。
- canonical conflict keyへ4 entityを追加した。`witnessSize`は3。
- 敵対的fixtureを追加した。field欠落、余分field、非canonical UUID、非UUID、3通りのpairwise一致、constraint ID重複、enumerable getter、prototype上の値。
- UIは`directConflictLabels`の専用日英labelを表示する。`GeometricConstraintPanel.tsx`は既存の`default`分岐でこのkindを扱うため変更不要だった（raw kind名・raw UUIDは表示しない）。placeholderを持たないためlocale間のplaceholder集合は空で一致する。
- catalogのSHA-256固定値を`c3a75cc73f71addce89291d73ab3bc115df31495cf6489aef0097cd0f4b8e60d`へ更新した。

### 3.5 件数整合

正本のvariant件数だけを17（13 pairwise + 4 general-graph）から18（14 pairwise + 4 general-graph）へ更新した。

- `docs/requirements-status.md`の2箇所と、EDT-009行へ新variantの証明境界の説明を追記。
- `docs/progress.md`の「現行の数え方は…へ統一する」1文の数値のみ。履歴節の記述は書き換えていない。
- `apps/desktop/tests/requirementsDesignEvidence.test.ts`のenum件数と文言assertion。
- `apps/desktop/tests/geometricConstraints.test.ts`のnormalize件数（17→18、test名も`eighteen`へ）。

完成度79.32%（表示79.3%）、EDT-009部分実装、MUST 85/2/0、pending 82.0%のCI gateは変更していない。

### 3.6 P3 focused検証

repository root（`CARGO_TARGET_DIR`未設定＝共有`target/`）:

- `cargo fmt --all -- --check` → exit 0
- `cargo test -p ori-core constraints` → 70 pass / 0 fail
- `cargo test -p ori-core constraint_solver` → 11 pass / 0 fail
- `cargo check -p ori-core --all-targets` → exit 0
- `cargo clippy -p ori-core --all-targets -- -D warnings` → exit 0

`apps/desktop`:

- `node --test tests/geometricConstraints.test.ts tests/geometricConstraintsIntegration.test.ts tests/geometricConstraintPanelText.test.ts tests/requirementsDesignEvidence.test.ts` → 29 pass / 0 fail
- `npx vitest run --config vitest.config.ts tests/geometricConstraintPanel.dom.test.tsx` → 13 pass / 0 fail
- `npx oxlint`（7 file）→ exit 0、`npx tsc -b` → exit 0、`npm run build` → exit 0

## 4. 全体回帰（`apps/desktop`）

- `npm run test:snap` → tests 1815 / pass 1815 / fail 0
- `npm run test:dom` → Test Files 60 passed、Tests 416 passed
- `npm run lint` → exit 0。警告は既存のもので、`App.tsx`、`coreClient.ts`、`EffectiveCutDiagnosticPanel.tsx`のreact-hooks/optional-chaining。担当差分由来ではない。
- `npm run build` → exit 0

## 5. 残存差分（`git status --short`）

私の担当外。stage・commit・restoreしていない。

- `crates/ori-core/src/constraints.rs`（M）: **P3 commit後にCodex側が加えた追補**。`conflict_sort_key`のdoc文言調整と、隣接binary64角度＋`f64::MIN_POSITIVE`半径で肯定する回帰`adjacent_binary64_rotation_angles_remain_distinct_proof_values`の追加。内容は定理と整合しており、こちらからは変更していない。
- `apps/desktop/src/components/FoldImportDialog.tsx`、`SvgImportDialog.tsx`、`src/lib/foldImportDialogText.ts`、`svgImportDialogText.ts`、および対応する5 testfile（M）: Codex側の取込dialog翻訳カタログ化。
- `docs/code-audit-round3-response-2026-07-26.md`、`docs/plans/code-audit-*.md`、`origami2-*.png`、`target-*`（??）: ユーザー所有または生成物。

## 6. 既知の未解決事項

- **cargo実行時のSmart App Control**。新規`CARGO_TARGET_DIR`（例`target-claude-p3`）では`proc-macro2`/`serde_json`のbuild scriptが`os error 4551`でブロックされ、再試行でも解除されなかった。既存の暖まったtarget dirか共有`target/`なら実行できる。新しく作成したtest binaryも一度ブロックされたが、共有`target/`では実行できた。隔離target dirでの検証を前提にする手順は、この環境では成立しないことがある。
- P3の`GeometricConstraintPanel.tsx`は変更不要だったため、指示書§6.5の対象fileのうち同fileと`geometricConstraintsIntegration.test.ts`、`geometricConstraintPanel.dom.test.tsx`は無変更で通過している。追加のUI表現が必要ならCodex側で判断のこと。
- `origin/main`より8 commit先行している（Claude 3件を含む）。push未実施。
