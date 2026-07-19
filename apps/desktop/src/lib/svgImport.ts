import {
  foldPreviewBounds,
  isValidFoldImportName,
  parseFoldImportScale,
  type FoldPreviewBounds,
} from './foldImport.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

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

const SVG_IMPORT_TARGET_LABELS = Object.freeze({
  boundary: localized('用紙境界', 'Paper boundary'),
  mountain: localized('山折り', 'Mountain fold'),
  valley: localized('谷折り', 'Valley fold'),
  auxiliary: localized('補助線', 'Auxiliary line'),
  cut: localized('切断線', 'Cut line'),
  ignore: localized('取り込まない', 'Do not import'),
}) satisfies Readonly<Record<SvgImportTarget, LocalizedText>>

const STYLE_LAYER = localized('レイヤー: {value}', 'Layer: {value}')
const STYLE_ID = localized('代表ID: {value}', 'Representative ID: {value}')
const STYLE_SEMANTIC = localized(
  '属性: data-origami-kind={value}',
  'Attribute: data-origami-kind={value}',
)
const STYLE_COLOR = localized('色: {value}', 'Color: {value}')
const STYLE_DASH = localized('線種: {value}', 'Dash pattern: {value}')
const STYLE_LINE_CAP = localized('線端: {value}', 'Line cap: {value}')
const UNKNOWN_TEXT = localized('不明', 'Unknown')
const NO_STYLE_TEXT = localized('属性指定なし', 'No style attributes')
const WARNING_PREVIEW_OMITTED = localized(
  '表示上限により{count}本の線をプレビューから省略しました。取込本体からは省略しません。',
  '{count} lines were omitted from the preview display limit. They will still be imported.',
)
const WARNING_GENERIC = localized(
  'SVGの一部の情報は取り込まれないか変更されます。',
  'Some SVG information will not be imported or will be changed.',
)

const SVG_WARNING_FIXED_EN = new Map<string, string>([
  [
    'SVG内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
    'The SVG title does not meet the project-name requirements, so the default project name will be used.',
  ],
  [
    'SVGのstroke色、透明度、線幅、破線・線端表現は線種確認にだけ使用し、取込後には保存しません。',
    'SVG stroke color, opacity, width, dash, and line-cap styling are used only to review line types and will not be saved after import.',
  ],
  [
    'SVGのレイヤー、class、代表ID、data-origami-kindは線種確認にだけ使用し、取込後には保存しません。',
    'SVG layers, classes, representative IDs, and data-origami-kind attributes are used only to review line types and will not be saved after import.',
  ],
])

const SVG_WARNING_PREFIX_EN: ReadonlyArray<readonly [string, LocalizedText]> = [
  ['未対応の要素', warningCount('Unsupported SVG elements were excluded')],
  ['未対応の属性', warningCount('Unsupported SVG attributes were ignored')],
  ['未対応のstyle property', warningCount('Unsupported SVG style properties were ignored')],
  ['未対応のCSS selector', warningCount('Unsupported CSS selectors were ignored')],
  ['曲線など未対応のpath command', warningCount('Paths with unsupported commands were excluded')],
  ['未対応のstroke指定', warningCount('Lines with unsupported stroke values were excluded')],
  ['解決できない長さ指定', warningCount('Shapes with unresolved length values were excluded')],
  ['外部参照', warningCount('External references were not fetched and were excluded')],
  ['非表示の形状', warningCount('Hidden shapes were excluded')],
  ['strokeのない形状', warningCount('Shapes without a stroke were excluded')],
  ['塗り情報', warningCount('Fill information will not be saved')],
  ['SVG metadata', warningCount('SVG metadata will not be saved')],
  ['空の形状', warningCount('Empty shapes were excluded')],
  ['物理寸法', warningCount('A scale must be entered because the physical size is ambiguous')],
  ['CSSの96 px', warningCount('The CSS conversion of 96 px per inch was used and may differ from the author’s intent')],
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

export function localizedSvgImportTargetOptions(
  boundaryCandidateId: number | null,
  locale: Locale = 'ja',
) {
  return Object.freeze(svgImportTargetOptions(boundaryCandidateId).map(({ value }) =>
    Object.freeze({
      value,
      label: selectLocalizedText(locale, SVG_IMPORT_TARGET_LABELS[value]),
    })))
}

export function svgImportStyleLabel(
  group: SvgImportStyleGroup,
  locale: Locale = 'ja',
) {
  const parts: string[] = []
  if (group.layer) {
    parts.push(formatLocalizedText(locale, STYLE_LAYER, { value: group.layer }))
  }
  if (group.classes.length > 0) parts.push(`class: ${group.classes.join(' ')}`)
  if (group.representative_id) {
    parts.push(formatLocalizedText(locale, STYLE_ID, {
      value: group.representative_id,
    }))
  }
  if (group.semantic_hint) {
    parts.push(formatLocalizedText(locale, STYLE_SEMANTIC, {
      value: group.semantic_hint,
    }))
  }
  if (group.stroke) {
    parts.push(formatLocalizedText(locale, STYLE_COLOR, { value: group.stroke }))
  }
  if (group.dash_array) {
    parts.push(formatLocalizedText(locale, STYLE_DASH, {
      value: group.dash_array,
    }))
  }
  parts.push(formatLocalizedText(locale, STYLE_LINE_CAP, {
    value: isSvgImportLineCap(group.line_cap)
      ? group.line_cap
      : selectLocalizedText(locale, UNKNOWN_TEXT),
  }))
  return parts.length > 0
    ? parts.join(' / ')
    : selectLocalizedText(locale, NO_STYLE_TEXT)
}

export function svgImportWarningText(
  warning: string,
  locale: Locale = 'ja',
): string {
  if (locale === 'ja') return warning
  const fixed = SVG_WARNING_FIXED_EN.get(warning)
  if (fixed) return fixed

  const omitted = /^表示上限により([0-9]+)本の線をプレビューから省略しました。取込本体からは省略しません。$/u
    .exec(warning)
  if (omitted) {
    return formatLocalizedText(locale, WARNING_PREVIEW_OMITTED, {
      count: omitted[1] ?? '?',
    })
  }

  const counted = /（([0-9]+)件）。$/u.exec(warning)
  const count = counted?.[1]
  if (count) {
    for (const [prefix, message] of SVG_WARNING_PREFIX_EN) {
      if (warning.startsWith(prefix)) {
        return formatLocalizedText(locale, message, { count })
      }
    }
  }
  return selectLocalizedText(locale, WARNING_GENERIC)
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

function warningCount(en: string): LocalizedText {
  return localized('{count}件', `${en} ({count} occurrences).`)
}

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}
