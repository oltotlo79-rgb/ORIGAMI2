import type {
  FoldAssignment,
  ProjectSnapshot,
  ProjectTopologyResponse,
} from './coreClient'

export const MAX_FOLD_PREVIEW_WORLD_SIZE = 4.4

const CANONICAL_ENTITY_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u

export type FoldPreviewWorldVertex = Readonly<{
  vertexId: string
  x: number
  z: number
}>

export type FoldPreviewFaceModel = Readonly<{
  id: string
  polygon: readonly FoldPreviewWorldVertex[]
}>

export type FoldPreviewHingeModel = Readonly<{
  edgeId: string
  start: FoldPreviewWorldVertex
  end: FoldPreviewWorldVertex
  /** Unit vector in the world XZ plane; Three.js uses `(x, 0, z)`. */
  axis: Readonly<{ x: number; z: number }>
  assignment: FoldAssignment
  /** Right-hand rotation multiplier around `axis`: mountain +1, valley -1. */
  rotationSign: 1 | -1
}>

type FoldPreviewModelBase = Readonly<{
  projectId: string
  revision: number
  /** World units per millimetre, also used to scale visual paper thickness. */
  worldUnitsPerMillimetre: number
  paperCenter: Readonly<{ x: number; y: number }>
  worldBounds: Readonly<{
    minX: number
    minZ: number
    maxX: number
    maxZ: number
  }>
}>

export type PlanarFoldPreviewModel = FoldPreviewModelBase & Readonly<{
  kind: 'planar'
  faces: readonly [FoldPreviewFaceModel]
  fixedFace: FoldPreviewFaceModel
  movingFace: null
  hinge: null
}>

export type SingleFoldPreviewModel = FoldPreviewModelBase & Readonly<{
  kind: 'single_fold'
  /** Geometric left/right order, independent of the FaceKey serialization order. */
  faces: readonly [FoldPreviewFaceModel, FoldPreviewFaceModel]
  fixedFace: FoldPreviewFaceModel
  movingFace: FoldPreviewFaceModel
  hinge: FoldPreviewHingeModel
}>

export type FoldPreviewModel = PlanarFoldPreviewModel | SingleFoldPreviewModel

type UnknownRecord = Record<string, unknown>
type PaperPoint = Readonly<{ vertexId: string; x: number; y: number }>
type SourceEdge = Readonly<{
  id: string
  start: string | null
  end: string | null
  kind: 'mountain' | 'valley' | 'auxiliary' | 'boundary' | 'cut'
}>
type PreviewFrame = FoldPreviewModelBase & Readonly<{
  toWorld: (point: PaperPoint) => FoldPreviewWorldVertex | null
}>
type ParsedHalfEdge = Readonly<{
  edge: string
  origin: string
  destination: string
}>
type ParsedFace = Readonly<{
  id: string
  key: readonly number[]
  halfEdges: readonly ParsedHalfEdge[]
  paperPolygon: readonly PaperPoint[]
  worldFace: FoldPreviewFaceModel
}>
type ParsedIncidence =
  | Readonly<{ kind: 'boundary'; edge: string; material: string }>
  | Readonly<{
      kind: 'hinge'
      edge: string
      left: string
      right: string
      assignment: FoldAssignment
    }>
  | Readonly<{ kind: 'auxiliary_ignored'; edge: string }>

type ParsedProject = Readonly<{
  projectId: string
  revision: number
  positions: ReadonlyMap<string, PaperPoint>
  edges: ReadonlyMap<string, SourceEdge>
  frame: PreviewFrame
}>

/**
 * Converts a revision-bound Rust topology response into renderer-ready data.
 *
 * The conversion is deliberately fail-closed. It never combines records from
 * different project revisions, and returns `null` for an unsupported or
 * malformed topology instead of guessing a visually plausible sheet.
 */
