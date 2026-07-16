import type { IntersectionSnapTarget } from './intersectionSnap'

export type SnapKind =
  | 'vertex'
  | 'intersection'
  | 'midpoint'
  | 'horizontal'
  | 'vertical'
  | 'edge'
  | 'grid'

type PointSnapKind = Exclude<SnapKind, 'intersection'>
type DirectionSnapKind = Extract<PointSnapKind, 'horizontal' | 'vertical'>
type OrdinaryPointSnapKind = Exclude<PointSnapKind, DirectionSnapKind>

export type SnapSettings = Readonly<{
  vertex: boolean
  intersection: boolean
  midpoint: boolean
  horizontal: boolean
  vertical: boolean
  edge: boolean
  grid: boolean
}>

export const DEFAULT_SNAP_SETTINGS: SnapSettings = Object.freeze({
  vertex: true,
  intersection: true,
  midpoint: true,
  horizontal: true,
  vertical: true,
  edge: true,
  grid: true,
})

export type SnapPoint = Readonly<{
  x: number
  y: number
}>

export type SnapVertex = Readonly<{
  id: string
  x: number
  y: number
}>

export type SnapSegment = Readonly<{
  id: string
  startVertexId: string
  endVertexId: string
  x1: number
  y1: number
  x2: number
  y2: number
}>

export type SnapGrid = Readonly<{
  xValues: readonly number[]
  yValues: readonly number[]
}>

export type SnapAnchor = Readonly<{
  id: string
  x: number
  y: number
}>

export function resolveUniqueSnapAnchor(
  vertices: readonly SnapVertex[],
  selectedVertexId: string | null,
): SnapAnchor | undefined {
  if (!selectedVertexId) return undefined
  let anchor: SnapAnchor | undefined
  for (const vertex of vertices) {
    if (vertex.id !== selectedVertexId) continue
    if (anchor || !isFinitePoint(vertex)) return undefined
    anchor = { id: vertex.id, x: vertex.x, y: vertex.y }
  }
  return anchor
}

type SnapTargetBase = Readonly<{
  key: string
  point: SnapPoint
  distancePx: number
  sourceId?: string
  sourceFraction?: number
}>

export type OrdinarySnapTarget = SnapTargetBase & Readonly<{
  kind: OrdinaryPointSnapKind
  anchorId?: never
  anchorPoint?: never
}>

export type DirectionSnapTarget = SnapTargetBase & Readonly<{
  kind: DirectionSnapKind
  sourceId: string
  sourceFraction?: never
  anchorId: string
  anchorPoint: SnapPoint
}>

export type SnapTarget = OrdinarySnapTarget | DirectionSnapTarget

export type AdditionSnapTarget = SnapTarget | IntersectionSnapTarget

export type SnapBounds = Readonly<{
  minX: number
  minY: number
  maxX: number
  maxY: number
}>

export type SnapThresholdsPx = Partial<Record<SnapKind, number>>

export type ResolveSnapTargetOptions = Readonly<{
  point: SnapPoint
  scale: number
  settings: SnapSettings
  vertices: readonly SnapVertex[]
  segments: readonly SnapSegment[]
  grid: SnapGrid
  anchor?: SnapAnchor
  excludedVertexId?: string
  accept?: (target: SnapTarget) => boolean
  thresholdsPx?: SnapThresholdsPx
}>

type RankedTarget = Readonly<{
  target: SnapTarget
  modelDistance: number
}>

const DEFAULT_THRESHOLDS_PX: Readonly<Record<PointSnapKind, number>> = Object.freeze({
  vertex: 10,
  midpoint: 9,
  horizontal: 8,
  vertical: 8,
  edge: 7,
  grid: 7,
})

const DEFAULT_DESIRED_INTERVALS = 20
const DEFAULT_MAX_GRID_VALUES = 100

