import {
  clusterIntersectionPointSearchRadius,
  clusterIntersectionPointsAreClose,
  clusterPointLiesOnSegment,
} from './intersectionClusterNumerics.ts'

export type IntersectionSnapPoint = Readonly<{
  x: number
  y: number
}>

export type IntersectionEdgeKind =
  | 'mountain'
  | 'valley'
  | 'auxiliary'
  | 'boundary'
  | 'cut'

export type IntersectionSnapSegment = Readonly<{
  id: string
  startVertexId: string
  endVertexId: string
  x1: number
  y1: number
  x2: number
  y2: number
  kind?: IntersectionEdgeKind
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

export type ClusterIntersectionSourceEdge = Readonly<{
  id: string
  fraction: number
  relation: 'interior'
  kind?: IntersectionEdgeKind
}> | Readonly<{
  id: string
  fraction: 0 | 1
  relation: 'endpoint'
  endpointVertexId: string
  kind?: IntersectionEdgeKind
}>

export type ClusterIntersectionSnapTarget = Readonly<{
  kind: 'intersection'
  classification: 'cluster'
  key: string
  point: IntersectionSnapPoint
  distancePx: number
  sourceEdges: readonly [
    ClusterIntersectionSourceEdge,
    ClusterIntersectionSourceEdge,
    ClusterIntersectionSourceEdge,
    ...ClusterIntersectionSourceEdge[],
  ]
  junctionVertexId?: string
}>

export type IntersectionSnapTarget =
  | ProperIntersectionSnapTarget
  | TJunctionIntersectionSnapTarget
  | ClusterIntersectionSnapTarget

export type IntersectionSnapQuery = Readonly<{
  point: IntersectionSnapPoint
  scale: number
  thresholdPx?: number
  maxPairTests?: number
  maxClusterTests?: number
  accept?: (target: IntersectionSnapTarget) => boolean
}>

export type IntersectionSnapQueryResult = Readonly<{
  target: IntersectionSnapTarget | null
  candidateSegmentCount: number
  testedPairCount: number
  truncated: boolean
  blocked: boolean
  blockedDistancePx: number | null
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

type IntersectionSelection = Readonly<{
  best: RankedTarget | null
  blocked: boolean
  nearestBlockedDistance: number | null
  truncated: boolean
}>

type PairCandidate = Readonly<{
  target: ProperIntersectionSnapTarget | TJunctionIntersectionSnapTarget
  modelDistance: number
  first: IndexedSegment
  second: IndexedSegment
}>

type ClusterExpansion =
  | Readonly<{ status: 'two-edge' }>
  | Readonly<{ status: 'blocked' }>
  | Readonly<{ status: 'budget-exhausted' }>
  | Readonly<{
      status: 'cluster'
      point: IntersectionSnapPoint
      sourceEdges: readonly [
        ClusterIntersectionSourceEdge,
        ClusterIntersectionSourceEdge,
        ClusterIntersectionSourceEdge,
        ...ClusterIntersectionSourceEdge[],
      ]
      junctionVertexId?: string
    }>

type SeedMembership = Readonly<{
  status: 'member'
  fraction: number
}> | Readonly<{ status: 'outside' }>
  | Readonly<{ status: 'ambiguous' }>

type EndpointPositionIndex = Readonly<{
  byPosition: ReadonlyMap<number, ReadonlyMap<number, string | null>>
  byVertex: ReadonlyMap<string, IntersectionSnapPoint | null>
  positionsByX: readonly IntersectionSnapPoint[]
}>

type CanonicalClusterPointResolution =
  | Readonly<{
      status: 'ready'
      point: IntersectionSnapPoint
      junctionVertexId?: string
    }>
  | Readonly<{ status: 'blocked' }>
  | Readonly<{ status: 'budget-exhausted' }>

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
export const DEFAULT_INTERSECTION_CLUSTER_TEST_LIMIT = 65_536
const LEAF_SIZE = 8

type ClusterWorkBudget = {
  tested: number
  limit: number
}

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
  const duplicateEntries = sourceSegments.flatMap((segment): IndexedSegment[] => {
    if (idCounts.get(segment.id) === 1) return []
    const indexed = indexSegment(segment)
    return indexed ? [indexed] : []
  })
  const root = buildSpatialTree(entries)
  const duplicateRoot = buildSpatialTree(duplicateEntries)

  return Object.freeze({
    segmentCount: entries.length,
    query: (options: IntersectionSnapQuery) => queryIndex(
      root,
      duplicateRoot,
      endpointPositions,
      options,
    ),
  })
}

function queryIndex(
  root: SpatialNode | null,
  duplicateRoot: SpatialNode | null,
  endpointPositions: EndpointPositionIndex,
  options: IntersectionSnapQuery,
): IntersectionSnapQueryResult {
  const thresholdPx = options.thresholdPx ?? DEFAULT_THRESHOLD_PX
  const pairLimit = normalizePairLimit(options.maxPairTests)
  const clusterLimit = normalizeClusterLimit(options.maxClusterTests)
  if (
    (!root && !duplicateRoot)
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
  if (root) collectIntersectingEntries(root, queryBounds, nearby)
  nearby.sort((left, right) => compareIds(left.segment.id, right.segment.id))

  let testedPairCount = 0
  let truncated = false
  const pairCandidates: PairCandidate[] = []
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
      const candidate: ProperIntersectionSnapTarget | TJunctionIntersectionSnapTarget =
        intersection.classification === 'proper'
        ? { ...common, classification: 'proper' }
        : {
            ...common,
            classification: 't-junction',
            junctionVertexId: intersection.junctionVertexId,
          }
      pairCandidates.push({
        modelDistance,
        target: candidate,
        first,
        second,
      })
    }
  }

  let selection: IntersectionSelection = {
    best: null,
    blocked: false,
    nearestBlockedDistance: null,
    truncated: false,
  }
  if (!truncated) {
    const duplicateNearby: IndexedSegment[] = []
    if (duplicateRoot) {
      collectIntersectingEntries(duplicateRoot, queryBounds, duplicateNearby)
    }
    selection = pairCandidates.length === 0 && duplicateNearby.length > 0
      ? inspectDuplicateOnlyContamination(
          duplicateNearby,
          options.point,
          modelRadius,
          clusterLimit,
        )
      : selectBestIntersectionTarget(
          pairCandidates,
          nearby,
          duplicateNearby,
          endpointPositions,
          options.accept,
          clusterLimit,
          options.point,
          options.scale,
          thresholdPx,
        )
  }
  truncated ||= selection.truncated

  return {
    target: truncated ? null : selection.best?.target ?? null,
    candidateSegmentCount: nearby.length,
    testedPairCount,
    truncated,
    blocked: selection.blocked,
    blockedDistancePx: selection.blocked && selection.nearestBlockedDistance !== null
      ? selection.nearestBlockedDistance * options.scale
      : null,
  }
}

