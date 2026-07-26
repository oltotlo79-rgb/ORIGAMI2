# Claude 作業指示: 折り手順の状態文言を翻訳カタログへ統合する

作成日: 2026-07-26
対象 repository: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
作業種別: frontend i18n catalog 抽出、公開 API 無変更の機械的移行
SIM-010 との干渉: 禁止
remote push: 禁止

## 1. 目的

`apps/desktop/src/lib/instructionTimeline.ts` に散在する折り手順の表示文言を、専用の閉じた翻訳カタログへ移す。

公開 API、返却文字列、fallback、forged input の既存挙動を一切変えない。文言の翻訳改善や wording 変更はこの task の範囲外である。

SIM-010 follow-up が同時進行している。次の担当 file 以外に触れず、SIM-010、stacked-fold、Rust、App、component file と衝突しないこと。

## 2. 担当 file

変更可能:

```text
apps/desktop/src/lib/instructionTimeline.ts
apps/desktop/tests/instructionTimeline.test.ts
```

新規作成:

```text
apps/desktop/src/lib/instructionTimelinePresentationText.ts
apps/desktop/tests/instructionTimelinePresentationText.test.ts
```

上記 4 file 以外は変更しない。

## 3. 基準挙動

作業前に次を実行して baseline を記録する。

```powershell
cd apps/desktop
node --test tests/instructionTimeline.test.ts
npx vitest run tests/instructionTimelinePanel.dom.test.tsx
```

監査時 baseline:

- Node: 20/20
- DOM: 20/20

実数が変わっている場合は、最新実数を report し、既存失敗を隠さない。

## 4. 新しい catalog

`instructionTimelinePresentationText.ts` から次を named export する。

```text
INSTRUCTION_TIMELINE_PRESENTATION_TEXT
```

key 順は次で固定する。

### 4.1 playback

直接の text leaf は 4 個。

```text
idle
applying
holding
complete
```

`stopped` は 1 個の文言に潰さず、次の `stopped` group へ dispatch する。

### 4.2 stopped

11 leaf:

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

### 4.3 notices

18 leaf:

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

### 4.4 capture

7 leaf:

```text
project_required
pose_required
pose_running
pose_invalid
pose_blocked
pose_indeterminate
pose_ready
```

### 4.5 editor

2 leaf:

```text
invalid_metadata
update_failed
```

### 4.6 duration

2 leaf:

```text
seconds
numberLocale
```

合計は 44 leaf である。全 leaf は own data property `ja` と `en` だけを持つ。key の挿入順も上の順序を維持する。

## 5. 値の不変条件

既存 `instructionTimeline.ts` の日本語/英語 string を byte-for-byte 移す。句読点、空白、引用符、placeholder、改行を変更しない。

catalog 全体を source-order pair array にした既存値の SHA-256 は次。

```text
735394adb353138c9dfbb3179852bd69184f799a271f41607ee5f404dda7867a
```

完成した catalog の canonical JSON SHA-256 は次。

```text
f4fbe713cab70b1ee286cde3291136c4737572d6ab8ddd586b59eac0fedb13cd
```

hash test は object key insertion orderを前提に曖昧な stringify をするだけでなく、期待 key 順、44 leaf、各 leaf の exact own keys を先に検証する。

## 6. freeze と型

- 各 `{ja, en}` leaf を `Object.freeze`。
- 各 group を `Object.freeze`。
- root を `Object.freeze`。
- `Object.isFrozen` を root/group/leaf 全件で確認。
- closed key union または `satisfies` を使い、typo、欠落、余分な key を compile time と runtime の両方で拒否。
- catalog file から consumer 用 helper function を export しない。
- catalog file に locale branch、DOM、Tauri invoke、business logic を入れない。

## 7. placeholder contract

placeholder を持つ leaf は次の 9 個だけ。

```text
playback.applying                       {step,title}
playback.holding                        {step,title}
notices.added                           {title}
notices.updated                         {title}
notices.pose_updated                    {title}
notices.deleted                         {title}
notices.pose_applying                   {title}
editor.invalid_metadata                 {titleMaximum,durationMinimum,durationMaximum}
duration.seconds                        {seconds}
```

各 leaf で日本語と英語の placeholder multiset が完全一致すること。その他 35 leaf は placeholder 0 個であること。