export function createVisibleGrid(
  bounds: SnapBounds,
  desiredIntervals = DEFAULT_DESIRED_INTERVALS,
  maxValues = DEFAULT_MAX_GRID_VALUES,
): SnapGrid {
  if (![bounds.minX, bounds.minY, bounds.maxX, bounds.maxY].every(Number.isFinite)) {
    return emptyGrid()
  }
  if (bounds.maxX < bounds.minX || bounds.maxY < bounds.minY) return emptyGrid()

  const width = bounds.maxX - bounds.minX
  const height = bounds.maxY - bounds.minY
  if (!Number.isFinite(width) || !Number.isFinite(height)) return emptyGrid()
  const largestSpan = Math.max(width, height)
  if (largestSpan <= 0) return emptyGrid()

  const intervalCount = positiveIntegerOr(desiredIntervals, DEFAULT_DESIRED_INTERVALS)
  const valueLimit = nonNegativeIntegerOr(maxValues, DEFAULT_MAX_GRID_VALUES)
  if (valueLimit === 0) return emptyGrid()

  let step = niceGridStep(largestSpan / intervalCount)
  if (!Number.isFinite(step) || step <= 0) return emptyGrid()
  while (
    alignedValueCount(bounds.minX, bounds.maxX, step) > valueLimit
    || alignedValueCount(bounds.minY, bounds.maxY, step) > valueLimit
  ) {
    step = nextNiceGridStep(step)
    if (!Number.isFinite(step) || step <= 0) return emptyGrid()
  }

  return {
    xValues: alignedGridValues(bounds.minX, bounds.maxX, step, valueLimit),
    yValues: alignedGridValues(bounds.minY, bounds.maxY, step, valueLimit),
  }
}

export function resolveSnapTarget(options: ResolveSnapTargetOptions): SnapTarget | null {
  const { point, scale, settings } = options
  if (!isFinitePoint(point) || !Number.isFinite(scale) || scale <= 0) return null
  if (
    !settings.vertex
    && !settings.intersection
    && !settings.midpoint
    && !settings.horizontal
    && !settings.vertical
    && !settings.edge
    && !settings.grid
  ) return null

  if (settings.vertex) {
    const threshold = thresholdFor('vertex', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const vertex of options.vertices) {
      if (vertex.id === options.excludedVertexId || !isFinitePoint(vertex)) continue
      best = considerTargetPoint(
        best,
        `vertex:${vertex.id}`,
        'vertex',
        vertex.x,
        vertex.y,
        point,
        scale,
        threshold,
        vertex.id,
        options.accept,
      )
    }
    if (best) return best.target
  }

  if (settings.midpoint) {
    const threshold = thresholdFor('midpoint', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const segment of options.segments) {
      const geometry = validSegmentGeometry(segment, options.excludedVertexId)
      if (!geometry) continue
      const midpointX = stableAverage(segment.x1, segment.x2)
      const midpointY = stableAverage(segment.y1, segment.y2)
      if (!Number.isFinite(midpointX) || !Number.isFinite(midpointY)) continue
      best = considerTargetPoint(
        best,
        `midpoint:${segment.id}`,
        'midpoint',
        midpointX,
        midpointY,
        point,
        scale,
        threshold,
        segment.id,
        options.accept,
        0.5,
      )
    }
    if (best) return best.target
  }

  if (settings.horizontal || settings.vertical) {
    const best = bestDirectionTarget(options)
    if (best) return best.target
  }

  if (settings.edge) {
    const threshold = thresholdFor('edge', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const segment of options.segments) {
      const geometry = validSegmentGeometry(segment, options.excludedVertexId)
      if (!geometry) continue
      const offsetX = point.x - segment.x1
      const offsetY = point.y - segment.y1
      if (!Number.isFinite(offsetX) || !Number.isFinite(offsetY)) continue
      const numerator = offsetX * geometry.dx + offsetY * geometry.dy
      if (!Number.isFinite(numerator)) continue
      const fraction = Math.max(0, Math.min(1, numerator / geometry.lengthSquared))
      if (!Number.isFinite(fraction) || fraction <= 0 || fraction >= 1) continue
      const projectionX = segment.x1 + fraction * geometry.dx
      const projectionY = segment.y1 + fraction * geometry.dy
      if (!Number.isFinite(projectionX) || !Number.isFinite(projectionY)) continue
      best = considerTargetPoint(
        best,
        `edge:${segment.id}`,
        'edge',
        projectionX,
        projectionY,
        point,
        scale,
        threshold,
        segment.id,
        options.accept,
        fraction,
      )
    }
    if (best) return best.target
  }

  if (settings.grid) {
    const threshold = thresholdFor('grid', options.thresholdsPx)
    const best = options.accept
      ? bestAcceptedGridTarget(options, threshold)
      : nearestGridTarget(options, threshold)
    if (best) return best.target
  }

  return null
}

export function prioritizeAdditionSnapTargets(
  pointTarget: SnapTarget | null,
  intersectionTarget: IntersectionSnapTarget | null,
): AdditionSnapTarget | null {
  if (pointTarget?.kind === 'vertex') {
    if (
      intersectionTarget?.classification === 't-junction'
      && pointTarget.sourceId === intersectionTarget.junctionVertexId
      && pointTarget.point.x === intersectionTarget.point.x
      && pointTarget.point.y === intersectionTarget.point.y
    ) return intersectionTarget
    return pointTarget
  }
  return intersectionTarget ?? pointTarget
}

