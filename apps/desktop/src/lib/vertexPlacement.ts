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

export function createVertexPlacement(
  point: SnapPoint,
  target: AdditionSnapTarget | null,
  segments: readonly SnapSegment[],
): VertexPlacement | null {
  if (target?.kind === 'intersection') {
    return intersectionPlacement(point, target, segments)
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
