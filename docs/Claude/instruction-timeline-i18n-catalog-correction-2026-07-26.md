# Claude 作業指示訂正: 折り手順 i18n catalog の hash と回帰境界

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
訂正対象: `docs/Claude/instruction-timeline-i18n-catalog-2026-07-26.md`
作業種別: frontend i18n catalog 抽出、公開 API 無変更の機械的移行
SIM-010 との干渉: 禁止
remote push: 禁止

## 1. この訂正文の優先順位

この文書は、既存指示
`docs/Claude/instruction-timeline-i18n-catalog-2026-07-26.md`
を置き換える全面改稿ではない。

ただし、次の項目についてはこの訂正文を正本とし、既存指示より優先する。

1. catalog の canonical JSON SHA-256
2. 監査時 baseline test の実数
3. forged input、hostile locale、duration、raw placeholder の必須 golden test
4. DOM の locale switch と ARIA live-region 回帰 test
5. shared worktree での外部 race 防止手順
6. 変更可能 file と commit 対象

上記以外の目的、文言 byte 不変、44 leaf、placeholder 9 leaf、
公開 API 不変、SIM-010 非干渉、remote push 禁止は既存指示を維持する。

## 2. 重大訂正: canonical JSON hash

既存指示に記載された次の canonical JSON SHA-256 は誤りである。

```text
f4fbe713cab70b1ee286cde3291136c4737572d6ab8ddd586b59eac0fedb13cd
```

この旧値を production、test、report の期待値として使用してはならない。
旧値に test を合わせるため、key 順、group 名、leaf 名、文言、locale key 順を
変更することも禁止する。

正しい canonical JSON SHA-256 は次である。

```text
b2089960622903710b5f562fc5205dc5f601f96fe342506f2a88a70b6ff4cb88
```

計算対象と計算方法は次で固定する。

```ts
createHash('sha256')
  .update(
    JSON.stringify(INSTRUCTION_TIMELINE_PRESENTATION_TEXT),
    'utf8',
  )
  .digest('hex')
```

この hash は次をすべて満たす object から計算する。

- root key 順:
  `playback`, `stopped`, `notices`, `capture`, `editor`, `duration`
- 各 group の leaf 順は既存指示の列挙順
- 各 leaf の own key 順は `ja`, `en`
- 現行 `instructionTimeline.ts` の文言を byte-for-byte 使用
- `JSON.stringify` の replacer と space は未指定
- stringify 結果の前後へ BOM、改行、空白を追加しない

一方、現行 source の `localized(ja, en)` 呼出しを source 順に
`{ ja, en }` object array として JSON 化した hash は、既存指示どおり次である。

```text
735394adb353138c9dfbb3179852bd69184f799a271f41607ee5f404dda7867a
```

この source-order pair hash は変更しない。

## 3. 監査時の現状

監査時点では次の新規 file は存在しない。

```text
apps/desktop/src/lib/instructionTimelinePresentationText.ts
apps/desktop/tests/instructionTimelinePresentationText.test.ts
```

したがって、catalog の作成と catalog 専用 test は未実装である。
既に存在するものとして上書き、削除、復元を行ってはならない。

監査時の対象 file SHA-256 は次である。

```text
apps/desktop/src/lib/instructionTimeline.ts
CE9CBC9B2ACCA92D8BD56E58C7BBEA487D5878294F78CCCAA26513F280D18AC9

apps/desktop/tests/instructionTimeline.test.ts
C5B98D51FD314E3E0786FF593151CDD8008E4CDFEAED42AABF6DEB6B46D75462

apps/desktop/tests/instructionTimelinePanel.dom.test.tsx
4FCF7729646F9205EC4CAE8FCE75FC0CC82488A699E05271B2766C00473F88E5
```

監査時 baseline の実数は次である。

```text
node --test tests/instructionTimeline.test.ts
17/17 pass

npx vitest run tests/instructionTimelinePanel.dom.test.tsx
20/20 pass
warning 1:
--localstorage-file was provided without a valid path
```

既存指示の Node `20/20` は古い実数である。
完了報告では期待値へ丸めず、各 command の実測値を記録する。
実装で test case が増えた後は、その最新実数を記録する。

