import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from './i18n.ts'
import {
  INSTRUCTION_EXPORT_ERROR_TEXT,
  INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT,
  INSTRUCTION_EXPORT_PHASE_TEXT,
  INSTRUCTION_EXPORT_PRESENTATION_TEXT,
  INSTRUCTION_EXPORT_WARNING_TEXT,
} from './instructionExportText.ts'

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
    message_ja: selectLocalizedText('ja', INSTRUCTION_EXPORT_ERROR_TEXT[category]),
  })
}

export function instructionExportErrorMessage(
  value: unknown,
  locale: Locale = 'ja',
) {
  let category: unknown
  try {
    if (typeof value !== 'object' || value === null) {
      return selectLocalizedText(
        locale,
        INSTRUCTION_EXPORT_ERROR_TEXT.unexpected_failure,
      )
    }
    category = Reflect.get(value, 'category')
  } catch {
    return selectLocalizedText(
      locale,
      INSTRUCTION_EXPORT_ERROR_TEXT.unexpected_failure,
    )
  }
  if (
    typeof category !== 'string'
    || !Object.prototype.hasOwnProperty.call(INSTRUCTION_EXPORT_ERROR_TEXT, category)
  ) {
    return selectLocalizedText(
      locale,
      INSTRUCTION_EXPORT_ERROR_TEXT.unexpected_failure,
    )
  }
  return selectLocalizedText(
    locale,
    INSTRUCTION_EXPORT_ERROR_TEXT[
      category as InstructionExportErrorCategory
    ],
  )
}

export function instructionExportFormatLabel(
  format: InstructionExportFormat,
  locale: Locale = 'ja',
) {
  return selectLocalizedText(locale, INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT[format])
}

export function instructionExportPhaseLabel(
  phase: InstructionExportPhase,
  locale: Locale = 'ja',
) {
  return selectLocalizedText(locale, INSTRUCTION_EXPORT_PHASE_TEXT[phase])
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
      INSTRUCTION_EXPORT_WARNING_TEXT,
      category,
    )
  ) {
    return selectLocalizedText(
      locale,
      INSTRUCTION_EXPORT_WARNING_TEXT[
        category as InstructionExportWarning['category']
      ],
    )
  }
  return selectLocalizedText(
    locale,
    INSTRUCTION_EXPORT_PRESENTATION_TEXT.unknownWarning,
  )
}

export function formatInstructionExportBytes(
  bytes: number,
  locale: Locale = 'ja',
) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) {
    return selectLocalizedText(
      locale,
      INSTRUCTION_EXPORT_PRESENTATION_TEXT.unknownBytes,
    )
  }
  const numberLocale = selectLocalizedText(
    locale,
    INSTRUCTION_EXPORT_PRESENTATION_TEXT.numberLocale,
  )
  if (bytes < 1_000) {
    return formatLocalizedText(
      locale,
      INSTRUCTION_EXPORT_PRESENTATION_TEXT.bytes,
      { value: bytes.toLocaleString(numberLocale) },
    )
  }
  if (bytes < 1_000_000) {
    return formatLocalizedText(
      locale,
      INSTRUCTION_EXPORT_PRESENTATION_TEXT.kilobytes,
      { value: (bytes / 1_000).toFixed(1) },
    )
  }
  return formatLocalizedText(
    locale,
    INSTRUCTION_EXPORT_PRESENTATION_TEXT.megabytes,
    { value: (bytes / 1_000_000).toFixed(1) },
  )
}
