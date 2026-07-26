import type { LocalizedText } from './i18n.ts'

type FoldTechniqueTimelineProposalTextKey =
  | 'techniqueTitle'
  | 'techniqueAndProvenance'
  | 'descriptionOnlyProposal'
  | 'parameterTitle'
  | 'parameterDefinition'
  | 'preconditionTitle'
  | 'preconditionCondition'
  | 'preconditionCaution'
  | 'operationTitle'
  | 'writtenFoldingCue'
  | 'layerSelectiveInstruction'
  | 'straightLineStackedFold'
  | 'insideReverseFold'
  | 'outsideReverseFold'
  | 'openSinkFold'
  | 'closedSinkFold'
  | 'unsupportedPhysicalOperation'
  | 'stackedFoldNotExecuted'
  | 'descriptionOnlyStep'
  | 'timelineCapacityError'
  | 'proposalSizeError'
  | 'proposalBuildError'
  | 'staleProposalError'
  | 'appendFailed'
  | 'appendSucceeded'

export type FoldTechniqueTimelineProposalText = Readonly<
  Record<FoldTechniqueTimelineProposalTextKey, LocalizedText>
>

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT = Object.freeze({
  techniqueTitle: localized(
    '技法: {name}',
    'Technique: {name}',
  ),
  techniqueAndProvenance: localized(
    '技法・出典情報',
    'Technique and provenance',
  ),
  descriptionOnlyProposal: localized(
    '説明専用の案です。3D姿勢や折り操作は実行しません。',
    'This is a description-only proposal. It does not apply a 3D pose or execute a fold.',
  ),
  parameterTitle: localized(
    '設定: {name}',
    'Parameter: {name}',
  ),
  parameterDefinition: localized(
    '設定値の定義',
    'Parameter definition',
  ),
  preconditionTitle: localized(
    '前提条件: {id}',
    'Precondition: {id}',
  ),
  preconditionCondition: localized(
    '実行前に確認する条件',
    'Condition to check before folding',
  ),
  preconditionCaution: localized(
    'この条件は自動判定しません。折り手が内容を確認してください。',
    'This condition is not evaluated automatically. The folder must verify it.',
  ),
  operationTitle: localized(
    '操作 {index}: {name}',
    'Operation {index}: {name}',
  ),
  writtenFoldingCue: localized(
    '文章による折り指示',
    'Written folding cue',
  ),
  layerSelectiveInstruction: localized(
    '層を選ぶ操作の説明',
    'Layer-selective instruction',
  ),
  straightLineStackedFold: localized(
    '一直線の折り重ね',
    'Straight-line stacked fold',
  ),
  insideReverseFold: localized(
    '中割り折り',
    'Inside reverse fold',
  ),
  outsideReverseFold: localized(
    'かぶせ折り',
    'Outside reverse fold',
  ),
  openSinkFold: localized(
    '開いた沈め折り',
    'Open sink fold',
  ),
  closedSinkFold: localized(
    '閉じた沈め折り',
    'Closed sink fold',
  ),
  unsupportedPhysicalOperation: localized(
    '未対応の物理操作（{operation}）です。説明テンプレートとしてのみ追加し、自動実行しません。',
    'Unsupported physical operation ({operation}). It is added only as an explanation template and is never auto-executed.',
  ),
  stackedFoldNotExecuted: localized(
    '折り重ね物理コマンドは実行しません。層・折り線を確認してから別途操作してください。',
    'No stacked-fold physical command is executed. Verify the layers and fold line before performing it separately.',
  ),
  descriptionOnlyStep: localized(
    '説明専用ステップです。3D姿勢は変更しません。',
    'This is a description-only step. It does not change the 3D pose.',
  ),
  timelineCapacityError: localized(
    '折り手順の上限内に追加できません（必要 {required}、空き {available}）。',
    'The proposal does not fit in the instruction limit (requires {required}, {available} available).',
  ),
  proposalSizeError: localized(
    '折り技法の説明案が安全な入力サイズ上限を超えています。',
    'The fold-technique proposal exceeds the safe input-size limit.',
  ),
  proposalBuildError: localized(
    '選択中の折り技法から説明案を作成できませんでした。',
    'Could not build a proposal from the selected fold technique.',
  ),
  staleProposalError: localized(
    'プロジェクトまたは選択中の技法が変わりました。案を閉じて作り直してください。',
    'The project or selected technique changed. Close and rebuild the proposal.',
  ),
  appendFailed: localized(
    '説明ステップを追加できませんでした。プロジェクトは変更されていません。',
    'Could not append the description steps. The project was not changed.',
  ),
  appendSucceeded: localized(
    '「{technique}」から説明専用の折り手順を追加しました。1回のUndoで戻せます。',
    'Added description-only steps from “{technique}”. One Undo removes the complete addition.',
  ),
}) satisfies FoldTechniqueTimelineProposalText
