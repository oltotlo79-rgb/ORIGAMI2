import type {
  LengthDisplayUnit,
  ProjectSnapshot,
} from './coreClient.ts'
import type { Locale } from './i18n.ts'

export type AbsoluteLengthDisplayUnit = 'mm' | 'cm' | 'inch'

export type BoundaryLengthReference = Readonly<{
  edgeId: string
  startVertexId: string
  endVertexId: string
  start: Readonly<{ x: number; y: number }>
  end: Readonly<{ x: number; y: number }>
  lengthMm: number
  boundaryIndex: number
}>

type ResolvedAbsoluteLengthDisplayUnit = Readonly<{
  mode: 'absolute'
  storedUnit: AbsoluteLengthDisplayUnit
  effectiveUnit: AbsoluteLengthDisplayUnit
  label: string
  millimetresPerUnit: number
  reference: null
  invalidReferenceEdgeId: null
  key: string
}>

type ResolvedPaperEdgeRatioDisplayUnit = Readonly<{
  mode: 'paper_edge_ratio'
  storedUnit: LengthDisplayUnit
  effectiveUnit: 'paper_edge_ratio'
  label: '紙辺比'
  millimetresPerUnit: number
  reference: BoundaryLengthReference
  invalidReferenceEdgeId: null
  key: string
}>

type InvalidPaperEdgeRatioDisplayUnit = Readonly<{
  mode: 'invalid_paper_edge_ratio'
  storedUnit: LengthDisplayUnit
  effectiveUnit: 'mm'
  label: 'mm'
  millimetresPerUnit: 1
  reference: null
  invalidReferenceEdgeId: string | null
  key: string
}>

export type ResolvedLengthDisplayUnit =
  | ResolvedAbsoluteLengthDisplayUnit
  | ResolvedPaperEdgeRatioDisplayUnit
  | InvalidPaperEdgeRatioDisplayUnit

const ABSOLUTE_UNIT_SCALE: Readonly<Record<
  AbsoluteLengthDisplayUnit,
  number
>> = Object.freeze({
  mm: 1,
  cm: 10,
  inch: 25.4,
})

const ABSOLUTE_UNIT_LABEL: Readonly<Record<
  AbsoluteLengthDisplayUnit,
  string
>> = Object.freeze({
  mm: 'mm',
  cm: 'cm',
  inch: 'in',
})

export const MILLIMETRE_LENGTH_DISPLAY_UNIT: ResolvedLengthDisplayUnit =
  Object.freeze({
    mode: 'absolute',
    storedUnit: 'mm',
    effectiveUnit: 'mm',
    label: 'mm',
    millimetresPerUnit: 1,
    reference: null,
    invalidReferenceEdgeId: null,
    key: 'mm',
  })

export function collectBoundaryLengthReferences(
  snapshot: ProjectSnapshot | null,
): readonly BoundaryLengthReference[] {
  if (!snapshot) return []
  const boundaryIds = snapshot.paper.boundary_vertices
  if (
    boundaryIds.length < 3
    || new Set(boundaryIds).size !== boundaryIds.length
  ) return []

  const verticesById = new Map<string, Array<{ x: number; y: number }>>()
  for (const vertex of snapshot.crease_pattern.vertices) {
    const current = verticesById.get(vertex.id)
    if (current) current.push(vertex.position)
    else verticesById.set(vertex.id, [vertex.position])
  }

  const edgeIdCounts = new Map<string, number>()
  const boundaryEdgesByFirstEndpoint = new Map<
    string,
    Map<string, ProjectSnapshot['crease_pattern']['edges']>
  >()
  for (const edge of snapshot.crease_pattern.edges) {
    edgeIdCounts.set(edge.id, (edgeIdCounts.get(edge.id) ?? 0) + 1)
    if (edge.kind !== 'boundary') continue
    const [firstEndpoint, secondEndpoint] = canonicalEndpointPair(
      edge.start,
      edge.end,
    )
    let bySecondEndpoint = boundaryEdgesByFirstEndpoint.get(firstEndpoint)
    if (!bySecondEndpoint) {
      bySecondEndpoint = new Map()
      boundaryEdgesByFirstEndpoint.set(firstEndpoint, bySecondEndpoint)
    }
    const matchingEdges = bySecondEndpoint.get(secondEndpoint)
    if (matchingEdges) matchingEdges.push(edge)
    else bySecondEndpoint.set(secondEndpoint, [edge])
  }

  const references: BoundaryLengthReference[] = []
  for (let index = 0; index < boundaryIds.length; index += 1) {
    const startVertexId = boundaryIds[index]
    const endVertexId = boundaryIds[(index + 1) % boundaryIds.length]
    const startMatches = verticesById.get(startVertexId)
    const endMatches = verticesById.get(endVertexId)
    if (startMatches?.length !== 1 || endMatches?.length !== 1) continue
    const start = startMatches[0]
    const end = endMatches[0]
    if (![start.x, start.y, end.x, end.y].every(Number.isFinite)) continue

    const [firstEndpoint, secondEndpoint] = canonicalEndpointPair(
      startVertexId,
      endVertexId,
    )
    const matchingEdges = boundaryEdgesByFirstEndpoint
      .get(firstEndpoint)
      ?.get(secondEndpoint) ?? []
    if (matchingEdges.length !== 1) continue
    const edge = matchingEdges[0]
    if (edgeIdCounts.get(edge.id) !== 1) continue

    const lengthMm = Math.hypot(end.x - start.x, end.y - start.y)
    if (!Number.isFinite(lengthMm) || lengthMm <= 0) continue
    references.push(Object.freeze({
      edgeId: edge.id,
      startVertexId,
      endVertexId,
      start: Object.freeze({ x: start.x, y: start.y }),
      end: Object.freeze({ x: end.x, y: end.y }),
      lengthMm,
      boundaryIndex: index,
    }))
  }
  return Object.freeze(references)
}

