import type { AdditionSnapTarget, SnapPoint, SnapSegment } from './snap'

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
}>

type IntersectionConnectionPlacement = Extract<
  VertexPlacement,
  { operation: 'connect-intersection' | 'connect-t-junction' }
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
): VertexPlacement | null {
  if (target?.kind === 'intersection') {
    return intersectionPlacement(point, target, segments)
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
    || target.sourceEdges.length !== 2
  ) return null

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

export function isSupportedIntersectionTarget(
  target: Extract<AdditionSnapTarget, { kind: 'intersection' }>,
  segments: readonly SnapSegment[],
) {
  if (!Array.isArray(target.sourceEdges) || target.sourceEdges.length !== 2) return false
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

function intersectionKey(firstId: string, secondId: string) {
  return `intersection:${JSON.stringify([firstId, secondId])}`
}

function finitePointPlacement(point: SnapPoint): VertexPlacement | null {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
  return { operation: 'add', x: point.x, y: point.y }
}
