import type { AdditionSnapTarget, SnapPoint, SnapSegment, SnapVertex } from './snap'
import { clusterPointLiesOnSegment } from './intersectionClusterNumerics.ts'

export type VertexPlacement = Readonly<{
  operation: 'add'
  x: number
  y: number
}> | Readonly<{
  operation: 'split-edge'
  edgeId: string
  fraction: number
}> | Readonly<{
  operation: 'connect-intersection'
  firstEdgeId: string
  secondEdgeId: string
}> | Readonly<{
  operation: 'connect-t-junction'
  firstEdgeId: string
  secondEdgeId: string
  junctionVertexId: string
}> | Readonly<{
  operation: 'connect-intersection-cluster'
  targets: readonly Readonly<{
    edgeId: string
    relation: 'interior' | 'endpoint'
  }>[]
  junctionVertexId?: string
}>

type IntersectionConnectionPlacement = Extract<
  VertexPlacement,
  {
    operation:
      | 'connect-intersection'
      | 'connect-t-junction'
      | 'connect-intersection-cluster'
  }
>

type PersistedIntersectionEdge = Readonly<{
  id: string
  start: string
  end: string
  kind: string
}>

export function createVertexPlacement(
  point: SnapPoint,
  target: AdditionSnapTarget | null,
  segments: readonly SnapSegment[],
  vertices: readonly SnapVertex[] = [],
): VertexPlacement | null {
  if (target?.kind === 'intersection') {
    return intersectionPlacement(point, target, segments, vertices)
  }
  if (
    target?.kind === 'horizontal'
    || target?.kind === 'vertical'
    || target?.kind === 'parallel'
    || target?.kind === 'angle'
  ) {
    return constrainedPointPlacement(point, target, segments)
  }
  if (target?.kind !== 'edge' && target?.kind !== 'midpoint') {
    return finitePointPlacement(point)
  }

  if (!target.sourceId) return null
  const segment = segments.find(({ id }) => id === target.sourceId)
  if (!segment) return null

  const fraction = target.sourceFraction
  if (
    fraction === undefined
    || !Number.isFinite(fraction)
    || fraction <= 0
    || fraction >= 1
  ) return null

  return {
    operation: 'split-edge',
    edgeId: segment.id,
    fraction,
  }
}

function constrainedPointPlacement(
  point: SnapPoint,
  target: Extract<
    AdditionSnapTarget,
    { kind: 'horizontal' | 'vertical' | 'parallel' | 'angle' }
  >,
  segments: readonly SnapSegment[],
): VertexPlacement | null {
  if (
    !Number.isFinite(point.x)
    || !Number.isFinite(point.y)
    || point.x !== target.point.x
    || point.y !== target.point.y
    || !Number.isFinite(target.distancePx)
    || target.distancePx < 0
  ) return null

  if (target.kind === 'parallel') {
    const reference = validatedParallelReference(point, target, segments)
    if (!reference) return null
    return splitOrAddPoint(point, segments, {
      anchorPoint: target.anchorPoint,
      direction: reference.direction,
    })
  } else if (target.kind === 'angle') {
    const direction = validatedAngleDirection(point, target, segments)
    if (!direction) return null
    return splitOrAddPoint(point, segments, {
      anchorPoint: target.anchorPoint,
      direction,
    })
  } else if (
    typeof target.anchorId !== 'string'
    || target.anchorId.length === 0
    || target.sourceId !== target.anchorId
    || target.key !== `${target.kind}:${JSON.stringify(target.anchorId)}`
    || !Number.isFinite(target.anchorPoint.x)
    || !Number.isFinite(target.anchorPoint.y)
    || (target.kind === 'horizontal' && point.y !== target.anchorPoint.y)
    || (target.kind === 'vertical' && point.x !== target.anchorPoint.x)
  ) return null

  return splitOrAddPoint(point, segments)
}

function splitOrAddPoint(
  point: SnapPoint,
  segments: readonly SnapSegment[],
  knownConstrainedLine?: Readonly<{
    anchorPoint: SnapPoint
    direction: SnapPoint
  }>,
): VertexPlacement | null {
  let split: Extract<VertexPlacement, { operation: 'split-edge' }> | null = null
  for (const segment of segments) {
    if (
      (point.x === segment.x1 && point.y === segment.y1)
      || (point.x === segment.x2 && point.y === segment.y2)
    ) return null

    const fraction = knownConstrainedLine
      && segmentSharesKnownLine(segment, knownConstrainedLine)
      ? strictSegmentFractionOnKnownLine(segment, point)
      : strictSegmentFractionAtPoint(segment, point)
    if (fraction === null) continue
    if (
      split
      || segments.filter(({ id }) => id === segment.id).length !== 1
    ) return null
    split = {
      operation: 'split-edge',
      edgeId: segment.id,
      fraction,
    }
  }
  return split ?? finitePointPlacement(point)
}

