import type { LocalizedText } from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const FOLD_TECHNIQUE_TIMELINE_PREVIEW_DIALOG_TEXT = Object.freeze({
  eyebrow: localized('適用前の確認', 'Review before applying'),
  title: localized('折り手順タイムライン案', 'Instruction timeline proposal'),
  safety: localized(
    '追加する全項目は説明専用です。現在の3D姿勢を変えず、折り重ねを含む物理コマンドを実行しません。確定すると、一覧全体を1回のUndoで戻せる形で追加します。',
    'Every item is description-only. The current 3D pose is unchanged and no physical command, including stacked folding, is executed. Confirming adds the complete list as one undoable edit.',
  ),
  technique: localized('技法', 'Technique'),
  operations: localized('元の操作数', 'Source operations'),
  steps: localized('追加する説明ステップ数', 'Description steps to add'),
  unsupported: localized('未対応の物理操作数', 'Unsupported physical operations'),
  unsupportedNote: localized(
    '中割り・かぶせ・沈め折り・層選択などの未対応操作は、注意付きの説明テンプレートとしてのみ追加します。',
    'Unsupported motions such as reverse folds, sinks, and layer selection are added only as explanation templates with cautions.',
  ),
  previewList: localized('追加順', 'Append order'),
  inertStep: localized('説明専用・{kind}', 'Description only · {kind}'),
  sourceKinds: Object.freeze({
    technique: localized('技法情報', 'Technique information'),
    parameter: localized('設定値', 'Parameter'),
    precondition: localized('前提条件', 'Precondition'),
    operation: localized('操作', 'Operation'),
  }),
  stale: localized(
    'プロジェクトまたは選択中の技法が変わりました。この案を閉じて作り直してください。',
    'The project or selected technique changed. Close this proposal and rebuild it.',
  ),
  applying: localized(
    '説明ステップを原子的に追加しています…',
    'Appending the description steps atomically…',
  ),
  cancel: localized('キャンセル', 'Cancel'),
  confirm: localized(
    '説明専用手順を追加',
    'Add description-only steps',
  ),
})
