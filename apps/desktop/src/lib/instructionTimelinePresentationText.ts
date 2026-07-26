import type { LocalizedText } from './i18n.ts'

type PlaybackTextKey =
  | 'idle'
  | 'applying'
  | 'holding'
  | 'complete'

type StoppedTextKey =
  | 'stale_step'
  | 'project_changed'
  | 'revision_changed'
  | 'model_changed'
  | 'manual_pose'
  | 'benchmark'
  | 'file_operation'
  | 'apply_failed'
  | 'hidden'
  | 'disposed'
  | 'canceled'

type NoticeTextKey =
  | 'add_failed'
  | 'added'
  | 'updated'
  | 'update_failed'
  | 'pose_updated'
  | 'pose_update_failed'
  | 'delete_failed'
  | 'deleted'
  | 'moved'
  | 'split'
  | 'merged'
  | 'move_failed'
  | 'stale_pose'
  | 'pose_apply_failed'
  | 'pose_applying'
  | 'model_required'
  | 'no_steps'
  | 'declarative_playback_unsupported'

type CaptureTextKey =
  | 'project_required'
  | 'pose_required'
  | 'pose_running'
  | 'pose_invalid'
  | 'pose_blocked'
  | 'pose_indeterminate'
  | 'pose_ready'

type EditorTextKey =
  | 'invalid_metadata'
  | 'update_failed'

type DurationTextKey =
  | 'seconds'
  | 'numberLocale'

type InstructionTimelinePresentationText = Readonly<{
  playback: Readonly<Record<PlaybackTextKey, LocalizedText>>
  stopped: Readonly<Record<StoppedTextKey, LocalizedText>>
  notices: Readonly<Record<NoticeTextKey, LocalizedText>>
  capture: Readonly<Record<CaptureTextKey, LocalizedText>>
  editor: Readonly<Record<EditorTextKey, LocalizedText>>
  duration: Readonly<Record<DurationTextKey, LocalizedText>>
}>

const playback = Object.freeze({
  idle: localized('再生停止中', 'Playback stopped'),
  applying: localized(
    '手順 {step}「{title}」を表示しています',
    'Applying step {step}, “{title}”',
  ),
  holding: localized(
    '手順 {step}「{title}」を表示中です',
    'Showing step {step}, “{title}”',
  ),
  complete: localized(
    '折り手順の段階再生が完了しました',
    'Finished playing all folding steps',
  ),
}) satisfies Readonly<Record<PlaybackTextKey, LocalizedText>>

const stopped = Object.freeze({
  stale_step: localized(
    '展開図が変わった手順のため再生を停止しました',
    'Playback stopped because the crease pattern changed for this step',
  ),
  project_changed: localized(
    'プロジェクトが変わったため再生を停止しました',
    'Playback stopped because the project changed',
  ),
  revision_changed: localized(
    '編集中の内容が変わったため再生を停止しました',
    'Playback stopped because the edited content changed',
  ),
  model_changed: localized(
    '3Dモデルが変わったため再生を停止しました',
    'Playback stopped because the 3D model changed',
  ),
  manual_pose: localized(
    '3D姿勢を手動変更したため再生を停止しました',
    'Playback stopped because the 3D pose was changed manually',
  ),
  benchmark: localized(
    '性能テストを開始したため再生を停止しました',
    'Playback stopped because a performance test started',
  ),
  file_operation: localized(
    'ファイル操作を開始したため再生を停止しました',
    'Playback stopped because a file operation started',
  ),
  apply_failed: localized(
    '3D姿勢を適用できなかったため再生を停止しました',
    'Playback stopped because the 3D pose could not be applied',
  ),
  hidden: localized(
    '画面が非表示になったため再生を停止しました',
    'Playback stopped because the window became hidden',
  ),
  disposed: localized(
    '画面を閉じたため再生を停止しました',
    'Playback stopped because the view was closed',
  ),
  canceled: localized(
    '折り手順の再生を停止しました',
    'Folding-step playback stopped',
  ),
}) satisfies Readonly<Record<StoppedTextKey, LocalizedText>>

