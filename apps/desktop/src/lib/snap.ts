import type { IntersectionSnapTarget } from './intersectionSnap'
import {
  createPointSpatialIndex,
  createSegmentSpatialIndex,
  type PointSpatialIndex,
  type SegmentSpatialIndex,
} from './nearestSpatialIndex.ts'

export type SnapKind =
  | 'vertex'
  | 'intersection'
  | 'midpoint'
  | 'horizontal'
  | 'vertical'
  | 'parallel'
  | 'angle'
  | 'circle-intersection'
  | 'edge'
  | 'grid'

type PointSnapKind = Exclude<SnapKind, 'intersection'>
type DirectionSnapKind = Extract<PointSnapKind, 'horizontal' | 'vertical'>
type OrdinaryPointSnapKind = Exclude<PointSnapKind, DirectionSnapKind | 'parallel' | 'angle'>

export type SnapSettings = Readonly<{
  vertex: boolean
  intersection: boolean
  midpoint: boolean
  horizontal: boolean
  vertical: boolean
  parallel: boolean
  angle: boolean
  edge: boolean
  grid: boolean
}>

export const DEFAULT_SNAP_SETTINGS: SnapSettings = Object.freeze({
  vertex: true,
  intersection: true,
  midpoint: true,
  horizontal: true,
  vertical: true,
  parallel: true,
  angle: true,
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
  kind?: 'mountain' | 'valley' | 'auxiliary' | 'boundary' | 'cut'
}>

export type SnapGrid = Readonly<{
  xValues: readonly number[]
  yValues: readonly number[]
  diagonals?: readonly Readonly<{
    x1: number
    y1: number
    x2: number
    y2: number
  }>[]
}>

export type CompassSnapCircle = Readonly<{
  centerX: number
  centerY: number
  radius: number
}>

export type SnapAnchor = Readonly<{
  id: string
  x: number
  y: number
}>

export type ParallelSnapReference = Readonly<{
  id: string
  x1: number
  y1: number
  x2: number
  y2: number
}>

export type AngleSnapReferenceKind = 'global-horizontal' | 'edge'

export type AngleSnapConfig = Readonly<{
  angleDegrees: number
  referenceKind: AngleSnapReferenceKind
}>

export const ANGLE_SNAP_PRESETS: readonly number[] = Object.freeze([
  11.25,
  15,
  22.5,
  30,
  45,
  60,
  67.5,
  90,
])

export const DEFAULT_ANGLE_SNAP_CONFIG: AngleSnapConfig = Object.freeze({
  angleDegrees: 45,
  referenceKind: 'global-horizontal',
})

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
  referenceEdgeId?: never
  referenceStartPoint?: never
  referenceEndPoint?: never
}>

export type DirectionSnapTarget = SnapTargetBase & Readonly<{
  kind: DirectionSnapKind
  sourceId: string
  sourceFraction?: never
  anchorId: string
  anchorPoint: SnapPoint
  referenceEdgeId?: never
  referenceStartPoint?: never
  referenceEndPoint?: never
}>

export type ParallelSnapTarget = SnapTargetBase & Readonly<{
  kind: 'parallel'
  sourceId: string
  sourceFraction?: never
  anchorId: string
  anchorPoint: SnapPoint
  referenceEdgeId: string
  referenceStartPoint: SnapPoint
  referenceEndPoint: SnapPoint
}>

type AngleSnapTargetBase = SnapTargetBase & Readonly<{
  kind: 'angle'
  sourceFraction?: never
  anchorId: string
  anchorPoint: SnapPoint
  rawPoint: SnapPoint
  angleDegrees: number
  angleSide: 'counterclockwise' | 'clockwise'
}>

export type AngleSnapTarget = AngleSnapTargetBase & (
  Readonly<{
    referenceKind: 'global-horizontal'
    referenceEdgeId?: never
    referenceStartPoint?: never
    referenceEndPoint?: never
  }>
  | Readonly<{
    referenceKind: 'edge'
    referenceEdgeId: string
    referenceStartPoint: SnapPoint
    referenceEndPoint: SnapPoint
  }>
)

