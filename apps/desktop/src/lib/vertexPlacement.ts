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
    target.classification !== 'proper'
    || !Number.isFinite(point.x)
    || !Number.isFinite(point.y)
    || !Number.isFinite(target.point.x)
    || !Number.isFinite(target.point.y)
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
    || !Number.isFinite(first.fraction)
    || !Number.isFinite(second.fraction)
    || first.fraction <= 0
    || first.fraction >= 1
    || second.fraction <= 0
    || second.fraction >= 1
  ) return null

  const firstMatches = segments.filter(({ id }) => id === first.id)
  const secondMatches = segments.filter(({ id }) => id === second.id)
  if (firstMatches.length !== 1 || secondMatches.length !== 1) return null
  const firstSegment = firstMatches[0]
  const secondSegment = secondMatches[0]
  if (
    firstSegment.startVertexId === secondSegment.startVertexId
    || firstSegment.startVertexId === secondSegment.endVertexId
    || firstSegment.endVertexId === secondSegment.startVertexId
    || firstSegment.endVertexId === secondSegment.endVertexId
  ) return null

  return {
    operation: 'connect-intersection',
    firstEdgeId: first.id,
    secondEdgeId: second.id,
  }
}

function finitePointPlacement(point: SnapPoint): VertexPlacement | null {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
  return { operation: 'add', x: point.x, y: point.y }
}
