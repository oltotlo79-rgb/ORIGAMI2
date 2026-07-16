export type IntersectionSnapPoint = Readonly<{
  x: number
  y: number
}>

export type IntersectionSnapSegment = Readonly<{
  id: string
  startVertexId: string
  endVertexId: string
  x1: number
  y1: number
  x2: number
  y2: number
}>

export type IntersectionSourceEdge = Readonly<{
  id: string
  fraction: number
}>

export type ProperIntersectionSnapTarget = Readonly<{
  kind: 'intersection'
  classification: 'proper'
  key: string
  point: IntersectionSnapPoint
  distancePx: number
  sourceEdges: readonly [IntersectionSourceEdge, IntersectionSourceEdge]
}>

export type IntersectionSnapQuery = Readonly<{
  point: IntersectionSnapPoint
  scale: number
  thresholdPx?: number
  maxPairTests?: number
  accept?: (target: ProperIntersectionSnapTarget) => boolean
}>

export type IntersectionSnapQueryResult = Readonly<{
  target: ProperIntersectionSnapTarget | null
  candidateSegmentCount: number
  testedPairCount: number
  truncated: boolean
}>

export type IntersectionSnapIndex = Readonly<{
  segmentCount: number
  query: (options: IntersectionSnapQuery) => IntersectionSnapQueryResult
}>

type Bounds = Readonly<{
  minX: number
  minY: number
  maxX: number
  maxY: number
}>

type IndexedSegment = Readonly<{
  segment: IntersectionSnapSegment
  bounds: Bounds
  centerX: number
  centerY: number
}>

type SpatialNode = Readonly<{
  bounds: Bounds
  left: SpatialNode | null
  right: SpatialNode | null
  entries: readonly IndexedSegment[] | null
}>

type RankedTarget = Readonly<{
  target: ProperIntersectionSnapTarget
  modelDistance: number
}>

const DEFAULT_THRESHOLD_PX = 8
export const DEFAULT_INTERSECTION_PAIR_LIMIT = 4096
const LEAF_SIZE = 8

export function createIntersectionSnapIndex(
  sourceSegments: readonly IntersectionSnapSegment[],
): IntersectionSnapIndex {
  const idCounts = new Map<string, number>()
  for (const segment of sourceSegments) {
    idCounts.set(segment.id, (idCounts.get(segment.id) ?? 0) + 1)
  }

  const entries = sourceSegments.flatMap((segment): IndexedSegment[] => {
    if (idCounts.get(segment.id) !== 1) return []
    const indexed = indexSegment(segment)
    return indexed ? [indexed] : []
  })
  const root = buildSpatialTree(entries)

  return Object.freeze({
    segmentCount: entries.length,
    query: (options: IntersectionSnapQuery) => queryIndex(root, options),
  })
}

