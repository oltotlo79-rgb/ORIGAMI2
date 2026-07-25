import type {
  FoldTechniqueActionKindV1,
  FoldTechniqueSourceV1,
  FoldTechniqueValidationErrorV1,
} from './foldTechniqueEditor.ts'
import type { Locale } from './i18n.ts'

type FoldTechniqueEditorDialogMode = 'create' | 'edit'

export type FoldTechniqueEditorDialogText = Readonly<{
  eyebrow: Readonly<Record<FoldTechniqueEditorDialogMode, string>>
  title: string
  close: string
  description: string
  inertTitle: string
  inert: string
  invalidInitial: string
  packageTitle: string
  packageId: string
  authors: string
  author: string
  addAuthor: string
  newAuthor: string
  removeAuthor: string
  source: string
  sourceKinds: Readonly<Record<FoldTechniqueSourceV1['kind'], string>>
  citation: string
  license: string
  techniqueTitle: string
  techniquePosition: string
  techniqueSelection: string
  techniqueId: string
  techniqueVersion: string
  nameJa: string
  nameEn: string
  descriptionJa: string
  descriptionEn: string
  preserved: string
  parameters: string
  preconditions: string
  operationsTitle: string
  operationsDescription: string
  addOperation: string
  operation: string
  operationId: string
  operationNameJa: string
  operationNameEn: string
  action: string
  actionLabels: Readonly<Record<FoldTechniqueActionKindV1, string>>
  instructionJa: string
  instructionEn: string
  sinkKind: string
  openSink: string
  closedSink: string
  support: string
  declarative: string
  unsupported: string
  moveUp: string
  moveDown: string
  removeOperation: string
  invalid: string
  validation: Readonly<Record<FoldTechniqueValidationErrorV1, string>>
  saveFailed: string
  noChanges: string
  cancel: string
  saving: string
  confirm: Readonly<Record<FoldTechniqueEditorDialogMode, string>>
}>

export const FOLD_TECHNIQUE_EDITOR_DIALOG_TEXT: Readonly<
  Record<Locale, FoldTechniqueEditorDialogText>
