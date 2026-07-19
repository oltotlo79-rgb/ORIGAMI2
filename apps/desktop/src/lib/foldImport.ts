import type { Locale } from './i18n.ts'

export const FOLD_ASSIGNMENT_CODES = ['M', 'V', 'F', 'U', 'C', 'J'] as const

export type FoldAssignmentCode = (typeof FOLD_ASSIGNMENT_CODES)[number]
export type FoldImportTarget = 'mountain' | 'valley' | 'auxiliary' | 'cut' | 'ignore'
export type FoldImportMapping = Partial<Record<FoldAssignmentCode, FoldImportTarget>>

export type FoldImportAssignmentSummary = Readonly<{
  assignment: FoldAssignmentCode | 'B'
  count: number
}>

export type FoldImportPreviewVertex = Readonly<{
  x: number
  y: number
}>

export type FoldImportPreviewEdge = Readonly<{
  start: number
  end: number
  assignment: FoldAssignmentCode | 'B'
}>

export type FoldImportPreview = Readonly<{
  import_id: string
  file_name: string
  suggested_name: string
  file_spec: string | null
  frame_unit: string | null
  default_mm_per_unit: number | null
  vertex_count: number
  edge_count: number
  boundary_edge_count: number
  assignments: readonly FoldImportAssignmentSummary[]
  preview_vertices: readonly FoldImportPreviewVertex[]
  preview_edges: readonly FoldImportPreviewEdge[]
  preview_truncated: boolean
  warnings: readonly string[]
}>

export type FoldImportSettings = Readonly<{
  importId: string
  name: string
  mmPerUnit: number
  mappings: FoldImportMapping
}>

export const FOLD_IMPORT_TARGET_OPTIONS: ReadonlyArray<Readonly<{
  value: FoldImportTarget
  label: string
}>> = [
  { value: 'mountain', label: '山折り' },
  { value: 'valley', label: '谷折り' },
  { value: 'auxiliary', label: '補助線' },
  { value: 'cut', label: '切断線' },
  { value: 'ignore', label: '取り込まない' },
]

const TARGETS_BY_ASSIGNMENT: Readonly<Record<
  FoldAssignmentCode,
  readonly FoldImportTarget[]
>> = {
  M: ['mountain'],
  V: ['valley'],
  F: ['auxiliary', 'ignore'],
  U: ['mountain', 'valley', 'auxiliary', 'ignore'],
  C: ['cut', 'ignore'],
  J: ['auxiliary', 'ignore'],
}

const DIRECT_DEFAULTS: Readonly<Partial<Record<FoldAssignmentCode, FoldImportTarget>>> = {
  M: 'mountain',
  V: 'valley',
  C: 'cut',
}

const ASSIGNMENT_LABELS: Readonly<Record<FoldAssignmentCode | 'B', string>> = {
  B: 'B · 用紙境界',
  M: 'M · 山折り',
  V: 'V · 谷折り',
  F: 'F · 平らな折り筋',
  U: 'U · 未割当',
  C: 'C · 切断・スリット',
  J: 'J · 面の結合',
}

const ENGLISH_ASSIGNMENT_LABELS:
Readonly<Record<FoldAssignmentCode | 'B', string>> = {
  B: 'B · Paper boundary',
  M: 'M · Mountain fold',
  V: 'V · Valley fold',
  F: 'F · Flat crease',
  U: 'U · Unassigned',
  C: 'C · Cut or slit',
  J: 'J · Face join',
}

const ENGLISH_TARGET_LABELS: Readonly<Record<FoldImportTarget, string>> = {
  mountain: 'Mountain fold',
  valley: 'Valley fold',
  auxiliary: 'Auxiliary line',
  cut: 'Cut line',
  ignore: 'Do not import',
}

export function foldAssignmentLabel(
  assignment: FoldAssignmentCode | 'B',
  locale: Locale = 'ja',
) {
  return locale === 'ja'
    ? ASSIGNMENT_LABELS[assignment]
    : ENGLISH_ASSIGNMENT_LABELS[assignment]
}

