import type { LocalizedText } from './i18n.ts'

export type Fold3dFramesLauncherText = Readonly<Record<
  | 'openError'
  | 'timelineError'
  | 'selectionError'
  | 'poseError'
  | 'launcher'
  | 'title'
  | 'close'
  | 'readOnlyExplanation'
  | 'frame'
  | 'frameOption'
  | 'framePreviewAlt'
  | 'compatiblePose'
  | 'incompatiblePose'
  | 'confirmPoseReplacement'
  | 'poseHistoryExplanation'
  | 'poseApplied'
  | 'applyPose'
  | 'timelineTitle'
  | 'timelineSummary'
  | 'confirmTimeline'
  | 'applyTimeline',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const FOLD3D_FRAMES_LAUNCHER_TEXT =
  Object.freeze({
    openError: text(
      'FOLD 3Dプレビューが古いか無効です。',
      'The FOLD 3D preview became stale or invalid.',
    ),
    timelineError: text(
      'プロジェクトが変更されたか、frameが互換性のある一本道ではありません。',
      'The project changed or these frames are not one compatible linear chain.',
    ),
    selectionError: text(
      'プレビューが古くなりました。閉じて再試行してください。',
      'This preview is stale. Close and retry.',
    ),
    poseError: text(
      'プロジェクトまたは姿勢が変更されました。閉じて再試行してください。',
      'The project or pose changed. Close and retry.',
    ),
    launcher: text(
      'FOLD 3Dフレームをプレビュー',
      'Preview FOLD 3D frames',
    ),
    title: text(
      'FOLD 3Dフレームプレビュー',
      'FOLD 3D frame preview',
    ),
    close: text('閉じる', 'Close'),
    readOnlyExplanation: text(
      '読み取り専用プレビューです。プロジェクトの取込・変更は行いません。',
      'Read-only preview. This never imports or changes the project.',
    ),
    frame: text('フレーム', 'Frame'),
    frameOption: text(
      'フレーム {index}・頂点 {vertexCount}',
      'Frame {index} · {vertexCount} vertices',
    ),
    framePreviewAlt: text(
      'フレーム {index} のネイティブプレビュー',
      'Native preview of frame {index}',
    ),
    compatiblePose: text(
      '互換性のあるネイティブ木構造姿勢・ヒンジ {hingeCount}',
      'Compatible native tree pose · {hingeCount} hinges',
    ),
    incompatiblePose: text(
      '現在のネイティブモデルとは互換性がありません。',
      'Not compatible with the current native model.',
    ),
    confirmPoseReplacement: text(
      '現在の3D姿勢だけを置換します。プロジェクト形状とrevisionは変更しません。',
      'Replace only the current 3D pose. Project geometry and revision stay unchanged.',
    ),
    poseHistoryExplanation: text(
      '姿勢の適用は形状編集コマンドではありません。エディタの元に戻す／やり直すに形状履歴は追加されません。',
      'This pose adoption is not an editor geometry command. Editor Undo/Redo does not create a separate geometry-history entry.',
    ),
    poseApplied: text('姿勢を適用しました', 'Pose applied'),
    applyPose: text('現在の3D姿勢へ適用', 'Apply current 3D pose'),
    timelineTitle: text(
      '全frameを折り手順へ追加',
      'Add all frames to instructions',
    ),
    timelineSummary: text(
      '{frameCount}件の完全poseを各1.0秒で一括追加します。geometryは不変で、Undo/Redoでは1件の履歴です。',
      '{frameCount} complete poses will be appended atomically at 1.0 second each. Geometry is unchanged; Undo/Redo treats this as one history entry.',
    ),
    confirmTimeline: text(
      '認証済みの全frame poseを折り手順へ追加することを確認します。',
      'I confirm adding every authenticated frame pose to the instruction timeline.',
    ),
    applyTimeline: text(
      '全frameを一括追加',
      'Add all frames atomically',
    ),
  }) satisfies Fold3dFramesLauncherText