export function toggleSnapSetting(settings: SnapSettings, kind: SnapKind): SnapSettings {
  return { ...settings, [kind]: !settings[kind] }
}

function emptyGrid(): SnapGrid {
  return { xValues: [], yValues: [] }
}

function positiveIntegerOr(value: number, fallback: number) {
  if (!Number.isFinite(value) || value <= 0) return fallback
  return Math.max(1, Math.floor(value))
}

function nonNegativeIntegerOr(value: number, fallback: number) {
  if (!Number.isFinite(value)) return fallback
  return Math.max(0, Math.floor(value))
}

function niceGridStep(rawStep: number) {
  if (!Number.isFinite(rawStep) || rawStep <= 0) return Number.NaN
  const exponent = Math.floor(Math.log10(rawStep))
  const magnitude = 10 ** exponent
  if (!Number.isFinite(magnitude) || magnitude <= 0) return rawStep
  const fraction = rawStep / magnitude
  const niceFraction = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10
  const step = niceFraction * magnitude
  return Number.isFinite(step) && step > 0 ? step : Number.NaN
}

function nextNiceGridStep(step: number) {
  const exponent = Math.floor(Math.log10(step))
  const magnitude = 10 ** exponent
  if (!Number.isFinite(magnitude) || magnitude <= 0) return Number.NaN
  const fraction = step / magnitude
  if (fraction < 2) return 2 * magnitude
  if (fraction < 5) return 5 * magnitude
  if (fraction < 10) return 10 * magnitude
  return 20 * magnitude
}

function alignedValueCount(minimum: number, maximum: number, step: number) {
  const firstIndex = Math.ceil(minimum / step)
  const lastIndex = Math.floor(maximum / step)
  if (!Number.isFinite(firstIndex) || !Number.isFinite(lastIndex)) {
    return Number.POSITIVE_INFINITY
  }
  if (lastIndex < firstIndex) return 0
  const count = lastIndex - firstIndex + 1
  return Number.isFinite(count) ? count : Number.POSITIVE_INFINITY
}

function alignedGridValues(minimum: number, maximum: number, step: number, limit: number) {
  const firstIndex = Math.ceil(minimum / step)
  const count = alignedValueCount(minimum, maximum, step)
  if (!Number.isFinite(firstIndex) || !Number.isFinite(count) || count <= 0) return []
  const values: number[] = []
  for (let offset = 0; offset < Math.min(count, limit); offset += 1) {
    const value = normalizeZero((firstIndex + offset) * step)
    if (Number.isFinite(value) && value >= minimum && value <= maximum) values.push(value)
  }
  return values
}

function validSegmentGeometry(segment: SnapSegment, excludedVertexId?: string) {
  if (
    segment.startVertexId === excludedVertexId
    || segment.endVertexId === excludedVertexId
    || ![segment.x1, segment.y1, segment.x2, segment.y2].every(Number.isFinite)
  ) {
    return null
  }
  const dx = segment.x2 - segment.x1
  const dy = segment.y2 - segment.y1
  const lengthSquared = dx * dx + dy * dy
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) return null
  if (!Number.isFinite(lengthSquared) || lengthSquared <= 0) return null
  return { dx, dy, lengthSquared }
}

function stableAverage(first: number, second: number) {
  if (first === second) return first
  const firstIsNegative = first < 0 || Object.is(first, -0)
  const secondIsNegative = second < 0 || Object.is(second, -0)
  return firstIsNegative === secondIsNegative
    ? first + (second - first) / 2
    : first / 2 + second / 2
}

function considerTargetPoint(
  best: RankedTarget | null,
  key: string,
  kind: OrdinaryPointSnapKind,
  x: number,
  y: number,
  inputPoint: SnapPoint,
  scale: number,
  thresholdPx: number,
  sourceId: string | undefined,
  accept?: (target: SnapTarget) => boolean,
  sourceFraction?: number,
) {
  const modelDistance = Math.hypot(x - inputPoint.x, y - inputPoint.y)
  const distancePx = modelDistance * scale
  if (
    !Number.isFinite(modelDistance)
    || !Number.isFinite(distancePx)
    || distancePx > thresholdPx
    || (best !== null
      && (modelDistance > best.modelDistance
        || (modelDistance === best.modelDistance && key >= best.target.key)))
  ) {
    return best
  }
  const candidate: SnapTarget = sourceId === undefined
    ? { key, kind, point: { x, y }, distancePx }
    : sourceFraction === undefined
      ? { key, kind, point: { x, y }, distancePx, sourceId }
      : { key, kind, point: { x, y }, distancePx, sourceId, sourceFraction }
  return accept && !accept(candidate) ? best : { target: candidate, modelDistance }
}

