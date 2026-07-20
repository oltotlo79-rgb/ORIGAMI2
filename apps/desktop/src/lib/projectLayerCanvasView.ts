import type { CreaseLine } from '../components/CreaseCanvas.tsx'
import {
  DEFAULT_PROJECT_LAYER_ID,
  type ProjectLayerDocumentV1,
} from './projectLayers.ts'
import type { VertexPlacement } from './vertexPlacement.ts'

type PatternVertex = Readonly<{
  id: string
  position: Readonly<{ x: number; y: number }>
}>

type PatternEdge = Readonly<{
  id: string
  start: string
  end: string
  kind: string
}>

export type ProjectLayerCanvasView = Readonly<{
  lines: CreaseLine[]
  vertices: Array<{ id: string; x: number; y: number }>
  lockedVertexIds: ReadonlySet<string>
  defaultLayerLocked: boolean
}>

const EMPTY_VIEW: ProjectLayerCanvasView = Object.freeze({
  lines: Object.freeze([]) as unknown as CreaseLine[],
  vertices: Object.freeze([]) as unknown as Array<{
    id: string
    x: number
    y: number
  }>,
  lockedVertexIds: new Set<string>(),
  defaultLayerLocked: false,
})

/**
 * Derives the only geometry admitted to 2D drawing, hit-testing, snapping,
 * and intersection discovery. Hidden and zero-opacity edges never enter the
 * returned line list. A vertex remains visible when at least one incident
 * edge remains visible; an isolated vertex follows the default layer.
 */
export function createProjectLayerCanvasView(
  document: ProjectLayerDocumentV1 | null | undefined,
  pattern: Readonly<{
    vertices: readonly PatternVertex[]
    edges: readonly PatternEdge[]
  }> | null | undefined,
): ProjectLayerCanvasView {
  if (!document || !pattern) return EMPTY_VIEW

  const positions = new Map(
    pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const layersById = new Map(
    document.layers.map((layer, index) => [
      layer.id,
      { layer, index },
    ]),
  )
  const assignedLayers = new Map(
    document.edge_assignments.map((assignment) => [
      assignment.edge,
      assignment.layer,
    ]),
  )
  const defaultLayer = layersById.get(DEFAULT_PROJECT_LAYER_ID)?.layer
  const visibleVertexIds = new Set<string>()
  const lockedVertexIds = new Set<string>()
  const incidentVertexIds = new Set<string>()
  const lines = pattern.edges.flatMap<CreaseLine>((edge) => {
    const start = positions.get(edge.start)
    const end = positions.get(edge.end)
    const layerId = assignedLayers.get(edge.id) ?? DEFAULT_PROJECT_LAYER_ID
    const layerEntry = layersById.get(layerId)
    const layer = layerEntry?.layer
    if (
      !start
      || !end
      || !layer
      || !layer.visible
      || layer.opacity === 0
      || !isCreaseLineKind(edge.kind)
    ) return []
    visibleVertexIds.add(edge.start)
    visibleVertexIds.add(edge.end)
    return [{
      id: edge.id,
      startVertexId: edge.start,
      endVertexId: edge.end,
      x1: start.x,
      y1: start.y,
      x2: end.x,
      y2: end.y,
      kind: edge.kind,
      layerId,
      layerOrder: layerEntry.index,
      opacity: layer.opacity,
      locked: layer.locked,
    }]
  })

  for (const edge of pattern.edges) {
    incidentVertexIds.add(edge.start)
    incidentVertexIds.add(edge.end)
    const layerId = assignedLayers.get(edge.id) ?? DEFAULT_PROJECT_LAYER_ID
    const layer = layersById.get(layerId)?.layer
    if (!layer || layer.locked) {
      lockedVertexIds.add(edge.start)
      lockedVertexIds.add(edge.end)
    }
  }
  for (const vertex of pattern.vertices) {
    if (!incidentVertexIds.has(vertex.id)) {
      if (defaultLayer?.visible && defaultLayer.opacity > 0) {
        visibleVertexIds.add(vertex.id)
      }
      if (!defaultLayer || defaultLayer.locked) {
        lockedVertexIds.add(vertex.id)
      }
    }
  }

  return {
    lines,
    vertices: pattern.vertices.flatMap((vertex) =>
      visibleVertexIds.has(vertex.id)
        ? [{
            id: vertex.id,
            x: vertex.position.x,
            y: vertex.position.y,
          }]
        : []),
    lockedVertexIds,
    defaultLayerLocked: defaultLayer?.locked ?? true,
  }
}

/**
 * Performs the UI-side lock preflight for every placement that changes an
 * existing edge. Missing target lines fail closed; native commands enforce
 * the same rule independently.
 */
export function placementTouchesLockedLayer(
  placement: VertexPlacement,
  view: ProjectLayerCanvasView,
): boolean {
  if (placement.operation === 'add') return view.defaultLayerLocked
  const targets = placement.operation === 'split-edge'
    ? [placement.edgeId]
    : placement.operation === 'connect-intersection-cluster'
      ? placement.targets.map(({ edgeId }) => edgeId)
      : [placement.firstEdgeId, placement.secondEdgeId]
  const byId = new Map(view.lines.map((line) => [line.id, line]))
  return targets.some((edgeId) => byId.get(edgeId)?.locked !== false)
}

function isCreaseLineKind(kind: string): kind is CreaseLine['kind'] {
  return kind === 'mountain'
    || kind === 'valley'
    || kind === 'auxiliary'
    || kind === 'boundary'
    || kind === 'cut'
}