export function buildFoldPreviewModel(
  project: ProjectSnapshot | null | undefined,
  topology: ProjectTopologyResponse | null | undefined,
): FoldPreviewModel | null {
  const parsedProject = parseProject(project)
  if (!parsedProject || !isRecord(topology)) return null

  const topologyProjectId = canonicalEntityId(topology.project_id)
  const topologyRevision = revisionNumber(topology.revision)
  if (
    topologyProjectId !== parsedProject.projectId
    || topologyRevision !== parsedProject.revision
    || topology.simulation_ready !== true
    || !Array.isArray(topology.issues)
    || !hasOnlyWellFormedWarnings(topology.issues)
    || !isRecord(topology.snapshot)
  ) return null

  const snapshot = topology.snapshot
  if (
    revisionNumber(snapshot.source_revision) !== parsedProject.revision
    || !Array.isArray(snapshot.faces)
    || !Array.isArray(snapshot.edge_incidence)
    || !Array.isArray(snapshot.hinge_adjacency)
  ) return null

  const faces = parseFaces(snapshot.faces, parsedProject.positions, parsedProject.frame)
  const incidences = parseIncidences(snapshot.edge_incidence)
  if (!faces || !incidences || !incidencesMatchProject(parsedProject.edges, faces, incidences)) {
    return null
  }

  const base = modelBase(parsedProject.frame)
  const hinges = incidences.filter(
    (incidence): incidence is Extract<ParsedIncidence, { kind: 'hinge' }> =>
      incidence.kind === 'hinge',
  )

  if (faces.length === 1) {
    if (hinges.length !== 0 || snapshot.hinge_adjacency.length !== 0) return null
    const fixedFace = faces[0].worldFace
    return {
      ...base,
      kind: 'planar',
      faces: [fixedFace],
      fixedFace,
      movingFace: null,
      hinge: null,
    }
  }

  if (faces.length !== 2 || hinges.length !== 1 || snapshot.hinge_adjacency.length !== 1) {
    return null
  }
  const hinge = hinges[0]
  if (hinge.left === hinge.right) return null
  const fixed = faces.find((face) => face.id === hinge.left)
  const moving = faces.find((face) => face.id === hinge.right)
  if (!fixed || !moving) return null

  const fixedHingeEdges = fixed.halfEdges.filter((edge) => edge.edge === hinge.edge)
  const movingHingeEdges = moving.halfEdges.filter((edge) => edge.edge === hinge.edge)
  if (fixedHingeEdges.length !== 1 || movingHingeEdges.length !== 1) return null
  const canonicalEdge = fixedHingeEdges[0]
  const oppositeEdge = movingHingeEdges[0]
  if (
    canonicalEdge.origin >= canonicalEdge.destination
    || oppositeEdge.origin !== canonicalEdge.destination
    || oppositeEdge.destination !== canonicalEdge.origin
  ) return null

  const sourceHinge = parsedProject.edges.get(hinge.edge)
  if (
    !sourceHinge
    || sourceHinge.kind !== hinge.assignment
    || !sourceHinge.start
    || !sourceHinge.end
    || !sameUndirectedEndpoints(
      sourceHinge.start,
      sourceHinge.end,
      canonicalEdge.origin,
      canonicalEdge.destination,
    )
  ) return null

  const adjacency = parseAdjacency(snapshot.hinge_adjacency[0])
  if (
    !adjacency
    || adjacency.edge !== hinge.edge
    || adjacency.assignment !== hinge.assignment
    || !sameUnorderedPair(adjacency.first, adjacency.second, hinge.left, hinge.right)
  ) return null

  const firstFace = faces.find((face) => face.id === adjacency.first)
  const secondFace = faces.find((face) => face.id === adjacency.second)
  if (!firstFace || !secondFace || compareFaceKeys(firstFace.key, secondFace.key) >= 0) return null

  const startPaper = parsedProject.positions.get(canonicalEdge.origin)
  const endPaper = parsedProject.positions.get(canonicalEdge.destination)
  if (!startPaper || !endPaper) return null
  const start = parsedProject.frame.toWorld(startPaper)
  const end = parsedProject.frame.toWorld(endPaper)
  if (!start || !end) return null
  const deltaX = end.x - start.x
  const deltaZ = end.z - start.z
  const length = Math.hypot(deltaX, deltaZ)
  if (!isPositiveFinite(length)) return null
  const axisX = normalizeZero(deltaX / length)
  const axisZ = normalizeZero(deltaZ / length)
  if (!Number.isFinite(axisX) || !Number.isFinite(axisZ)) return null

  const fixedFace = fixed.worldFace
  const movingFace = moving.worldFace
  return {
    ...base,
    kind: 'single_fold',
    faces: [fixedFace, movingFace],
    fixedFace,
    movingFace,
    hinge: {
      edgeId: hinge.edge,
      start,
      end,
      axis: { x: axisX, z: axisZ },
      assignment: hinge.assignment,
      rotationSign: hinge.assignment === 'mountain' ? 1 : -1,
    },
  }
}