export function resolveLengthDisplayUnit(
  snapshot: ProjectSnapshot | null,
  references = collectBoundaryLengthReferences(snapshot),
): ResolvedLengthDisplayUnit {
  const stored = snapshot?.paper.length_display_unit as unknown
  if (stored === 'mm' || stored === 'cm' || stored === 'inch') {
    return absoluteLengthDisplayUnit(stored)
  }

  const referenceEdgeId = readPaperEdgeRatioReference(stored)
  if (referenceEdgeId !== undefined) {
    const matches = typeof referenceEdgeId === 'string'
      ? references.filter((reference) => reference.edgeId === referenceEdgeId)
      : []
    if (typeof referenceEdgeId === 'string' && matches.length === 1) {
      const reference = matches[0]
      return Object.freeze({
        mode: 'paper_edge_ratio',
        storedUnit: {
          paper_edge_ratio: { reference_edge: referenceEdgeId },
        },
        effectiveUnit: 'paper_edge_ratio',
        label: '紙辺比',
        millimetresPerUnit: reference.lengthMm,
        reference,
        invalidReferenceEdgeId: null,
        key: `paper_edge_ratio:${referenceEdgeId}:${float64Token(reference.lengthMm)}`,
      })
    }
    return invalidPaperEdgeRatioDisplayUnit(
      typeof referenceEdgeId === 'string' ? referenceEdgeId : null,
    )
  }

  // Legacy projects without this field are displayed in millimetres. A
  // malformed non-ratio value is also fail-closed to the repair unit.
  return absoluteLengthDisplayUnit('mm')
}

export function lengthDisplaySelectionValue(
  unit: ResolvedLengthDisplayUnit,
): AbsoluteLengthDisplayUnit | 'paper_edge_ratio' {
  return unit.mode === 'absolute' ? unit.storedUnit : 'paper_edge_ratio'
}

export function makePaperEdgeRatioUnit(referenceEdge: string): LengthDisplayUnit {
  return { paper_edge_ratio: { reference_edge: referenceEdge } }
}

export function lengthMillimetresToDisplay(
  valueMm: number,
  unit: ResolvedLengthDisplayUnit,
): number {
  return normalizeNegativeZero(valueMm / unit.millimetresPerUnit)
}

export function lengthDisplayToMillimetres(
  value: number,
  unit: ResolvedLengthDisplayUnit,
): number {
  return normalizeNegativeZero(value * unit.millimetresPerUnit)
}

export function parseLengthDisplayInput(
  input: string,
  unit: ResolvedLengthDisplayUnit,
): number | null {
  if (!input.trim()) return null
  const value = Number(input)
  if (!Number.isFinite(value)) return null
  const millimetres = lengthDisplayToMillimetres(value, unit)
  return Number.isFinite(millimetres) ? millimetres : null
}

export function formatLengthInput(
  valueMm: number | null | undefined,
  unit: ResolvedLengthDisplayUnit,
): string {
  if (typeof valueMm !== 'number' || !Number.isFinite(valueMm)) return ''
  const displayed = lengthMillimetresToDisplay(valueMm, unit)
  return Number.isFinite(displayed) ? String(displayed) : ''
}

export function formatLengthValue(
  valueMm: number | null | undefined,
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
  maximumFractionDigits = defaultMaximumFractionDigits(unit),
): string {
  if (typeof valueMm !== 'number' || !Number.isFinite(valueMm)) {
    return unavailableLengthText(locale)
  }
  const displayed = lengthMillimetresToDisplay(valueMm, unit)
  if (!Number.isFinite(displayed)) return unavailableLengthText(locale)
  return normalizeNegativeZero(displayed).toLocaleString(
    locale === 'en' ? 'en-US' : 'ja-JP',
    {
    maximumFractionDigits,
    },
  )
}

export function formatLength(
  valueMm: number | null | undefined,
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
  maximumFractionDigits = defaultMaximumFractionDigits(unit),
): string {
  const value = formatLengthValue(
    valueMm,
    unit,
    locale,
    maximumFractionDigits,
  )
  return value === unavailableLengthText(locale)
    ? value
    : `${value} ${lengthDisplayUnitLabel(unit, locale)}`
}