function inspectDuplicateOnlyContamination(
  duplicates: readonly IndexedSegment[],
  point: IntersectionSnapPoint,
  modelRadius: number,
  clusterLimit: number,
): IntersectionSelection {
  if (duplicates.length > clusterLimit) {
    return {
      best: null,
      blocked: false,
      nearestBlockedDistance: null,
      truncated: true,
    }
  }
  let nearestDistance: number | null = null
  for (const duplicate of duplicates) {
    const distance = distanceToSegment(point, duplicate.segment)
    if (distance === null) {
      return {
        best: null,
        blocked: true,
        nearestBlockedDistance: 0,
        truncated: false,
      }
    }
    if (nearestDistance === null || distance < nearestDistance) {
      nearestDistance = distance
    }
  }
  return {
    best: null,
    blocked: nearestDistance !== null && nearestDistance <= modelRadius,
    nearestBlockedDistance: nearestDistance,
    truncated: false,
  }
}

function distanceToSegment(
  point: IntersectionSnapPoint,
  segment: IntersectionSnapSegment,
) {
  const dx = segment.x2 - segment.x1
  const dy = segment.y2 - segment.y1
  const offsetX = point.x - segment.x1
  const offsetY = point.y - segment.y1
  const lengthSquared = dx * dx + dy * dy
  const numerator = offsetX * dx + offsetY * dy
  if (
    !Number.isFinite(lengthSquared)
    || lengthSquared <= 0
    || !Number.isFinite(numerator)
  ) return null
  const fraction = Math.max(0, Math.min(1, numerator / lengthSquared))
  const projectionX = segment.x1 + fraction * dx
  const projectionY = segment.y1 + fraction * dy
  const distance = Math.hypot(point.x - projectionX, point.y - projectionY)
  return Number.isFinite(distance) ? distance : null
}