function parseProject(project: ProjectSnapshot | null | undefined): ParsedProject | null {
  if (!isRecord(project)) return null
  const projectId = canonicalEntityId(project.project_id)
  const revision = revisionNumber(project.revision)
  const pattern = isRecord(project.crease_pattern) ? project.crease_pattern : null
  const paper = isRecord(project.paper) ? project.paper : null
  if (
    !projectId
    || revision === null
    || !pattern
    || !paper
    || !Array.isArray(pattern.vertices)
    || !Array.isArray(pattern.edges)
    || !Array.isArray(paper.boundary_vertices)
  ) return null

  const boundaryIds: string[] = []
  const seenBoundaryIds = new Set<string>()
  for (const rawId of paper.boundary_vertices) {
    const id = canonicalEntityId(rawId)
    if (!id || seenBoundaryIds.has(id)) return null
    seenBoundaryIds.add(id)
    boundaryIds.push(id)
  }
  if (boundaryIds.length < 3) return null

  // Auxiliary geometry is annotation-only for this topology slice. Preserve
  // global record-ID checks, but do not make a missing/non-finite Auxiliary
  // endpoint prevent an otherwise safe material preview.
  const edges = new Map<string, SourceEdge>()
  const participantVertexIds = new Set(boundaryIds)
  for (const rawEdge of pattern.edges) {
    if (!isRecord(rawEdge)) return null
    const id = canonicalEntityId(rawEdge.id)
    const kind = sourceEdgeKind(rawEdge.kind)
    if (!id || !kind || edges.has(id)) return null
    const start = canonicalEntityId(rawEdge.start)
    const end = canonicalEntityId(rawEdge.end)
    if (!start || !end) return null
    if (kind !== 'auxiliary') {
      if (start === end) return null
      participantVertexIds.add(start)
      participantVertexIds.add(end)
    }
    edges.set(id, { id, start, end, kind })
  }

  const positions = new Map<string, PaperPoint>()
  const allVertexIds = new Set<string>()
  const occupiedParticipantPositions = new Set<string>()
  for (const rawVertex of pattern.vertices) {
    if (!isRecord(rawVertex)) return null
    const id = canonicalEntityId(rawVertex.id)
    if (!id || allVertexIds.has(id)) return null
    allVertexIds.add(id)
    if (!participantVertexIds.has(id)) continue
    if (!isRecord(rawVertex.position)) return null
    const x = finiteNumber(rawVertex.position.x)
    const y = finiteNumber(rawVertex.position.y)
    if (x === null || y === null) return null
    const positionKey = JSON.stringify([normalizeZero(x), normalizeZero(y)])
    if (occupiedParticipantPositions.has(positionKey)) return null
    occupiedParticipantPositions.add(positionKey)
    positions.set(id, { vertexId: id, x: normalizeZero(x), y: normalizeZero(y) })
  }
  if (
    [...participantVertexIds].some((id) => !positions.has(id))
    || !boundaryEdgesMatchProject(boundaryIds, edges)
  ) return null

  const frame = createPreviewFrame(projectId, revision, boundaryIds, positions)
  return frame ? { projectId, revision, positions, edges, frame } : null
}

function boundaryEdgesMatchProject(
  boundaryIds: readonly string[],
  edges: ReadonlyMap<string, SourceEdge>,
) {
  const expectedPairs = new Set<string>()
  for (let index = 0; index < boundaryIds.length; index += 1) {
    expectedPairs.add(undirectedPairKey(
      boundaryIds[index],
      boundaryIds[(index + 1) % boundaryIds.length],
    ))
  }
  const actualPairs = new Set<string>()
  for (const edge of edges.values()) {
    if (edge.kind !== 'boundary') continue
    if (!edge.start || !edge.end) return false
    const key = undirectedPairKey(edge.start, edge.end)
    if (actualPairs.has(key)) return false
    actualPairs.add(key)
  }
  return expectedPairs.size === actualPairs.size
    && [...expectedPairs].every((key) => actualPairs.has(key))
}

