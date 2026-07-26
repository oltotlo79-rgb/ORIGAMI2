# Claude 作業指示: SIM-010 viewer frontend 反射境界・再取得境界 監査追補

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
基準 branch: `main`
基準 commit: `2bfbf5ed86ba960e1ad3bdac74624fa7bfcc2e1e`
remote push: 禁止

## 1. 目的と優先順位

この文書は次の正本を置き換えず、frontend 担当範囲の監査残件を追加する。

- `docs/Claude/sim010-non-flat-layer-viewer-2026-07-26.md`
- `docs/Claude/sim010-viewer-blocker-resolution-2026-07-26.md`
- `docs/Claude/sim010-viewer-completion-followup-2026-07-26.md`

上記 3 文書を UTF-8 で最初から最後まで読み、この追補と合わせて適用すること。衝突時は、より厳しく fail-closed となり、かつ既存の公開 API と正本 wire contract を壊さない条件を採用する。

この追補の主眼は次の 4 点である。

1. object/array を 1 回の descriptor snapshot から読む。
2. parser と request builder が getter、iterator、`get` trap を実行しない。
3. source object reference を非同期 response と描画状態へ完全に結合する。
4. 原指示 §13、§14 の frontend matrix を case ID 単位で完成する。

難しい fixture、test 実行時間、既に多くの test を追加したことを理由に省略しない。既に同等以上の修正が入っている項目は二重実装せず、その挙動を厳密な regression で固定する。

## 2. baseline と共有 worktree の扱い

この追補の監査 baseline は上記 `2bfbf5e...` である。ただし、作成時点の共有 worktree には Claude による未 commit の SIM-010 変更が存在した。

観測済みの進行中変更:

- literal NUL の除去
- `source` object reference を用いる viewer effect
- stale source を render 時に隠す `ViewState.source` の導入
- tree/graph pose model ID の closed union
- lower/upper face 表示と keyboard 選択
- parser §13 matrix の拡張

2026-07-26 の現行外部差分に対する確認済み validation:

- `node --test tests/currentNonFlatLayerOrderView.test.ts`: 108/108 pass
- `npx tsc -b --pretty false`: pass
- focused oxlint: fail
  - `tests/currentNonFlatLayerOrderView.test.ts:578` の callback parameter `value` が未使用
  - `tests/currentNonFlatLayerOrderView.test.ts:583` の callback parameter `value` が未使用
  - warning は合計 2 件
  - `--max-warnings=0` を満たさない

108 件は現時点では normalizer/parser test であり、client の native invoke、request detach、response/request binding を直接実行する test は含まれていない。108/108 pass や TypeScript compile pass を理由に frontend client 完了と判断しない。未使用 parameter は意味のある fixture helper signature に直し、disable comment、名前だけの prefix、warning 許容で隠さない。

これらは他者の変更として保護する。

- `reset`、`restore`、`checkout`、rebase、amend、squash を行わない。
- dirty file を正本 commit の内容へ戻さない。
- 同じ file の hash または内容が作業中に変化した場合、上書きせず変更者と調整する。
- 現在の実装がこの追補を既に満たす場合、不要な rewrite をしない。
- test を弱めて進行中実装へ合わせない。

既存 3 commit は変更しない。

- `358eeabcd69fa9a5eff39f8cf8694ae36cbeb131`
- `ed7375938e8fb6c05623de5296be222bdf2c8cd5`
- `9135cf7acf9c0ae44251e40f0ea8a915d38a7b9f`

## 3. blocking defect A: reflection を 1 snapshot にする

### 3.1 現行反例

監査時点の `denseArray` は、概ね次の順で値を読む。

```text
Array.isArray(value)
value.length
Object.getOwnPropertyNames(value)
Object.getOwnPropertySymbols(value)
value.length
Object.getOwnPropertyDescriptor(value, index)
```

これは次の理由で不十分である。

- revoked Proxy に対する `Array.isArray` が throw し、parser 外へ例外が漏れる。
- array Proxy の `get("length")` trap を実行する。
- `length` を複数回読むため、stateful Proxy で判定中に値が変わり得る。
- `Object.getOwnPropertyNames` と `Object.getOwnPropertySymbols` が別々に `ownKeys` trap を呼び、一貫した key snapshot にならない。
- request hinge の spread、`.map`、property access も iterator/getter/`get` trap を実行する。

次の counterexample を必ず regression 化する。

```ts
let lengthReads = 0
const proxied = new Proxy(raw.faces, {
  get(target, key, receiver) {
    if (key === 'length') {
      lengthReads += 1
      throw new Error('length get trap')
    }
    return Reflect.get(target, key, receiver)
  },
})
raw.faces = proxied
```

