export type InstructionExportFormat = 'pdf' | 'svg_zip'
export const INSTRUCTION_EXPORT_PROFILE = 'instruction_export_v1' as const
export const INSTRUCTION_EXPORT_PROJECTION_PROFILE = 'orthographic_isometric_v1' as const

export type InstructionExportErrorCategory =
  | 'state_unavailable'
  | 'generation_unavailable'
  | 'generation_replaced'
  | 'generation_cancelled'
  | 'project_changed'
  | 'timeline_empty'
  | 'timeline_stale'
  | 'source_limit_exceeded'
  | 'topology_unsupported'
  | 'document_input_invalid'
  | 'document_limit_exceeded'
  | 'document_generation_failed'
  | 'document_contract_invalid'
  | 'warning_acknowledgement_required'
  | 'save_target_invalid'
  | 'save_failed'
  | 'unexpected_failure'

export type InstructionExportCommandError = Readonly<{
  category: InstructionExportErrorCategory
  message_ja: string
}>

const INSTRUCTION_EXPORT_ERROR_MESSAGES = Object.freeze({
  state_unavailable: '折り図書き出しの状態を利用できません。アプリを再起動してください。',
  generation_unavailable: 'この折り図生成は利用できません。現在の編集内容から作り直してください。',
  generation_replaced: 'この折り図生成は新しい処理に置き換えられました。',
  generation_cancelled: '折り図の生成はキャンセルされました。',
  project_changed: '生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。',
  timeline_empty: '折り手順が1件もないため、折り図を書き出せません。',
  timeline_stale: '現在の展開図より古い折り手順があります。該当する姿勢を取り直してください。',
  source_limit_exceeded: '折り図の元データが初版の処理上限を超えています。',
  topology_unsupported: '現在の展開図は3D折り図を生成できる面構造になっていません。',
  document_input_invalid: '折り図に含められない文字または手順情報があります。',
  document_limit_exceeded: '折り図のページ数またはデータ量が初版の出力上限を超えています。',
  document_generation_failed: '折り図データを生成できませんでした。',
  document_contract_invalid: '生成された折り図が対応する出力仕様と一致しません。',
  warning_acknowledgement_required: '折り図の制約に関する確認が必要です。',
  save_target_invalid: '選択された保存先を折り図の保存先として使用できません。',
  save_failed: '折り図ファイルを安全に保存できませんでした。保存先を変えて再試行してください。',
  unexpected_failure: '折り図書き出しを完了できませんでした。',
} satisfies Readonly<Record<InstructionExportErrorCategory, string>>)

export type InstructionExportPhase =
  | 'validating'
  | 'analyzing_topology'
  | 'building_document'
  | 'ready'

export type InstructionExportBeginResponse = Readonly<{
  export_id: string
  profile: typeof INSTRUCTION_EXPORT_PROFILE
}>

export type InstructionExportProgressResponse = Readonly<{
  export_id: string
  phase: InstructionExportPhase
}>

export type InstructionExportWarning = Readonly<{
  category:
    | 'fixed_automatic_camera'
    | 'visual_effects_omitted'
    | 'authored_guides_omitted'
    | 'discrete_step_endpoints_only'
  message_ja: string
}>

export type InstructionExportPreview = Readonly<{
  export_id: string
  expected_project_id: string
  expected_revision: number
  format: InstructionExportFormat
  profile: typeof INSTRUCTION_EXPORT_PROFILE
  projection_profile: typeof INSTRUCTION_EXPORT_PROJECTION_PROFILE
  format_summary: string
  suggested_file_name: string
  byte_count: number
  step_count: number
  page_count: number
  caution_count: number
  warnings: readonly InstructionExportWarning[]
}>

export type InstructionExportPreviewResponse = Readonly<{
  preview: InstructionExportPreview
}>

export type InstructionExportSaveResponse = Readonly<{
  canceled: boolean
}>

export const INSTRUCTION_EXPORT_FORMATS:
ReadonlyArray<Readonly<{ value: InstructionExportFormat; label: string; detail: string }>> =
  Object.freeze([
    {
      value: 'pdf',
      label: 'PDF 1.7',
      detail: '固定アイソメトリック視点の折り図を、複数ページのPDFにまとめます',
    },
    {
      value: 'svg_zip',
      label: 'SVG画像 ZIP',
      detail: '手順ごとのベクターSVG画像を、1つのZIPにまとめます',
    },
  ])

export function isInstructionExportFormat(
  value: unknown,
): value is InstructionExportFormat {
  return value === 'pdf' || value === 'svg_zip'
}

export function createInstructionExportError(
  category: InstructionExportErrorCategory,
): InstructionExportCommandError {
  return Object.freeze({
    category,
    message_ja: INSTRUCTION_EXPORT_ERROR_MESSAGES[category],
  })
}

export function instructionExportErrorMessage(value: unknown) {
  let category: unknown
  try {
    if (typeof value !== 'object' || value === null) {
      return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure
    }
    category = Reflect.get(value, 'category')
  } catch {
    return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure
  }
  if (
    typeof category !== 'string'
    || !Object.prototype.hasOwnProperty.call(INSTRUCTION_EXPORT_ERROR_MESSAGES, category)
  ) {
    return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure
  }
  return INSTRUCTION_EXPORT_ERROR_MESSAGES[
    category as InstructionExportErrorCategory
  ]
}

export function instructionExportFormatLabel(format: InstructionExportFormat) {
  switch (format) {
    case 'pdf':
      return 'PDF 1.7'
    case 'svg_zip':
      return 'SVG画像 ZIP'
  }
}

export function instructionExportPhaseLabel(phase: InstructionExportPhase) {
  switch (phase) {
    case 'validating':
      return '入力を検証しています'
    case 'analyzing_topology':
      return '面構造を解析しています'
    case 'building_document':
      return 'ページとファイルを生成しています'
    case 'ready':
      return '生成が完了しました'
  }
}

export function formatInstructionExportBytes(bytes: number) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) return '不明'
  if (bytes < 1_000) return `${bytes.toLocaleString('ja-JP')} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}