function createPreviewFrame(
  projectId: string,
  revision: number,
  boundaryIds: readonly string[],
  positions: ReadonlyMap<string, PaperPoint>,
): PreviewFrame | null {
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const id of boundaryIds) {
    const point = positions.get(id)
    if (!point) return null
    minX = Math.min(minX, point.x)
    minY = Math.min(minY, point.y)
    maxX = Math.max(maxX, point.x)
    maxY = Math.max(maxY, point.y)
  }
  const width = maxX - minX
  const height = maxY - minY
  const largestDimension = Math.max(width, height)
  const centerX = minX / 2 + maxX / 2
  const centerY = minY / 2 + maxY / 2
  const normalizedWidth = width / largestDimension
  const normalizedHeight = height / largestDimension
  const scale = MAX_FOLD_PREVIEW_WORLD_SIZE / largestDimension
  if (
    !isPositiveFinite(width)
    || !isPositiveFinite(height)
    || !isPositiveFinite(normalizedWidth)
    || !isPositiveFinite(normalizedHeight)
    || !isPositiveFinite(scale)
    || !Number.isFinite(centerX)
    || !Number.isFinite(centerY)
  ) return null

  const toWorld = (point: PaperPoint): FoldPreviewWorldVertex | null => {
    // Normalize relative to the minimum before centering. A mathematical
    // midpoint may be unrepresentable when min/max are adjacent huge floats.
    const x = normalizeZero((
      (point.x - minX) / largestDimension - normalizedWidth / 2
    ) * MAX_FOLD_PREVIEW_WORLD_SIZE)
    const z = normalizeZero(-(
      (point.y - minY) / largestDimension - normalizedHeight / 2
    ) * MAX_FOLD_PREVIEW_WORLD_SIZE)
    return Number.isFinite(x) && Number.isFinite(z)
      ? { vertexId: point.vertexId, x, z }
      : null
  }
  const halfWorldWidth = normalizeZero(normalizedWidth * MAX_FOLD_PREVIEW_WORLD_SIZE / 2)
  const halfWorldHeight = normalizeZero(normalizedHeight * MAX_FOLD_PREVIEW_WORLD_SIZE / 2)
  if (!isPositiveFinite(halfWorldWidth) || !isPositiveFinite(halfWorldHeight)) return null

  return {
    projectId,
    revision,
    worldUnitsPerMillimetre: scale,
    paperCenter: { x: normalizeZero(centerX), y: normalizeZero(centerY) },
    worldBounds: {
      minX: -halfWorldWidth,
      minZ: -halfWorldHeight,
      maxX: halfWorldWidth,
      maxZ: halfWorldHeight,
    },
    toWorld,
  }
}

function parseFaces(
  rawFaces: readonly unknown[],
  positions: ReadonlyMap<string, PaperPoint>,
  frame: PreviewFrame,
): ParsedFace[] | null {
  if (rawFaces.length < 1 || rawFaces.length > 2) return null
  const faces: ParsedFace[] = []
  const faceIds = new Set<string>()
  const faceKeys = new Set<string>()
  for (const rawFace of rawFaces) {
    if (!isRecord(rawFace) || !isRecord(rawFace.outer)) return null
    const id = canonicalEntityId(rawFace.id)
    const key = faceKey(rawFace.key)
    const area = positiveNumber(rawFace.area)
    const signedDoubleArea = positiveNumber(rawFace.outer.signed_double_area)
    if (
      !id
      || !key
      || area === null
      || signedDoubleArea === null
      || area !== signedDoubleArea * 0.5
      || faceIds.has(id)
    ) return null
    const keyToken = key.join(',')
    if (faceKeys.has(keyToken) || !Array.isArray(rawFace.outer.half_edges)) return null
    faceIds.add(id)
    faceKeys.add(keyToken)

    const halfEdges = parseHalfEdges(rawFace.outer.half_edges, positions)
    if (!halfEdges) return null
    const paperPolygon = halfEdges.map((edge) => positions.get(edge.origin))
    if (paperPolygon.some((point) => !point)) return null
    const resolvedPolygon = paperPolygon as PaperPoint[]
    const worldPolygon = resolvedPolygon.map((point) => frame.toWorld(point))
    if (worldPolygon.some((point) => !point)) return null
    const resolvedWorldPolygon = worldPolygon as FoldPreviewWorldVertex[]
    if (!hasRenderableCounterClockwiseArea(resolvedWorldPolygon)) return null

    faces.push({
      id,
      key,
      halfEdges,
      paperPolygon: resolvedPolygon,
      worldFace: { id, polygon: resolvedWorldPolygon },
    })
  }
  return faces
}