function selectBestIntersectionTarget(
  pairCandidates: readonly PairCandidate[],
  nearby: readonly IndexedSegment[],
  duplicateNearby: readonly IndexedSegment[],
  endpointPositions: EndpointPositionIndex,
  accept: IntersectionSnapQuery['accept'],
  clusterLimit: number,
  queryPoint: IntersectionSnapPoint,
  scale: number,
  thresholdPx: number,
): IntersectionSelection {
  const expansionsByX = new Map<number, Map<number, ClusterExpansion>>()
  const expansionsByPairKey = new Map<string, ClusterExpansion>()
  const emittedClusterKeys = new Set<string>()
  let best: RankedTarget | null = null
  let nearestBlockedDistance: number | null = null
  const budget: ClusterWorkBudget = { tested: 0, limit: clusterLimit }

  for (const pair of pairCandidates) {
    const point = pair.target.point
    let expansion = expansionsByPairKey.get(pair.target.key)
    if (!expansion) {
      let expansionsByY = expansionsByX.get(point.x)
      if (!expansionsByY) {
        expansionsByY = new Map()
        expansionsByX.set(point.x, expansionsByY)
      }
      expansion = expansionsByY.get(point.y)
      if (!expansion) {
        expansion = expandIntersectionCluster(
          pair,
          nearby,
          duplicateNearby,
          endpointPositions,
          budget,
        )
        expansionsByY.set(point.y, expansion)
      }
      if (expansion.status === 'cluster') {
        registerClusterPairExpansions(expansionsByPairKey, expansion)
      } else {
        expansionsByPairKey.set(pair.target.key, expansion)
      }
    }

    if (expansion.status === 'budget-exhausted') {
      return {
        best: null,
        blocked: false,
        nearestBlockedDistance: null,
        truncated: true,
      }
    }
    if (expansion.status === 'blocked') {
      if (
        (!accept || accept(pair.target))
        && (
          nearestBlockedDistance === null
          || pair.modelDistance < nearestBlockedDistance
        )
      ) nearestBlockedDistance = pair.modelDistance
      continue
    }
    let candidate: IntersectionSnapTarget = pair.target
    let candidateModelDistance = pair.modelDistance
    if (expansion.status === 'cluster') {
      candidateModelDistance = Math.hypot(
        expansion.point.x - queryPoint.x,
        expansion.point.y - queryPoint.y,
      )
      const distancePx = candidateModelDistance * scale
      const key = intersectionKey(...expansion.sourceEdges.map(({ id }) => id))
      candidate = {
        kind: 'intersection',
        classification: 'cluster',
        key,
        point: expansion.point,
        distancePx,
        sourceEdges: expansion.sourceEdges,
        ...(expansion.junctionVertexId
          ? { junctionVertexId: expansion.junctionVertexId }
          : {}),
      }
      if (
        !Number.isFinite(candidateModelDistance)
        || !Number.isFinite(distancePx)
        || distancePx > thresholdPx
      ) {
        if (
          (!accept || accept(candidate))
          && (
            nearestBlockedDistance === null
            || pair.modelDistance < nearestBlockedDistance
          )
        ) nearestBlockedDistance = pair.modelDistance
        continue
      }
      if (emittedClusterKeys.has(key)) continue
      emittedClusterKeys.add(key)
    }

    if (accept && !accept(candidate)) continue
    if (
      best
      && (candidateModelDistance > best.modelDistance
        || (candidateModelDistance === best.modelDistance
          && compareIntersectionTargets(candidate, best.target) >= 0))
    ) continue
    best = { target: candidate, modelDistance: candidateModelDistance }
  }

  return {
    best,
    blocked: nearestBlockedDistance !== null
      && (!best || nearestBlockedDistance <= best.modelDistance),
    nearestBlockedDistance,
    truncated: false,
  }
}

