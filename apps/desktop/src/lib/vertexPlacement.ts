import type { SnapPoint, SnapSegment, SnapTarget } from './snap'

export type VertexPlacement = Readonly<{
  operation: 'add'
  x: number
  y: number
}> | Readonly<{
  operation: 'split-edge'
  edgeId: string
  fraction: number
}>

export function createVertexPlacement(
  point: SnapPoint,
  target: SnapTarget | null,
  segments: readonly SnapSegment[],
): VertexPlacement | null {
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

function finitePointPlacement(point: SnapPoint): VertexPlacement | null {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
  return { operation: 'add', x: point.x, y: point.y }
}