正しい parser は `get` trap を実行しない。data descriptor が正規なら受理してよいが、少なくとも throw せず `lengthReads === 0` でなければならない。

revoked Proxy では次を満たす。

```ts
const { proxy, revoke } = Proxy.revocable([], {})
revoke()
assert.doesNotThrow(() => normalize(proxy))
assert.equal(normalize(proxy), null)
```

同じ regression を root object と nested array の両方へ設ける。

### 3.2 厳密修正条件

`exactRecord` と `denseArray` は次の原則で作る。

1. `typeof` と `null` の primitive gate を先に行う。
2. `Array.isArray` と `Object.getOwnPropertyDescriptors` を同じ `try` 内で行う。
3. `Object.getOwnPropertyDescriptors(value)` の返した descriptor map に対して `Reflect.ownKeys` を 1 回行う。
4. symbol key が 1 件でもあれば拒否する。
5. string key は code-unit 順に sort し、expected と長さ・各 index を element-wise 比較する。
6. sentinel join は使わない。
7. required property は descriptor に `value` がある own data property だけを許可する。
8. getter/setter/accessor descriptor を実行せず拒否する。
9. reflection trap が throw したら catch して `null` とする。
10. raw object の property、array index、`length`、iterator を直接読まない。

`denseArray` の `length` は descriptor map の own `length` data descriptor だけから得る。

- finite safe integer
- `0 <= length <= maximum`
- own string key set が `length` と `0..length-1` の完全一致
- hole なし
- index accessor なし
- named extra property なし
- symbol property なし

`Object.getOwnPropertyNames` と `Object.getOwnPropertySymbols` を別々に呼んで「同等」としない。`Object.getOwnPropertyDescriptors` による 1 snapshot を使う。

## 4. blocking defect B: request source を hostile input として detach する

### 4.1 stable request の厳密 validation

`getCurrentNonFlatLayerOrderViewV1` は TypeScript type assertion を runtime authority とみなさない。少なくとも次を own data descriptor から detach して検証する。

root source:

- `projectInstanceId`
- `projectId`
- `revision`
- `foldModelFingerprintSha256`
- `appliedPose`

applied pose:

- `state`
- `projectId`
- `revision`
- `fixedFaceId`
- `hingeAngles`

stable request では次を invoke 前に満たす。

- project instance ID、project ID、fixed face ID は canonical non-nil UUID
- revision は non-negative safe integer、`-0` ではない
- fingerprint は lowercase SHA-256 64 hex
- hinge count は `1..=4096`
- hinge entry は exact own fields `edgeId`、`angleDegrees`
- edge ID は canonical non-nil UUID
- angle は finite、`-0` ではなく、`0..=180`
- code-unit 順へ copy/sort
- duplicate edge ID なし

sort comparator は equal の場合に必ず `0` を返す。`left < right ? -1 : 1` のように equal を `1` とする comparator は使わない。

malformed stable request は native evidence absence に軟化しない。

- invoke は 0 回
- `CurrentNonFlatLayerOrderViewError('invalid_evidence')` として data-free に reject
- raw ID、angle、fingerprint、例外本文を error message に入れない

次の正規 gate は従来どおり invoke 0 回でよい。

- `running`
- `blocked`
- `indeterminate`
- project ID mismatch
- revision mismatch
- `fixedFaceId === null`

これらと malformed stable request を同じ test case にまとめない。

### 4.2 request の accessor/Proxy 反例

少なくとも次を test する。

- `appliedPose.hingeAngles` getter は 0 回のまま reject
- hinge array index getter は 0 回のまま reject
- hinge entry の `angleDegrees` getter は 0 回のまま reject
- hinge array の `Symbol.iterator` getter は 0 回
- hinge array Proxy の `get("length")` は 0 回
- throwing `ownKeys` trap は data-free reject
- throwing `getOwnPropertyDescriptor` trap は data-free reject
- revoked source/applied-pose/hinge-array Proxy は uncaught exception を出さない

### 4.3 response/request hinge binding

non-null response は project binding に加えて、request hinge と次を完全一致させる。

- vector length
- canonical index
- edge ID
- angle の `Object.is` 一致

次を個別 case とする。

- missing response hinge
- extra response hinge
- duplicate response hinge
- out-of-order response hinge
- edge ID mismatch
- angle 1-bit mismatch

1-bit 隣接値は丸めた decimal を手書きせず、`DataView` または `BigUint64Array` で finite `f64` の bit pattern を 1 増減して作る。