function validatedParallelReference(
  point: SnapPoint,
  target: Extract<AdditionSnapTarget, { kind: 'parallel' }>,
  segments: readonly SnapSegment[],
) {
  if (
    typeof target.anchorId !== 'string'
    || target.anchorId.length === 0
    || !Number.isFinite(target.anchorPoint.x)
    || !Number.isFinite(target.anchorPoint.y)
    || typeof target.referenceEdgeId !== 'string'
    || target.referenceEdgeId.length === 0
    || target.sourceId !== target.referenceEdgeId
    || target.key !== parallelKey(target.anchorId, target.referenceEdgeId)
    || !isFinitePoint(target.referenceStartPoint)
    || !isFinitePoint(target.referenceEndPoint)
    || comparePoints(target.referenceStartPoint, target.referenceEndPoint) >= 0
  ) return null

  const referenceMatches = segments.filter(({ id }) => id === target.referenceEdgeId)
  if (referenceMatches.length !== 1) return null
  const reference = referenceMatches[0]
  if (!isValidSegment(reference)) return null
  const endpoints = canonicalSegmentEndpoints(reference)
  if (
    !samePoint(target.referenceStartPoint, endpoints.start)
    || !samePoint(target.referenceEndPoint, endpoints.end)
  ) return null

  const direction = stableDirectionComponents(
    target.referenceStartPoint,
    target.referenceEndPoint,
  )
  return direction && pointIsOnDirectionLine(
    point,
    target.anchorPoint,
    direction,
  )
    ? { reference, direction }
    : null
}

function validatedAngleDirection(
  point: SnapPoint,
  target: Extract<AdditionSnapTarget, { kind: 'angle' }>,
  segments: readonly SnapSegment[],
) {
  if (
    typeof target.anchorId !== 'string'
    || target.anchorId.length === 0
    || !isFinitePoint(target.anchorPoint)
    || !isFinitePoint(target.rawPoint)
    || (
      target.rawPoint.x === target.anchorPoint.x
      && target.rawPoint.y === target.anchorPoint.y
    )
    || point.x === target.anchorPoint.x && point.y === target.anchorPoint.y
    || !Number.isFinite(target.angleDegrees)
    || target.angleDegrees <= 0
    || target.angleDegrees > 90
    || (target.angleSide !== 'counterclockwise' && target.angleSide !== 'clockwise')
    || (target.referenceKind !== 'global-horizontal' && target.referenceKind !== 'edge')
    || (target.angleDegrees === 90 && target.angleSide !== 'counterclockwise')
  ) return null

  let baseDirection: SnapPoint
  let referenceEdgeId: string | undefined
  if (target.referenceKind === 'global-horizontal') {
    if (
      target.referenceEdgeId !== undefined
      || target.referenceStartPoint !== undefined
      || target.referenceEndPoint !== undefined
    ) return null
    baseDirection = { x: 1, y: 0 }
  } else {
    if (
      typeof target.referenceEdgeId !== 'string'
      || target.referenceEdgeId.length === 0
      || !isFinitePoint(target.referenceStartPoint)
      || !isFinitePoint(target.referenceEndPoint)
      || comparePoints(target.referenceStartPoint, target.referenceEndPoint) >= 0
    ) return null
    const matches = segments.filter(({ id }) => id === target.referenceEdgeId)
    if (matches.length !== 1 || !isValidSegment(matches[0])) return null
    const endpoints = canonicalSegmentEndpoints(matches[0])
    if (
      !samePoint(target.referenceStartPoint, endpoints.start)
      || !samePoint(target.referenceEndPoint, endpoints.end)
    ) return null
    const referenceDirection = stableUnitDirection(endpoints.start, endpoints.end)
    if (!referenceDirection) return null
    baseDirection = referenceDirection
    referenceEdgeId = target.referenceEdgeId
  }

  if (
    target.key !== angleKey(
      target.anchorId,
      target.referenceKind,
      target.angleDegrees,
      target.angleSide,
      referenceEdgeId,
    )
  ) return null

  const radians = target.angleDegrees * Math.PI / 180
  const cosine = target.angleDegrees === 90 ? 0 : Math.cos(radians)
  const sine = target.angleDegrees === 90 ? 1 : Math.sin(radians)
  if (
    ![radians, cosine, sine].every(Number.isFinite)
    || radians <= 0
    || sine <= 0
  ) return null
  const direction = rotatedUnitDirection(
    baseDirection,
    cosine,
    sine,
    target.angleSide,
  )
  if (!direction) return null
  const recomputedPoint = projectOntoAnchoredDirection(
    target.rawPoint,
    target.anchorPoint,
    direction,
  )
  return recomputedPoint && samePoint(point, recomputedPoint) ? direction : null
}