function parseHalfEdges(
  rawHalfEdges: readonly unknown[],
  positions: ReadonlyMap<string, PaperPoint>,
): ParsedHalfEdge[] | null {
  if (rawHalfEdges.length < 3) return null
  const halfEdges: ParsedHalfEdge[] = []
  const edgeIds = new Set<string>()
  const origins = new Set<string>()
  for (const rawHalfEdge of rawHalfEdges) {
    if (!isRecord(rawHalfEdge)) return null
    const edge = canonicalEntityId(rawHalfEdge.edge)
    const origin = canonicalEntityId(rawHalfEdge.origin)
    const destination = canonicalEntityId(rawHalfEdge.destination)
    const originPosition = origin ? positions.get(origin) : null
    const destinationPosition = destination ? positions.get(destination) : null
    if (
      !edge
      || !origin
      || !destination
      || origin === destination
      || !originPosition
      || !destinationPosition
      || (originPosition.x === destinationPosition.x && originPosition.y === destinationPosition.y)
      || edgeIds.has(edge)
      || origins.has(origin)
    ) return null
    edgeIds.add(edge)
    origins.add(origin)
    halfEdges.push({ edge, origin, destination })
  }
  for (let index = 0; index < halfEdges.length; index += 1) {
    if (halfEdges[index].destination !== halfEdges[(index + 1) % halfEdges.length].origin) {
      return null
    }
  }
  return halfEdges
}

function hasRenderableCounterClockwiseArea(polygon: readonly FoldPreviewWorldVertex[]) {
  // World Z is negative paper Y, so a valid CCW paper walk has negative XZ area.
  let signedDoubleArea = 0
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    signedDoubleArea += current.x * next.z - next.x * current.z
  }
  return Number.isFinite(signedDoubleArea) && signedDoubleArea < 0
}

function parseIncidences(rawIncidences: readonly unknown[]): ParsedIncidence[] | null {
  const incidences: ParsedIncidence[] = []
  const edgeIds = new Set<string>()
  for (const rawEntry of rawIncidences) {
    if (!Array.isArray(rawEntry) || rawEntry.length !== 2 || !isRecord(rawEntry[1])) return null
    const edge = canonicalEntityId(rawEntry[0])
    const incidence = rawEntry[1]
    const kind = incidence.kind
    if (!edge || edgeIds.has(edge)) return null
    edgeIds.add(edge)
    if (kind === 'boundary') {
      const material = canonicalEntityId(incidence.material)
      if (!material) return null
      incidences.push({ kind, edge, material })
    } else if (kind === 'hinge') {
      const left = canonicalEntityId(incidence.left)
      const right = canonicalEntityId(incidence.right)
      const assignment = foldAssignment(incidence.assignment)
      if (!left || !right || !assignment) return null
      incidences.push({ kind, edge, left, right, assignment })
    } else if (kind === 'auxiliary_ignored') {
      incidences.push({ kind, edge })
    } else {
      return null
    }
  }
  return incidences
}