## 4. 変更可能 file

production と unit catalog 作業で変更可能:

```text
apps/desktop/src/lib/instructionTimeline.ts
apps/desktop/tests/instructionTimeline.test.ts
```

新規作成:

```text
apps/desktop/src/lib/instructionTimelinePresentationText.ts
apps/desktop/tests/instructionTimelinePresentationText.test.ts
```

DOM 回帰 test の assertion 追加だけに変更可能:

```text
apps/desktop/tests/instructionTimelinePanel.dom.test.tsx
```

`InstructionTimelinePanel.tsx` の現行 production DOM はこの task では変更しない。
DOM test file の変更は、後述する既存 ARIA と locale-switch 契約の固定だけに限定する。

上記最大 5 file 以外を変更しない。
実際に変更していない file を stage しない。

## 5. shared worktree の外部 race 防止

この repository は Codex と複数 agent が同時に使用している。
次の手順を省略しない。

### 5.1 編集前

```powershell
git status --short -- `
  apps/desktop/src/lib/instructionTimeline.ts `
  apps/desktop/src/lib/instructionTimelinePresentationText.ts `
  apps/desktop/tests/instructionTimeline.test.ts `
  apps/desktop/tests/instructionTimelinePresentationText.test.ts `
  apps/desktop/tests/instructionTimelinePanel.dom.test.tsx

Get-FileHash -Algorithm SHA256 `
  apps/desktop/src/lib/instructionTimeline.ts, `
  apps/desktop/tests/instructionTimeline.test.ts, `
  apps/desktop/tests/instructionTimelinePanel.dom.test.tsx
```

- 既存 3 file の hash が第 3 節と異なる場合、先に最新内容を読み直す。
- 対象 file に自分が作成していない変更がある場合、上書きしない。
- 未実装だった 2 file が出現した場合、別担当が作業を開始している。
  その file を編集せず、exact path、status、hash、検出時刻を報告する。
- 他担当の変更を stash、checkout、reset、restore、削除してはならない。

### 5.2 編集中と検証前

- 各対象 file の hash と `git status --short -- <exact paths>` を再確認する。
- 読み込み後に外部変更を検出した file は、その場で編集を停止する。
- merge を推測で行わず、外部差分を保持して blocker として報告する。
- repository 全体の dirty state を clean にしようとしてはならない。

### 5.3 stage 前

検証完了後にも対象 file の hash と diff を再確認する。
自分の差分だけと確認できるまで stage しない。

## 6. catalog の exact contract

named export は次の 1 個である。

```text
INSTRUCTION_TIMELINE_PRESENTATION_TEXT
```

catalog file から consumer helper を export しない。

### 6.1 root と leaf 数

root group 順と leaf 数:

```text
playback   4
stopped   11
notices   18
capture    7
editor     2
duration   2
total     44
```

### 6.2 playback

```text
idle
applying
holding
complete
```

### 6.3 stopped

```text
stale_step
project_changed
revision_changed
model_changed
manual_pose
benchmark
file_operation
apply_failed
hidden
disposed
canceled
```

### 6.4 notices

```text
add_failed
added
updated
update_failed
pose_updated
pose_update_failed
delete_failed
deleted
moved
split
merged
move_failed
stale_pose
pose_apply_failed
pose_applying
model_required
no_steps
declarative_playback_unsupported
```

### 6.5 capture

```text
project_required
pose_required
pose_running
pose_invalid
pose_blocked
pose_indeterminate
pose_ready
```

### 6.6 editor

```text
invalid_metadata
update_failed
```

### 6.7 duration

```text
seconds
numberLocale
```

### 6.8 descriptor、型、freeze

- 全 leaf は own data property `ja`, `en` だけを持つ。
- `Reflect.ownKeys(leaf)` は exact に `['ja', 'en']`。
- symbol、extra key、accessor、inherited locale 値を持たせない。
- `ja`, `en` は string data property。
- 各 leaf、各 group、root を `Object.freeze`。
- root、全 6 group、全 44 leaf で `Object.isFrozen(...) === true`。
- closed key union または `satisfies` により、欠落、typo、余分な key を
  compile time と runtime test の両方で拒否する。
