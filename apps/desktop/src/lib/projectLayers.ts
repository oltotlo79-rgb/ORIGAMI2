import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export const PROJECT_LAYER_SCHEMA_VERSION = 1 as const
export const MAX_PROJECT_LAYERS = 256
export const MAX_LAYER_EDGE_ASSIGNMENTS = 100_000
export const MAX_PROJECT_LAYER_INDEX_EDGES = 100_000
export const MAX_LAYER_NAME_CHARS = 120
export const DEFAULT_PROJECT_LAYER_ID =
  '00000000-0000-4000-8000-000000000001' as const
export const DEFAULT_PROJECT_LAYER_NAME = 'Crease Pattern' as const

export type LayerContentKindV1 =
  | 'crease_pattern'
  | 'annotation'
  | 'underlay'

export type LayerRecordV1 = Readonly<{
  id: string
  name: string
  content_kind: LayerContentKindV1
}>

export type EdgeLayerAssignmentV1 = Readonly<{
  edge: string
  layer: string
}>

export type ProjectLayerDocumentV1 = Readonly<{
  schema_version: typeof PROJECT_LAYER_SCHEMA_VERSION
  layers: readonly LayerRecordV1[]
  edge_assignments: readonly EdgeLayerAssignmentV1[]
}>

export const DEFAULT_PROJECT_LAYER_DOCUMENT_V1: ProjectLayerDocumentV1 =
  Object.freeze({
    schema_version: PROJECT_LAYER_SCHEMA_VERSION,
    layers: Object.freeze([
      Object.freeze({
        id: DEFAULT_PROJECT_LAYER_ID,
        name: DEFAULT_PROJECT_LAYER_NAME,
        content_kind: 'crease_pattern' as const,
      }),
    ]),
    edge_assignments: Object.freeze([]),
  })

/**
 * Detaches and validates the exact LIN-004 V1 wire document.
 *
 * `patternEdgeRecords` must come from the already-admitted crease-pattern
 * snapshot. Empty assignment maps deliberately avoid indexing that geometry,
 * matching the native legacy-migration contract.
 */
export function normalizeProjectLayerDocument(
  value: unknown,
  patternEdgeRecords: readonly Readonly<{ id: string }>[],
): ProjectLayerDocumentV1 | null {
  try {
    const document = exactDataRecord(value, [
      'schema_version',
      'layers',
      'edge_assignments',
    ])
    if (
      !document
      || document.schema_version !== PROJECT_LAYER_SCHEMA_VERSION
    ) return null

    const layerSource = snapshotExactArray(
      document.layers,
      MAX_PROJECT_LAYERS,
    )
    const assignmentSource = snapshotExactArray(
      document.edge_assignments,
      MAX_LAYER_EDGE_ASSIGNMENTS,
    )
    if (!layerSource || layerSource.length === 0 || !assignmentSource) {
      return null
    }

    const layers: LayerRecordV1[] = []
    const layerKinds = new Map<string, LayerContentKindV1>()
    for (const rawLayer of layerSource) {
      const layer = exactDataRecord(rawLayer, [
        'id',
        'name',
        'content_kind',
      ])
      if (
        !layer
        || !isCanonicalNonNilUuid(layer.id)
        || typeof layer.name !== 'string'
        || !isProjectLayerName(layer.name)
        || !isProjectLayerContentKind(layer.content_kind)
        || layerKinds.has(layer.id)
        || (
          layer.id === DEFAULT_PROJECT_LAYER_ID
          && layer.content_kind !== 'crease_pattern'
        )
      ) return null
      layerKinds.set(layer.id, layer.content_kind)
      layers.push(Object.freeze({
        id: layer.id,
        name: layer.name,
        content_kind: layer.content_kind,
      }))
    }
    if (!layerKinds.has(DEFAULT_PROJECT_LAYER_ID)) return null

    let patternEdgeIndex: Set<string> | null = null
    if (assignmentSource.length > 0) {
      const patternEdgeSource = snapshotExactArray(
        patternEdgeRecords,
        MAX_PROJECT_LAYER_INDEX_EDGES,
      )
      if (!patternEdgeSource) return null
      patternEdgeIndex = new Set<string>()
      for (const rawEdge of patternEdgeSource) {
        const edge = snapshotDataRecord(rawEdge)
        if (
          !edge
          || !isCanonicalNonNilUuid(edge.id)
          || patternEdgeIndex.has(edge.id)
        ) return null
        patternEdgeIndex.add(edge.id)
      }
    }

    const edgeAssignments: EdgeLayerAssignmentV1[] = []
    let previousEdge: string | null = null
    for (const rawAssignment of assignmentSource) {
      const assignment = exactDataRecord(rawAssignment, ['edge', 'layer'])
      if (
        !assignment
        || !isCanonicalNonNilUuid(assignment.edge)
        || !isCanonicalNonNilUuid(assignment.layer)
        || assignment.layer === DEFAULT_PROJECT_LAYER_ID
        || layerKinds.get(assignment.layer) !== 'crease_pattern'
        || !patternEdgeIndex?.has(assignment.edge)
        || (previousEdge !== null && previousEdge >= assignment.edge)
      ) return null
      previousEdge = assignment.edge
      edgeAssignments.push(Object.freeze({
        edge: assignment.edge,
        layer: assignment.layer,
      }))
    }

    return Object.freeze({
      schema_version: PROJECT_LAYER_SCHEMA_VERSION,
      layers: Object.freeze(layers),
      edge_assignments: Object.freeze(edgeAssignments),
    })
  } catch {
    return null
  }
}

export function isProjectLayerContentKind(
  value: unknown,
): value is LayerContentKindV1 {
  return value === 'crease_pattern'
    || value === 'annotation'
    || value === 'underlay'
}

export function isProjectLayerName(value: unknown): value is string {
  if (typeof value !== 'string') return false
  if (
    value.length > MAX_LAYER_NAME_CHARS * 2
    || value.trim().length === 0
    || /\p{Cc}/u.test(value)
  ) return false
  return [...value].length <= MAX_LAYER_NAME_CHARS
}

function snapshotDataRecord(
  value: unknown,
): Record<string, unknown> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key !== 'string') return null
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotDataRecord(value)
  return record && hasExactKeys(record, keys)
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function hasExactKeys(
  record: Readonly<Record<string, unknown>>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(record)
  return actual.length === expected.length
    && expected.every((key) => Object.hasOwn(record, key))
}

function snapshotExactArray(
  value: unknown,
  maximum: number,
): unknown[] | null {
  if (!Array.isArray(value)) return null
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length')
  if (
    !lengthDescriptor
    || !('value' in lengthDescriptor)
    || lengthDescriptor.enumerable
    || !Number.isSafeInteger(lengthDescriptor.value)
    || lengthDescriptor.value < 0
    || lengthDescriptor.value > maximum
  ) return null
  const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
    Record<PropertyKey, PropertyDescriptor>
  const keys = Reflect.ownKeys(descriptors)
  if (
    keys.some((key) => typeof key !== 'string')
    || keys.length !== lengthDescriptor.value + 1
  ) return null
  const result: unknown[] = []
  for (let index = 0; index < lengthDescriptor.value; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    result.push(descriptor.value)
  }
  return result
}