function incidencesMatchProject(
  sourceEdges: ReadonlyMap<string, SourceEdge>,
  faces: readonly ParsedFace[],
  incidences: readonly ParsedIncidence[],
) {
  if (sourceEdges.size !== incidences.length) return false
  const faceIds = new Set(faces.map((face) => face.id))
  const occurrences = new Map<string, string[]>()
  for (const face of faces) {
    for (const halfEdge of face.halfEdges) {
      const source = sourceEdges.get(halfEdge.edge)
      if (
        !source
        || source.kind === 'auxiliary'
        || !source.start
        || !source.end
        || !sameUndirectedEndpoints(
          source.start,
          source.end,
          halfEdge.origin,
          halfEdge.destination,
        )
      ) return false
      const materials = occurrences.get(halfEdge.edge) ?? []
      materials.push(face.id)
      occurrences.set(halfEdge.edge, materials)
    }
  }

  for (const incidence of incidences) {
    const source = sourceEdges.get(incidence.edge)
    if (!source) return false
    const materials = occurrences.get(incidence.edge) ?? []
    if (incidence.kind === 'boundary') {
      if (
        source.kind !== 'boundary'
        || !faceIds.has(incidence.material)
        || materials.length !== 1
        || materials[0] !== incidence.material
      ) return false
    } else if (incidence.kind === 'auxiliary_ignored') {
      if (source.kind !== 'auxiliary' || materials.length !== 0) return false
    } else if (
      source.kind !== incidence.assignment
      || incidence.left === incidence.right
      || !faceIds.has(incidence.left)
      || !faceIds.has(incidence.right)
      || materials.length !== 2
      || !materials.includes(incidence.left)
      || !materials.includes(incidence.right)
    ) return false
  }
  return true
}

function parseAdjacency(rawAdjacency: unknown) {
  if (!isRecord(rawAdjacency)) return null
  const edge = canonicalEntityId(rawAdjacency.edge)
  const first = canonicalEntityId(rawAdjacency.first)
  const second = canonicalEntityId(rawAdjacency.second)
  const assignment = foldAssignment(rawAdjacency.assignment)
  return edge && first && second && first !== second && assignment
    ? { edge, first, second, assignment }
    : null
}

function modelBase(frame: PreviewFrame): FoldPreviewModelBase {
  return {
    projectId: frame.projectId,
    revision: frame.revision,
    worldUnitsPerMillimetre: frame.worldUnitsPerMillimetre,
    paperCenter: frame.paperCenter,
    worldBounds: frame.worldBounds,
  }
}

function hasOnlyWellFormedWarnings(issues: readonly unknown[]) {
  return issues.every((issue) => {
    if (!isRecord(issue) || issue.severity !== 'warning' || !isRecord(issue.kind)) return false
    return nonEmptyString(issue.kind.kind) !== null
  })
}

function compareFaceKeys(first: readonly number[], second: readonly number[]) {
  for (let index = 0; index < 32; index += 1) {
    const difference = first[index] - second[index]
    if (difference !== 0) return difference
  }
  return 0
}

function faceKey(value: unknown): number[] | null {
  if (
    !Array.isArray(value)
    || value.length !== 32
    || !value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  ) return null
  return value as number[]
}

function sameUndirectedEndpoints(
  firstStart: string,
  firstEnd: string,
  secondStart: string,
  secondEnd: string,
) {
  return (firstStart === secondStart && firstEnd === secondEnd)
    || (firstStart === secondEnd && firstEnd === secondStart)
}

function sameUnorderedPair(first: string, second: string, left: string, right: string) {
  return (first === left && second === right) || (first === right && second === left)
}

function undirectedPairKey(first: string, second: string) {
  return JSON.stringify(first < second ? [first, second] : [second, first])
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function nonEmptyString(value: unknown) {
  return typeof value === 'string' && value.trim().length > 0 ? value : null
}

function canonicalEntityId(value: unknown) {
  return typeof value === 'string' && CANONICAL_ENTITY_ID.test(value) ? value : null
}

function finiteNumber(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function positiveNumber(value: unknown) {
  const number = finiteNumber(value)
  return number !== null && number > 0 ? number : null
}

function revisionNumber(value: unknown) {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null
}

function sourceEdgeKind(value: unknown): SourceEdge['kind'] | null {
  return value === 'mountain'
    || value === 'valley'
    || value === 'auxiliary'
    || value === 'boundary'
    || value === 'cut'
    ? value
    : null
}

function foldAssignment(value: unknown): FoldAssignment | null {
  return value === 'mountain' || value === 'valley' ? value : null
}

function isPositiveFinite(value: number) {
  return Number.isFinite(value) && value > 0
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