native の `null` だけを evidence absence として返す。`undefined`、malformed object、binding mismatch は `invalid_evidence` とする。

## 5. blocking defect C: tree/graph pose model ID の closed union

正本 literal は次の 2 件である。

```text
tree_absolute_hinge_angles_v1
closed_graph_absolute_hinge_angles_v1
```

source of truth:

- `crates/ori-core/src/applied_pose.rs`
- `APPLIED_POSE_MODEL_ID_V1`
- `CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1`

frontend はこの 2 件だけを pose model ID として受理する。

既存 export を不必要に破壊しない。特に既存名
`CURRENT_NON_FLAT_LAYER_ORDER_POSE_MODEL_ID_V1`
を tree literal の互換 alias として維持するか、repository 内の全 consumer を同じ commit で明示移行する。中間状態で import error を残さない。

必須 test:

- tree positive
- graph positive
- tree/graph 以外の unknown literal negative
- case difference negative
- empty string negative
- normalized output が入力と同じ正本 model ID を保持

native 側で tree/graph issuer に応じた model ID を返す修正は別担当である。frontend で native の未修正を隠すために response model ID を書き換えない。

## 6. blocking defect D: key、digest、exact byte aggregate

### 6.1 uniqueness と canonical order

個別 regression を設ける。

- duplicate/out-of-order hinge edge ID
- duplicate/out-of-order face ID
- duplicate `faceKeySha256`
- duplicate/out-of-order `cellKeySha256`

`faceKeySha256` は face ID を preimage に含むため、同一 response 内の duplicate を拒否する。

一方、`exactBoundarySha256` は face pair を preimage に含まない。同じ exact polygon が異なる face pair に現れる可能性があるため、根拠なく全 response で unique と仮定しない。形式と cell key binding は検証するが、native contract に別の一意性保証がない限り、digest が同じという理由だけで拒否しない。

### 6.2 exact magnitude 8 MiB は response 全体

frontend の 1 個の checked budget を次の全体で共有する。

- 全 face の affine rational 6 個
- 全 cell の exact boundary `u/v` rational

必須 test:

- affine だけで exact cap
- affine と cell exact point を分割して合計 exact cap
- affine と cell exact point の合計 cap + 1
- 各部分は単独では cap 以下だが、合計だけ cap + 1

巨大 fixture の計算を magic number にしない。baseline fixture が消費する canonical magnitude byte 数を helper で明示計算し、変更時に off-by-one が露呈するようにする。

## 7. literal NUL と text source の固定

対象 production source:

- `apps/desktop/src/lib/currentNonFlatLayerOrderView.ts`
- `apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx`

必須条件:

- literal `U+0000` byte が 0
- sentinel join が 0
- `"\0"` または `"\u0000"` へ置換して sentinel を残す修正は禁止
- Node test が両 source file を byte/read-string で検査し、literal NUL 0 を固定

旧 parent blob に NUL がある commit との直接 diff は Git が binary と判定し得る。したがって検証は正本 baseline の次の command でも行う。

```powershell
git diff --numstat f9913149b69ad1bc83d89681aa9309b986063cc5..HEAD -- `
  apps/desktop/src/lib/currentNonFlatLayerOrderView.ts `
  apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
```

両 path が追加/削除行数で表示されること。`--text` だけで強制表示して「Git が text と判定した」と報告しない。ただし旧 binary parent との差分原因の診断には `git diff --text` を使ってよい。

## 8. viewer source/ABA/selection の厳密条件

effect dependency は次の 2 件だけを request 境界とする。

```text
[source, reloadToken]
```

locale は dependency に含めない。

ただし passive effect が動く前の 1 frame に old geometry を描画してはならない。ready/failed/absent/loading state は、それを発行した exact `source` object reference を保持し、render 時に current prop と同一参照でなければ描画しない。

必須挙動:

- new source reference へ変わった render で old polygons が同期的に消える
- semantic 値が同じ new source object でも refetch
- hinge angle 1-bit 違いの new source でも refetch
- A1 -> B -> A2 の A1/A2 が semantic 同値でも 3 回取得
- late A1 resolve/reject は B または A2 を上書きしない
- source `null` への変更で同期的に非表示
- running/blocked/indeterminate/mismatch source で invoke 0 または current load 破棄
- unmount 後の resolve/reject で state update なし
- new response では face/cell selection を canonical first へ reset
- locale-only rerender は refetch 0、selection/geometry 維持
- locale-only rerenderで visible text と ARIA は即時切替

`requestRef` token と active cleanup の両方を維持する。source reference gate だけ、token だけ、cleanup だけのいずれか 1 個で完了扱いにしない。