export type SnapTarget =
  | OrdinarySnapTarget
  | DirectionSnapTarget
  | ParallelSnapTarget
  | AngleSnapTarget

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
  parallelReference?: ParallelSnapReference
  angleConfig?: AngleSnapConfig
  excludedVertexId?: string
  accept?: (target: SnapTarget) => boolean
  thresholdsPx?: SnapThresholdsPx
  spatialIndex?: SnapSpatialIndex
}>

export type SnapSpatialIndex = Readonly<{
  sourceVertices: readonly SnapVertex[]
  sourceSegments: readonly SnapSegment[]
  vertices: PointSpatialIndex<SnapVertex>
  midpoints: PointSpatialIndex<SnapSegment>
  segments: SegmentSpatialIndex<SnapSegment>
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
  parallel: 8,
  angle: 8,
  'circle-intersection': 8,
  edge: 7,
  grid: 7,
})

const DEFAULT_DESIRED_INTERVALS = 20
const DEFAULT_MAX_GRID_VALUES = 100
const MAX_DIVISION_GRID_DIVISIONS = 63
const SNAP_CATEGORY_BIAS_PX: Readonly<Record<PointSnapKind, number>> = Object.freeze({
  vertex: 0,
  midpoint: 0.2,
  horizontal: 0.4,
  vertical: 0.4,
  parallel: 0.6,
  angle: 0.6,
  'circle-intersection': 0.3,
  edge: 0.8,
  grid: 1,
})

export function createSnapSpatialIndex(
  vertices: readonly SnapVertex[],
  segments: readonly SnapSegment[],
): SnapSpatialIndex {
  return Object.freeze({
    sourceVertices: vertices,
    sourceSegments: segments,
    vertices: createPointSpatialIndex(vertices.map((vertex) => ({
      key: `vertex:${vertex.id}`,
      x: vertex.x,
      y: vertex.y,
      value: vertex,
    }))),
    midpoints: createPointSpatialIndex(segments.map((segment) => ({
      key: `midpoint:${segment.id}`,
      x: stableAverage(segment.x1, segment.x2),
      y: stableAverage(segment.y1, segment.y2),
      value: segment,
    }))),
    segments: createSegmentSpatialIndex(segments.map((segment) => ({
      key: `edge:${segment.id}`,
      x1: segment.x1,
      y1: segment.y1,
      x2: segment.x2,
      y2: segment.y2,
      value: segment,
    }))),
  })
}

export function prioritizePointSnapTargets(
  first: SnapTarget | null,
  second: SnapTarget | null,
): SnapTarget | null {
  if (!first) return second
  if (!second) return first
  const firstScore = first.distancePx + SNAP_CATEGORY_BIAS_PX[first.kind]
  const secondScore = second.distancePx + SNAP_CATEGORY_BIAS_PX[second.kind]
  return secondScore < firstScore
    || (secondScore === firstScore && second.key < first.key)
    ? second
    : first
}

