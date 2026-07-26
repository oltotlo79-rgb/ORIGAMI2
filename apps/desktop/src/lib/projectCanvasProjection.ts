import type {
  CreaseCanvasAnnotation,
  CreaseCanvasFace,
} from '../components/CreaseCanvas.tsx'
import { rgbaToCss } from './appElementMetadata.ts'
import type {
  ProjectSnapshot,
  ProjectTopologyResponse,
} from './coreClient.ts'

export function createCanvasFaces(
  snapshot: ProjectSnapshot | null,
  topologyResponse: ProjectTopologyResponse | null,
): readonly CreaseCanvasFace[] {
  const topology = topologyResponse?.snapshot
  if (
    !snapshot
    || !topology
    || topologyResponse.project_id !== snapshot.project_id
    || topologyResponse.revision !== snapshot.revision
    || topology.source_revision !== snapshot.revision
  ) return []

  const positions = new Map<string, Array<{ x: number; y: number }>>()
  for (const vertex of snapshot.crease_pattern.vertices) {
    const matches = positions.get(vertex.id)
    if (matches) matches.push(vertex.position)
    else positions.set(vertex.id, [vertex.position])
  }

  const faces: CreaseCanvasFace[] = []
  for (const face of topology.faces) {
    const polygon: Array<{ x: number; y: number }> = []
    let valid = face.outer.half_edges.length >= 3
    for (const halfEdge of face.outer.half_edges) {
      const matches = positions.get(halfEdge.origin)
      if (matches?.length !== 1) {
        valid = false
        break
      }
      polygon.push({ x: matches[0].x, y: matches[0].y })
    }
    if (!valid) continue

    const color = snapshot.element_metadata.faces.find(
      (record) => record.face === face.id,
    )?.metadata.color
    faces.push(Object.freeze({
      id: face.id,
      vertexIds: Object.freeze(
        face.outer.half_edges.map((halfEdge) => halfEdge.origin),
      ),
      edgeIds: Object.freeze(
        face.outer.half_edges.map((halfEdge) => halfEdge.edge),
      ),
      polygon: Object.freeze(polygon),
      ...(color ? { color: rgbaToCss(color) } : {}),
    }))
  }
  return Object.freeze(faces)
}

export function createCanvasAnnotations(
  snapshot: ProjectSnapshot | null,
): readonly CreaseCanvasAnnotation[] {
  if (!snapshot?.annotations) return []
  const vertices = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [
      vertex.id,
      vertex.position,
    ]),
  )
  const layers = new Map(
    snapshot.project_layers.layers.map((layer) => [layer.id, layer]),
  )
  return snapshot.annotations.annotations.flatMap((annotation) => {
    const layer = layers.get(annotation.layer)
    if (
      !layer
      || layer.content_kind !== 'annotation'
      || !layer.visible
    ) return []
    const anchor = annotation.anchor.kind === 'absolute'
      ? annotation.anchor.position
      : vertices.get(annotation.anchor.vertex)
    if (!anchor) return []
    const offset = annotation.anchor.kind === 'vertex'
      ? annotation.anchor.offset
      : { x: 0, y: 0 }
    return [{
      id: annotation.id,
      text: annotation.text,
      x: anchor.x + offset.x,
      y: anchor.y + offset.y,
      color: rgbaToCss(annotation.style.color),
      opacity: layer.opacity,
      fontSizeMm: annotation.style.font_size_mm,
      bold: annotation.style.bold,
      italic: annotation.style.italic,
    }]
  })
}