export function foldImportTargetLabel(
  target: FoldImportTarget,
  locale: Locale = 'ja',
) {
  if (locale === 'en') return ENGLISH_TARGET_LABELS[target]
  return FOLD_IMPORT_TARGET_OPTIONS.find((option) => option.value === target)
    ?.label ?? target
}

export function foldImportWarningMessage(
  warning: unknown,
  locale: Locale = 'ja',
) {
  const category = classifyFoldImportWarning(warning)
  if (locale === 'ja') {
    return category === null
      ? '取り込まれないFOLD情報があります。'
      : warning as string
  }
  switch (category) {
    case 'missing_spec':
      return 'The FOLD specification version is missing, so the file will be interpreted conservatively within the supported range.'
    case 'unit_needs_scale':
      return 'The file has no unit information that can be converted to physical size. Enter the millimetres per FOLD unit.'
    case 'ignored_metadata':
      return 'Some FOLD metadata will not be imported.'
    case 'invalid_title':
      return 'The title in the FOLD file does not meet the work-name requirements, so the default name will be used.'
    case 'flat_crease':
      return 'F (flat crease) has no equivalent line type and must be converted to an auxiliary line or excluded.'
    case 'unassigned':
      return 'U (unassigned) must be mapped to a mountain fold, valley fold, auxiliary line, or exclusion.'
    case 'face_join':
      return 'J (face join) has no equivalent line type and must be converted to an auxiliary line or excluded.'
    default:
      return 'Some FOLD information will not be imported.'
  }
}

export function foldImportPreviewFileName(
  nativeLabel: unknown,
  locale: Locale = 'ja',
) {
  if (
    typeof nativeLabel === 'string'
    && nativeLabel !== '選択したFOLDファイル'
    && nativeLabel !== 'Selected FOLD file'
    && isSafeFoldImportFileName(nativeLabel)
  ) {
    return nativeLabel
  }
  return locale === 'en' ? 'Selected FOLD file' : '選択したFOLDファイル'
}

export function isFoldImportFallbackName(value: unknown): value is string {
  return value === 'FOLDインポート' || value === 'FOLD import'
}

export function foldImportSuggestedName(
  value: string,
  locale: Locale = 'ja',
) {
  if (!isFoldImportFallbackName(value)) return value
  return locale === 'en' ? 'FOLD import' : 'FOLDインポート'
}

export function foldImportTargetOptions(assignment: FoldAssignmentCode) {
  const allowed = new Set(TARGETS_BY_ASSIGNMENT[assignment])
  return FOLD_IMPORT_TARGET_OPTIONS.filter(({ value }) => allowed.has(value))
}

type FoldImportWarningCategory =
  | 'missing_spec'
  | 'unit_needs_scale'
  | 'ignored_metadata'
  | 'invalid_title'
  | 'flat_crease'
  | 'unassigned'
  | 'face_join'

const FOLD_IGNORED_METADATA_LABELS = new Set([
  '複数フレーム',
  '作成ソフト情報',
  '作者情報',
  '説明',
  'ファイル分類',
  'フレーム分類',
  'フレーム属性',
  'フレーム名',
  'フレーム継承',
  '面情報（辺から再計算）',
  '重なり順',
  '折り角度',
  '辺長メタデータ',
  'フレーム変換',
])