export function resolveCompassIntersectionSnap(options: Readonly<{
  point: SnapPoint
  scale: number
  circles: readonly CompassSnapCircle[]
  segments: readonly SnapSegment[]
  thresholdPx?: number
  accept?: (target: SnapTarget) => boolean
}>): SnapTarget | null {
  if (!isFinitePoint(options.point) || !Number.isFinite(options.scale) || options.scale <= 0
    || options.circles.length > 64) return null
  const threshold = options.thresholdPx ?? DEFAULT_THRESHOLDS_PX['circle-intersection']
  if (!Number.isFinite(threshold) || threshold < 0) return null
  let best: SnapTarget | null = null
  const admit = (key: string, x: number, y: number) => {
    const candidate = considerTargetPoint(null, key, 'circle-intersection', x, y, options.point,
      options.scale, threshold, undefined, options.accept)
    if (candidate) best = prioritizePointSnapTargets(best, candidate.target)
  }
  for (let circleIndex = 0; circleIndex < options.circles.length; circleIndex += 1) {
    const circle = options.circles[circleIndex]!
    if (!Number.isFinite(circle.centerX) || !Number.isFinite(circle.centerY)
      || !Number.isFinite(circle.radius) || circle.radius <= 0) continue
    for (const segment of options.segments) {
      const geometry = validSegmentGeometry(segment)
      if (!geometry) continue
      const ox = segment.x1 - circle.centerX
      const oy = segment.y1 - circle.centerY
      const b = 2 * (ox * geometry.dx + oy * geometry.dy)
      const c = ox * ox + oy * oy - circle.radius * circle.radius
      const discriminant = b * b - 4 * geometry.lengthSquared * c
      if (!Number.isFinite(discriminant) || discriminant < 0) continue
      const root = Math.sqrt(discriminant)
      const roots = root === 0
        ? [[0, -b / (2 * geometry.lengthSquared)]] as const
        : [[0, (-b - root) / (2 * geometry.lengthSquared)],
            [1, (-b + root) / (2 * geometry.lengthSquared)]] as const
      for (const [side, fraction] of roots) {
        if (!Number.isFinite(fraction) || fraction < 0 || fraction > 1) continue
        admit(`circle-line:${circleIndex}:${segment.id}:${side}`,
          segment.x1 + fraction * geometry.dx, segment.y1 + fraction * geometry.dy)
      }
    }
    for (let otherIndex = circleIndex + 1; otherIndex < options.circles.length; otherIndex += 1) {
      const other = options.circles[otherIndex]!
      if (!Number.isFinite(other.centerX) || !Number.isFinite(other.centerY)
        || !Number.isFinite(other.radius) || other.radius <= 0) continue
      const dx = other.centerX - circle.centerX
      const dy = other.centerY - circle.centerY
      const distance = Math.hypot(dx, dy)
      if (!Number.isFinite(distance) || distance === 0
        || distance > circle.radius + other.radius
        || distance < Math.abs(circle.radius - other.radius)) continue
      const along = (circle.radius ** 2 - other.radius ** 2 + distance ** 2) / (2 * distance)
      const heightSquared = circle.radius ** 2 - along ** 2
      if (!Number.isFinite(heightSquared) || heightSquared < 0) continue
      const height = Math.sqrt(Math.max(0, heightSquared))
      const baseX = circle.centerX + along * dx / distance
      const baseY = circle.centerY + along * dy / distance
      const perpendicularX = -dy * height / distance
      const perpendicularY = dx * height / distance
      admit(`circle-circle:${circleIndex}:${otherIndex}:0`, baseX + perpendicularX, baseY + perpendicularY)
      if (height !== 0) {
        admit(`circle-circle:${circleIndex}:${otherIndex}:1`, baseX - perpendicularX, baseY - perpendicularY)
      }
    }
  }
  return best
}

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