- catalog file に DOM、Tauri `invoke`、mutation callback、business logic、
  locale 判定 branch を入れない。

## 7. placeholder contract

placeholder を持つ leaf は次の 9 個だけである。

```text
playback.applying
  {step}
  {title}

playback.holding
  {step}
  {title}

notices.added
  {title}

notices.updated
  {title}

notices.pose_updated
  {title}

notices.deleted
  {title}

notices.pose_applying
  {title}

editor.invalid_metadata
  {titleMaximum}
  {durationMinimum}
  {durationMaximum}

duration.seconds
  {seconds}
```

- 日本語と英語で placeholder multiset を exact に一致させる。
- 重複があれば set 化で隠さず、multiset として検証する。
- その他 35 leaf は placeholder 0 個。
- placeholder 名、大小文字、出現回数を変更しない。

## 8. consumer 移行

`instructionTimeline.ts` では catalog を `TEXT` alias で import する。

```ts
import {
  INSTRUCTION_TIMELINE_PRESENTATION_TEXT as TEXT,
} from './instructionTimelinePresentationText.ts'
```

次の public API の signature と runtime behavior を変更しない。

```text
instructionPlaybackStatusText
instructionTimelineNoticeText
instructionCaptureStatusText
instructionEditorErrorText
formatInstructionDuration
```

consumer から次を削除する。

```text
type LocalizedText import
local localized helper
PLAYBACK_* inline constants
NOTICE_* inline constants
EDITOR_* inline constants
CAPTURE_STATUS_TEXT inline constant
DURATION_* inline constants
```

`selectLocalizedText` と `formatLocalizedText` は既存 helper を使用する。
duration の number locale は exact に次から選ぶ。

```ts
TEXT.duration.numberLocale
```

`playbackStopText` の 11-case switch は維持する。
forged reason を `TEXT.stopped[reason]` のように直接 index してはならない。

## 9. forged input と hostile locale の golden contract

型 cast を使った runtime test で、少なくとも次を exact に固定する。

### 9.1 forged discriminant

```text
unknown playback status
=> undefined

known stopped + unknown stop reason
=> undefined

unknown timeline notice kind
=> undefined

unknown capture status
=> TypeError

unknown editor error + default locale
=> 日本語 invalid_metadata の固定文言

unknown editor error + valid "en" locale
=> 英語 invalid_metadata の固定文言
```

unknown capture の test は単に throw することではなく、
`TypeError` であることを確認する。

unknown editor error は locale が英語でも、現行 branch の結果として
invalid metadata に dispatch される。locale 自体が有効な `en` なら
英語文言になる点と、既定または hostile locale なら日本語 fallback になる点を
混同しない。

### 9.2 hostile locale

次の 3 種を test する。

```text
"fr"
Symbol("hostile-locale")
全 trap が throw する Proxy
```

有効 locale ではないため、各 public text API は日本語へ fallback する。
Proxy の raw trap error を外へ出さない。

capture status の forged discriminant による `TypeError` と、
既知 capture status に hostile locale を渡した日本語 fallback は
別々に test する。

### 9.3 raw authored title

title は escape、translate、trim、再帰 format しない。
次のような sentinel を使い、空白、HTML-like text、literal placeholder を
そのまま一度だけ substitution することを exact に確認する。

```text
"  <b>{title}</b>  "
```

applying の現行 expected output:

```text
ja:
手順 1「  <b>{title}</b>  」を表示しています

en:
Applying step 1, “  <b>{title}</b>  ”
```

`{title}` を二回目の format 対象にしてはならない。

stopped state では forged raw reason、`stepId`、title 相当の sentinel を
文言や DOM へ反射しない。

## 10. duration の exact golden contract

現行実装の特殊値を「改善」してはならない。
少なくとも次を exact に固定する。

