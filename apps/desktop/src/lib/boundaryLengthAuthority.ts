import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from './deterministicTranscendentalModel.ts'

export const BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1 = 1
export const BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1 =
  'ori_boundary_edge_length_binary64_native_v1'

export type BoundaryLengthAuthorityEntryV1 = Readonly<{
  boundary_index: number
  edge_id: string
  start_vertex_id: string
  end_vertex_id: string
  length_mm: number
  length_bits_be: readonly number[]
}>

export type BoundaryLengthAuthorityV1 = Readonly<{
  schema_version: typeof BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1
  model_id: typeof BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1
  transcendental_model_id: typeof DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
  project_instance_id: string
  project_id: string
  revision: number
  status: 'available' | 'unavailable'
  entries: readonly BoundaryLengthAuthorityEntryV1[]
}>

type AuthoritySnapshotContext = Readonly<{
  project_instance_id: unknown
  project_id: unknown
  revision: unknown
  paper: Readonly<{ boundary_vertices?: unknown }>
  crease_pattern: Readonly<{
    vertices?: unknown
    edges?: unknown
  }>
}>

const MAX_AUTHORITY_ENTRIES = 1_000_000

export function normalizeBoundaryLengthAuthorityV1(
  value: unknown,
  snapshot: AuthoritySnapshotContext,
): BoundaryLengthAuthorityV1 | null {
  const snapshotRecord = dataRecord(snapshot)
  if (!snapshotRecord) return null
  const record = exactDataRecord(value, [
    'schema_version',
    'model_id',
    'transcendental_model_id',
    'project_instance_id',
    'project_id',
    'revision',
    'status',
    'entries',
  ])
  if (
    !record
    || record.schema_version !== BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1
    || record.model_id !== BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1
    || record.transcendental_model_id
      !== DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    || !isCanonicalNonNilUuid(record.project_instance_id)
    || record.project_instance_id !== snapshotRecord.project_instance_id
    || !isCanonicalNonNilUuid(record.project_id)
    || record.project_id !== snapshotRecord.project_id
    || !isNonNegativeSafeInteger(record.revision)
    || record.revision !== snapshotRecord.revision
    || (record.status !== 'available' && record.status !== 'unavailable')
  ) return null
  const wireEntries = exactDataArray(
    record.entries,
    MAX_AUTHORITY_ENTRIES,
  )
  if (!wireEntries) return null

  if (record.status === 'unavailable') {
    if (wireEntries.length !== 0) return null
    return Object.freeze({
      schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
      model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
      transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
      status: 'unavailable',
      entries: Object.freeze([]),
    })
  }

  const paper = dataRecord(snapshotRecord.paper)
  const creasePattern = dataRecord(snapshotRecord.crease_pattern)
  if (!paper || !creasePattern) return null
  const boundary = exactDataArray(
    paper.boundary_vertices,
    MAX_AUTHORITY_ENTRIES,
  )
  const vertices = exactDataArray(
    creasePattern.vertices,
    MAX_AUTHORITY_ENTRIES,
  )
  const edges = exactDataArray(
    creasePattern.edges,
    MAX_AUTHORITY_ENTRIES,
  )
  if (
    !boundary
    || boundary.length < 3
    || wireEntries.length !== boundary.length
    || !vertices
    || !edges
  ) return null

  const positions = new Map<string, Readonly<{ x: number; y: number }>>()
  for (const value of vertices) {
    const vertex = exactDataRecord(value, ['id', 'position'])
    const position = vertex
      ? exactDataRecord(vertex.position, ['x', 'y'])
      : null
    if (
      !vertex
      || !isCanonicalNonNilUuid(vertex.id)
      || positions.has(vertex.id)
      || !position
      || typeof position.x !== 'number'
      || !Number.isFinite(position.x)
      || typeof position.y !== 'number'
      || !Number.isFinite(position.y)
    ) return null
    positions.set(vertex.id, Object.freeze({ x: position.x, y: position.y }))
  }

  const boundaryIds = new Set<string>()
  for (const id of boundary) {
    if (
      !isCanonicalNonNilUuid(id)
      || boundaryIds.has(id)
      || !positions.has(id)
    ) return null
    boundaryIds.add(id)
  }

  const edgeIds = new Set<string>()
  const boundaryEdges = new Map<string, Readonly<{
    id: string
    start: string
    end: string
  }>>()
  const ambiguousBoundaryPairs = new Set<string>()
  for (const value of edges) {
    const edge = exactDataRecord(value, ['id', 'start', 'end', 'kind'])
    if (
      !edge
      || !isCanonicalNonNilUuid(edge.id)
      || edgeIds.has(edge.id)
      || !isCanonicalNonNilUuid(edge.start)
      || !isCanonicalNonNilUuid(edge.end)
      || edge.start === edge.end
      || !positions.has(edge.start)
      || !positions.has(edge.end)
    ) return null
    edgeIds.add(edge.id)
    if (edge.kind !== 'boundary') continue
    const key = canonicalPairKey(edge.start, edge.end)
    if (boundaryEdges.has(key)) ambiguousBoundaryPairs.add(key)
    else {
      boundaryEdges.set(key, Object.freeze({
        id: edge.id,
        start: edge.start,
        end: edge.end,
      }))
    }
  }
  if (
    boundaryEdges.size !== boundary.length
    || ambiguousBoundaryPairs.size !== 0
  ) return null

  const normalizedEntries: BoundaryLengthAuthorityEntryV1[] = []
  const authorityEdgeIds = new Set<string>()
  for (let index = 0; index < boundary.length; index += 1) {
    const startVertexId = boundary[index] as string
    const endVertexId = boundary[(index + 1) % boundary.length] as string
    const key = canonicalPairKey(startVertexId, endVertexId)
    const expectedEdge = boundaryEdges.get(key)
    const entry = exactDataRecord(wireEntries[index], [
      'boundary_index',
      'edge_id',
      'start_vertex_id',
      'end_vertex_id',
      'length_mm',
      'length_bits_be',
    ])
    if (
      !expectedEdge
      || ambiguousBoundaryPairs.has(key)
      || !entry
      || entry.boundary_index !== index
      || entry.edge_id !== expectedEdge.id
      || authorityEdgeIds.has(entry.edge_id)
      || entry.start_vertex_id !== startVertexId
      || entry.end_vertex_id !== endVertexId
      || typeof entry.length_mm !== 'number'
      || !Number.isFinite(entry.length_mm)
      || entry.length_mm <= 0
    ) return null
    const lengthBitsBe = exactDataArray(entry.length_bits_be, 8)
    if (
      !lengthBitsBe
      || !isByteTuple(lengthBitsBe, 8)
      || !Object.is(
        binary64FromBigEndianBytes(lengthBitsBe),
        entry.length_mm,
      )
    ) return null
    authorityEdgeIds.add(entry.edge_id)
    normalizedEntries.push(Object.freeze({
      boundary_index: index,
      edge_id: entry.edge_id,
      start_vertex_id: startVertexId,
      end_vertex_id: endVertexId,
      length_mm: entry.length_mm,
      length_bits_be: Object.freeze(lengthBitsBe),
    }))
  }

  return Object.freeze({
    schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
    model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    project_instance_id: record.project_instance_id,
    project_id: record.project_id,
    revision: record.revision,
    status: 'available',
    entries: Object.freeze(normalizedEntries),
  })
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = dataRecord(value)
  if (!record) return null
  const actual = Object.keys(record)
  if (actual.length !== keys.length || keys.some((key) => !actual.includes(key))) {
    return null
  }
  return record as Readonly<Record<Keys[number], unknown>>
}

