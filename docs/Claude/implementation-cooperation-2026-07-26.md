# Claude向け実装協力指示（2026-07-26）

## 1. 目的

ORIGAMI2の現在の正本完成度は`docs/progress.md`記載の79.32%（表示79.3%）である。Codex側ではRustの衝突計測共通化、`FoldPreview`の翻訳カタログ化、監査round 3のnative期待状態検証を並行している。Claudeは競合しない次の3パッケージを順番に実装し、表示文言の分散とlocale分岐を減らすとともに、EDT-009のsoundな診断範囲を拡張すること。

1. `ProtrusionDimensionEditor`の表示文言を型付き翻訳カタログへ完全移行する。
2. `ProofScopeSummary`の表示文言と状態ラベルを型付き翻訳カタログへ完全移行する。
3. EDT-009のsoundな限定拡張として、同一対象へ異なる回転角を指定した回転対称制約を、正の半径証拠がある場合だけ直接矛盾として検出する。

これは文言の単純置換ではない。既存の入力制約、callback回数、DOM構造、ARIA名、証明状態の意味、locale切替時の状態保持を完全に維持し、静的契約とDOM回帰を追加すること。

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

同じworktreeにCodex側の未commit差分が存在し得る。自分が担当するファイル以外を整形、stage、restore、削除してはならない。特に次はユーザー所有または生成物なので、絶対にstage・commit・削除しないこと。

- `docs/plans/code-audit-2026-07-22.md`
- `docs/plans/code-audit-round3-2026-07-23.md`
- `origami2-collision-ab-verification.png`
- `origami2-global-flat-foldability-panel.png`
- `target-*`

作業開始時に担当ファイルへ既存の未commit差分があれば、上書きせず競合として報告すること。`git reset --hard`、`git checkout --`、`git clean`は禁止する。

## 3. 共通設計規約

### 3.1 翻訳カタログ

既存の`apps/desktop/src/lib/effectiveCutDiagnosticPanelText.ts`および`apps/desktop/tests/effectiveCutDiagnosticPanelText.test.ts`を規範とする。

- カタログは`Readonly<Record<... , LocalizedText>>`で閉じたkey集合を型として宣言する。
- 各`{ ja, en }`を`Object.freeze`し、最上位objectも`Object.freeze`する。
- `satisfies`でkey不足、余分なkey、型ずれをコンパイル時に検出する。
- 固定文言は`selectLocalizedText`、変数を含む文言は`formatLocalizedText`を使う。
- 日本語版と英語版のplaceholder名・個数を一致させる。
- placeholder値は`string | number`だけを渡す。`null`や`undefined`を渡さない。
- 既存文言の大文字小文字、空白、中点、句読点、単位、ARIA名を一字単位で維持する。
- enumやrecordのwire値、`option value`、callback payloadは翻訳しない。
- 新しい依存package、独自format関数、文字列置換の正規表現を追加しない。

### 3.2 コンポーネント

担当コンポーネントから次を除去する。

- 日本語文字の直接記述
- `locale === 'ja'`、`locale !== 'ja'`などの表示文言選択
- `['日本語', 'English']`のような位置依存ラベル配列
- 同じ意味の表示文言の重複定義

locale値そのものを子へ渡す既存propは維持してよい。入力検証、state更新、callback、非同期処理、DOM要素の種類と順序、class名、`data-*`、`aria-*`の意味は変更しない。

### 3.3 テスト

各カタログにNode契約テストを新設する。

- keyの完全一致と順序
- 最上位および全locale recordのfreeze
- 全keyが`ja`と`en`だけを持つこと
- placeholder集合のlocale間一致
- 代表文言の完全一致
- component sourceがカタログをimportしていること
- component sourceに日本語文字、inline `{ ja: ... }`、表示用`locale === 'ja'`が残らないこと

DOMテストでは既存のcallback意味を維持し、同一component instanceでlocaleだけを切り替えた場合に、表示とARIA名が即時に切り替わり、選択・入力・診断値・callback回数が変わらないことを追加する。

## 4. パッケージP1: ProtrusionDimensionEditor

### 4.1 担当ファイル

変更または追加してよいファイルは原則として次だけとする。

- `apps/desktop/src/components/ProtrusionDimensionEditor.tsx`
- `apps/desktop/src/lib/protrusionDimensionEditorText.ts`（新規）
- `apps/desktop/tests/protrusionDimensionEditorText.test.ts`（新規）
- `apps/desktop/tests/protrusionDimensionEditor.dom.test.tsx`
- `apps/desktop/tests/beginnerDesignProfileIntegration.test.ts`