export function createDivisionGrid(
  bounds: SnapBounds,
  divisions: number,
  maxValues = DEFAULT_MAX_GRID_VALUES,
  includeDiagonals = false,
): SnapGrid {
  if (
    ![bounds.minX, bounds.minY, bounds.maxX, bounds.maxY].every(Number.isFinite)
    || bounds.maxX < bounds.minX
    || bounds.maxY < bounds.minY
    || !Number.isSafeInteger(divisions)
    || divisions < 2
    || divisions > MAX_DIVISION_GRID_DIVISIONS
  ) return emptyGrid()
  const valueLimit = nonNegativeIntegerOr(maxValues, DEFAULT_MAX_GRID_VALUES)
  if (divisions + 1 > valueLimit) return emptyGrid()
  const axis = (minimum: number, maximum: number) => Array.from(
    { length: divisions + 1 },
    (_, index) => {
      if (index === 0) return minimum
      if (index === divisions) return maximum
      const fraction = index / divisions
      return minimum * (1 - fraction) + maximum * fraction
    },
  )
  const xValues = axis(bounds.minX, bounds.maxX)
  const yValues = axis(bounds.minY, bounds.maxY)
  if (![...xValues, ...yValues].every(Number.isFinite)) return emptyGrid()
  return {
    xValues,
    yValues,
    ...(includeDiagonals ? {
      diagonals: [
        { x1: bounds.minX, y1: bounds.minY, x2: bounds.maxX, y2: bounds.maxY },
        { x1: bounds.minX, y1: bounds.maxY, x2: bounds.maxX, y2: bounds.minY },
      ],
    } : {}),
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
    && !settings.parallel
    && !settings.angle
    && !settings.edge
    && !settings.grid
  ) return null
  const candidates: SnapTarget[] = []

  if (settings.vertex) {
    const threshold = thresholdFor('vertex', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const vertex of snapVertexCandidates(options, threshold)) {
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
    if (best) candidates.push(best.target)
  }

  if (settings.midpoint) {
    const threshold = thresholdFor('midpoint', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const segment of snapMidpointCandidates(options, threshold)) {
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
    if (best) candidates.push(best.target)
  }

  if (settings.horizontal || settings.vertical) {
    const best = bestDirectionTarget(options)
    if (best) candidates.push(best.target)
  }

  if (settings.parallel) {
    const target = parallelSnapTarget(options)
    if (target && (!options.accept || options.accept(target))) candidates.push(target)
  }

  if (settings.angle) {
    const target = angleSnapTarget(options)
    if (target) candidates.push(target)
  }

  if (settings.edge) {
    const threshold = thresholdFor('edge', options.thresholdsPx)
    let best: RankedTarget | null = null
    for (const segment of snapSegmentCandidates(options, threshold)) {
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
    if (best) candidates.push(best.target)
  }

  if (settings.grid) {
    const threshold = thresholdFor('grid', options.thresholdsPx)
    const best = options.accept
      ? bestAcceptedGridTarget(options, threshold)
      : nearestGridTarget(options, threshold)
    if (best) candidates.push(best.target)
  }

  return candidates.reduce<SnapTarget | null>((best, candidate) => {
    if (!best) return candidate
    const candidateScore = candidate.distancePx + SNAP_CATEGORY_BIAS_PX[candidate.kind]
    const bestScore = best.distancePx + SNAP_CATEGORY_BIAS_PX[best.kind]
    return candidateScore < bestScore
      || (candidateScore === bestScore && candidate.key < best.key)
      ? candidate
      : best
  }, null)
}

export function prioritizeAdditionSnapTargets(
  pointTarget: SnapTarget | null,
  intersectionTarget: IntersectionSnapTarget | null,
): AdditionSnapTarget | null {
  if (pointTarget?.kind === 'vertex') {
    if (
      intersectionTarget
      && (
        intersectionTarget.classification === 't-junction'
        || intersectionTarget.classification === 'cluster'
      )
      && pointTarget.sourceId === intersectionTarget.junctionVertexId
      && pointTarget.point.x === intersectionTarget.point.x
      && pointTarget.point.y === intersectionTarget.point.y
    ) return intersectionTarget
    return pointTarget
  }
  return intersectionTarget ?? pointTarget
}

export function vertexSnapOutranksBlockedIntersection(
  pointTarget: SnapTarget | null,
  blockedDistancePx: number | null,
) {
  return pointTarget?.kind === 'vertex'
    && Number.isFinite(pointTarget.distancePx)
    && pointTarget.distancePx >= 0
    && blockedDistancePx !== null
    && Number.isFinite(blockedDistancePx)
    && blockedDistancePx >= 0
    && pointTarget.distancePx < blockedDistancePx
}

export function toggleSnapSetting(settings: SnapSettings, kind: keyof SnapSettings): SnapSettings {
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

function snapVertexCandidates(
  options: ResolveSnapTargetOptions,
  thresholdPx: number,
): readonly SnapVertex[] {
  const index = options.spatialIndex
  const radius = expandedModelRadius(thresholdPx, options.scale)
  if (!index || index.sourceVertices !== options.vertices || radius === null) {
    return options.vertices
  }
  return index.vertices.withinRadius({ point: options.point, radius }).matches
    .map(({ value }) => value)
}

function snapMidpointCandidates(
  options: ResolveSnapTargetOptions,
  thresholdPx: number,
): readonly SnapSegment[] {
  const index = options.spatialIndex
  const radius = expandedModelRadius(thresholdPx, options.scale)
  if (!index || index.sourceSegments !== options.segments || radius === null) {
    return options.segments
  }
  return index.midpoints.withinRadius({ point: options.point, radius }).matches
    .map(({ value }) => value)
}

function snapSegmentCandidates(
  options: ResolveSnapTargetOptions,
  thresholdPx: number,
): readonly SnapSegment[] {
  const index = options.spatialIndex
  const radius = expandedModelRadius(thresholdPx, options.scale)
  if (!index || index.sourceSegments !== options.segments || radius === null) {
    return options.segments
  }
  return index.segments.withinRadius({ point: options.point, radius }).matches
    .map(({ value }) => value)
}

function expandedModelRadius(thresholdPx: number, scale: number) {
  const radius = thresholdPx / scale
  if (!Number.isFinite(radius) || radius < 0) return null
  // Candidate collection is deliberately a few ULPs wider than the pixel
  // authority below. This prevents division-vs-multiplication roundoff from
  // dropping a threshold candidate; considerTargetPoint remains authoritative.
  const expansion = Math.max(
    Number.MIN_VALUE,
    Math.abs(radius) * Number.EPSILON * 8,
  )
  const expanded = radius + expansion
  return Number.isFinite(expanded) ? expanded : null
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

function parallelSnapTarget(options: ResolveSnapTargetOptions): ParallelSnapTarget | null {
  const anchor = options.anchor
  const reference = options.parallelReference
  if (
    !anchor
    || typeof anchor.id !== 'string'
    || anchor.id.length === 0
    || !isFinitePoint(anchor)
    || !reference
    || typeof reference.id !== 'string'
    || reference.id.length === 0
  ) return null

  const endpoints = canonicalReferenceEndpoints(reference)
  if (!endpoints) return null
  const direction = stableDirectionComponents(endpoints.start, endpoints.end)
  if (!direction) return null

  const offsetX = options.point.x - anchor.x
  const offsetY = options.point.y - anchor.y
  if (!Number.isFinite(offsetX) || !Number.isFinite(offsetY)) return null
  const offsetScale = Math.max(Math.abs(offsetX), Math.abs(offsetY))
  if (!Number.isFinite(offsetScale)) return null
  const normalizedOffsetX = offsetScale === 0 ? 0 : offsetX / offsetScale
  const normalizedOffsetY = offsetScale === 0 ? 0 : offsetY / offsetScale
  const dotX = normalizedOffsetX * direction.x
  const dotY = normalizedOffsetY * direction.y
  if (!Number.isFinite(dotX) || !Number.isFinite(dotY)) return null
  const normalizedNumerator = dotX + dotY
  const denominator = direction.x * direction.x + direction.y * direction.y
  if (
    !Number.isFinite(normalizedNumerator)
    || !Number.isFinite(denominator)
    || denominator <= 0
  ) return null
  const normalizedFactor = normalizedNumerator / denominator
  if (!Number.isFinite(normalizedFactor)) return null
  const factor = normalizedFactor * offsetScale
  if (!Number.isFinite(factor)) return null
  const projectedOffsetX = factor * direction.x
  const projectedOffsetY = factor * direction.y
  if (!Number.isFinite(projectedOffsetX) || !Number.isFinite(projectedOffsetY)) return null
  const projectedX = normalizeZero(anchor.x + projectedOffsetX)
  const projectedY = normalizeZero(anchor.y + projectedOffsetY)
  if (!Number.isFinite(projectedX) || !Number.isFinite(projectedY)) return null

  const correctionX = projectedX - options.point.x
  const correctionY = projectedY - options.point.y
  if (!Number.isFinite(correctionX) || !Number.isFinite(correctionY)) return null
  const modelDistance = stableHypot(correctionX, correctionY)
  const distancePx = modelDistance * options.scale
  if (
    !Number.isFinite(modelDistance)
    || !Number.isFinite(distancePx)
    || distancePx > thresholdFor('parallel', options.thresholdsPx)
  ) return null

  return {
    key: parallelKey(anchor.id, reference.id),
    kind: 'parallel',
    point: { x: projectedX, y: projectedY },
    distancePx,
    sourceId: reference.id,
    anchorId: anchor.id,
    anchorPoint: {
      x: normalizeZero(anchor.x),
      y: normalizeZero(anchor.y),
    },
    referenceEdgeId: reference.id,
    referenceStartPoint: endpoints.start,
    referenceEndPoint: endpoints.end,
  }
}

function angleSnapTarget(options: ResolveSnapTargetOptions): AngleSnapTarget | null {
  const anchor = options.anchor
  const config = options.angleConfig
  if (
    !anchor
    || typeof anchor.id !== 'string'
    || anchor.id.length === 0
    || !isFinitePoint(anchor)
    || !config
    || !Number.isFinite(config.angleDegrees)
    || config.angleDegrees <= 0
    || config.angleDegrees > 90
    || (config.referenceKind !== 'global-horizontal' && config.referenceKind !== 'edge')
    || (options.point.x === anchor.x && options.point.y === anchor.y)
  ) return null

  let referenceMetadata:
    | Readonly<{
      referenceKind: 'global-horizontal'
    }>
    | Readonly<{
      referenceKind: 'edge'
      referenceEdgeId: string
      referenceStartPoint: SnapPoint
      referenceEndPoint: SnapPoint
    }>
  let baseDirection: SnapPoint
  if (config.referenceKind === 'global-horizontal') {
    referenceMetadata = { referenceKind: 'global-horizontal' }
    baseDirection = { x: 1, y: 0 }
  } else {
    const reference = options.parallelReference
    if (
      !reference
      || typeof reference.id !== 'string'
      || reference.id.length === 0
    ) return null
    const endpoints = canonicalReferenceEndpoints(reference)
    if (!endpoints) return null
    const direction = stableUnitDirection(endpoints.start, endpoints.end)
    if (!direction) return null
    referenceMetadata = {
      referenceKind: 'edge',
      referenceEdgeId: reference.id,
      referenceStartPoint: endpoints.start,
      referenceEndPoint: endpoints.end,
    }
    baseDirection = direction
  }

  const angleDegrees = normalizeZero(config.angleDegrees)
  const angleRadians = angleDegrees * Math.PI / 180
  const cosine = angleDegrees === 90 ? 0 : Math.cos(angleRadians)
  const sine = angleDegrees === 90 ? 1 : Math.sin(angleRadians)
  if (
    ![angleRadians, cosine, sine].every(Number.isFinite)
    || angleRadians <= 0
    || sine <= 0
  ) return null

  const sides: readonly AngleSnapTarget['angleSide'][] = angleDegrees === 90
    ? ['counterclockwise']
    : ['counterclockwise', 'clockwise']
  const candidates: RankedTarget[] = []
  for (const angleSide of sides) {
    const direction = rotatedUnitDirection(baseDirection, cosine, sine, angleSide)
    if (!direction) continue
    const projection = projectOntoAnchoredDirection(options.point, anchor, direction)
    if (
      !projection
      || (projection.point.x === anchor.x && projection.point.y === anchor.y)
    ) continue
    const distancePx = projection.modelDistance * options.scale
    if (
      !Number.isFinite(distancePx)
      || distancePx > thresholdFor('angle', options.thresholdsPx)
    ) continue

    const target: AngleSnapTarget = {
      key: angleKey(
        anchor.id,
        referenceMetadata.referenceKind,
        angleDegrees,
        angleSide,
        referenceMetadata.referenceKind === 'edge'
          ? referenceMetadata.referenceEdgeId
          : undefined,
      ),
      kind: 'angle',
      point: projection.point,
      distancePx,
      anchorId: anchor.id,
      anchorPoint: {
        x: normalizeZero(anchor.x),
        y: normalizeZero(anchor.y),
      },
      rawPoint: {
        x: normalizeZero(options.point.x),
        y: normalizeZero(options.point.y),
      },
      angleDegrees,
      angleSide,
      ...referenceMetadata,
    }
    candidates.push({ target, modelDistance: projection.modelDistance })
  }

  candidates.sort((first, second) => {
    if (first.modelDistance < second.modelDistance) return -1
    if (first.modelDistance > second.modelDistance) return 1
    const firstSide = (first.target as AngleSnapTarget).angleSide
    const secondSide = (second.target as AngleSnapTarget).angleSide
    if (firstSide === secondSide) return 0
    return firstSide === 'counterclockwise' ? -1 : 1
  })
  for (const candidate of candidates) {
    if (!options.accept || options.accept(candidate.target)) return candidate.target as AngleSnapTarget
  }
  return null
}

function rotatedUnitDirection(
  base: SnapPoint,
  cosine: number,
  sine: number,
  side: AngleSnapTarget['angleSide'],
) {
  const signedSine = side === 'counterclockwise' ? sine : -sine
  const x = base.x * cosine - base.y * signedSine
  const y = base.x * signedSine + base.y * cosine
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null
  const length = Math.hypot(x, y)
  if (!Number.isFinite(length) || length <= 0) return null
  const unitX = x / length
  const unitY = y / length
  return Number.isFinite(unitX) && Number.isFinite(unitY)
    ? { x: unitX, y: unitY }
    : null
}

function projectOntoAnchoredDirection(
  point: SnapPoint,
  anchor: SnapPoint,
  direction: SnapPoint,
) {
  // Keep the operation order in sync with vertexPlacement.ts: rawPoint lets
  // placement re-run this projection exactly without a world-coordinate epsilon.
  const offset = stableNormalizedDifference(anchor, point)
  if (!offset) return null
  const firstTerm = offset.x * direction.x
  const secondTerm = offset.y * direction.y
  const normalizedFactor = firstTerm + secondTerm
  if (![firstTerm, secondTerm, normalizedFactor].every(Number.isFinite)) return null
  const normalizedProjectedOffsetX = normalizedFactor * direction.x
  const normalizedProjectedOffsetY = normalizedFactor * direction.y
  if (
    !Number.isFinite(normalizedProjectedOffsetX)
    || !Number.isFinite(normalizedProjectedOffsetY)
  ) return null
  const projectedOffsetX = normalizedProjectedOffsetX * offset.scale
  const projectedOffsetY = normalizedProjectedOffsetY * offset.scale
  if (!Number.isFinite(projectedOffsetX) || !Number.isFinite(projectedOffsetY)) return null
  const projectedX = normalizeZero(anchor.x + projectedOffsetX)
  const projectedY = normalizeZero(anchor.y + projectedOffsetY)
  if (!Number.isFinite(projectedX) || !Number.isFinite(projectedY)) return null

  const correctionX = projectedX - point.x
  const correctionY = projectedY - point.y
  if (!Number.isFinite(correctionX) || !Number.isFinite(correctionY)) return null
  const modelDistance = stableHypot(correctionX, correctionY)
  if (!Number.isFinite(modelDistance)) return null
  return {
    point: { x: projectedX, y: projectedY },
    modelDistance,
  }
}

function stableNormalizedDifference(start: SnapPoint, end: SnapPoint) {
  const dx = end.x - start.x
  const dy = end.y - start.y
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) return null
  const scale = Math.max(Math.abs(dx), Math.abs(dy))
  if (!Number.isFinite(scale) || scale <= 0) return null
  const x = dx / scale
  const y = dy / scale
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y, scale } : null
}

function canonicalReferenceEndpoints(reference: ParallelSnapReference) {
  if (![reference.x1, reference.y1, reference.x2, reference.y2].every(Number.isFinite)) {
    return null
  }
  const first = { x: normalizeZero(reference.x1), y: normalizeZero(reference.y1) }
  const second = { x: normalizeZero(reference.x2), y: normalizeZero(reference.y2) }
  if (first.x === second.x && first.y === second.y) return null
  return comparePoints(first, second) <= 0
    ? { start: first, end: second }
    : { start: second, end: first }
}

function stableDirectionComponents(start: SnapPoint, end: SnapPoint) {
  let dx = end.x - start.x
  let dy = end.y - start.y
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) {
    const coordinateScale = Math.max(
      Math.abs(start.x),
      Math.abs(start.y),
      Math.abs(end.x),
      Math.abs(end.y),
    )
    if (!Number.isFinite(coordinateScale) || coordinateScale <= 0) return null
    dx = end.x / coordinateScale - start.x / coordinateScale
    dy = end.y / coordinateScale - start.y / coordinateScale
  }
  const maximumComponent = Math.max(Math.abs(dx), Math.abs(dy))
  if (!Number.isFinite(maximumComponent) || maximumComponent <= 0) return null
  const x = dx / maximumComponent
  const y = dy / maximumComponent
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null
}

function stableUnitDirection(start: SnapPoint, end: SnapPoint) {
  const direction = stableDirectionComponents(start, end)
  if (!direction) return null
  const length = Math.hypot(direction.x, direction.y)
  if (!Number.isFinite(length) || length <= 0) return null
  const x = direction.x / length
  const y = direction.y / length
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null
}

function stableHypot(x: number, y: number) {
  const maximumComponent = Math.max(Math.abs(x), Math.abs(y))
  if (!Number.isFinite(maximumComponent)) return Number.POSITIVE_INFINITY
  if (maximumComponent === 0) return 0
  const normalized = Math.hypot(x / maximumComponent, y / maximumComponent)
  const result = maximumComponent * normalized
  return Number.isFinite(result) ? result : Number.POSITIVE_INFINITY
}

function comparePoints(first: SnapPoint, second: SnapPoint) {
  return first.x < second.x || (first.x === second.x && first.y < second.y) ? -1 : 1
}

function parallelKey(anchorId: string, referenceEdgeId: string) {
  return `parallel:${JSON.stringify([anchorId, referenceEdgeId])}`
}

function angleKey(
  anchorId: string,
  referenceKind: AngleSnapReferenceKind,
  angleDegrees: number,
  angleSide: AngleSnapTarget['angleSide'],
  referenceEdgeId?: string,
) {
  return referenceKind === 'edge'
    ? `angle:${JSON.stringify([
      anchorId,
      referenceKind,
      referenceEdgeId,
      normalizeZero(angleDegrees),
      angleSide,
    ])}`
    : `angle:${JSON.stringify([
      anchorId,
      referenceKind,
      normalizeZero(angleDegrees),
      angleSide,
    ])}`
}

function nearestGridTarget(options: ResolveSnapTargetOptions, thresholdPx: number) {
  const x = nearestGridValue(options.grid.xValues, options.point.x)
  const y = nearestGridValue(options.grid.yValues, options.point.y)
  let best = x === null || y === null ? null : considerTargetPoint(
    null, `grid:${numberKey(x)}:${numberKey(y)}`, 'grid', x, y,
    options.point, options.scale, thresholdPx, undefined,
  )
  for (const [index, diagonal] of (options.grid.diagonals ?? []).entries()) {
    const projected = projectGridDiagonal(options.point, diagonal)
    if (!projected) continue
    best = considerTargetPoint(
      best, `grid-diagonal:${index}`, 'grid', projected.x, projected.y,
      options.point, options.scale, thresholdPx, undefined,
    )
  }
  return best
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
  for (const [index, diagonal] of (options.grid.diagonals ?? []).entries()) {
    const projected = projectGridDiagonal(options.point, diagonal)
    if (!projected) continue
    best = considerTargetPoint(
      best, `grid-diagonal:${index}`, 'grid', projected.x, projected.y,
      options.point, options.scale, thresholdPx, undefined, options.accept,
    )
  }
  return best
}

function projectGridDiagonal(
  point: SnapPoint,
  diagonal: Readonly<{ x1: number; y1: number; x2: number; y2: number }>,
) {
  if (![point.x, point.y, diagonal.x1, diagonal.y1, diagonal.x2, diagonal.y2]
    .every(Number.isFinite)) return null
  const dx = diagonal.x2 - diagonal.x1
  const dy = diagonal.y2 - diagonal.y1
  const denominator = dx * dx + dy * dy
  if (!Number.isFinite(denominator) || denominator <= 0) return null
  const fraction = (
    (point.x - diagonal.x1) * dx + (point.y - diagonal.y1) * dy
  ) / denominator
  if (!Number.isFinite(fraction) || fraction < 0 || fraction > 1) return null
  const x = diagonal.x1 * (1 - fraction) + diagonal.x2 * fraction
  const y = diagonal.y1 * (1 - fraction) + diagonal.y2 * fraction
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null
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