function queryIndex(
  root: SpatialNode | null,
  options: IntersectionSnapQuery,
): IntersectionSnapQueryResult {
  const thresholdPx = options.thresholdPx ?? DEFAULT_THRESHOLD_PX
  const pairLimit = normalizePairLimit(options.maxPairTests)
  if (
    !root
    || !isFinitePoint(options.point)
    || !Number.isFinite(options.scale)
    || options.scale <= 0
    || !Number.isFinite(thresholdPx)
    || thresholdPx < 0
  ) return emptyQueryResult()

  const modelRadius = thresholdPx / options.scale
  if (Number.isNaN(modelRadius) || modelRadius < 0) return emptyQueryResult()
  const queryBounds: Bounds = {
    minX: options.point.x - modelRadius,
    minY: options.point.y - modelRadius,
    maxX: options.point.x + modelRadius,
    maxY: options.point.y + modelRadius,
  }
  const nearby: IndexedSegment[] = []
  collectIntersectingEntries(root, queryBounds, nearby)
  nearby.sort((left, right) => compareIds(left.segment.id, right.segment.id))

  let testedPairCount = 0
  let truncated = false
  let best: RankedTarget | null = null
  pairLoop:
  for (let leftIndex = 0; leftIndex < nearby.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < nearby.length; rightIndex += 1) {
      if (testedPairCount >= pairLimit) {
        truncated = true
        break pairLoop
      }
      testedPairCount += 1

      const first = nearby[leftIndex]
      const second = nearby[rightIndex]
      if (
        !boundsOverlap(first.bounds, second.bounds)
        || sharesVertexId(first.segment, second.segment)
      ) continue
      const intersection = properIntersection(first.segment, second.segment)
      if (!intersection) continue

      const modelDistance = Math.hypot(
        intersection.point.x - options.point.x,
        intersection.point.y - options.point.y,
      )
      const distancePx = modelDistance * options.scale
      if (
        !Number.isFinite(modelDistance)
        || !Number.isFinite(distancePx)
        || distancePx > thresholdPx
      ) continue

      const key = intersectionKey(first.segment.id, second.segment.id)
      const candidate: ProperIntersectionSnapTarget = {
        kind: 'intersection',
        classification: 'proper',
        key,
        point: intersection.point,
        distancePx,
        sourceEdges: [
          { id: first.segment.id, fraction: intersection.firstFraction },
          { id: second.segment.id, fraction: intersection.secondFraction },
        ],
      }
      if (options.accept && !options.accept(candidate)) continue
      if (
        best
        && (modelDistance > best.modelDistance
          || (modelDistance === best.modelDistance && key >= best.target.key))
      ) continue

      best = {
        modelDistance,
        target: candidate,
      }
    }
  }

  return {
    target: truncated ? null : best?.target ?? null,
    candidateSegmentCount: nearby.length,
    testedPairCount,
    truncated,
  }
}

function emptyQueryResult(): IntersectionSnapQueryResult {
  return {
    target: null,
    candidateSegmentCount: 0,
    testedPairCount: 0,
    truncated: false,
  }
}

function normalizePairLimit(value: number | undefined) {
  if (value === undefined) return DEFAULT_INTERSECTION_PAIR_LIMIT
  if (!Number.isFinite(value) || value <= 0) return 0
  return Math.floor(value)
}

function indexSegment(segment: IntersectionSnapSegment): IndexedSegment | null {
  if (
    !segment.id
    || segment.startVertexId === segment.endVertexId
    || ![segment.x1, segment.y1, segment.x2, segment.y2].every(Number.isFinite)
    || (segment.x1 === segment.x2 && segment.y1 === segment.y2)
  ) return null

  const bounds: Bounds = {
    minX: Math.min(segment.x1, segment.x2),
    minY: Math.min(segment.y1, segment.y2),
    maxX: Math.max(segment.x1, segment.x2),
    maxY: Math.max(segment.y1, segment.y2),
  }
  return {
    segment,
    bounds,
    centerX: stableAverage(bounds.minX, bounds.maxX),
    centerY: stableAverage(bounds.minY, bounds.maxY),
  }
}

function buildSpatialTree(entries: readonly IndexedSegment[]): SpatialNode | null {
  if (entries.length === 0) return null
  const bounds = unionBounds(entries)
  if (entries.length <= LEAF_SIZE) {
    return { bounds, left: null, right: null, entries: [...entries] }
  }

  const width = bounds.maxX - bounds.minX
  const height = bounds.maxY - bounds.minY
  const useX = !Number.isFinite(height) || (Number.isFinite(width) && width >= height)
  const sorted = [...entries].sort((left, right) => {
    const coordinateOrder = useX
      ? left.centerX - right.centerX
      : left.centerY - right.centerY
    return coordinateOrder || compareIds(left.segment.id, right.segment.id)
  })
  const middle = Math.floor(sorted.length / 2)
  return {
    bounds,
    left: buildSpatialTree(sorted.slice(0, middle)),
    right: buildSpatialTree(sorted.slice(middle)),
    entries: null,
  }
}

