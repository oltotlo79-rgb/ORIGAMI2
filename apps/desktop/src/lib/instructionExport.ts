import type { Locale } from './i18n.ts'

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
  state_unavailable: {
    ja: '折り図書き出しの状態を利用できません。アプリを再起動してください。',
    en: 'Instruction export state is unavailable. Restart the app.',
  },
  generation_unavailable: {
    ja: 'この折り図生成は利用できません。現在の編集内容から作り直してください。',
    en: 'This instruction generation is unavailable. Rebuild it from the current edits.',
  },
  generation_replaced: {
    ja: 'この折り図生成は新しい処理に置き換えられました。',
    en: 'This instruction generation was replaced by a newer operation.',
  },
  generation_cancelled: {
    ja: '折り図の生成はキャンセルされました。',
    en: 'Instruction generation was canceled.',
  },
  project_changed: {
    ja: '生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。',
    en: 'The project changed after generation started. Rebuild from the current edits.',
  },
  timeline_empty: {
    ja: '折り手順が1件もないため、折り図を書き出せません。',
    en: 'Instructions cannot be exported because the timeline has no steps.',
  },
  timeline_stale: {
    ja: '現在の展開図より古い折り手順があります。該当する姿勢を取り直してください。',
    en: 'Some instruction steps predate the current crease pattern. Recapture their poses.',
  },
  source_limit_exceeded: {
    ja: '折り図の元データが初版の処理上限を超えています。',
    en: 'The instruction source exceeds the processing limits of this release.',
  },
  topology_unsupported: {
    ja: '現在の展開図は3D折り図を生成できる面構造になっていません。',
    en: 'The current crease pattern does not have a face structure supported for 3D instructions.',
  },
  document_input_invalid: {
    ja: '折り図に含められない文字または手順情報があります。',
    en: 'Some characters or step data cannot be included in the instructions.',
  },
  document_limit_exceeded: {
    ja: '折り図のページ数またはデータ量が初版の出力上限を超えています。',
    en: 'The page count or data size exceeds the export limits of this release.',
  },
  document_generation_failed: {
    ja: '折り図データを生成できませんでした。',
    en: 'Instruction data could not be generated.',
  },
  document_contract_invalid: {
    ja: '生成された折り図が対応する出力仕様と一致しません。',
    en: 'The generated instructions do not match the supported export contract.',
  },
  warning_acknowledgement_required: {
    ja: '折り図の制約に関する確認が必要です。',
    en: 'The instruction limitations must be acknowledged.',
  },
  save_target_invalid: {
    ja: '選択された保存先を折り図の保存先として使用できません。',
    en: 'The selected destination cannot be used for instruction export.',
  },
  save_failed: {
    ja: '折り図ファイルを安全に保存できませんでした。保存先を変えて再試行してください。',
    en: 'The instruction file could not be saved safely. Choose another destination and retry.',
  },
  unexpected_failure: {
    ja: '折り図書き出しを完了できませんでした。',
    en: 'Instruction export could not be completed.',
  },
} satisfies Readonly<
  Record<InstructionExportErrorCategory, Readonly<Record<Locale, string>>>
>)

const INSTRUCTION_EXPORT_WARNING_MESSAGES = Object.freeze({
  fixed_automatic_camera: {
    ja: '固定自動カメラで生成され、現在のカメラや作家指定カメラは使用されません。',
    en: 'A fixed automatic camera is used; the current camera and author-defined cameras are not used.',
  },
  visual_effects_omitted: {
    ja: 'テクスチャ、照明、影、透明効果を省略し、単色の表裏色と白背景で描画します。',
    en: 'Textures, lighting, shadows, and transparency are omitted; pages use solid front/back colors on white.',
  },
  authored_guides_omitted: {
    ja: 'カメラ遷移、矢印、注目箇所、指先、つまみ、押さえ、手の移動、持ち替えは出力されません。',
    en: 'Camera transitions, arrows, callouts, fingers, pinches, holds, hand movements, and regrips are not exported.',
  },
  discrete_step_endpoints_only: {
    ja: '各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。',
    en: 'Each step shows only its saved endpoint pose; continuous motion between steps is not exported.',
  },
} satisfies Readonly<
  Record<InstructionExportWarning['category'], Readonly<Record<Locale, string>>>
>)

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
    message_ja: INSTRUCTION_EXPORT_ERROR_MESSAGES[category].ja,
  })
}

export function instructionExportErrorMessage(
  value: unknown,
  locale: Locale = 'ja',
) {
  let category: unknown
  try {
    if (typeof value !== 'object' || value === null) {
      return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure[locale]
    }
    category = Reflect.get(value, 'category')
  } catch {
    return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure[locale]
  }
  if (
    typeof category !== 'string'
    || !Object.prototype.hasOwnProperty.call(INSTRUCTION_EXPORT_ERROR_MESSAGES, category)
  ) {
    return INSTRUCTION_EXPORT_ERROR_MESSAGES.unexpected_failure[locale]
  }
  return INSTRUCTION_EXPORT_ERROR_MESSAGES[
    category as InstructionExportErrorCategory
  ][locale]
}

export function instructionExportFormatLabel(
  format: InstructionExportFormat,
  locale: Locale = 'ja',
) {
  switch (format) {
    case 'pdf':
      return 'PDF 1.7'
    case 'svg_zip':
      return locale === 'ja' ? 'SVG画像 ZIP' : 'SVG images ZIP'
  }
}

export function instructionExportPhaseLabel(
  phase: InstructionExportPhase,
  locale: Locale = 'ja',
) {
  switch (phase) {
    case 'validating':
      return locale === 'ja' ? '入力を検証しています' : 'Validating input'
    case 'analyzing_topology':
      return locale === 'ja' ? '面構造を解析しています' : 'Analyzing face topology'
    case 'building_document':
      return locale === 'ja'
        ? 'ページとファイルを生成しています'
        : 'Generating pages and files'
    case 'ready':
      return locale === 'ja' ? '生成が完了しました' : 'Generation complete'
  }
}

export function instructionExportWarningMessage(
  warning: unknown,
  locale: Locale = 'ja',
) {
  let category: unknown
  try {
    category = Reflect.get(Object(warning), 'category')
  } catch {
    category = null
  }
  if (
    typeof category === 'string'
    && Object.prototype.hasOwnProperty.call(
      INSTRUCTION_EXPORT_WARNING_MESSAGES,
      category,
    )
  ) {
    return INSTRUCTION_EXPORT_WARNING_MESSAGES[
      category as InstructionExportWarning['category']
    ][locale]
  }
  return locale === 'ja'
    ? '折り図の制約を識別できません。'
    : 'An instruction export limitation could not be identified.'
}

export function formatInstructionExportBytes(
  bytes: number,
  locale: Locale = 'ja',
) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) {
    return locale === 'ja' ? '不明' : 'Unknown'
  }
  const numberLocale = locale === 'ja' ? 'ja-JP' : 'en-US'
  if (bytes < 1_000) return `${bytes.toLocaleString(numberLocale)} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}
