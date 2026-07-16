export type BenchmarkPatternData = Readonly<{
  requested_edge_count: number
  vertex_count: number
  edge_count: number
  vertices: ReadonlyArray<Readonly<{
    id: string
    position: Readonly<{ x: number; y: number }>
  }>>
  edges: ReadonlyArray<Readonly<{
    id: string
    start: string
    end: string
    kind: 'mountain' | 'valley'
  }>>
}>

export type BenchmarkRenderData = Readonly<{
  requestedEdgeCount: number
  lines: ReadonlyArray<Readonly<{
    id: string
    startVertexId: string
    endVertexId: string
    x1: number
    y1: number
    x2: number
    y2: number
    kind: 'mountain' | 'valley'
  }>>
  vertices: ReadonlyArray<Readonly<{ id: string; x: number; y: number }>>
  bounds: Readonly<{ minX: number; minY: number; maxX: number; maxY: number }>
}>

const EMPTY_BOUNDS = Object.freeze({ minX: 0, minY: 0, maxX: 1, maxY: 1 })

export function prepareBenchmarkRenderData(
  pattern: BenchmarkPatternData,
): BenchmarkRenderData {
  assertCount('requested_edge_count', pattern.requested_edge_count)
  assertCount('vertex_count', pattern.vertex_count)
  assertCount('edge_count', pattern.edge_count)
  if (pattern.vertex_count !== pattern.vertices.length) {
    throw new Error('benchmark vertex count mismatch')
  }
  if (pattern.edge_count !== pattern.edges.length) {
    throw new Error('benchmark edge count mismatch')
  }

  const positions = new Map<string, Readonly<{ x: number; y: number }>>()
  const vertices: Array<{ id: string; x: number; y: number }> = []
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const vertex of pattern.vertices) {
    if (!vertex.id || positions.has(vertex.id)) {
      throw new Error('benchmark vertex id is empty or duplicated')
    }
    if (!Number.isFinite(vertex.position.x) || !Number.isFinite(vertex.position.y)) {
      throw new Error('benchmark vertex position is not finite')
    }
    positions.set(vertex.id, vertex.position)
    vertices.push({ id: vertex.id, x: vertex.position.x, y: vertex.position.y })
    minX = Math.min(minX, vertex.position.x)
    minY = Math.min(minY, vertex.position.y)
    maxX = Math.max(maxX, vertex.position.x)
    maxY = Math.max(maxY, vertex.position.y)
  }

  const edgeIds = new Set<string>()
  const lines = pattern.edges.map((edge) => {
    if (!edge.id || edgeIds.has(edge.id)) {
      throw new Error('benchmark edge id is empty or duplicated')
    }
    edgeIds.add(edge.id)
    const start = positions.get(edge.start)
    const end = positions.get(edge.end)
    if (!start || !end) throw new Error('benchmark edge references a missing vertex')
    if (edge.start === edge.end || (start.x === end.x && start.y === end.y)) {
      throw new Error('benchmark edge is degenerate')
    }
    if (edge.kind !== 'mountain' && edge.kind !== 'valley') {
      throw new Error('benchmark edge kind is unsupported')
    }
    return {
      id: edge.id,
      startVertexId: edge.start,
      endVertexId: edge.end,
      x1: start.x,
      y1: start.y,
      x2: end.x,
      y2: end.y,
      kind: edge.kind,
    }
  })

  return {
    requestedEdgeCount: pattern.requested_edge_count,
    vertices,
    lines,
    bounds: vertices.length === 0
      ? EMPTY_BOUNDS
      : expandDegenerateBounds({ minX, minY, maxX, maxY }),
  }
}

export function measureBenchmarkPayloadBytes(pattern: BenchmarkPatternData): number {
  return new TextEncoder().encode(JSON.stringify(pattern)).byteLength
}

function assertCount(label: string, count: number) {
  if (!Number.isSafeInteger(count) || count < 0) {
    throw new Error(`benchmark ${label} is invalid`)
  }
}

function expandDegenerateBounds(bounds: {
  minX: number
  minY: number
  maxX: number
  maxY: number
}) {
  const next = { ...bounds }
  if (next.minX === next.maxX) {
    next.minX -= 0.5
    next.maxX += 0.5
  }
  if (next.minY === next.maxY) {
    next.minY -= 0.5
    next.maxY += 0.5
  }
  return next
}