| input `durationMs` | ja | en |
|---:|---|---|
| `NaN` | `NaN:NaN` | `NaN:NaN` |
| `-1` | `0秒` | `0 seconds` |
| `-Infinity` | `0秒` | `0 seconds` |
| `Infinity` | `Infinity:NaN` | `Infinity:NaN` |
| `0` | `0秒` | `0 seconds` |
| `99` | `0.1秒` | `0.1 seconds` |
| `100` | `0.1秒` | `0.1 seconds` |
| `1_500` | `1.5秒` | `1.5 seconds` |
| `59_949` | `59.9秒` | `59.9 seconds` |
| `59_950` | `60秒` | `60 seconds` |
| `59_999` | `60秒` | `60 seconds` |
| `60_000` | `1:00` | `1:00` |
| `90_000` | `1:30` | `1:30` |

60 秒未満は `TEXT.duration.numberLocale` を使用する。
60 秒以上は現行 `m:ss` を維持する。

## 11. catalog 専用 test

新規 `instructionTimelinePresentationText.test.ts` では次を個別に検証する。

1. exact root key 順
2. 全 group の exact leaf key 順
3. leaf 合計 44
4. 全 leaf の exact own keys `ja`, `en`
5. own data descriptor であり accessor ではない
6. symbol、extra key、inherited locale 値がない
7. root、group、leaf の deep freeze
8. placeholder を持つ exact 9 leaf
9. その他 35 leaf の placeholder 0
10. 日英 placeholder multiset equivalence
11. source-order pair hash
12. canonical catalog JSON hash
13. catalog source に Tauri invoke がない
14. catalog source に mutation callback がない
15. catalog source に locale branch がない
16. catalog file が consumer helper を export しない

hash assertion の前に key 順、leaf 数、descriptor、placeholder を検証し、
hash だけで構造不良を隠さない。

## 12. consumer unit test

既存 `instructionTimeline.test.ts` の現状は網羅的ではない。
次を exact 日英 golden output へ拡張する。

```text
playback direct state:
idle, applying, holding, complete

stopped:
11 reason

timeline notice:
18 non-playback kind

timeline notice playback forwarding:
direct state test と別に確認

capture:
7 status

editor:
2 error

duration:
第 10 節の boundary と特殊値
```

次の現行弱い assertion を exact output に置き換えるか補強する。

- stop reason の日本語を `length > 0` だけで済ませない。
- editor invalid metadata の英語を regex だけで済ませない。
- declarative notice の英語を `/cannot be played/` だけで済ませない。
- raw title を単純な `Crane wing` だけで済ませない。
- hostile locale を unsupported string 1 個だけで済ませない。

公開 API output の wording を test 更新で変更してはならない。

## 13. DOM locale switch と ARIA

現行 production DOM の契約は次である。

```text
visible notice:
aria-hidden="true"

screen-reader notice:
aria-live="polite"
aria-atomic="true"
visually-hidden
```

既存 DOM test は同一 render のまま日本語から英語へ切り替え、
draft、selected button、callback count を保持するところまでは確認している。

同じ test または限定した新規 test で、次を直接 assertion する。

1. visible notice node が `aria-hidden="true"`。
2. screen-reader notice node が `aria-live="polite"`。
3. screen-reader notice node が `aria-atomic="true"`。
4. locale switch 前後で両 node の identity が同じ。
5. locale switch 前後で両 node の text が同じ locale に再翻訳される。
6. component を unmount/remount しない。
7. unsaved editor draft を保持する。
8. selected step の `aria-pressed` を保持する。
9. locale switch 自体が native edit、pose apply、export、
   animation export、onion-skin callback を発火しない。
10. capture guidance を使用する button の `title` 属性も live に再翻訳される。

ARIA assertion のために production component を変更しない。
既存 DOM が契約を満たさない事実を検出した場合だけ blocker として報告し、
担当外 component を自己判断で編集しない。

## 14. 必須検証

作業開始時 baseline:

```powershell
cd C:\Users\oltot\Documents\git-projects\ORIGAMI2\apps\desktop
node --test tests/instructionTimeline.test.ts
npx vitest run tests/instructionTimelinePanel.dom.test.tsx
```

実装後:

```powershell
cd C:\Users\oltot\Documents\git-projects\ORIGAMI2\apps\desktop

node --test tests/instructionTimelinePresentationText.test.ts

node --test `
  tests/instructionTimeline.test.ts `
  tests/instructionTimelinePresentationText.test.ts

npx vitest run tests/instructionTimelinePanel.dom.test.tsx
npx tsc -b --pretty false
npm run lint
npm run build
npm test
```