const notices = Object.freeze({
  add_failed: localized(
    '現在の3D姿勢を手順へ追加できませんでした',
    'Could not add the current 3D pose as a step',
  ),
  added: localized(
    '「{title}」を追加しました',
    'Added “{title}”',
  ),
  updated: localized(
    '「{title}」を更新しました',
    'Updated “{title}”',
  ),
  update_failed: localized(
    '手順を更新できませんでした',
    'Could not update the step',
  ),
  pose_updated: localized(
    '「{title}」の姿勢を現在の3D表示で更新しました',
    'Updated the pose for “{title}” from the current 3D view',
  ),
  pose_update_failed: localized(
    '手順の姿勢を更新できませんでした',
    'Could not update the step pose',
  ),
  delete_failed: localized(
    '手順を削除できませんでした',
    'Could not delete the step',
  ),
  deleted: localized(
    '「{title}」を削除しました',
    'Deleted “{title}”',
  ),
  moved: localized(
    '手順の順番を変更しました',
    'Changed the step order',
  ),
  split: localized(
    '手順を分割しました',
    'Split the step',
  ),
  merged: localized(
    '手順を次の手順と結合しました',
    'Merged the step with the next step',
  ),
  move_failed: localized(
    '手順を移動できませんでした',
    'Could not move the step',
  ),
  stale_pose: localized(
    '展開図が変更された手順です。「現在の3D姿勢で更新」してから表示してください',
    'The crease pattern changed for this step. Update it with the current 3D pose before showing it.',
  ),
  pose_apply_failed: localized(
    'この手順の姿勢は現在の3Dモデルへ適用できません',
    'This step pose cannot be applied to the current 3D model',
  ),
  pose_applying: localized(
    '「{title}」の保存姿勢を3Dへ適用しています',
    'Applying the saved pose for “{title}” to the 3D view',
  ),
  model_required: localized(
    '再生できる3Dモデルを準備してください',
    'Prepare a 3D model that can be played',
  ),
  no_steps: localized(
    '再生する手順がありません',
    'There are no steps to play',
  ),
  declarative_playback_unsupported: localized(
    '説明専用ステップは3D姿勢を持たないため再生できません。内容は一覧で確認してください',
    'Description-only steps have no 3D pose and cannot be played. Review them in the timeline list.',
  ),
}) satisfies Readonly<Record<NoticeTextKey, LocalizedText>>

const capture = Object.freeze({
  project_required: localized(
    'プロジェクトを読み込んでください。',
    'Open a project first.',
  ),
  pose_required: localized(
    '現在のrevisionの3D表示を準備しています。',
    'Preparing the 3D view for the current revision.',
  ),
  pose_running: localized(
    '3Dの動作が止まってから記録できます。',
    'Wait for the 3D motion to stop before recording.',
  ),
  pose_invalid: localized(
    '現在の3D姿勢は手順として安全に読み取れません。',
    'The current 3D pose cannot be read safely as a step.',
  ),
  pose_blocked: localized(
    '衝突境界で安全に停止している表示姿勢を記録します。',
    'Records the displayed pose that stopped safely at a collision boundary.',
  ),
  pose_indeterminate: localized(
    '経路判定不能で停止した現在の表示姿勢だけを記録します。',
    'Records only the current displayed pose that stopped because the path was indeterminate.',
  ),
  pose_ready: localized(
    '現在3Dに安全に表示されている姿勢を記録します。',
    'Records the pose currently shown safely in 3D.',
  ),
}) satisfies Readonly<Record<CaptureTextKey, LocalizedText>>

const editor = Object.freeze({
  invalid_metadata: localized(
    'タイトルは必須・改行なし{titleMaximum}文字以内、表示時間は{durationMinimum}〜{durationMaximum}msです。',
    'The title is required, must be one line, and must be at most {titleMaximum} characters. Display time must be {durationMinimum}–{durationMaximum} ms.',
  ),
  update_failed: localized(
    '手順の説明を更新できませんでした',
    'Could not update the step details',
  ),
}) satisfies Readonly<Record<EditorTextKey, LocalizedText>>

const duration = Object.freeze({
  seconds: localized('{seconds}秒', '{seconds} seconds'),
  numberLocale: localized('ja-JP', 'en-US'),
}) satisfies Readonly<Record<DurationTextKey, LocalizedText>>

export const INSTRUCTION_TIMELINE_PRESENTATION_TEXT = Object.freeze({
  playback,
  stopped,
  notices,
  capture,
  editor,
  duration,
}) satisfies InstructionTimelinePresentationText

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}
