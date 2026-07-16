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

export type IntersectionSnapVertex = Readonly<{
  id: string
  x: number
  y: number
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

export type TJunctionIntersectionSnapTarget = Readonly<{
  kind: 'intersection'
  classification: 't-junction'
  key: string
  point: IntersectionSnapPoint
  distancePx: number
  sourceEdges: readonly [IntersectionSourceEdge, IntersectionSourceEdge]
  junctionVertexId: string
}>

export type IntersectionSnapTarget =
  | ProperIntersectionSnapTarget
  | TJunctionIntersectionSnapTarget

export type IntersectionSnapQuery = Readonly<{
  point: IntersectionSnapPoint
  scale: number
  thresholdPx?: number
  maxPairTests?: number
  accept?: (target: IntersectionSnapTarget) => boolean
}>

export type IntersectionSnapQueryResult = Readonly<{
  target: IntersectionSnapTarget | null
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
  target: IntersectionSnapTarget
  modelDistance: number
}>

type EndpointPositionIndex = Readonly<{
  byPosition: ReadonlyMap<number, ReadonlyMap<number, string | null>>
  byVertex: ReadonlyMap<string, IntersectionSnapPoint | null>
}>

type ProperClassifiedIntersection = Readonly<{
  classification: 'proper'
  point: IntersectionSnapPoint
  firstFraction: number
  secondFraction: number
}>

type TJunctionClassifiedIntersection = Readonly<{
  classification: 't-junction'
  point: IntersectionSnapPoint
  firstFraction: number
  secondFraction: number
  junctionVertexId: string
}>

type ClassifiedIntersection =
  | ProperClassifiedIntersection
  | TJunctionClassifiedIntersection

const DEFAULT_THRESHOLD_PX = 8
export const DEFAULT_INTERSECTION_PAIR_LIMIT = 4096
const LEAF_SIZE = 8

export function createIntersectionSnapIndex(
  sourceSegments: readonly IntersectionSnapSegment[],
  sourceVertices: readonly IntersectionSnapVertex[] = [],
): IntersectionSnapIndex {
  // Passing the complete vertex snapshot also makes isolated same-position IDs
  // visible to T-junction ambiguity checks. Segment endpoints remain indexed as
  // a conservative fallback for callers that only need proper intersections.
  const endpointPositions = buildEndpointPositionIndex(sourceSegments, sourceVertices)
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
    query: (options: IntersectionSnapQuery) => queryIndex(root, endpointPositions, options),
  })
}