function collectIntersectingEntries(
  node: SpatialNode,
  queryBounds: Bounds,
  output: IndexedSegment[],
) {
  if (!boundsOverlap(node.bounds, queryBounds)) return
  if (node.entries) {
    for (const entry of node.entries) {
      if (boundsOverlap(entry.bounds, queryBounds)) output.push(entry)
    }
    return
  }
  if (node.left) collectIntersectingEntries(node.left, queryBounds, output)
  if (node.right) collectIntersectingEntries(node.right, queryBounds, output)
}

function unionBounds(entries: readonly IndexedSegment[]): Bounds {
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const entry of entries) {
    minX = Math.min(minX, entry.bounds.minX)
    minY = Math.min(minY, entry.bounds.minY)
    maxX = Math.max(maxX, entry.bounds.maxX)
    maxY = Math.max(maxY, entry.bounds.maxY)
  }
  return { minX, minY, maxX, maxY }
}

function boundsOverlap(first: Bounds, second: Bounds) {
  return first.minX <= second.maxX
    && second.minX <= first.maxX
    && first.minY <= second.maxY
    && second.minY <= first.maxY
}

function properIntersection(
  first: IntersectionSnapSegment,
  second: IntersectionSnapSegment,
) {
  const firstDirection = {
    x: first.x2 - first.x1,
    y: first.y2 - first.y1,
  }
  const secondDirection = {
    x: second.x2 - second.x1,
    y: second.y2 - second.y1,
  }
  const secondOffset = {
    x: second.x1 - first.x1,
    y: second.y1 - first.y1,
  }
  if (
    !isFinitePoint(firstDirection)
    || !isFinitePoint(secondDirection)
    || !isFinitePoint(secondOffset)
  ) return null

  const denominator = checkedCross(firstDirection, secondDirection)
  if (denominator === null || denominator === 0) return null
  const firstNumerator = checkedCross(secondOffset, secondDirection)
  const secondNumerator = checkedCross(secondOffset, firstDirection)
  if (firstNumerator === null || secondNumerator === null) return null
  const firstFraction = firstNumerator / denominator
  const secondFraction = secondNumerator / denominator
  if (
    !Number.isFinite(firstFraction)
    || !Number.isFinite(secondFraction)
    || firstFraction <= 0
    || firstFraction >= 1
    || secondFraction <= 0
    || secondFraction >= 1
  ) return null

  const point = {
    x: stableConvexCombination(first.x1, first.x2, firstFraction),
    y: stableConvexCombination(first.y1, first.y2, firstFraction),
  }
  return isFinitePoint(point)
    ? { point, firstFraction, secondFraction }
    : null
}

function checkedCross(first: IntersectionSnapPoint, second: IntersectionSnapPoint) {
  const value = first.x * second.y - first.y * second.x
  return Number.isFinite(value) ? value : null
}

function stableConvexCombination(start: number, end: number, fraction: number) {
  const startIsNegative = start < 0 || Object.is(start, -0)
  const endIsNegative = end < 0 || Object.is(end, -0)
  return startIsNegative === endIsNegative
    ? start + (end - start) * fraction
    : start * (1 - fraction) + end * fraction
}

function stableAverage(first: number, second: number) {
  if (first === second) return first
  const firstIsNegative = first < 0 || Object.is(first, -0)
  const secondIsNegative = second < 0 || Object.is(second, -0)
  return firstIsNegative === secondIsNegative
    ? first + (second - first) / 2
    : first / 2 + second / 2
}

function sharesVertexId(first: IntersectionSnapSegment, second: IntersectionSnapSegment) {
  return first.startVertexId === second.startVertexId
    || first.startVertexId === second.endVertexId
    || first.endVertexId === second.startVertexId
    || first.endVertexId === second.endVertexId
}

function isFinitePoint(point: IntersectionSnapPoint) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}

function compareIds(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}

function intersectionKey(firstId: string, secondId: string) {
  return `intersection:${JSON.stringify([firstId, secondId])}`
}