function registerClusterPairExpansions(
  expansions: Map<string, ClusterExpansion>,
  expansion: Extract<ClusterExpansion, Readonly<{ status: 'cluster' }>>,
) {
  for (let leftIndex = 0; leftIndex < expansion.sourceEdges.length; leftIndex += 1) {
    for (
      let rightIndex = leftIndex + 1;
      rightIndex < expansion.sourceEdges.length;
      rightIndex += 1
    ) {
      expansions.set(intersectionKey(
        expansion.sourceEdges[leftIndex].id,
        expansion.sourceEdges[rightIndex].id,
      ), expansion)
    }
  }
}

function expandIntersectionCluster(
  seed: PairCandidate,
  nearby: readonly IndexedSegment[],
  duplicateNearby: readonly IndexedSegment[],
  endpointPositions: EndpointPositionIndex,
  budget: ClusterWorkBudget,
): ClusterExpansion {
  const canonicalPoint = resolveCanonicalClusterPoint(
    seed.target.point,
    seed.first.segment,
    seed.second.segment,
    endpointPositions,
    budget,
  )
  if (canonicalPoint.status !== 'ready') return canonicalPoint
  const { point } = canonicalPoint
  const seedFractions = new Map<string, number>(seed.target.sourceEdges.map(
    ({ id, fraction }) => [id, fraction],
  ))
  const members: Array<Readonly<{
    segment: IntersectionSnapSegment
    sourceEdge: ClusterIntersectionSourceEdge
  }>> = []

  for (const entry of nearby) {
    if (!consumeClusterTest(budget)) return { status: 'budget-exhausted' }
    const seedFraction = seedFractions.get(entry.segment.id)
    const canonicalEndpointFraction = endpointFractionAtPoint(entry.segment, point)
    const membership: SeedMembership = canonicalEndpointFraction !== null
      ? { status: 'member', fraction: canonicalEndpointFraction }
      : seedFraction === undefined
        ? membershipAtSeedIntersection(
            entry.segment,
            seed.first.segment,
            seed.second.segment,
            point,
          )
        : { status: 'member', fraction: seedFraction }
    if (membership.status === 'ambiguous') return { status: 'blocked' }
    if (membership.status === 'outside') continue
    const { fraction } = membership
    const sourceEdge = clusterSourceEdge(entry.segment, fraction, point)
    if (!sourceEdge) return { status: 'blocked' }
    members.push({ segment: entry.segment, sourceEdge })
  }

  for (const duplicate of duplicateNearby) {
    if (!consumeClusterTest(budget)) return { status: 'budget-exhausted' }
    const membership = membershipAtSeedIntersection(
      duplicate.segment,
      seed.first.segment,
      seed.second.segment,
      point,
    )
    if (membership.status !== 'outside') return { status: 'blocked' }
  }
  if (members.length < 3) return { status: 'two-edge' }
  if (
    members.length > 64
    || members.some(({ segment }) => segment.kind === 'boundary')
  ) return { status: 'blocked' }
  for (let leftIndex = 0; leftIndex < members.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < members.length; rightIndex += 1) {
      if (!consumeClusterTest(budget)) return { status: 'budget-exhausted' }
      const overlap = hasPositiveCollinearOverlap(
        members[leftIndex].segment,
        members[rightIndex].segment,
      )
      if (
        overlap === null
        || overlap
        || !segmentsMeetAtClusterPoint(
          members[leftIndex].segment,
          members[rightIndex].segment,
          point,
        )
      ) return { status: 'blocked' }
    }
  }

  members.sort((left, right) => compareIds(left.segment.id, right.segment.id))
  const endpointIds = new Set(members.flatMap(({ sourceEdge }) =>
    sourceEdge.relation === 'endpoint' ? [sourceEdge.endpointVertexId] : []))
  if (endpointIds.size > 1) return { status: 'blocked' }

  let junctionVertexId = canonicalPoint.junctionVertexId
  if (endpointIds.size === 1) {
    const endpointJunctionVertexId = endpointIds.values().next().value
    if (
      !endpointJunctionVertexId
      || (junctionVertexId !== undefined
        && junctionVertexId !== endpointJunctionVertexId)
      || !isUnambiguousEndpoint(
        endpointPositions,
        point,
        endpointJunctionVertexId,
      )
    ) return { status: 'blocked' }
    junctionVertexId = endpointJunctionVertexId
  } else if (junctionVertexId === undefined) {
    const positionVertex = endpointPositions.byPosition.get(point.x)?.get(point.y)
    if (positionVertex === null) return { status: 'blocked' }
    if (positionVertex !== undefined) {
      if (!isUnambiguousEndpoint(endpointPositions, point, positionVertex)) {
        return { status: 'blocked' }
      }
      junctionVertexId = positionVertex
    }
  }

  const sourceEdges = members.map(({ sourceEdge }) => sourceEdge)
  if (sourceEdges.length < 3) return { status: 'two-edge' }
  return {
    status: 'cluster',
    point,
    sourceEdges: sourceEdges as [
      ClusterIntersectionSourceEdge,
      ClusterIntersectionSourceEdge,
      ClusterIntersectionSourceEdge,
      ...ClusterIntersectionSourceEdge[],
    ],
    ...(junctionVertexId ? { junctionVertexId } : {}),
  }
}

