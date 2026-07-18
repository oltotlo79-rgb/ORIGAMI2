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

export function foldAssignmentLabel(assignment: FoldAssignmentCode | 'B') {
  return ASSIGNMENT_LABELS[assignment]
}

export function foldImportTargetOptions(assignment: FoldAssignmentCode) {
  const allowed = new Set(TARGETS_BY_ASSIGNMENT[assignment])
  return FOLD_IMPORT_TARGET_OPTIONS.filter(({ value }) => allowed.has(value))
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
