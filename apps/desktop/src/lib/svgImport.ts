import {
  foldPreviewBounds,
  isValidFoldImportName,
  parseFoldImportScale,
  type FoldPreviewBounds,
} from './foldImport.ts'

export type SvgImportTarget =
  | 'boundary'
  | 'mountain'
  | 'valley'
  | 'auxiliary'
  | 'cut'
  | 'ignore'

export type SvgImportMapping = Readonly<Record<string, SvgImportTarget | undefined>>

export type SvgImportLineCap = 'butt' | 'round' | 'square'

export type SvgRootLengthUnit =
  | 'mm'
  | 'cm'
  | 'in'
  | 'pt'
  | 'pc'
  | 'q'
  | 'px'
  | 'unitless'
  | 'em'
  | 'ex'
  | 'percent'

export type SvgImportPreviewVertex = Readonly<{
  x: number
  y: number
}>

export type SvgImportPreviewEdge = Readonly<{
  start: number
  end: number
  group_id: number
}>

export type SvgImportStyleGroup = Readonly<{
  group_id: number
  element_count: number
  segment_count: number
  stroke: string | null
  stroke_color: string | null
  dash_array: string | null
  line_cap: SvgImportLineCap
  classes: readonly string[]
  layer: string | null
  representative_id: string | null
  semantic_hint: SvgImportTarget | null
}>

export type SvgBoundaryCandidate = Readonly<{
  candidate_id: number
  kind: 'polygon' | 'polyline' | 'rectangle' | 'closed_path' | 'view_box'
  segment_count: number
  width: number
  height: number
  vertices: readonly SvgImportPreviewVertex[]
}>

export type SvgImportPreview = Readonly<{
  import_id: string
  file_name: string
  suggested_name: string
  default_mm_per_unit: number | null
  root_view_box: Readonly<{
    x: number
    y: number
    width: number
    height: number
  }> | null
  root_physical_size: Readonly<{
    width_millimetres: number | null
    height_millimetres: number | null
    width_unit: SvgRootLengthUnit | null
    height_unit: SvgRootLengthUnit | null
  }>
  source_segment_count: number
  style_groups: readonly SvgImportStyleGroup[]
  boundary_candidates: readonly SvgBoundaryCandidate[]
  preview_vertices: readonly SvgImportPreviewVertex[]
  preview_edges: readonly SvgImportPreviewEdge[]
  preview_truncated: boolean
  warnings: readonly string[]
}>

export type SvgImportSettings = Readonly<{
  importId: string
  validationId: string
  name: string
  mmPerUnit: number
  boundaryCandidateId: number | null
  boundaryConfirmed: boolean
  mappings: SvgImportMapping
  warningsAcknowledged: boolean
  cuttingAllowedConfirmed: boolean
}>

export type SvgImportSettingsDraft = Readonly<{
  importId: string
  mmPerUnit: number
  boundaryCandidateId: number | null
  mappings: SvgImportMapping
}>

export type SvgImportSettingsValidation = Readonly<{
  validation_id: string
  preview_id: string
  expected_project_id: string
  expected_revision: number
  millimeters_per_unit: number
  boundary_candidate_id: number | null
  width_mm: number
  height_mm: number
  has_cuts: boolean
}>

export const SVG_IMPORT_TARGET_OPTIONS: ReadonlyArray<Readonly<{
  value: SvgImportTarget
  label: string
}>> = [
  { value: 'boundary', label: '用紙境界' },
  { value: 'mountain', label: '山折り' },
  { value: 'valley', label: '谷折り' },
  { value: 'auxiliary', label: '補助線' },
  { value: 'cut', label: '切断線' },
  { value: 'ignore', label: '取り込まない' },
]

const SVG_IMPORT_TARGETS = new Set(
  SVG_IMPORT_TARGET_OPTIONS.map(({ value }) => value),
)

const SVG_IMPORT_LINE_CAPS = new Set<SvgImportLineCap>([
  'butt',
  'round',
  'square',
])

export function isSvgImportLineCap(value: unknown): value is SvgImportLineCap {
  return typeof value === 'string'
    && SVG_IMPORT_LINE_CAPS.has(value as SvgImportLineCap)
}

export function isSvgImportTarget(value: unknown): value is SvgImportTarget {
  return typeof value === 'string'
    && SVG_IMPORT_TARGETS.has(value as SvgImportTarget)
}

export function initialSvgImportMapping(
  groups: readonly SvgImportStyleGroup[],
): SvgImportMapping {
  const mapping: Record<string, SvgImportTarget> = {}
  for (const group of groups) {
    if (isValidSvgGroupId(group.group_id) && isSvgImportTarget(group.semantic_hint)) {
      mapping[String(group.group_id)] = group.semantic_hint
    }
  }
  return mapping
}

export function unresolvedSvgImportGroups(
  groups: readonly SvgImportStyleGroup[],
  mapping: SvgImportMapping,
) {
  return groups.filter((group) => {
    const target = mapping[String(group.group_id)]
    return group.segment_count > 0
      && (
        !isValidSvgGroupId(group.group_id)
        || !isSvgImportTarget(target)
        || !isSvgImportLineCap(group.line_cap)
      )
  })
}

export function svgImportBoundaryIsValid(
  preview: SvgImportPreview,
  boundaryCandidateId: number | null,
  mapping: SvgImportMapping,
) {
  const mappedBoundaryCount = preview.style_groups.filter(
    (group) => mapping[String(group.group_id)] === 'boundary',
  ).length
  if (boundaryCandidateId === null) return mappedBoundaryCount > 0
  return mappedBoundaryCount === 0
    && preview.boundary_candidates.some(
      (candidate) => candidate.candidate_id === boundaryCandidateId,
    )
}

export function svgImportTargetOptions(boundaryCandidateId: number | null) {
  return boundaryCandidateId === null
    ? SVG_IMPORT_TARGET_OPTIONS
    : SVG_IMPORT_TARGET_OPTIONS.filter(({ value }) => value !== 'boundary')
}

export function svgImportStyleLabel(group: SvgImportStyleGroup) {
  const parts: string[] = []
  if (group.layer) parts.push(`レイヤー: ${group.layer}`)
  if (group.classes.length > 0) parts.push(`class: ${group.classes.join(' ')}`)
  if (group.representative_id) parts.push(`代表ID: ${group.representative_id}`)
  if (group.semantic_hint) parts.push(`属性: data-origami-kind=${group.semantic_hint}`)
  if (group.stroke) parts.push(`色: ${group.stroke}`)
  if (group.dash_array) parts.push(`線種: ${group.dash_array}`)
  parts.push(`線端: ${isSvgImportLineCap(group.line_cap) ? group.line_cap : '不明'}`)
  return parts.length > 0 ? parts.join(' / ') : '属性指定なし'
}

export function safeSvgStrokeColor(value: string | null) {
  if (!value) return null
  const normalized = value.trim().toLowerCase()
  return /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/u.test(normalized)
    ? normalized
    : null
}

export function svgImportPreviewBounds(
  vertices: readonly SvgImportPreviewVertex[],
): FoldPreviewBounds | null {
  return foldPreviewBounds(vertices)
}

export const parseSvgImportScale = parseFoldImportScale
export const isValidSvgImportName = isValidFoldImportName

function isValidSvgGroupId(value: number) {
  return Number.isSafeInteger(value) && value >= 0 && value < 64
}