> = Object.freeze({
  ja: Object.freeze({
    eyebrow: Object.freeze({
      create: '名前付き折り技法の作成',
      edit: '名前付き折り技法の編集',
    }),
    title: '説明テンプレートを編集',
    close: '閉じる',
    description:
      '技法名と順序付き手順を、共有可能なV1宣言データとして編集します。この画面から折り操作やプロジェクト変更は実行しません。',
    inertTitle: '安全上の重要事項',
    inert:
      '中割り、かぶせ、沈め、層を選ぶ操作は説明metadataとして保存されるだけで、自動実行されません。必要な未対応物理操作も文書内へ明示されます。',
    invalidInitial:
      '編集元の技法データが厳密なV1契約を満たしていないため、開けません。',
    packageTitle: '共有パッケージ',
    packageId: 'パッケージID',
    authors: '作成者',
    author: '作成者名',
    addAuthor: '作成者を追加',
    newAuthor: 'New author',
    removeAuthor: 'この作成者を削除',
    source: '出典区分',
    sourceKinds: Object.freeze({
      user_authored: '利用者が新規作成',
      adapted: '既存資料をもとに改作',
      published_reference: '公開資料を参照',
    }),
    citation: '出典の記述（参照されないplain text）',
    license: 'SPDXライセンスID',
    techniqueTitle: '技法',
    techniquePosition: '編集中の技法',
    techniqueSelection: '編集する技法',
    techniqueId: '技法ID',
    techniqueVersion: '技法の改訂番号',
    nameJa: '技法名（日本語）',
    nameEn: '技法名（英語）',
    descriptionJa: '説明（日本語）',
    descriptionEn: '説明（英語）',
    preserved:
      '既存のparameterとpreconditionは変更せず保持します。この初期UIでは順序付きoperationを編集します。',
    parameters: 'parameter',
    preconditions: 'precondition',
    operationsTitle: '順序付き手順',
    operationsDescription:
      '2〜256件。上下の順序は共有ファイルでもそのまま保持されます。',
    addOperation: '説明手順を追加',
    operation: '手順',
    operationId: '手順ID',
    operationNameJa: '手順名（日本語）',
    operationNameEn: '手順名（英語）',
    action: '動作区分',
    actionLabels: Object.freeze({
      instruction_cue: '文章による案内',
      straight_line_stacked_fold: '一直線の折り重ね',
      inside_reverse_fold: '中割り折り',
      outside_reverse_fold: 'かぶせ折り',
      sink_fold: '沈め折り',
      layer_selective_manipulation: '層を選ぶ操作',
    }),
    instructionJa: '案内文（日本語）',
    instructionEn: '案内文（英語）',
    sinkKind: '沈め方',
    openSink: 'オープンシンク',
    closedSink: 'クローズドシンク',
    support: '実行support',
    declarative:
      '宣言のみ。自動実行の許可や物理的な成立証明ではありません。',
    unsupported:
      '未対応物理操作として保存します。現在のsimulatorは自動実行しません。',
    moveUp: '上へ移動',
    moveDown: '下へ移動',
    removeOperation: 'この手順を削除',
    invalid: '入力内容を確認してください。',
    validation: Object.freeze({
      invalid_structure: '文書構造に認識できない値があります。',
      unsupported_schema: '対応していないschemaです。',
      unsupported_version: '対応していないfile versionです。',
      resource_limit: '件数または構造の固定上限を超えています。',
      invalid_field: 'ID、文字、locale、数値範囲のいずれかが不正です。',
      duplicate_identifier: '同じID、locale、作成者または参照が重複しています。',
      missing_reference: 'parameterまたはpreconditionへの参照が見つかりません。',
      parameter_type_mismatch: 'parameterの型、範囲または比較が一致しません。',
      inconsistent_execution_support:
        '動作、必要capability、未対応物理操作metadataが一致しません。',
      encoded_size_limit: '保存後のJSONが1 MiB上限を超えます。',
    }),
    saveFailed: '技法データを確定できませんでした。もう一度お試しください。',
    noChanges: '変更はありません。',
    cancel: 'キャンセル',
    saving: '処理中…',
    confirm: Object.freeze({
      create: '技法を作成',
      edit: '変更を確定',
    }),
  }),
  en: Object.freeze({
    eyebrow: Object.freeze({
      create: 'Create named fold technique',
      edit: 'Edit named fold technique',
    }),
    title: 'Edit the instruction template',
    close: 'Close',
    description:
      'Edit the technique name and ordered steps as shareable declarative V1 data. This dialog never performs folds or changes a project.',
    inertTitle: 'Important safety boundary',
    inert:
      'Inside reverse, outside reverse, sink, and layer-selective actions are stored only as descriptive metadata and are never executed automatically. Their unsupported physical operation is recorded explicitly.',
    invalidInitial:
      'The source technique data does not satisfy the strict V1 contract and cannot be opened.',
    packageTitle: 'Shared package',
    packageId: 'Package ID',
    authors: 'Authors',
    author: 'Author name',
    addAuthor: 'Add author',
    newAuthor: 'New author',
    removeAuthor: 'Remove this author',
    source: 'Source provenance',
    sourceKinds: Object.freeze({
      user_authored: 'User authored',
      adapted: 'Adapted from a source',
      published_reference: 'Published reference',
    }),
    citation: 'Citation text (inert plain text; never fetched)',
    license: 'SPDX license ID',
    techniqueTitle: 'Technique',
    techniquePosition: 'Technique being edited',
    techniqueSelection: 'Technique to edit',
    techniqueId: 'Technique ID',
    techniqueVersion: 'Technique revision',
    nameJa: 'Technique name (Japanese)',
    nameEn: 'Technique name (English)',
    descriptionJa: 'Description (Japanese)',
    descriptionEn: 'Description (English)',
    preserved:
      'Existing parameters and preconditions are preserved unchanged. This initial UI edits the ordered operations.',
    parameters: 'parameters',
    preconditions: 'preconditions',
    operationsTitle: 'Ordered steps',
    operationsDescription:
      '2–256 steps. Their order is preserved in the shared file.',
    addOperation: 'Add instruction step',
    operation: 'Step',
    operationId: 'Step ID',
    operationNameJa: 'Step name (Japanese)',
    operationNameEn: 'Step name (English)',
    action: 'Action kind',
    actionLabels: Object.freeze({
      instruction_cue: 'Instruction cue',
      straight_line_stacked_fold: 'Straight-line stacked fold',
      inside_reverse_fold: 'Inside reverse fold',
      outside_reverse_fold: 'Outside reverse fold',
      sink_fold: 'Sink fold',
      layer_selective_manipulation: 'Layer-selective manipulation',
    }),
    instructionJa: 'Instruction (Japanese)',
    instructionEn: 'Instruction (English)',
    sinkKind: 'Sink kind',
    openSink: 'Open sink',
    closedSink: 'Closed sink',
    support: 'Execution support',
    declarative:
      'Declarative only. This is not execution permission or proof of physical validity.',
    unsupported:
      'Stored as an unsupported physical operation. The current simulator does not execute it.',
    moveUp: 'Move up',
    moveDown: 'Move down',
    removeOperation: 'Remove this step',
    invalid: 'Review the entered values.',
    validation: Object.freeze({
      invalid_structure: 'The document contains an unrecognized structure.',
      unsupported_schema: 'The schema is not supported.',
      unsupported_version: 'The file version is not supported.',
      resource_limit: 'A fixed collection or structure limit was exceeded.',
      invalid_field: 'An ID, text, locale, or numeric range is invalid.',
      duplicate_identifier: 'An ID, locale, author, or reference is duplicated.',
      missing_reference: 'A parameter or precondition reference is missing.',
      parameter_type_mismatch:
        'A parameter type, range, or comparison does not match.',
      inconsistent_execution_support:
        'The action, required capability, and physical-support metadata disagree.',
      encoded_size_limit: 'The encoded JSON exceeds the 1 MiB limit.',
    }),
    saveFailed: 'The technique data could not be committed. Try again.',
    noChanges: 'No changes.',
    cancel: 'Cancel',
    saving: 'Processing…',
    confirm: Object.freeze({
      create: 'Create technique',
      edit: 'Apply changes',
    }),
  }),
})