別ファイルの変更が不可避なら、変更前に理由を報告すること。

### 4.2 実装要件

現在`ProtrusionDimensionEditor.tsx`内にある50個の`locale === 'ja'`表示分岐をすべてカタログ経由へ移す。少なくとも次の文言群を含めること。

- binding summary、symmetry名、count
- part kind、symmetry、root/tip width、length、thickness、bilateral spacing
- mount vertical/fore-aft、direction horizontal/vertical
- curvature、motion minimum/maximum、joint、side、priority
- fixed/hinge/ball、front/back/either
- remove、move up、move down
- 全入力とselectのARIA名
- `mm`、`degrees`を含む単位付き文言

`partKinds`の`value`、`target.joint`、`target.side`などdomain/wire値は変更しない。現在option本文にdomain値をそのまま表示しているpart kindは、別要件として勝手にwire値や順序を変えないこと。

既存の数値境界を厳密に維持する。

- length: 正値、tenths-mmへ丸め、最大`1_000_000`
- thickness/root width/tip width: 正値、最大`10_000` tenths-mm
- optional widthの空文字はfield削除
- position: 絶対値`10_000` mm以下
- direction: `[-1, 1]`、全成分0を拒否
- motion: 整数`[-360, 360]`かつminimum以下maximum
- priority: 整数`[1, 100]`
- symmetry変更時の`count = 1 | 2`

summaryとARIA文言には`{id}`、`{count}`等の名前付きplaceholderを使う。文字列連結でlocale別文を再構成しない。

`beginnerDesignProfileIntegration.test.ts`は現在component sourceだけを`protrusionEditor`へ読み込み、英日ラベルをsource検索している。カタログ移行後はcomponentと新カタログを連結して同じ機能契約を検査するよう更新する。assertionを削除して検査を弱めてはならない。

### 4.3 P1受入条件

- 既存DOMテストがすべて通る。
- locale propを`en -> ja -> en`とrerenderしても、controlled値とtarget identityを勝手に変更せず、`onChange`、`onRemove`、reorder callbackを呼ばない。
- component sourceに日本語文字および表示用locale二項分岐が0件。
- catalogの全placeholderが日英同一。
- 既存の英日表示文言とARIA名が完全一致。

### 4.4 P1検証コマンド

`apps/desktop`を作業ディレクトリとして実行する。

```powershell
node --test tests/protrusionDimensionEditorText.test.ts tests/beginnerDesignProfileIntegration.test.ts
npx vitest run --config vitest.config.ts tests/protrusionDimensionEditor.dom.test.tsx
npx oxlint src/components/ProtrusionDimensionEditor.tsx src/lib/protrusionDimensionEditorText.ts tests/protrusionDimensionEditorText.test.ts tests/protrusionDimensionEditor.dom.test.tsx tests/beginnerDesignProfileIntegration.test.ts
npx tsc -b
```

P1だけをstageし、`git diff --cached --check`と`git diff --cached --name-only`で対象を確認する。commitする場合の日本語messageは次とする。

```text
突起寸法編集の翻訳文言をカタログ化する
```

## 5. パッケージP2: ProofScopeSummary

P1が検証済みになってから着手する。P1が失敗した状態でP2へ進んではならない。

### 5.1 担当ファイル

変更または追加してよいファイルは原則として次だけとする。

- `apps/desktop/src/components/ProofScopeSummary.tsx`
- `apps/desktop/src/lib/proofScopeSummaryText.ts`（新規）
- `apps/desktop/tests/proofScopeSummaryText.test.ts`（新規）
- `apps/desktop/tests/proofScopeSummary.dom.test.tsx`

### 5.2 実装要件

次の全表示をカタログへ移す。

- sectionのARIA名とheading
- 全体、全体certificate、対象範囲、局所summary、局所certificate
- global status 6種
- local status 3種
- local unavailable
- necessary failed / sufficiency proven / indeterminateの件数summary
- related vertices、`Vertex {index}`、hidden vertex count
- deterministic diagnostics summary
- diagnostics JSON textareaのARIA名
- 全体証明・局所必要条件・局所十分性を混同しない説明文

`createProofScopePresentation`の呼出し、redacted diagnostics JSON、model/version表示、最大表示頂点、`aria-pressed`、`onSelectVertex`を変更しない。未知global statusは従来どおり`unavailable`表示へ閉じる。未知local statusは従来どおり`indeterminate`表示へ閉じる。