function resolveCanonicalClusterPoint(
  seedPoint: IntersectionSnapPoint,
  firstSeed: IntersectionSnapSegment,
  secondSeed: IntersectionSnapSegment,
  endpointPositions: EndpointPositionIndex,
  budget: ClusterWorkBudget,
): CanonicalClusterPointResolution {
  const radius = clusterIntersectionPointSearchRadius(
    seedPoint,
    firstSeed,
    secondSeed,
  )
  if (!radius) return { status: 'blocked' }
  const rawMinX = seedPoint.x - radius.x
  const rawMaxX = seedPoint.x + radius.x
  const minX = Number.isFinite(rawMinX) ? rawMinX : -Number.MAX_VALUE
  const maxX = Number.isFinite(rawMaxX) ? rawMaxX : Number.MAX_VALUE
  const positions = endpointPositions.positionsByX
  let index = lowerBoundPositionX(positions, minX)
  let match: Readonly<{
    point: IntersectionSnapPoint
    vertexId: string
  }> | null = null
  let ambiguous = false

  while (index < positions.length && positions[index].x <= maxX) {
    if (!consumeClusterTest(budget)) return { status: 'budget-exhausted' }
    const candidate = positions[index]
    index += 1
    if (
      Math.abs(candidate.y - seedPoint.y) > radius.y
      || !clusterIntersectionPointsAreClose(
        candidate,
        seedPoint,
        firstSeed,
        secondSeed,
      )
      || !clusterPointLiesOnSegment(candidate, firstSeed)
      || !clusterPointLiesOnSegment(candidate, secondSeed)
    ) continue

    const vertexId = endpointPositions.byPosition
      .get(candidate.x)
      ?.get(candidate.y)
    if (
      !vertexId
      || !isUnambiguousEndpoint(endpointPositions, candidate, vertexId)
    ) {
      ambiguous = true
      continue
    }
    if (
      match
      && (
        match.vertexId !== vertexId
        || match.point.x !== candidate.x
        || match.point.y !== candidate.y
      )
    ) {
      ambiguous = true
      continue
    }
    match = { point: candidate, vertexId }
  }

  if (ambiguous) return { status: 'blocked' }
  return match
    ? {
        status: 'ready',
        point: match.point,
        junctionVertexId: match.vertexId,
      }
    : { status: 'ready', point: seedPoint }
}

function lowerBoundPositionX(
  positions: readonly IntersectionSnapPoint[],
  minimumX: number,
) {
  let low = 0
  let high = positions.length
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2)
    if (positions[middle].x < minimumX) low = middle + 1
    else high = middle
  }
  return low
}