## 8. consumer 移行

`instructionTimeline.ts` は catalog を import し、既存 function を同じ signature のまま維持する。

対象 public API:

```text
instructionPlaybackStatusText
instructionTimelineNoticeText
instructionCaptureStatusText
instructionEditorErrorText
formatInstructionDuration
```

移行後、consumer から次を削除する。

- `type LocalizedText` import
- local `localized` helper
- `PLAYBACK_*`、`NOTICE_*`、`EDITOR_*`、`CAPTURE_STATUS_TEXT`、`DURATION_*` の inline catalog constants

`selectLocalizedText` と `formatLocalizedText` は引き続き既存 i18n helper を使う。

duration の locale は必ず次の catalog leaf から選ぶ。

```text
TEXT.duration.numberLocale
```

## 9. 既存挙動を固定する

次の forged/runtime behavior を変更しない。

- unknown playback `status`: `undefined`
- known `stopped` + unknown stop reason: `undefined`
- unknown notice kind: `undefined`
- unknown capture status: 現行どおり `TypeError`
- unknown editor error: 現行どおり日本語の `invalid_metadata` 固定文言
- hostile locale `"fr"`: 日本語 fallback
- locale `Symbol`: 日本語 fallback
- throwing `Proxy` locale: 日本語 fallback、raw trap/error 非露出
- authored `title` は escape/translate/trim せず既存どおり raw substitution
- stop reason を title や DOM へ反射しない
- 60 秒未満の duration formatting と 60 秒以上の `m:ss` を変更しない
- `NaN`、negative、Infinity 等の duration は現行挙動を golden test で固定

`playbackStopText` の 11-case switch は維持する。forged reason を catalog へ直接 index してはならない。

## 10. test

新規 `instructionTimelinePresentationText.test.ts`:

- exact root/group key 順
- 44 leaf
-全 leaf `{ja,en}` own data propertyのみ
- symbol/extra/accessorなし
- deep freeze
- placeholder 9 leaf の exact set
- placeholder locale equivalence
- source-order pair hash
- canonical catalog JSON hash
- presentation catalog に Tauri invoke、mutation callback、locale branch がない

既存 `instructionTimeline.test.ts`:

-全 public API の日英 golden output
- 11 stop reason
- 18 notice kind（playback forward は別）
- 7 capture status
- 2 editor error
- duration boundary
- forged status/reason/kind/error
- hostile locale 3 種
- raw authored title の保持

既存 API の output snapshot を更新して値を変えることは禁止。

## 11. 必須検証

```powershell
cd apps/desktop
node --test tests/instructionTimelinePresentationText.test.ts
node --test tests/instructionTimeline.test.ts tests/instructionTimelinePresentationText.test.ts
npx vitest run tests/instructionTimelinePanel.dom.test.tsx
npx tsc -b --pretty false
npm run lint
npm run build
npm test
```

repository root:

```powershell
git diff --check
git status --short
git diff --stat
git diff -- apps/desktop/src/lib/instructionTimeline.ts apps/desktop/src/lib/instructionTimelinePresentationText.ts apps/desktop/tests/instructionTimeline.test.ts apps/desktop/tests/instructionTimelinePresentationText.test.ts
```

## 12. commit

Git identity を変更しない。

```text
yuya
oltotlo79@gmail.com
```

exact 4 path だけを stage し、次の日本語 message で 1 commit にする。

```text
折り手順の状態文言を翻訳カタログへ統合する
```

push はしない。

## 13. 保護対象

次を変更、削除、stage、commit しない。

- SIM-010 関連 file
- `docs/progress.md`
- `docs/requirements-status.md`
- `docs/plans/**`
- `docs/Codex/**`
- `origami2-*.png`
- `target-*`
- 担当 4 file 以外の全 file

## 14. 完了報告

新規 report:

```text
docs/Codex/claude-instruction-timeline-i18n-report-2026-07-26.md
```

report は stage/commit しない。次を含める。

- commit hash、author、message
- exact 4 changed paths
- 44 leaf、placeholder 9 leaf、2 hash
- public API 不変の test 対応
- 各 validation command の個別 pass 数
- warning 数
- `git status --short`
- 未実施があれば完了と書かず exact blocker