function bestDirectionTarget(options: ResolveSnapTargetOptions) {
  const anchor = options.anchor
  if (
    !anchor
    || typeof anchor.id !== 'string'
    || anchor.id.length === 0
    || !isFinitePoint(anchor)
  ) return null

  let best: RankedTarget | null = null
  if (options.settings.horizontal) {
    best = considerDirectionTarget(
      best,
      'horizontal',
      { x: normalizeZero(options.point.x), y: normalizeZero(anchor.y) },
      options,
      anchor,
    )
  }
  if (options.settings.vertical) {
    best = considerDirectionTarget(
      best,
      'vertical',
      { x: normalizeZero(anchor.x), y: normalizeZero(options.point.y) },
      options,
      anchor,
    )
  }
  return best
}

function considerDirectionTarget(
  best: RankedTarget | null,
  kind: DirectionSnapKind,
  point: SnapPoint,
  options: ResolveSnapTargetOptions,
  anchor: SnapAnchor,
) {
  const modelDistance = Math.hypot(
    point.x - options.point.x,
    point.y - options.point.y,
  )
  const distancePx = modelDistance * options.scale
  const thresholdPx = thresholdFor(kind, options.thresholdsPx)
  const key = `${kind}:${JSON.stringify(anchor.id)}`
  if (
    !Number.isFinite(modelDistance)
    || !Number.isFinite(distancePx)
    || distancePx > thresholdPx
    || (best !== null
      && (modelDistance > best.modelDistance
        || (modelDistance === best.modelDistance && key >= best.target.key)))
  ) return best

  const candidate: DirectionSnapTarget = {
    key,
    kind,
    point,
    distancePx,
    sourceId: anchor.id,
    anchorId: anchor.id,
    anchorPoint: {
      x: normalizeZero(anchor.x),
      y: normalizeZero(anchor.y),
    },
  }
  return options.accept && !options.accept(candidate)
    ? best
    : { target: candidate, modelDistance }
}

function nearestGridTarget(options: ResolveSnapTargetOptions, thresholdPx: number) {
  const x = nearestGridValue(options.grid.xValues, options.point.x)
  const y = nearestGridValue(options.grid.yValues, options.point.y)
  if (x === null || y === null) return null
  return considerTargetPoint(
    null,
    `grid:${numberKey(x)}:${numberKey(y)}`,
    'grid',
    x,
    y,
    options.point,
    options.scale,
    thresholdPx,
    undefined,
  )
}

function bestAcceptedGridTarget(options: ResolveSnapTargetOptions, thresholdPx: number) {
  const modelThreshold = thresholdPx / options.scale
  if (Number.isNaN(modelThreshold) || modelThreshold < 0) return null
  let best: RankedTarget | null = null
  for (const rawX of options.grid.xValues) {
    if (!Number.isFinite(rawX) || Math.abs(rawX - options.point.x) > modelThreshold) continue
    const x = normalizeZero(rawX)
    for (const rawY of options.grid.yValues) {
      if (!Number.isFinite(rawY) || Math.abs(rawY - options.point.y) > modelThreshold) continue
      const y = normalizeZero(rawY)
      best = considerTargetPoint(
        best,
        `grid:${numberKey(x)}:${numberKey(y)}`,
        'grid',
        x,
        y,
        options.point,
        options.scale,
        thresholdPx,
        undefined,
        options.accept,
      )
    }
  }
  return best
}

function nearestGridValue(values: readonly number[], coordinate: number) {
  let bestValue: number | null = null
  let bestDistance = Number.POSITIVE_INFINITY
  for (const rawValue of values) {
    if (!Number.isFinite(rawValue)) continue
    const value = normalizeZero(rawValue)
    const distance = Math.abs(value - coordinate)
    if (!Number.isFinite(distance)) continue
    if (
      distance < bestDistance
      || (distance === bestDistance
        && (bestValue === null || numberKey(value) < numberKey(bestValue)))
    ) {
      bestValue = value
      bestDistance = distance
    }
  }
  return bestValue
}

function thresholdFor(kind: PointSnapKind, overrides?: SnapThresholdsPx) {
  const override = overrides?.[kind]
  return override !== undefined && Number.isFinite(override) && override >= 0
    ? override
    : DEFAULT_THRESHOLDS_PX[kind]
}

function isFinitePoint(point: SnapPoint) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function numberKey(value: number) {
  return String(normalizeZero(value))
}