件数summary、vertex表示、hidden countは名前付きplaceholderを使用する。日本語と英語で語順が違っても、同じplaceholder集合を持たせる。

### 5.3 P2受入条件

- 既存英語・日本語DOMテストが通る。
- 同一`LocaleStore`で`setLocale('ja')`と`setLocale('en')`を行うと、再mountなしで表示、ARIA名、status名が切り替わる。
- locale切替だけでは`onSelectVertex`を呼ばず、`selectedVertexId`、diagnostics JSON、certificate model/version、status countを変えない。
- diagnostics JSONへproject ID、instance ID、fingerprint、vertex IDが混入しない既存検査を維持する。
- component sourceに日本語文字および表示用locale二項分岐が0件。

### 5.4 P2検証コマンド

`apps/desktop`を作業ディレクトリとして実行する。

```powershell
node --test tests/proofScopeSummaryText.test.ts
npx vitest run --config vitest.config.ts tests/proofScopeSummary.dom.test.tsx
npx oxlint src/components/ProofScopeSummary.tsx src/lib/proofScopeSummaryText.ts tests/proofScopeSummaryText.test.ts tests/proofScopeSummary.dom.test.tsx
npx tsc -b
```

P2だけをstageし、`git diff --cached --check`と`git diff --cached --name-only`で対象を確認する。commitする場合の日本語messageは次とする。

```text
証明範囲表示の翻訳文言をカタログ化する
```

## 6. パッケージP3: 回転対称制約のsoundな直接矛盾

P1とP2を独立commitとして完了した後に着手する。P3は表示上の推測診断ではなく、数学的に証明できる場合だけ肯定する。証明境界を満たせないcaseは既存どおり`Unknown(SolverRequiredConstraintKinds)`へ閉じること。

### 6.1 証明する定理

同じ役割順の3頂点`center/source/target`に対して、次の2制約があるとする。

```text
target - center = Rot(alpha) * (source - center)
target - center = Rot(beta)  * (source - center)
```

`0 < alpha,beta < 360`かつ`alpha != beta`なら、両式が同時に成立するのは`source == center`、したがって`target == center`の場合だけである。これは2次元回転行列について`det(Rot(alpha) - Rot(beta)) = 4 sin²((alpha - beta) / 2) > 0`となり、差の行列が正則であることによる。

そこで、unordered endpointsが`{center, source}`または`{center, target}`である実在edgeへ正の`FixedLength`が指定されている場合だけ、3制約をsoundな矛盾原因として発行する。

`FixedLength`は既存document validationで有限かつ正値に限定される。選ばれた一方の半径が正であり、回転は長さを保存するため両方の半径が正になる。したがってzero-radius解を排除できる。

### 6.2 絶対に肯定してはいけないcase

次では直接矛盾を発行しないこと。

- 回転対称2件だけで、正の半径を証明する`FixedLength`がない
- 現在のvertex座標が離れているだけ
- vertex IDが異なるだけ
- center/source/targetの役割順が異なる、またはsourceとtargetが反転している
- 角度がbit-identical
- 半径edgeではない別edgeの`FixedLength`
- 不整合な複数`FixedLength`を都合よく一件だけ採用するcase
- solverの非収束、rank、残差閾値だけを根拠にしたcase
- epsilon比較、丸めた度数、表示文字列比較だけを根拠にしたcase

ゼロ長へcollapseできるcaseを誤肯定しないことが最重要である。現在座標は制約solverで移動可能なので、現在の幾何距離を非ゼロ証拠に使ってはならない。

### 6.3 Rust実装

主担当は次とする。

- `crates/ori-core/src/constraints.rs`

必要な変更:

1. `DirectConstraintConflictKindV1`へ、意味が限定境界を正確に表す新variantを1つ追加する。推奨名は`DifferentRotationalSymmetryAnglesWithFixedRadius`。
2. variantのfieldには少なくとも`center_vertex`、`source_vertex`、`target_vertex`、`fixed_radius_edge`を含める。
3. preflightの単一canonical record走査で、同一役割順のrotationを`BTreeMap`へ収集する。
4. source patternのedge端点をcanonical unordered vertex pairで参照できるようにし、`{center,source}`と`{center,target}`の両pairを既存`fixed_lengths`のconsistentな正値assignmentへ結合する。`edge_id_lookup()`だけでは端点関係を証明できないので、必ず`set.source_pattern.edges`を1回走査する。
5. 角度が異なる2件は既存の決定的なscalar witness選択規約を再利用する。prepared validationの`0 < angle < 360`とstored scalarの`to_bits()`不一致を使い、`% 360`、epsilon、solverの`sin/cos`丸めを根拠にしない。
6. 両pairを合わせた全radius候補から`(fixed constraint ID canonical bytes, edge ID canonical bytes)`が最小の1件だけを選ぶ。source側またはtarget側を無条件に優先しない。
7. `constraint_ids`はrotation 2件とfixed length 1件の3 ID、canonical sort済み、重複なしとする。
8. `fixed_lengths[edge].consistent_assignment()`が`Some`のedgeだけを候補にする。同じbit値の重複`FixedLength`はcanonical最小IDを使い、異なる値が混在するgroupから都合のよい1件を採用しない。不整合groupは既存`DifferentFixedLengths`だけへ任せる。
9. conflict sort key、dedup、serialization、bounded direct MUSへ接続する。新variantの4 entity IDをsort keyへ含めるため、必要なら既存全armのtupleへ末尾zeroを追加し、同一triple・異なるradius edgeも全順序にする。
10. 既存のwork ceilingを緩めない。走査は準備済みrecord数とpattern edge数に対し有界で、rotation×edgeの無制限cross productを作らない。
11. conflictが成立しないrotation IDは、従来どおりunchecked IDとして残す。

実装名は上記推奨名から変更してもよいが、「一般回転対称が解けた」と読める過大な名前にしないこと。

### 6.4 Rust必須回帰

少なくとも次を追加する。

- 同一triple、異なる角度、center-source正固定長で肯定
- 同一triple、異なる角度、center-target正固定長で肯定
- rotation 2件だけではzero-radius escapeがあるため`Unknown`
- 現在座標が離れていても固定長なしでは`Unknown`
- 同一角度2件と正固定長は非肯定
- fixed lengthが無関係edgeなら非肯定
- center/source/targetのいずれかの役割が異なる場合は非肯定
- 入力record順とpattern edge順を反転してもbyte-equivalent outcome
- source側・target側を含む複数の正固定長候補から全体canonical最小witnessを選択
- 同bit値の重複固定長ではcanonical最小IDを採用
- 異なる固定長が混在する不整合groupを半径証拠へ採用しない
- witnessから3 IDのどれか1件を除くと直接矛盾を肯定しない削除最小性
- 4/8/16 recordで`find_bounded_direct_mus_v1`が同じ3 IDを返し、oracle call上限内
- 17 recordでは既存どおりMUS最小化だけ`Unknown { oracle_calls: 0 }`で、preflightの直接矛盾自体は維持
- serde JSONが新kind名と4 entity field、canonical 3 IDを固定

solver residualのfixtureも使い、固定長なしで全3頂点を同一点へcollapseすると異なるrotation角2件の残差がともに0になることを明示回帰する。ただしpreflightの肯定をsolverの数値結果へ依存させてはならない。

### 6.5 TypeScript strict boundaryとUI

Rust wireへ新kindを追加したら、次も同じcommitで更新する。

- `apps/desktop/src/lib/geometricConstraints.ts`
- `apps/desktop/src/components/GeometricConstraintPanel.tsx`
- `apps/desktop/src/lib/geometricConstraintPanelText.ts`
- `apps/desktop/tests/geometricConstraints.test.ts`
- `apps/desktop/tests/geometricConstraintsIntegration.test.ts`
- `apps/desktop/tests/geometricConstraintPanelText.test.ts`
- `apps/desktop/tests/geometricConstraintPanel.dom.test.tsx`
- `apps/desktop/tests/requirementsDesignEvidence.test.ts`
- `docs/requirements-status.md`
- `docs/progress.md`

要件:

- discriminated unionへ4 entity fieldを持つ新kindを追加する。
- parserはown data property、exact key集合、canonical UUID、未知field拒否を既存kindと同じ強度で行う。
- `center_vertex`、`source_vertex`、`target_vertex`はpairwise distinctでなければ拒否する。`fixed_radius_edge`は別entity型なので、vertex UUIDとの文字列一致だけを理由に拒否しない。
- detach/deep-freeze、canonical conflict key、duplicate/order検査へ接続する。
- field欠落、余分なfield、getter、prototype値、invalid UUID、constraint ID重複を拒否するadversarial fixtureを追加する。
- UIは専用の日英ラベルを必ず表示し、raw kind名やraw UUID全体を表示しない。
- 翻訳catalogのkey閉集合、deep freeze、placeholder一致を更新する。
- 全direct conflict kind列挙テストを18 variant（14 fixed-pattern + 4 general-graph）へ更新する。既存17 variantのassertionを削除・緩和しない。
- bounded MUS表示、unknown表示、retry、locale live switchを壊さない。