function membershipAtSeedIntersection(
  candidate: IntersectionSnapSegment,
  firstSeed: IntersectionSnapSegment,
  secondSeed: IntersectionSnapSegment,
  point: IntersectionSnapPoint,
): SeedMembership {
  const endpointFraction = endpointFractionAtPoint(candidate, point)
  if (endpointFraction !== null) {
    return { status: 'member', fraction: endpointFraction }
  }
  const candidateContainsPoint = clusterPointLiesOnSegment(point, candidate)
  let ambiguous = false
  for (const seed of [firstSeed, secondSeed]) {
    if (candidate.id === seed.id) continue
    const intersection = singlePointIntersection(seed, candidate)
    if (!intersection) continue
    if (
      candidateContainsPoint
      && clusterPointLiesOnSegment(point, seed)
    ) return { status: 'member', fraction: intersection.secondFraction }
    if (clusterIntersectionPointsAreClose(
      intersection.point,
      point,
      seed,
      candidate,
    )) ambiguous = true
  }
  return {
    status: ambiguous || candidateContainsPoint ? 'ambiguous' : 'outside',
  }
}

function singlePointIntersection(
  first: IntersectionSnapSegment,
  second: IntersectionSnapSegment,
): Readonly<{
  point: IntersectionSnapPoint
  firstFraction: number
  secondFraction: number
}> | null {
  const firstDirection = { x: first.x2 - first.x1, y: first.y2 - first.y1 }
  const secondDirection = { x: second.x2 - second.x1, y: second.y2 - second.y1 }
  const secondOffset = { x: second.x1 - first.x1, y: second.y1 - first.y1 }
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
    !isSegmentFraction(firstFraction)
    || !isSegmentFraction(secondFraction)
  ) return null
  const point = {
    x: stableConvexCombination(first.x1, first.x2, firstFraction),
    y: stableConvexCombination(first.y1, first.y2, firstFraction),
  }
  return isFinitePoint(point)
    ? { point, firstFraction, secondFraction }
    : null
}

function clusterSourceEdge(
  segment: IntersectionSnapSegment,
  fraction: number,
  point: IntersectionSnapPoint,
): ClusterIntersectionSourceEdge | null {
  const pointEndpointFraction = endpointFractionAtPoint(segment, point)
  if (pointEndpointFraction !== null) {
    const atStart = pointEndpointFraction === 0
    const endpointVertexId = atStart
      ? segment.startVertexId
      : segment.endVertexId
    if (!endpointVertexId) return null
    return {
      id: segment.id,
      fraction: pointEndpointFraction,
      relation: 'endpoint',
      endpointVertexId,
      ...(segment.kind ? { kind: segment.kind } : {}),
    }
  }
  if (isStrictInteriorFraction(fraction)) {
    if (!clusterPointLiesOnSegment(point, segment)) return null
    return {
      id: segment.id,
      fraction,
      relation: 'interior',
      ...(segment.kind ? { kind: segment.kind } : {}),
    }
  }
  const endpointFraction = exactEndpointFraction(fraction)
  if (endpointFraction === null) return null
  const atStart = endpointFraction === 0
  const endpointPoint = {
    x: atStart ? segment.x1 : segment.x2,
    y: atStart ? segment.y1 : segment.y2,
  }
  const endpointVertexId = atStart
    ? segment.startVertexId
    : segment.endVertexId
  if (!endpointVertexId || !pointsAreEqual(endpointPoint, point)) return null
  return {
    id: segment.id,
    fraction: endpointFraction,
    relation: 'endpoint',
    endpointVertexId,
    ...(segment.kind ? { kind: segment.kind } : {}),
  }
}

function endpointFractionAtPoint(
  segment: IntersectionSnapSegment,
  point: IntersectionSnapPoint,
): 0 | 1 | null {
  if (segment.x1 === point.x && segment.y1 === point.y) return 0
  if (segment.x2 === point.x && segment.y2 === point.y) return 1
  return null
}

function isSegmentFraction(fraction: number) {
  return Number.isFinite(fraction)
    && (isStrictInteriorFraction(fraction) || exactEndpointFraction(fraction) !== null)
}

function pointsAreEqual(first: IntersectionSnapPoint, second: IntersectionSnapPoint) {
  return first.x === second.x && first.y === second.y
}

