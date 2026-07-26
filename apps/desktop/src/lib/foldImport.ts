import { selectLocalizedText, type Locale } from './i18n.ts'
import { classifyFoldImportNativeWarning } from './foldImportNativeWarningInput.ts'
import {
  FOLD_IMPORT_PRESENTATION_TEXT as PRESENTATION_TEXT,
  formatFoldImportBoundaryCandidatePresentation,
  formatFoldImportWarningPresentation,
} from './foldImportPresentationText.ts'

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
  source_index: number
  start: number
  end: number
  assignment: FoldAssignmentCode | 'B'
}>

export type FoldBoundaryCandidateSource =
  | 'assigned_boundary'
  | 'inferred_outer_face'

export type FoldImportBoundaryCandidate = Readonly<{
  id: number
  source: FoldBoundaryCandidateSource
  edge_indices: readonly number[]
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
  boundary_candidates: readonly FoldImportBoundaryCandidate[]
  fixed_boundary_candidate_id: number | null
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
  boundaryCandidateId: number
}>

export const FOLD_IMPORT_TARGET_OPTIONS: ReadonlyArray<Readonly<{
  value: FoldImportTarget
  label: string
}>> = [
  { value: 'mountain', label: PRESENTATION_TEXT.targetLabels.mountain.ja },
  { value: 'valley', label: PRESENTATION_TEXT.targetLabels.valley.ja },
  { value: 'auxiliary', label: PRESENTATION_TEXT.targetLabels.auxiliary.ja },
  { value: 'cut', label: PRESENTATION_TEXT.targetLabels.cut.ja },
  { value: 'ignore', label: PRESENTATION_TEXT.targetLabels.ignore.ja },
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

export function foldAssignmentLabel(
  assignment: FoldAssignmentCode | 'B',
  locale: Locale = 'ja',
) {
  return selectLocalizedText(
    locale,
    PRESENTATION_TEXT.assignmentLabels[assignment],
  )
}

export function foldImportTargetLabel(
  target: FoldImportTarget,
  locale: Locale = 'ja',
) {
  const option = FOLD_IMPORT_TARGET_OPTIONS.find(({ value }) => value === target)
  return option === undefined
    ? target
    : selectLocalizedText(
      locale,
      PRESENTATION_TEXT.targetLabels[option.value],
    )
}

export function foldImportWarningMessage(
  warning: unknown,
  locale: Locale = 'ja',
) {
  const classification = classifyFoldImportNativeWarning(warning)
  return formatFoldImportWarningPresentation(
    classification?.category ?? null,
    classification?.ignoredMetadata ?? null,
    locale,
  )
}

export function foldImportPreviewFileName(
  nativeLabel: unknown,
  locale: Locale = 'ja',
) {
  if (
    typeof nativeLabel === 'string'
    && nativeLabel !== PRESENTATION_TEXT.previewFileNameFallback.ja
    && nativeLabel !== PRESENTATION_TEXT.previewFileNameFallback.en
    && isSafeFoldImportFileName(nativeLabel)
  ) {
    return nativeLabel
  }
  return selectLocalizedText(
    locale,
    PRESENTATION_TEXT.previewFileNameFallback,
  )
}

export function isFoldImportFallbackName(value: unknown): value is string {
  return value === PRESENTATION_TEXT.suggestedNameFallback.ja
    || value === PRESENTATION_TEXT.suggestedNameFallback.en
}

export function foldImportSuggestedName(
  value: string,
  locale: Locale = 'ja',
) {
  if (!isFoldImportFallbackName(value)) return value
  return selectLocalizedText(locale, PRESENTATION_TEXT.suggestedNameFallback)
}

export function foldImportTargetOptions(assignment: FoldAssignmentCode) {
  const allowed = new Set(TARGETS_BY_ASSIGNMENT[assignment])
  return FOLD_IMPORT_TARGET_OPTIONS.filter(({ value }) => allowed.has(value))
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

export function initialFoldBoundaryCandidateId(
  preview: Pick<
    FoldImportPreview,
    'boundary_candidates' | 'fixed_boundary_candidate_id'
  >,
) {
  const fixed = preview.fixed_boundary_candidate_id
  return fixed !== null
    && isFoldBoundaryCandidateId(fixed)
    && preview.boundary_candidates.some((candidate) => candidate.id === fixed)
    ? fixed
    : null
}

export function foldBoundaryCandidate(
  preview: Pick<FoldImportPreview, 'boundary_candidates'>,
  candidateId: number | null,
) {
  if (candidateId === null || !isFoldBoundaryCandidateId(candidateId)) return null
  return preview.boundary_candidates.find(({ id }) => id === candidateId) ?? null
}

export function foldBoundaryCandidateLabel(
  candidate: FoldImportBoundaryCandidate,
  locale: Locale = 'ja',
) {
  return formatFoldImportBoundaryCandidatePresentation(
    candidate.source,
    candidate.id,
    candidate.edge_indices.length,
    locale,
  )
}

export function foldBoundaryPreviewEdgeSet(
  preview: Pick<FoldImportPreview, 'boundary_candidates'>,
  candidateId: number | null,
) {
  const candidate = foldBoundaryCandidate(preview, candidateId)
  return new Set(candidate?.edge_indices ?? [])
}

function isFoldBoundaryCandidateId(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= 65_535
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