function projectOntoAnchoredDirection(
  point: SnapPoint,
  anchor: SnapPoint,
  direction: SnapPoint,
) {
  // This deliberately mirrors snap.ts so generated angle targets can be
  // revalidated exactly while edge coincidence keeps its stricter test.
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
  return Number.isFinite(modelDistance) ? { x: projectedX, y: projectedY } : null
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

function stableHypot(x: number, y: number) {
  const maximumComponent = Math.max(Math.abs(x), Math.abs(y))
  if (!Number.isFinite(maximumComponent)) return Number.POSITIVE_INFINITY
  if (maximumComponent === 0) return 0
  const normalized = Math.hypot(x / maximumComponent, y / maximumComponent)
  const result = maximumComponent * normalized
  return Number.isFinite(result) ? result : Number.POSITIVE_INFINITY
}

function segmentSharesKnownLine(
  segment: SnapSegment,
  knownLine: Readonly<{
    anchorPoint: SnapPoint
    direction: SnapPoint
  }>,
) {
  if (!isValidSegment(segment)) return false
  const anchorIsEndpoint = (
    knownLine.anchorPoint.x === segment.x1
    && knownLine.anchorPoint.y === segment.y1
  ) || (
    knownLine.anchorPoint.x === segment.x2
    && knownLine.anchorPoint.y === segment.y2
  )
  if (!anchorIsEndpoint) return false
  const segmentDirection = stableDirectionComponents(
    { x: segment.x1, y: segment.y1 },
    { x: segment.x2, y: segment.y2 },
  )
  if (!segmentDirection || !isFinitePoint(knownLine.direction)) return false
  const firstTerm = segmentDirection.x * knownLine.direction.y
  const secondTerm = segmentDirection.y * knownLine.direction.x
  const cross = firstTerm - secondTerm
  if (![firstTerm, secondTerm, cross].every(Number.isFinite)) return false
  const tolerance = 64 * Number.EPSILON
    * (1 + Math.abs(firstTerm) + Math.abs(secondTerm))
  return Math.abs(cross) <= tolerance
}

function pointIsOnParallelLine(
  point: SnapPoint,
  anchor: SnapPoint,
  referenceStart: SnapPoint,
  referenceEnd: SnapPoint,
  includeCoordinateRounding = true,
) {
  const direction = stableDirectionComponents(referenceStart, referenceEnd)
  if (!direction) return false
  return pointIsOnDirectionLine(
    point,
    anchor,
    direction,
    includeCoordinateRounding,
  )
}

function pointIsOnDirectionLine(
  point: SnapPoint,
  anchor: SnapPoint,
  direction: SnapPoint,
  includeCoordinateRounding = true,
) {
  if (!isFinitePoint(direction)) return false
  const offsetX = point.x - anchor.x
  const offsetY = point.y - anchor.y
  if (!Number.isFinite(offsetX) || !Number.isFinite(offsetY)) return false
  const maximumOffset = Math.max(Math.abs(offsetX), Math.abs(offsetY))
  if (!Number.isFinite(maximumOffset)) return false
  if (maximumOffset === 0) return true
  const normalizedX = offsetX / maximumOffset
  const normalizedY = offsetY / maximumOffset
  const firstTerm = normalizedX * direction.y
  const secondTerm = normalizedY * direction.x
  const cross = firstTerm - secondTerm
  if (![firstTerm, secondTerm, cross].every(Number.isFinite)) return false
  // This bound covers the fixed arithmetic depth of the normalized cross. It
  // deliberately excludes absolute world coordinates: a one-ULP local offset
  // at a large translation must remain distinguishable from the line.
  let tolerance = 64 * Number.EPSILON * (1 + Math.abs(firstTerm) + Math.abs(secondTerm))
  if (includeCoordinateRounding) {
    const coordinateScale = Math.max(
      1,
      Math.abs(point.x),
      Math.abs(point.y),
      Math.abs(anchor.x),
      Math.abs(anchor.y),
    )
    const normalizedRounding = 16 * Number.EPSILON * coordinateScale / maximumOffset
    tolerance += normalizedRounding * (Math.abs(direction.x) + Math.abs(direction.y))
  }
  return Math.abs(cross) <= tolerance
}

function canonicalSegmentEndpoints(segment: SnapSegment) {
  const first = { x: normalizeZero(segment.x1), y: normalizeZero(segment.y1) }
  const second = { x: normalizeZero(segment.x2), y: normalizeZero(segment.y2) }
  return comparePoints(first, second) < 0
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

function rotatedUnitDirection(
  base: SnapPoint,
  cosine: number,
  sine: number,
  side: 'counterclockwise' | 'clockwise',
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

function comparePoints(first: SnapPoint, second: SnapPoint) {
  if (first.x < second.x || (first.x === second.x && first.y < second.y)) return -1
  if (first.x === second.x && first.y === second.y) return 0
  return 1
}

function samePoint(first: SnapPoint, second: SnapPoint) {
  return first.x === second.x && first.y === second.y
}

function isFinitePoint(point: SnapPoint) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function parallelKey(anchorId: string, referenceEdgeId: string) {
  return `parallel:${JSON.stringify([anchorId, referenceEdgeId])}`
}

function angleKey(
  anchorId: string,
  referenceKind: 'global-horizontal' | 'edge',
  angleDegrees: number,
  angleSide: 'counterclockwise' | 'clockwise',
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

function strictSegmentFractionAtPoint(segment: SnapSegment, point: SnapPoint) {
  if (!isValidSegment(segment)) return null
  const start = { x: segment.x1, y: segment.y1 }
  const end = { x: segment.x2, y: segment.y2 }
  const midpoint = {
    x: stableConvexCombination(segment.x1, segment.x2, 0.5),
    y: stableConvexCombination(segment.y1, segment.y2, 0.5),
  }
  if (
    !isFinitePoint(midpoint)
    || !pointIsOnParallelLine(point, midpoint, start, end, false)
  ) return null
  return strictSegmentFractionOnKnownLine(segment, point)
}

function strictSegmentFractionOnKnownLine(segment: SnapSegment, point: SnapPoint) {
  if (!isValidSegment(segment)) return null
  const direction = stableDirectionComponents(
    { x: segment.x1, y: segment.y1 },
    { x: segment.x2, y: segment.y2 },
  )
  if (!direction) return null
  const fraction = Math.abs(direction.x) >= Math.abs(direction.y)
    ? stableCoordinateFraction(segment.x1, segment.x2, point.x)
    : stableCoordinateFraction(segment.y1, segment.y2, point.y)
  return Number.isFinite(fraction) && isStrictInterior(fraction) ? fraction : null
}

function stableCoordinateFraction(start: number, end: number, value: number) {
  const delta = end - start
  const offset = value - start
  if (Number.isFinite(delta) && delta !== 0 && Number.isFinite(offset)) {
    return offset / delta
  }
  const scale = Math.max(Math.abs(start), Math.abs(end), Math.abs(value))
  if (!Number.isFinite(scale) || scale <= 0) return Number.NaN
  const scaledDelta = end / scale - start / scale
  const scaledOffset = value / scale - start / scale
  return Number.isFinite(scaledDelta)
    && scaledDelta !== 0
    && Number.isFinite(scaledOffset)
    ? scaledOffset / scaledDelta
    : Number.NaN
}

function stableConvexCombination(start: number, end: number, fraction: number) {
  const startIsNegative = start < 0 || Object.is(start, -0)
  const endIsNegative = end < 0 || Object.is(end, -0)
  return startIsNegative === endIsNegative
    ? start + (end - start) * fraction
    : start * (1 - fraction) + end * fraction
}

function intersectionPlacement(
  point: SnapPoint,
  target: Extract<AdditionSnapTarget, { kind: 'intersection' }>,
  segments: readonly SnapSegment[],
  vertices: readonly SnapVertex[],
): VertexPlacement | null {
  if (
    !Number.isFinite(point.x)
    || !Number.isFinite(point.y)
    || !Number.isFinite(target.point.x)
    || !Number.isFinite(target.point.y)
    || !Number.isFinite(target.distancePx)
    || target.distancePx < 0
    || point.x !== target.point.x
    || point.y !== target.point.y
    || !Array.isArray(target.sourceEdges)
  ) return null

  if (target.classification === 'cluster') {
    return clusterIntersectionPlacement(target, segments, vertices)
  }
  if (target.sourceEdges.length !== 2) return null

  const [first, second] = target.sourceEdges
  if (
    !first
    || !second
    || !first.id
    || !second.id
    || first.id >= second.id
    || target.key !== intersectionKey(first.id, second.id)
    || !Number.isFinite(first.fraction)
    || !Number.isFinite(second.fraction)
  ) return null

  const firstMatches = segments.filter(({ id }) => id === first.id)
  const secondMatches = segments.filter(({ id }) => id === second.id)
  if (firstMatches.length !== 1 || secondMatches.length !== 1) return null
  const firstSegment = firstMatches[0]
  const secondSegment = secondMatches[0]
  if (
    !isValidSegment(firstSegment)
    || !isValidSegment(secondSegment)
    ||
    firstSegment.startVertexId === secondSegment.startVertexId
    || firstSegment.startVertexId === secondSegment.endVertexId
    || firstSegment.endVertexId === secondSegment.startVertexId
    || firstSegment.endVertexId === secondSegment.endVertexId
  ) return null

  if (!isSupportedIntersectionTarget(target, segments)) return null

  if (target.classification === 'proper') {
    if (!isStrictInterior(first.fraction) || !isStrictInterior(second.fraction)) return null
    return {
      operation: 'connect-intersection',
      firstEdgeId: first.id,
      secondEdgeId: second.id,
    }
  }
  if (target.classification !== 't-junction') return null

  const firstEndpoint = exactEndpoint(first.fraction)
  const secondEndpoint = exactEndpoint(second.fraction)
  if (
    (firstEndpoint === null) === (secondEndpoint === null)
    || (firstEndpoint === null && !isStrictInterior(first.fraction))
    || (secondEndpoint === null && !isStrictInterior(second.fraction))
  ) return null

  const endpointSegment = firstEndpoint === null ? secondSegment : firstSegment
  const endpointFraction = firstEndpoint ?? secondEndpoint
  if (endpointFraction === null) return null
  const junctionVertexId = endpointFraction === 0
    ? endpointSegment.startVertexId
    : endpointSegment.endVertexId
  const junctionPoint = endpointFraction === 0
    ? { x: endpointSegment.x1, y: endpointSegment.y1 }
    : { x: endpointSegment.x2, y: endpointSegment.y2 }
  if (
    !target.junctionVertexId
    || target.junctionVertexId !== junctionVertexId
    || junctionPoint.x !== target.point.x
    || junctionPoint.y !== target.point.y
    || !junctionIdentityIsUnambiguous(
      segments,
      target.junctionVertexId,
      target.point,
    )
  ) return null

  return {
    operation: 'connect-t-junction',
    firstEdgeId: first.id,
    secondEdgeId: second.id,
    junctionVertexId: target.junctionVertexId,
  }
}

function clusterIntersectionPlacement(
  target: Extract<AdditionSnapTarget, { classification: 'cluster' }>,
  segments: readonly SnapSegment[],
  vertices: readonly SnapVertex[],
): VertexPlacement | null {
  if (
    target.sourceEdges.length < 3
    || target.sourceEdges.length > 64
  ) return null

  const sourceIds = target.sourceEdges.map(({ id }) => id)
  if (
    sourceIds.some((id, index) => !id || (index > 0 && sourceIds[index - 1] >= id))
    || target.key !== intersectionKey(...sourceIds)
  ) return null

  const sourceIdSet = new Set(sourceIds)
  const segmentIdCounts = new Map<string, number>()
  for (const segment of segments) {
    segmentIdCounts.set(segment.id, (segmentIdCounts.get(segment.id) ?? 0) + 1)
  }

  const matchedSegments: SnapSegment[] = []
  const endpointIds = new Set<string>()
  let interiorCount = 0
  for (const source of target.sourceEdges) {
    const matches = segments.filter(({ id }) => id === source.id)
    if (matches.length !== 1) return null
    const segment = matches[0]
    if (
      !isValidSegment(segment)
      || segment.kind === 'boundary'
      || (
        (source.kind !== undefined || segment.kind !== undefined)
        && source.kind !== segment.kind
      )
    ) return null
    if (source.relation === 'interior') {
      if (
        !isStrictInterior(source.fraction)
        || (target.point.x === segment.x1 && target.point.y === segment.y1)
        || (target.point.x === segment.x2 && target.point.y === segment.y2)
      ) return null
      interiorCount += 1
    } else if (
      source.relation !== 'endpoint'
      || (source.fraction !== 0 && source.fraction !== 1)
      || source.endpointVertexId !== (source.fraction === 0
        ? segment.startVertexId
        : segment.endVertexId)
      || target.point.x !== (source.fraction === 0 ? segment.x1 : segment.x2)
      || target.point.y !== (source.fraction === 0 ? segment.y1 : segment.y2)
    ) return null
    if (source.relation === 'endpoint') endpointIds.add(source.endpointVertexId)
    matchedSegments.push(segment)
  }
  if (endpointIds.size > 1 || interiorCount === 0) return null

  for (let index = 0; index < target.sourceEdges.length; index += 1) {
    const source = target.sourceEdges[index]
    if (source.relation !== 'interior') continue
    const segment = matchedSegments[index]
    const fractionIsConfirmed = matchedSegments.some((other, otherIndex) => {
      if (otherIndex === index) return false
      const forward = singlePointIntersection(segment, other)
      if (
        forward
        && clusterPointLiesOnSegment(target.point, segment)
        && clusterPointLiesOnSegment(target.point, other)
        && forward.firstFraction === source.fraction
      ) return true
      const reverse = singlePointIntersection(other, segment)
      return Boolean(
        reverse
        && clusterPointLiesOnSegment(target.point, segment)
        && clusterPointLiesOnSegment(target.point, other)
        && reverse.secondFraction === source.fraction,
      )
    })
    if (!fractionIsConfirmed) return null
  }

  for (let first = 0; first < matchedSegments.length; first += 1) {
    for (let second = first + 1; second < matchedSegments.length; second += 1) {
      const overlap = hasPositiveCollinearOverlap(
        matchedSegments[first],
        matchedSegments[second],
      )
      if (
        overlap === null
        || overlap
        || !segmentsMeetAtClusterPoint(
          matchedSegments[first],
          matchedSegments[second],
          target.point,
        )
      ) return null
    }
  }

  for (const segment of segments) {
    if (!isValidSegment(segment)) continue
    if (!segmentContainsClusterPoint(segment, target.point, matchedSegments)) continue
    if (
      segment.kind === 'boundary'
      || segmentIdCounts.get(segment.id) !== 1
      || !sourceIdSet.has(segment.id)
    ) return null
  }

  const endpointJunctionId = endpointIds.values().next().value as string | undefined
  if (endpointJunctionId && target.junctionVertexId !== endpointJunctionId) return null
  const occupiedVertexId = uniqueVertexAtPoint(vertices, segments, target.point)
  if (occupiedVertexId === null) return null
  if (target.junctionVertexId !== undefined) {
    if (occupiedVertexId !== target.junctionVertexId) return null
  } else if (occupiedVertexId !== undefined) {
    return null
  }

  return {
    operation: 'connect-intersection-cluster',
    targets: target.sourceEdges.map(({ id, relation }) => ({
      edgeId: id,
      relation,
    })),
    ...(target.junctionVertexId
      ? { junctionVertexId: target.junctionVertexId }
      : {}),
  }
}

function segmentContainsClusterPoint(
  segment: SnapSegment,
  point: SnapPoint,
  references: readonly SnapSegment[],
) {
  if (
    (point.x === segment.x1 && point.y === segment.y1)
    || (point.x === segment.x2 && point.y === segment.y2)
  ) return true
  return references.some((reference) => {
    if (reference === segment) return true
    const forward = singlePointIntersection(reference, segment)
    if (
      forward
      && clusterPointLiesOnSegment(point, reference)
      && clusterPointLiesOnSegment(point, segment)
    ) return true
    const reverse = singlePointIntersection(segment, reference)
    return Boolean(
      reverse
      && clusterPointLiesOnSegment(point, reference)
      && clusterPointLiesOnSegment(point, segment),
    )
  })
}

function singlePointIntersection(
  first: SnapSegment,
  second: SnapSegment,
): Readonly<{
  point: SnapPoint
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
  const denominator = checkedFiniteCross(firstDirection, secondDirection)
  if (denominator === null || denominator === 0) return null
  const firstNumerator = checkedFiniteCross(secondOffset, secondDirection)
  const secondNumerator = checkedFiniteCross(secondOffset, firstDirection)
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

function isSegmentFraction(fraction: number) {
  return Number.isFinite(fraction) && fraction >= 0 && fraction <= 1
}

function hasPositiveCollinearOverlap(
  first: SnapSegment,
  second: SnapSegment,
): boolean | null {
  const firstDirection = { x: first.x2 - first.x1, y: first.y2 - first.y1 }
  const secondDirection = { x: second.x2 - second.x1, y: second.y2 - second.y1 }
  const offset = { x: second.x1 - first.x1, y: second.y1 - first.y1 }
  const directionCross = checkedFiniteCross(firstDirection, secondDirection)
  const offsetCross = checkedFiniteCross(firstDirection, offset)
  if (directionCross === null || offsetCross === null) return null
  if (directionCross !== 0 || offsetCross !== 0) return false
  const useX = Math.abs(firstDirection.x) >= Math.abs(firstDirection.y)
  const firstStart = useX ? first.x1 : first.y1
  const firstEnd = useX ? first.x2 : first.y2
  const secondStart = useX ? second.x1 : second.y1
  const secondEnd = useX ? second.x2 : second.y2
  return Math.max(Math.min(firstStart, firstEnd), Math.min(secondStart, secondEnd))
    < Math.min(Math.max(firstStart, firstEnd), Math.max(secondStart, secondEnd))
}

function segmentsMeetAtClusterPoint(
  first: SnapSegment,
  second: SnapSegment,
  point: SnapPoint,
) {
  const intersection = singlePointIntersection(first, second)
  if (intersection) {
    return clusterPointLiesOnSegment(point, first)
      && clusterPointLiesOnSegment(point, second)
  }
  return segmentHasExactEndpoint(first, point)
    && segmentHasExactEndpoint(second, point)
}

function segmentHasExactEndpoint(segment: SnapSegment, point: SnapPoint) {
  return (segment.x1 === point.x && segment.y1 === point.y)
    || (segment.x2 === point.x && segment.y2 === point.y)
}

function checkedFiniteCross(first: SnapPoint, second: SnapPoint) {
  const value = first.x * second.y - first.y * second.x
  return Number.isFinite(value) ? value : null
}

function uniqueVertexAtPoint(
  vertices: readonly SnapVertex[],
  segments: readonly SnapSegment[],
  point: SnapPoint,
): string | null | undefined {
  const positionsById = new Map<string, SnapPoint[]>()
  const vertexRecordCounts = new Map<string, number>()
  const add = (id: string, candidate: SnapPoint) => {
    if (!id || !isFinitePoint(candidate)) return
    const positions = positionsById.get(id)
    if (positions) positions.push(candidate)
    else positionsById.set(id, [candidate])
  }
  for (const vertex of vertices) {
    vertexRecordCounts.set(vertex.id, (vertexRecordCounts.get(vertex.id) ?? 0) + 1)
    add(vertex.id, vertex)
  }
  for (const segment of segments) {
    add(segment.startVertexId, { x: segment.x1, y: segment.y1 })
    add(segment.endVertexId, { x: segment.x2, y: segment.y2 })
  }
  const occupants: string[] = []
  for (const [id, positions] of positionsById) {
    if (positions.some((position) => position.x === point.x && position.y === point.y)) {
      occupants.push(id)
    }
  }
  if (occupants.length > 1) return null
  const occupant = occupants[0]
  if (
    occupant
    && (
      (vertexRecordCounts.get(occupant) ?? 0) > 1
      || positionsById.get(occupant)?.some(
        (position) => position.x !== point.x || position.y !== point.y,
      )
    )
  ) return null
  return occupant
}

export function isSupportedIntersectionTarget(
  target: Extract<AdditionSnapTarget, { kind: 'intersection' }>,
  segments: readonly SnapSegment[],
) {
  if (!Array.isArray(target.sourceEdges)) return false
  if (target.classification === 'cluster') {
    if (target.sourceEdges.length < 3 || target.sourceEdges.length > 64) return false
    const sourceIds = target.sourceEdges.map(({ id }) => id)
    if (
      sourceIds.some(
        (id, index) => !id || (index > 0 && sourceIds[index - 1] >= id),
      )
      || target.key !== intersectionKey(...sourceIds)
    ) return false

    return target.sourceEdges.every((source) => {
      const matches = segments.filter(({ id }) => id === source.id)
      if (matches.length !== 1) return false
      const segment = matches[0]
      return segment.kind !== 'boundary'
        && (source.kind === undefined || source.kind === segment.kind)
    })
  }
  if (target.sourceEdges.length !== 2) return false
  const [firstSource, secondSource] = target.sourceEdges
  if (!firstSource || !secondSource) return false
  const firstMatches = segments.filter(({ id }) => id === firstSource.id)
  const secondMatches = segments.filter(({ id }) => id === secondSource.id)
  if (firstMatches.length !== 1 || secondMatches.length !== 1) return false

  const firstIsBoundary = firstMatches[0].kind === 'boundary'
  const secondIsBoundary = secondMatches[0].kind === 'boundary'
  if (target.classification === 'proper') {
    return !firstIsBoundary && !secondIsBoundary
  }
  if (target.classification !== 't-junction') return false
  if (firstIsBoundary && secondIsBoundary) return false
  if (!firstIsBoundary && !secondIsBoundary) return true

  const boundarySource = firstIsBoundary ? firstSource : secondSource
  const otherSource = firstIsBoundary ? secondSource : firstSource
  return isStrictInterior(boundarySource.fraction)
    && exactEndpoint(otherSource.fraction) !== null
}

export function isSupportedIntersectionPlacement(
  placement: IntersectionConnectionPlacement,
  edges: readonly PersistedIntersectionEdge[],
) {
  if (placement.operation === 'connect-intersection-cluster') {
    if (placement.targets.length < 3 || placement.targets.length > 64) return false
    const edgeById = new Map<string, PersistedIntersectionEdge | null>()
    for (const edge of edges) {
      edgeById.set(edge.id, edgeById.has(edge.id) ? null : edge)
    }
    let interiorCount = 0
    let previousId: string | undefined
    for (const target of placement.targets) {
      if (
        !target.edgeId
        || (previousId !== undefined && previousId >= target.edgeId)
        || (target.relation !== 'interior' && target.relation !== 'endpoint')
      ) return false
      previousId = target.edgeId
      const edge = edgeById.get(target.edgeId)
      if (
        !edge
        || edge.kind === 'boundary'
        || !['mountain', 'valley', 'auxiliary', 'cut'].includes(edge.kind)
      ) return false
      if (target.relation === 'interior') {
        interiorCount += 1
        if (
          placement.junctionVertexId
          && (edge.start === placement.junctionVertexId
            || edge.end === placement.junctionVertexId)
        ) return false
      } else if (
        !placement.junctionVertexId
        || (edge.start !== placement.junctionVertexId
          && edge.end !== placement.junctionVertexId)
      ) return false
    }
    return interiorCount > 0
      && (placement.junctionVertexId === undefined
        || placement.junctionVertexId.length > 0)
      && (placement.junctionVertexId !== undefined
        || interiorCount === placement.targets.length)
  }

  if (placement.firstEdgeId >= placement.secondEdgeId) return false
  const firstMatches = edges.filter(({ id }) => id === placement.firstEdgeId)
  const secondMatches = edges.filter(({ id }) => id === placement.secondEdgeId)
  if (firstMatches.length !== 1 || secondMatches.length !== 1) return false
  const first = firstMatches[0]
  const second = secondMatches[0]
  const boundaryCount = Number(first.kind === 'boundary')
    + Number(second.kind === 'boundary')
  if (boundaryCount > 1) return false
  if (placement.operation === 'connect-intersection') return boundaryCount === 0

  const firstCarriesJunction = first.start === placement.junctionVertexId
    || first.end === placement.junctionVertexId
  const secondCarriesJunction = second.start === placement.junctionVertexId
    || second.end === placement.junctionVertexId
  if (firstCarriesJunction === secondCarriesJunction) return false
  const interiorEdge = firstCarriesJunction ? second : first
  return boundaryCount === 0 || interiorEdge.kind === 'boundary'
}

function isValidSegment(segment: SnapSegment) {
  return Boolean(segment.id)
    && Boolean(segment.startVertexId)
    && Boolean(segment.endVertexId)
    && segment.startVertexId !== segment.endVertexId
    && [segment.x1, segment.y1, segment.x2, segment.y2].every(Number.isFinite)
    && (segment.x1 !== segment.x2 || segment.y1 !== segment.y2)
}

function isStrictInterior(fraction: number) {
  return fraction > 0 && fraction < 1
}

function exactEndpoint(fraction: number): 0 | 1 | null {
  if (fraction === 0) return 0
  if (fraction === 1) return 1
  return null
}

function junctionIdentityIsUnambiguous(
  segments: readonly SnapSegment[],
  junctionVertexId: string,
  point: SnapPoint,
) {
  let foundJunction = false
  for (const segment of segments) {
    const endpoints = [
      { id: segment.startVertexId, x: segment.x1, y: segment.y1 },
      { id: segment.endVertexId, x: segment.x2, y: segment.y2 },
    ]
    for (const endpoint of endpoints) {
      if (endpoint.id === junctionVertexId) {
        if (endpoint.x !== point.x || endpoint.y !== point.y) return false
        foundJunction = true
      } else if (endpoint.x === point.x && endpoint.y === point.y) {
        return false
      }
    }
  }
  return foundJunction
}

function intersectionKey(...edgeIds: readonly string[]) {
  return `intersection:${JSON.stringify(edgeIds)}`
}

function finitePointPlacement(point: SnapPoint): VertexPlacement | null {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
  return { operation: 'add', x: point.x, y: point.y }
}