function hasPositiveCollinearOverlap(
  first: IntersectionSnapSegment,
  second: IntersectionSnapSegment,
): boolean | null {
  const firstDirection = { x: first.x2 - first.x1, y: first.y2 - first.y1 }
  const secondDirection = { x: second.x2 - second.x1, y: second.y2 - second.y1 }
  const offset = { x: second.x1 - first.x1, y: second.y1 - first.y1 }
  const directionCross = checkedCross(firstDirection, secondDirection)
  const offsetCross = checkedCross(firstDirection, offset)
  if (directionCross === null || offsetCross === null) return null
  if (directionCross !== 0 || offsetCross !== 0) return false

  const useX = Math.abs(firstDirection.x) >= Math.abs(firstDirection.y)
  const firstStart = useX ? first.x1 : first.y1
  const firstEnd = useX ? first.x2 : first.y2
  const secondStart = useX ? second.x1 : second.y1
  const secondEnd = useX ? second.x2 : second.y2
  const overlapStart = Math.max(
    Math.min(firstStart, firstEnd),
    Math.min(secondStart, secondEnd),
  )
  const overlapEnd = Math.min(
    Math.max(firstStart, firstEnd),
    Math.max(secondStart, secondEnd),
  )
  return overlapStart < overlapEnd
}

function segmentsMeetAtClusterPoint(
  first: IntersectionSnapSegment,
  second: IntersectionSnapSegment,
  point: IntersectionSnapPoint,
) {
  const intersection = singlePointIntersection(first, second)
  if (intersection) {
    return clusterPointLiesOnSegment(point, first)
      && clusterPointLiesOnSegment(point, second)
  }
  return segmentHasExactEndpoint(first, point)
    && segmentHasExactEndpoint(second, point)
}

function segmentHasExactEndpoint(
  segment: IntersectionSnapSegment,
  point: IntersectionSnapPoint,
) {
  return (segment.x1 === point.x && segment.y1 === point.y)
    || (segment.x2 === point.x && segment.y2 === point.y)
}

function emptyQueryResult(): IntersectionSnapQueryResult {
  return {
    target: null,
    candidateSegmentCount: 0,
    testedPairCount: 0,
    truncated: false,
    blocked: false,
    blockedDistancePx: null,
  }
}

function normalizePairLimit(value: number | undefined) {
  if (value === undefined) return DEFAULT_INTERSECTION_PAIR_LIMIT
  if (!Number.isFinite(value) || value <= 0) return 0
  return Math.floor(value)
}

function normalizeClusterLimit(value: number | undefined) {
  if (value === undefined) return DEFAULT_INTERSECTION_CLUSTER_TEST_LIMIT
  if (!Number.isFinite(value) || value <= 0) return 0
  return Math.floor(value)
}

function consumeClusterTest(budget: ClusterWorkBudget) {
  if (budget.tested >= budget.limit) return false
  budget.tested += 1
  return true
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
  if (target.classification === 't-junction') return 0
  if (target.classification === 'cluster') {
    return target.junctionVertexId ? 0 : 1
  }
  return 1
}

function intersectionKey(...edgeIds: readonly string[]) {
  return `intersection:${JSON.stringify(edgeIds)}`
}

function buildEndpointPositionIndex(
  segments: readonly IntersectionSnapSegment[],
  vertices: readonly IntersectionSnapVertex[],
): EndpointPositionIndex {
  const byPosition = new Map<number, Map<number, string | null>>()
  const byVertex = new Map<string, IntersectionSnapPoint | null>()
  const vertexRecordCounts = new Map<string, number>()
  for (const vertex of vertices) {
    vertexRecordCounts.set(vertex.id, (vertexRecordCounts.get(vertex.id) ?? 0) + 1)
    registerEndpoint(byPosition, byVertex, vertex.id, vertex.x, vertex.y)
  }
  for (const segment of segments) {
    registerEndpoint(byPosition, byVertex, segment.startVertexId, segment.x1, segment.y1)
    registerEndpoint(byPosition, byVertex, segment.endVertexId, segment.x2, segment.y2)
  }
  for (const [vertexId, count] of vertexRecordCounts) {
    if (vertexId && count > 1) byVertex.set(vertexId, null)
  }
  const positionsByX = [...byPosition].flatMap(([x, byY]) =>
    [...byY.keys()].map((y) => ({ x, y })))
  positionsByX.sort((first, second) =>
    first.x - second.x || first.y - second.y)
  return { byPosition, byVertex, positionsByX }
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