## 9. DOM/UI 必須 matrix

`apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx` で case ID を test 名に入れる。

### 9.1 lifecycle

- `UI-LIFE-01` loading
- `UI-LIFE-02` native `null` absence
- `UI-LIFE-03` ready
- `UI-LIFE-04` stale_authority
- `UI-LIFE-05` invalid_evidence
- `UI-LIFE-06` resource_limit
- `UI-LIFE-07` internal_failure
- `UI-LIFE-08` unknown/raw native error は closed internal message
- `UI-LIFE-09` native `undefined` は invalid evidence

### 9.2 source と async stale

- `UI-SRC-01` same source + locale switch は invoke 1 のまま
- `UI-SRC-02` semantic 同値の new source は invoke +1
- `UI-SRC-03` hinge angle 1-bit new source は invoke +1
- `UI-SRC-04` new source rerender 直後に old SVG/polygon 0
- `UI-SRC-05` late A resolve は B を上書きしない
- `UI-SRC-06` A1 -> B -> A2 で old A1 を再利用しない
- `UI-SRC-07` source null で同期非表示
- `UI-SRC-08` unmount 後 resolve
- `UI-SRC-09` unmount 後 reject

deferred Promise を使い、resolve 順を test 自身で制御する。単に最後の DOM だけを見るのではなく、各 rerender 直後と各 resolve 後を確認する。

### 9.3 request gate

- `UI-GATE-01` running
- `UI-GATE-02` blocked
- `UI-GATE-03` indeterminate
- `UI-GATE-04` project mismatch
- `UI-GATE-05` revision mismatch
- `UI-GATE-06` null fixed face
- `UI-GATE-07` duplicate request hinge
- `UI-GATE-08` invalid request edge UUID
- `UI-GATE-09` NaN/Infinity/-Infinity
- `UI-GATE-10` `-0`
- `UI-GATE-11` angle out of range
- `UI-GATE-12` hinge cap + 1
- `UI-GATE-13` request accessor/Proxy

全 case で invoke count を明示確認する。

### 9.4 response binding

- `UI-BIND-01` fixed face mismatch
- `UI-BIND-02` missing hinge
- `UI-BIND-03` extra hinge
- `UI-BIND-04` edge ID mismatch
- `UI-BIND-05` angle 1-bit mismatch
- `UI-BIND-06` response project instance mismatch
- `UI-BIND-07` response project mismatch
- `UI-BIND-08` response revision mismatch
- `UI-BIND-09` response fingerprint mismatch

いずれも old geometry を残さず、raw value を DOM に出さない。

### 9.5 geometry、selection、accessibility

- world pane polygon は `worldOuterBoundaryXyzMm` だけで変化
- projection pane polygon は `roundedBoundaryUvMm` だけで変化
- exact numerator/denominator は DOM に出ない
- dropped X/Y/Z の 3 label
- face 選択
- cell 選択
- new response 後は canonical first selection
- lower/upper face の visible/semantic highlight
- ArrowUp/Down/Left/Right、Home、End の keyboard 選択
- keyboard 選択後に `aria-selected` と focus target が一致
- zero-cell warning は collision-free proof と主張しない
- read-only badge と no-mutation explanation
- Apply/Commit/Adopt 等の mutation control なし

lower/upper 用 class を追加するだけで CSS がなければ visual highlight ではない。既存 `App.css` に対応 style が既にあれば再利用し、なければ original allowed scope 内で最小 style を追加して DOM class と一緒に検証する。

## 10. parser test matrix の粒度

`apps/desktop/tests/currentNonFlatLayerOrderView.test.ts` では「33 fixture を 1 test」の形式を使わない。最低限次を個別 test 名または明示 case ID にする。

- root、pose、face、face projection、affine rational、cell、cell projection、work の各 level:
  - missing
  - extra
  - inherited-only required field
  - accessor
  - symbol own property
- array:
  - hole
  - index accessor
  - named enumerable extra
  - named non-enumerable extra
  - symbol extra
- Proxy:
  - revoked
  - throwing ownKeys
  - throwing getOwnPropertyDescriptor
  - `get("length")` counter 0
  - iterator getter counter 0
- primitive:
  - wrong type
  - NaN、Infinity、-Infinity、-0
  - unsafe integer
- ID/digest/generation/model:
  - invalid/nil UUID
  - uppercase/short/long digest
  - generation 0、leading zero、u64 max + 1
  - tree/graph positive、unknown negative
