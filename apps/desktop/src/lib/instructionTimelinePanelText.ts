import type { LocalizedText } from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const INSTRUCTION_TIMELINE_PANEL_TEXT = Object.freeze({
  defaultStepTitle: localized('手順 {step}', 'Step {step}'),
  deleteConfirmation: localized(
    '「{title}」を削除しますか？',
    'Delete “{title}”?',
  ),
  showFirstStep: localized(
    '先頭の手順を3Dに表示',
    'Show the first step in 3D',
  ),
  showFirstPhysicalStep: localized(
    '最初の実姿勢手順を3Dに表示',
    'Show the first physical-pose step in 3D',
  ),
  stopPlayback: localized('再生を停止', 'Stop playback'),
  playFromSelection: localized(
    '選択手順から再生',
    'Play from the selected step',
  ),
  heading: localized('折り手順', 'Folding instructions'),
  stepCount: localized('{count}手順', '{count} steps'),
  stepCountOne: localized('{count}手順', '{count} step'),
  totalDuration: localized('・合計 {duration}', ' · Total {duration}'),
  endpointSafety: localized(
    '保存した姿勢を段階表示します。姿勢間の連続した折り経路の安全性は保証しません。',
    'Shows saved poses step by step. It does not guarantee a safe continuous folding path between poses.',
  ),
  exportStaleTitle: localized(
    '展開図が変わったため、要更新の手順を作り直してください。',
    'The crease pattern changed. Recreate every step that needs updating.',
  ),
  exportTitle: localized(
    '現在の折り手順をPDFまたはSVG画像一式へ書き出します。',
    'Exports the current folding instructions as a PDF or a set of SVG images.',
  ),
  exportAction: localized('折り図を書き出す', 'Export diagrams'),
  invalidTimeline: localized(
    '折り手順データを安全に読み取れないため、編集と再生を停止しました。',
    'Editing and playback were stopped because the folding-step data could not be read safely.',
  ),
  timelineList: localized('折り手順一覧', 'Folding-step list'),
  needsUpdate: localized('要更新', 'Needs update'),
  descriptionOnly: localized('説明専用', 'Description only'),
  shownIn3d: localized('3D表示中', 'Shown in 3D'),
  addCurrentPose: localized(
    '＋ 現在の3D姿勢を追加',
    '＋ Add current 3D pose',
  ),
  titleLabel: localized('タイトル', 'Title'),
  descriptionLabel: localized('説明', 'Description'),
  cautionLabel: localized('注意', 'Caution'),
  durationLabel: localized('表示時間', 'Display time'),
  saveMetadata: localized('説明を保存', 'Save details'),
  captureCamera: localized('現在のカメラを取得', 'Capture current camera'),
  visualLabel: localized(
    'カメラ・矢印・注目箇所・手指ガイド（JSON）',
    'Camera, arrows, focus points, and hand guides (JSON)',
  ),
  visualHelp: localized(
    'camera、arrows、focus_pointsに加え、hand_guidesへpinch/hold/push/regripとposition/direction/labelを指定します。',
    'Set camera, arrows, focus_points, and hand_guides with pinch/hold/push/regrip plus position/direction/label.',
  ),
  showIn3d: localized('3Dに表示', 'Show in 3D'),
  updateCurrentPose: localized(
    '現在の3D姿勢で更新',
    'Update with current 3D pose',
  ),
  moveEarlier: localized('← 前へ', '← Earlier'),
  moveLater: localized('次へ →', 'Later →'),
  moveFirst: localized('先頭へ', 'Move to first'),
  moveLast: localized('末尾へ', 'Move to last'),
  duplicateAction: localized('手順を複製', 'Duplicate step'),
  deleteAction: localized('削除', 'Delete'),
  staleGuidance: localized(
    '展開図が記録時から変わりました。内容を確認し、現在の3D姿勢で更新すると再生できます。',
    'The crease pattern changed after this step was recorded. Review it and update it with the current 3D pose before playback.',
  ),
  declarativeGuidance: localized(
    '名前付き技法から追加された説明専用ステップです。3D表示・姿勢更新・自動再生・物理的な折り操作は行いません。',
    'This description-only step came from a named technique. It cannot show or update a 3D pose, play automatically, or execute a physical fold.',
  ),
  currentPose: localized(
    'この保存姿勢を3Dに表示中です。',
    'This saved pose is currently shown in 3D.',
  ),
  emptyTimeline: localized(
    '現在の3D姿勢を最初の手順として追加できます。{captureStatus}',
    'Add the current 3D pose as the first step. {captureStatus}',
  ),
  selectStep: localized(
    '手順を選択すると説明・姿勢・順番を編集できます。',
    'Select a step to edit its details, pose, and order.',
  ),
  onionLegend: localized(
    '隣接手順のオニオンスキン',
    'Adjacent-step onion skin',
  ),
  onionOff: localized('非表示', 'Off'),
  onionPrevious: localized('直前', 'Previous'),
  onionNext: localized('直後', 'Next'),
  onionHidden: localized('ghostは非表示です。', 'Ghost is hidden.'),
  onionUnavailable: localized(
    '隣接する有効な物理手順がないため表示できません。',
    'Unavailable because there is no eligible adjacent physical step.',
  ),
  onionAvailable: localized(
    '配列上で隣接する有効な物理手順だけをread-only表示します。',
    'Only the immediately adjacent eligible physical step is shown read-only.',
  ),
  onionPreparing: localized('ghostを準備しています。', 'Preparing ghost.'),
})