function dataRecord(value: unknown): Readonly<Record<string, unknown>> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  try {
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of Reflect.ownKeys(descriptors)) {
      if (typeof key !== 'string') return null
      const descriptor = descriptors[key]
      if (!('value' in descriptor) || !descriptor.enumerable) return null
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch {
    return null
  }
}

function exactDataArray(value: unknown, maximum: number): unknown[] | null {
  if (!Array.isArray(value)) return null
  try {
    const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
      Record<PropertyKey, PropertyDescriptor | undefined>
    const keys = Reflect.ownKeys(descriptors)
    const length = descriptors.length
    if (
      keys.some((key) => typeof key !== 'string')
      || !length
      || !('value' in length)
      || length.enumerable
      || !isNonNegativeSafeInteger(length.value)
      || length.value > maximum
      || keys.length !== length.value + 1
    ) return null
    const snapshot: unknown[] = []
    for (let index = 0; index < length.value; index += 1) {
      const descriptor = descriptors[String(index)]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot.push(descriptor.value)
    }
    return snapshot
  } catch {
    return null
  }
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && !Object.is(value, -0)
    && value >= 0
}

function isByteTuple(value: unknown, length: number): value is number[] {
  return Array.isArray(value)
    && value.length === length
    && value.every((byte) =>
      Number.isSafeInteger(byte) && byte >= 0 && byte <= 255)
}

function binary64FromBigEndianBytes(bytes: readonly number[]) {
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  bytes.forEach((byte, index) => view.setUint8(index, byte))
  return view.getFloat64(0, false)
}

function canonicalPairKey(first: string, second: string) {
  return first < second ? `${first}:${second}` : `${second}:${first}`
}