推奨表示文言:

```text
ja: 同じ回転対称対象へ異なる角度が指定され、正の固定半径と両立しません
en: Different angles target the same rotational-symmetry relation and conflict with a positive fixed radius
```

必要なら短縮IDをplaceholderで表示してよいが、日英でplaceholder集合を一致させる。

### 6.6 P3検証コマンド

repository root:

```powershell
cargo fmt --all -- --check
cargo test -p ori-core constraints
cargo test -p ori-core constraint_solver
cargo check -p ori-core --all-targets
cargo clippy -p ori-core --all-targets -- -D warnings
```

`apps/desktop`:

```powershell
node --test tests/geometricConstraints.test.ts tests/geometricConstraintsIntegration.test.ts tests/geometricConstraintPanelText.test.ts
npx vitest run --config vitest.config.ts tests/geometricConstraintPanel.dom.test.tsx
npx oxlint src/lib/geometricConstraints.ts src/components/GeometricConstraintPanel.tsx src/lib/geometricConstraintPanelText.ts tests/geometricConstraints.test.ts tests/geometricConstraintsIntegration.test.ts tests/geometricConstraintPanelText.test.ts tests/geometricConstraintPanel.dom.test.tsx
npx tsc -b
npm run build
```

P3だけを明示stageし、`git diff --cached --check`、`git diff --cached --name-only`、commit authorを確認する。日本語commit messageは次とする。

```text
回転対称制約の確定矛盾を検出する
```

P3では正本のvariant件数とそのhard assertionだけを、17（13 fixed-pattern + 4 general-graph）から18（14 fixed-pattern + 4 general-graph）へ更新する。`docs/requirements-status.md`と`docs/progress.md`の現在値、および`apps/desktop/tests/requirementsDesignEvidence.test.ts`を同じcommitで整合させる。過去時点を記録した監査原文や履歴節は書き換えない。EDT-009は部分実装、完成度79.32%（表示79.3%）、MUST 85/2/0、pending 82.0%のCI gateは一切変更しない。

## 7. 全体回帰

P1、P2、P3のfocused検証がすべて成功した後、`apps/desktop`で次を実行する。

```powershell
npm run test:snap
npm run test:dom
npm run lint
npm run build
```

既存の別作業中差分が原因で失敗した場合も、失敗を省略しない。次を報告すること。

- 実行した正確なcommand
- pass/fail件数
- 最初の失敗testまたはcompiler error
- 自分の担当差分との因果関係
- focused検証が成功しているか

失敗testをskip、削除、期待値緩和、広い正規表現への置換で通してはならない。

## 8. commit・引渡し規約

- commit messageは日本語とする。
- P1、P2、P3はそれぞれ独立commitにする。
- `git add .`、`git add -A`は禁止し、担当ファイルだけを明示stageする。
- authorは必ず`yuya <oltotlo79@gmail.com>`であることを各commit後に確認する。
- mainおよびremoteへ直接pushしない。commit SHAと検証結果をCodexへ渡し、Codex側で同時作業との差分を再検証してまとめてpushする。
- P1/P2では`docs/progress.md`、`docs/requirements-status.md`、完成度、MUST集計を変更しない。P3は上記で明示した現在のvariant件数だけを18へ整合させ、完成度79.3%、EDT-009部分実装、MUST 85/2/0を維持する。
- unrelated refactor、命名一括変更、formatterによる全file書換え、dependency更新は行わない。

## 9. 完了報告形式

次の順で、事実だけを報告すること。

1. P1 commit SHA、変更file、focused test件数
2. P2 commit SHA、変更file、focused test件数
3. P3 commit SHA、追加した定理の短い証明、肯定しないzero-radius境界、focused test件数
4. full snapshot/DOM/lint/buildとRust format/test/check/Clippyの結果
5. `git status --short`の残存差分（担当外を区別）
6. 各commitのauthor
7. 既知の未解決事項

「完了」と報告できるのは、P1/P2/P3の受入条件と全focused検証を満たした場合だけである。全体回帰が担当外差分で失敗した場合は「focused完了、全体回帰は外部要因で未確定」と正確に記載すること。