repository root:

```powershell
cd C:\Users\oltot\Documents\git-projects\ORIGAMI2

git diff --check

git status --short -- `
  apps/desktop/src/lib/instructionTimeline.ts `
  apps/desktop/src/lib/instructionTimelinePresentationText.ts `
  apps/desktop/tests/instructionTimeline.test.ts `
  apps/desktop/tests/instructionTimelinePresentationText.test.ts `
  apps/desktop/tests/instructionTimelinePanel.dom.test.tsx

git diff --stat -- `
  apps/desktop/src/lib/instructionTimeline.ts `
  apps/desktop/src/lib/instructionTimelinePresentationText.ts `
  apps/desktop/tests/instructionTimeline.test.ts `
  apps/desktop/tests/instructionTimelinePresentationText.test.ts `
  apps/desktop/tests/instructionTimelinePanel.dom.test.tsx

git diff -- `
  apps/desktop/src/lib/instructionTimeline.ts `
  apps/desktop/src/lib/instructionTimelinePresentationText.ts `
  apps/desktop/tests/instructionTimeline.test.ts `
  apps/desktop/tests/instructionTimelinePresentationText.test.ts `
  apps/desktop/tests/instructionTimelinePanel.dom.test.tsx
```

各 command の exit code、pass 数、fail 数、warning 数を個別に記録する。
総括の「全部 pass」だけで済ませない。

`npm test` が既存の repository-wide failure を含む場合、隠さず、
最初の失敗 test、exit code、対象 task との関係を報告する。

## 15. Git identity、commit、push

Git identity を変更しない。

```text
user.name  = yuya
user.email = oltotlo79@gmail.com
```

確認:

```powershell
git config user.name
git config user.email
```

検証と外部 race 再確認が完了するまで `git add` と `git commit` を行わない。

stage 時は実際に変更した許可 path だけを exact に指定する。

```powershell
git add -- <実際に変更した許可 path だけ>
```

禁止:

```text
git add .
git add -A
git commit -a
```

commit message は日本語で次を使用する。

```text
折り手順の状態文言を翻訳カタログへ統合する
```

remote push は行わない。

この訂正文
`docs/Claude/instruction-timeline-i18n-catalog-correction-2026-07-26.md`
自体を implementation commit に含めない。

## 16. 保護対象

次を変更、削除、stage、commit しない。

- `apps/desktop/src/components/InstructionTimelinePanel.tsx`
- SIM-010 関連 file
- stacked-fold 関連 file
- Rust file
- `docs/progress.md`
- `docs/requirements-status.md`
- `docs/plans/**`
- 既存 `docs/Codex/**`
- `origami2-*.png`
- `target-*`
- 第 4 節の許可 file 以外の全 file

完了報告だけは、既存指示で指定された次の新規 path を使用してよい。

```text
docs/Codex/claude-instruction-timeline-i18n-report-2026-07-26.md
```

report は stage、commit しない。

## 17. 完了報告

report には次を exact に含める。

1. commit hash、author、message
2. 実際に変更した exact path
3. catalog 44 leaf
4. placeholder 9 leaf とその他 35 leaf
5. source-order pair hash
   `735394adb353138c9dfbb3179852bd69184f799a271f41607ee5f404dda7867a`
6. corrected canonical JSON hash
   `b2089960622903710b5f562fc5205dc5f601f96fe342506f2a88a70b6ff4cb88`
7. 旧 canonical hash を test 期待値に使用していないこと
8. forged discriminant の実測結果
9. hostile locale 3 種の実測結果
10. raw authored title sentinel の exact output
11. duration 特殊値と boundary の exact output
12. DOM locale switch と ARIA assertion
13. 各 validation command の pass、fail、warning、exit code
14. `git status --short`
15. push を行っていないこと
16. 未実施、失敗、外部 race があれば exact blocker

検証未完了、外部 race 未解決、hash 不一致、既存 failure の未記録がある場合、
「完了」と書かない。