export function formatLengthPoint(
  xMm: number | null | undefined,
  yMm: number | null | undefined,
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
): string {
  const unavailable = unavailableLengthText(locale)
  const x = formatLengthValue(xMm, unit, locale)
  const y = formatLengthValue(yMm, unit, locale)
  return x === unavailable || y === unavailable
    ? unavailable
    : `(${x}, ${y}) ${lengthDisplayUnitLabel(unit, locale)}`
}

export function lengthDisplayUnitLabel(
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
) {
  return unit.mode === 'paper_edge_ratio'
    ? locale === 'en' ? 'paper-edge ratio' : '紙辺比'
    : unit.label
}

export function lengthInputSourceToken(
  valueMm: number,
  unit: ResolvedLengthDisplayUnit,
): string {
  return `${float64Token(valueMm)}:${unit.key}`
}

/**
 * Reads a converted length field while preserving the exact source number when
 * the user did not edit it. The source token prevents a stale DOM node from
 * being mistaken for the current snapshot after a unit/reference change.
 */
export function readLengthInputMillimetres(
  form: HTMLFormElement,
  name: string,
  originalMm: number,
  unit: ResolvedLengthDisplayUnit,
): number | null {
  const field = form.elements.namedItem(name)
  if (!(field instanceof HTMLInputElement)) return null
  const expectedToken = lengthInputSourceToken(originalMm, unit)
  const steppedThicknessMillimetres = Number(
    field.dataset.paperThicknessSteppedMillimetres,
  )
  if (
    field.dataset.lengthDirty === 'true'
    && field.dataset.lengthSourceToken === expectedToken
    && field.dataset.paperThicknessSteppedMillimetres !== undefined
    && Number.isFinite(steppedThicknessMillimetres)
    && steppedThicknessMillimetres >= 0
    && field.value === formatLengthInput(steppedThicknessMillimetres, unit)
  ) return steppedThicknessMillimetres
  if (
    field.dataset.lengthDirty === 'false'
    && field.dataset.lengthSourceToken === expectedToken
  ) return originalMm
  return parseLengthDisplayInput(field.value, unit)
}

export function ratioReferenceAxis(
  unit: ResolvedLengthDisplayUnit,
): 'width' | 'height' | null {
  if (unit.mode !== 'paper_edge_ratio') return null
  const deltaX = unit.reference.end.x - unit.reference.start.x
  const deltaY = unit.reference.end.y - unit.reference.start.y
  if (!Number.isFinite(deltaX) || !Number.isFinite(deltaY)) return null
  if (deltaY === 0 && deltaX !== 0) return 'width'
  if (deltaX === 0 && deltaY !== 0) return 'height'
  return null
}

function absoluteLengthDisplayUnit(
  storedUnit: AbsoluteLengthDisplayUnit,
): ResolvedAbsoluteLengthDisplayUnit {
  if (storedUnit === 'mm') {
    return MILLIMETRE_LENGTH_DISPLAY_UNIT as ResolvedAbsoluteLengthDisplayUnit
  }
  return Object.freeze({
    mode: 'absolute',
    storedUnit,
    effectiveUnit: storedUnit,
    label: ABSOLUTE_UNIT_LABEL[storedUnit],
    millimetresPerUnit: ABSOLUTE_UNIT_SCALE[storedUnit],
    reference: null,
    invalidReferenceEdgeId: null,
    key: storedUnit,
  })
}

function invalidPaperEdgeRatioDisplayUnit(
  referenceEdgeId: string | null,
): InvalidPaperEdgeRatioDisplayUnit {
  return Object.freeze({
    mode: 'invalid_paper_edge_ratio',
    storedUnit: {
      paper_edge_ratio: { reference_edge: referenceEdgeId ?? '' },
    },
    effectiveUnit: 'mm',
    label: 'mm',
    millimetresPerUnit: 1,
    reference: null,
    invalidReferenceEdgeId: referenceEdgeId,
    key: `invalid_paper_edge_ratio:${referenceEdgeId ?? ''}:mm-repair`,
  })
}

function readPaperEdgeRatioReference(value: unknown): string | null | undefined {
  if (!value || typeof value !== 'object') return undefined
  const ratio = Reflect.get(value, 'paper_edge_ratio')
  if (!ratio || typeof ratio !== 'object') return undefined
  const reference = Reflect.get(ratio, 'reference_edge')
  return typeof reference === 'string' && reference.length > 0
    ? reference
    : null
}

function defaultMaximumFractionDigits(unit: ResolvedLengthDisplayUnit) {
  if (unit.effectiveUnit === 'paper_edge_ratio') return 6
  if (unit.effectiveUnit === 'inch') return 5
  if (unit.effectiveUnit === 'cm') return 4
  return 3
}

function unavailableLengthText(locale: Locale) {
  return locale === 'en' ? 'Unavailable' : '計測不可'
}

function canonicalEndpointPair(first: string, second: string) {
  return first <= second ? [first, second] : [second, first]
}

function float64Token(value: number) {
  const bytes = new ArrayBuffer(8)
  const view = new DataView(bytes)
  view.setFloat64(0, value, false)
  return `${view.getUint32(0, false).toString(16).padStart(8, '0')}${
    view.getUint32(4, false).toString(16).padStart(8, '0')
  }`
}

function normalizeNegativeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