function classifyFoldImportWarning(
  warning: unknown,
): FoldImportWarningCategory | null {
  if (typeof warning !== 'string') return null
  switch (warning) {
    case 'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。':
      return 'missing_spec'
    case '実寸へ換算できる単位情報がないため、1単位あたりのmm値を指定してください。':
      return 'unit_needs_scale'
    case 'FOLD内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。':
      return 'invalid_title'
    case 'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。':
      return 'flat_crease'
    case 'U（未割当）は山折り・谷折り・補助線・除外のいずれかを選ぶ必要があります。':
      return 'unassigned'
    case 'J（面の結合）は同じ意味の線種がないため、補助線または除外へ変換します。':
      return 'face_join'
    default: {
      const ignored = /^取り込まないFOLD情報: ([^。\r\n]{1,500})。$/u
        .exec(warning)
      if (!ignored) return null
      const labels = ignored[1].split('、')
      return labels.length > 0 && labels.every((label) => (
        FOLD_IGNORED_METADATA_LABELS.has(label)
        || isBoundedUnknownFoldMetadataCount(label)
      ))
        ? 'ignored_metadata'
        : null
    }
  }
}

function isBoundedUnknownFoldMetadataCount(value: string) {
  const count = /^その他の拡張フィールド([0-9]{1,20})件$/u.exec(value)
  return count !== null
    && Number.isSafeInteger(Number(count[1]))
    && Number(count[1]) > 0
}

function isSafeFoldImportFileName(value: string) {
  const characters = [...value]
  return characters.length > 0
    && characters.length <= 255
    && value !== '.'
    && value !== '..'
    && !/[\\/:]/u.test(value)
    && !/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/u.test(value)
}

export function isAllowedFoldImportTarget(
  assignment: FoldAssignmentCode,
  target: FoldImportTarget,
) {
  return TARGETS_BY_ASSIGNMENT[assignment].includes(target)
}

export function initialFoldImportMapping(
  assignments: readonly FoldImportAssignmentSummary[],
): FoldImportMapping {
  const mapping: FoldImportMapping = {}
  for (const { assignment, count } of assignments) {
    if (assignment === 'B' || count <= 0) continue
    const direct = DIRECT_DEFAULTS[assignment]
    if (direct) mapping[assignment] = direct
  }
  return mapping
}

export function unresolvedFoldAssignments(
  assignments: readonly FoldImportAssignmentSummary[],
  mapping: FoldImportMapping,
) {
  return assignments
    .filter(({ assignment, count }) => (
      assignment !== 'B'
      && count > 0
      && (
        !mapping[assignment]
        || !isAllowedFoldImportTarget(assignment, mapping[assignment])
      )
    ))
    .map(({ assignment }) => assignment as FoldAssignmentCode)
}

export function parseFoldImportScale(value: string) {
  if (value.trim().length === 0) return null
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 1_000_000_000) return null
  return parsed
}

export function isValidFoldImportName(value: string) {
  const trimmed = value.trim()
  return trimmed.length > 0
    && [...trimmed].length <= 120
    && !Array.from(trimmed).some((character) => {
      const code = character.codePointAt(0)
      return code !== undefined && (code <= 0x1f || (code >= 0x7f && code <= 0x9f))
    })
}

export type FoldPreviewBounds = Readonly<{
  minX: number
  minY: number
  width: number
  height: number
}>

export function foldPreviewBounds(
  vertices: readonly FoldImportPreviewVertex[],
): FoldPreviewBounds | null {
  if (vertices.length === 0) return null
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const vertex of vertices) {
    if (!Number.isFinite(vertex.x) || !Number.isFinite(vertex.y)) return null
    minX = Math.min(minX, vertex.x)
    minY = Math.min(minY, vertex.y)
    maxX = Math.max(maxX, vertex.x)
    maxY = Math.max(maxY, vertex.y)
  }
  const rawWidth = maxX - minX
  const rawHeight = maxY - minY
  if (!Number.isFinite(rawWidth) || !Number.isFinite(rawHeight)) return null
  const reference = Math.max(rawWidth, rawHeight, 1)
  const minimumSpan = reference * 0.01
  const width = Math.max(rawWidth, minimumSpan)
  const height = Math.max(rawHeight, minimumSpan)
  const bounds = {
    minX: minX - (width - rawWidth) / 2,
    minY: minY - (height - rawHeight) / 2,
    width,
    height,
  }
  return Object.values(bounds).every(Number.isFinite) ? bounds : null
}