function queryIndex(
  root: SpatialNode | null,
  endpointPositions: EndpointPositionIndex,
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
      const intersection = classifyIntersection(
        first.segment,
        second.segment,
        endpointPositions,
      )
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
      const common = {
        kind: 'intersection',
        key,
        point: intersection.point,
        distancePx,
        sourceEdges: [
          { id: first.segment.id, fraction: intersection.firstFraction },
          { id: second.segment.id, fraction: intersection.secondFraction },
        ],
      } as const
      const candidate: IntersectionSnapTarget = intersection.classification === 'proper'
        ? { ...common, classification: 'proper' }
        : {
            ...common,
            classification: 't-junction',
            junctionVertexId: intersection.junctionVertexId,
          }
      if (options.accept && !options.accept(candidate)) continue
      if (
        best
        && (modelDistance > best.modelDistance
          || (modelDistance === best.modelDistance
            && compareIntersectionTargets(candidate, best.target) >= 0))
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
    || !segment.startVertexId
    || !segment.endVertexId
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

function classifyIntersection(
  first: IntersectionSnapSegment,
  second: IntersectionSnapSegment,
  endpointPositions: EndpointPositionIndex,
): ClassifiedIntersection | null {
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
  ) return null

  const firstIsInterior = isStrictInteriorFraction(firstFraction)
  const secondIsInterior = isStrictInteriorFraction(secondFraction)
  if (firstIsInterior && secondIsInterior) {
    const point = {
      x: stableConvexCombination(first.x1, first.x2, firstFraction),
      y: stableConvexCombination(first.y1, first.y2, firstFraction),
    }
    return isFinitePoint(point)
      ? { classification: 'proper', point, firstFraction, secondFraction }
      : null
  }

  const firstEndpointFraction = exactEndpointFraction(firstFraction)
  const secondEndpointFraction = exactEndpointFraction(secondFraction)
  if (firstEndpointFraction !== null && secondIsInterior) {
    return tJunctionIntersection(
      first,
      firstEndpointFraction,
      firstEndpointFraction,
      secondFraction,
      endpointPositions,
    )
  }
  if (secondEndpointFraction !== null && firstIsInterior) {
    return tJunctionIntersection(
      second,
      secondEndpointFraction,
      firstFraction,
      secondEndpointFraction,
      endpointPositions,
    )
  }
  return null
}

function tJunctionIntersection(
  endpointSegment: IntersectionSnapSegment,
  endpointFraction: 0 | 1,
  firstFraction: number,
  secondFraction: number,
  endpointPositions: EndpointPositionIndex,
): ClassifiedIntersection | null {
  const atStart = endpointFraction === 0
  const point = {
    x: atStart ? endpointSegment.x1 : endpointSegment.x2,
    y: atStart ? endpointSegment.y1 : endpointSegment.y2,
  }
  const junctionVertexId = atStart
    ? endpointSegment.startVertexId
    : endpointSegment.endVertexId
  if (
    !junctionVertexId
    || !isFinitePoint(point)
    || !isUnambiguousEndpoint(endpointPositions, point, junctionVertexId)
  ) return null
  return {
    classification: 't-junction',
    point,
    firstFraction,
    secondFraction,
    junctionVertexId,
  }
}

function isStrictInteriorFraction(fraction: number) {
  return fraction > 0 && fraction < 1
}

function exactEndpointFraction(fraction: number): 0 | 1 | null {
  if (fraction === 0) return 0
  if (fraction === 1) return 1
  return null
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

function compareIntersectionTargets(
  first: IntersectionSnapTarget,
  second: IntersectionSnapTarget,
) {
  const classificationOrder = intersectionClassificationOrder(first)
    - intersectionClassificationOrder(second)
  return classificationOrder || compareIds(first.key, second.key)
}

function intersectionClassificationOrder(target: IntersectionSnapTarget) {
  return target.classification === 't-junction' ? 0 : 1
}

function intersectionKey(firstId: string, secondId: string) {
  return `intersection:${JSON.stringify([firstId, secondId])}`
}

function buildEndpointPositionIndex(
  segments: readonly IntersectionSnapSegment[],
  vertices: readonly IntersectionSnapVertex[],
): EndpointPositionIndex {
  const byPosition = new Map<number, Map<number, string | null>>()
  const byVertex = new Map<string, IntersectionSnapPoint | null>()
  for (const vertex of vertices) {
    registerEndpoint(byPosition, byVertex, vertex.id, vertex.x, vertex.y)
  }
  for (const segment of segments) {
    registerEndpoint(byPosition, byVertex, segment.startVertexId, segment.x1, segment.y1)
    registerEndpoint(byPosition, byVertex, segment.endVertexId, segment.x2, segment.y2)
  }
  return { byPosition, byVertex }
}

function registerEndpoint(
  byPosition: Map<number, Map<number, string | null>>,
  byVertex: Map<string, IntersectionSnapPoint | null>,
  vertexId: string,
  x: number,
  y: number,
) {
  if (!vertexId || !Number.isFinite(x) || !Number.isFinite(y)) return

  let byY = byPosition.get(x)
  if (!byY) {
    byY = new Map()
    byPosition.set(x, byY)
  }
  if (!byY.has(y)) byY.set(y, vertexId)
  else if (byY.get(y) !== vertexId) byY.set(y, null)

  if (!byVertex.has(vertexId)) {
    byVertex.set(vertexId, { x, y })
    return
  }
  const current = byVertex.get(vertexId)
  if (!current || current.x !== x || current.y !== y) byVertex.set(vertexId, null)
}

function isUnambiguousEndpoint(
  index: EndpointPositionIndex,
  point: IntersectionSnapPoint,
  vertexId: string,
) {
  const positionVertex = index.byPosition.get(point.x)?.get(point.y)
  const vertexPosition = index.byVertex.get(vertexId)
  return positionVertex === vertexId
    && vertexPosition !== null
    && vertexPosition !== undefined
    && vertexPosition.x === point.x
    && vertexPosition.y === point.y
}