- rational:
  - malformed sign
  - odd/uppercase/leading-zero hex
  - zero denominator
  - zero sign/nonzero numerator
  - nonzero sign/zero numerator
  - zero denominator が `"01"` 以外
- counts/caps/order/binding:
  - 原指示 §13.1、§13.2 の全項目

getter test は counter 0 を assertion する。getter が 1 回動いた後で reject されても失敗である。

## 11. 担当可能 file

原則として次だけを変更してよい。

```text
apps/desktop/src/lib/currentNonFlatLayerOrderView.ts
apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
apps/desktop/tests/currentNonFlatLayerOrderView.test.ts
apps/desktop/tests/currentNonFlatLayerOrderViewer.dom.test.tsx
```

lower/upper highlight の既存進行中 style を完成する必要がある場合だけ:

```text
apps/desktop/src/App.css
```

次は別担当であり変更しない。

- `apps/desktop/src-tauri/**`
- `crates/**`
- `apps/desktop/src/App.tsx`
- `apps/desktop/src/components/StackedFoldPanel.tsx`
- `apps/desktop/tests/stackedFoldPanel.dom.test.tsx`
- progress/status/requirements document

担当外 defect を見つけた場合、範囲を広げず report に exact path、反例、必要修正を書く。

## 12. 禁止事項と保護 path

禁止:

- remote push
- amend、rebase、squash、reset、restore、checkout
- Git identity の変更
- raw native error、ID、座標、exact magnitude の DOM/error message への露出
- viewer への mutation callback/authority の追加
- test skip、`.only`、warning の意図的扱い
- 実行していない test の pass 報告
- existing dirty change の削除または上書き

変更・削除・stage・commit 禁止:

- `docs/Codex/**`
- `docs/plans/**`
- `docs/progress.md`
- `docs/requirements-status.md`
- `docs/requirements-evidence.v1.json`
- `origami2-*.png`
- `target-*`

この作業の report を `docs/Codex` に作る場合も untracked のままにし、stage しない。

## 13. commit 条件

既存 commit を amend しない。

外部Claudeの現在の Stage 1 commit がまだ未 commit なら、意味が同じ frontend correction として元の予定 commit に含めてよい。既に commit 済みなら、新しい commit を次の日本語 message で積む。

```text
非平坦層順ビューの反射境界回帰を完成する
```

commit 前に exact path だけを stage し、次で確認する。

```powershell
git diff --cached --name-only
git diff --cached --check
```

Git identity:

```text
user.name  = yuya
user.email = oltotlo79@gmail.com
```

## 14. 必須検証 command

repository root:

```powershell
git status --short
git diff --check
git config user.name
git config user.email
```

frontend:

```powershell
cd apps/desktop
node --test tests/currentNonFlatLayerOrderView.test.ts tests/currentNonFlatLayerOrderViewerText.test.ts tests/tauriCapabilityContract.test.ts
npx vitest run tests/currentNonFlatLayerOrderViewer.dom.test.tsx tests/stackedFoldPanel.dom.test.tsx
npx tsc -b --pretty false
npm run lint
npm run build
npm run test:snap
npm run test:dom
```

literal NUL:

```powershell
node --input-type=module -e "import fs from 'node:fs'; const paths=['src/lib/currentNonFlatLayerOrderView.ts','src/components/CurrentNonFlatLayerOrderViewer.tsx']; for (const path of paths) { const bytes=fs.readFileSync(path); if (bytes.includes(0)) throw new Error(path + ': literal NUL'); }"
```

repository root へ戻り、最後に次を行う。

```powershell
git diff --check
git diff --numstat f9913149b69ad1bc83d89681aa9309b986063cc5..HEAD -- apps/desktop/src/lib/currentNonFlatLayerOrderView.ts apps/desktop/src/components/CurrentNonFlatLayerOrderViewer.tsx
git status --short
```

各 command の pass 件数、warning 件数、未実施 command を分けて記録する。複数 command の合計を standalone test 件数として報告しない。

## 15. 完了報告

新規 report:

```text
docs/Codex/claude-sim010-viewer-frontend-audit-addendum-report-2026-07-26.md
```

stage/commit しない。次を記載する。

- baseline と作業開始時 HEAD
- commit hash、author、message
- changed path
- 本文の各 case ID と exact test 名の対応
- reflection counter が 0 である test
- request malformed と native null absence の区別
- tree/graph model ID test
- NUL byte 0
- f991 baseline の text numstat
- 各 validation command の個別 pass 件数
- warning 件数
- `git status --short`
- 未実施項目または blocker

未実施項目が 1 件でもある場合、「frontend 追補完了」と書かない。
