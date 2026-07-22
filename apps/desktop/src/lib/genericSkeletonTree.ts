export type GenericSkeletonTreeStatus =
  | 'empty' | 'tree' | 'resource_limit' | 'degenerate' | 'duplicate_edge' | 'disconnected' | 'cycle'

type Point = Readonly<{ x_tenths_mm: number, y_tenths_mm: number }>
type Segment = Readonly<{ id: number, start: Point, end: Point }>

const pointKey = (point: Point) => `${point.x_tenths_mm}:${point.y_tenths_mm}`

export function analyzeGenericSkeletonTree(segments: readonly Segment[]): Readonly<{
  status: GenericSkeletonTreeStatus
  pointCount: number
  edgeCount: number
}> {
  if (segments.length === 0) return Object.freeze({ status: 'empty', pointCount: 0, edgeCount: 0 })
  if (segments.length > 8) return Object.freeze({ status: 'resource_limit', pointCount: 0, edgeCount: segments.length })
  const adjacency = new Map<string, Set<string>>()
  const edges = new Set<string>()
  for (const segment of segments) {
    const start = pointKey(segment.start), end = pointKey(segment.end)
    if (start === end) return Object.freeze({ status: 'degenerate', pointCount: adjacency.size, edgeCount: edges.size })
    const edge = start < end ? `${start}|${end}` : `${end}|${start}`
    if (edges.has(edge)) return Object.freeze({ status: 'duplicate_edge', pointCount: adjacency.size, edgeCount: edges.size })
    edges.add(edge)
    if (!adjacency.has(start)) adjacency.set(start, new Set())
    if (!adjacency.has(end)) adjacency.set(end, new Set())
    adjacency.get(start)!.add(end); adjacency.get(end)!.add(start)
  }
  const root = adjacency.keys().next().value as string
  const visited = new Set([root]), pending = [root]
  while (pending.length > 0) {
    for (const next of adjacency.get(pending.pop()!) ?? []) {
      if (!visited.has(next)) { visited.add(next); pending.push(next) }
    }
  }
  const status: GenericSkeletonTreeStatus = visited.size !== adjacency.size
    ? 'disconnected'
    : edges.size === adjacency.size - 1 ? 'tree' : 'cycle'
  return Object.freeze({ status, pointCount: adjacency.size, edgeCount: edges.size })
}
