import assert from 'node:assert/strict'
import test from 'node:test'

import {
  BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
  BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
  normalizeBoundaryLengthAuthorityV1,
} from '../src/lib/boundaryLengthAuthority.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'

const INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'
const VERTEX_IDS = [
  '30000000-0000-4000-8000-000000000003',
  '30000000-0000-4000-8000-000000000004',
  '30000000-0000-4000-8000-000000000005',
  '30000000-0000-4000-8000-000000000006',
] as const
const EDGE_IDS = [
  '40000000-0000-4000-8000-000000000003',
  '40000000-0000-4000-8000-000000000004',
  '40000000-0000-4000-8000-000000000005',
  '40000000-0000-4000-8000-000000000006',
] as const

test('native boundary length authority is revision-bound and bit-exact', () => {
  const snapshot = context()
  const wire = authority(snapshot)
  const parsed = normalizeBoundaryLengthAuthorityV1(wire, snapshot)

  assert.equal(parsed?.status, 'available')
  assert.equal(parsed?.revision, 7)
  assert.deepEqual(
    parsed?.entries.map((entry) => entry.edge_id),
    EDGE_IDS,
  )
  assert.equal(parsed?.entries[0]?.length_mm, 400)
  assert.deepEqual(parsed?.entries[0]?.length_bits_be, float64Bytes(400))

  const oneUlpAbove = nextUp(400)
  wire.entries[0].length_mm = oneUlpAbove
  wire.entries[0].length_bits_be = float64Bytes(oneUlpAbove)
  const oneUlpParsed = normalizeBoundaryLengthAuthorityV1(wire, snapshot)
  assert.ok(Object.is(oneUlpParsed?.entries[0]?.length_mm, oneUlpAbove))
})

test('stale OCC, model drift, forged bits and unknown fields fail closed', () => {
  const snapshot = context()
  const mutations: Array<(wire: ReturnType<typeof authority>) => void> = [
    (wire) => { wire.revision += 1 },
    (wire) => { wire.project_instance_id = PROJECT_ID },
    (wire) => { wire.project_id = INSTANCE_ID },
    (wire) => { wire.model_id = 'future-model' },
    (wire) => { wire.transcendental_model_id = 'future-transcendental' },
    (wire) => { wire.entries[0].length_bits_be = float64Bytes(401) },
    (wire) => { Object.assign(wire, { future: true }) },
    (wire) => { Object.assign(wire.entries[0], { future: true }) },
  ]

  for (const mutate of mutations) {
    const wire = authority(snapshot)
    mutate(wire)
    assert.equal(normalizeBoundaryLengthAuthorityV1(wire, snapshot), null)
  }
})

test('duplicate, missing, extra and ambiguous topology cannot carry authority', () => {
  const duplicateId = context()
  duplicateId.crease_pattern.edges[1].id = EDGE_IDS[0]
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(authority(duplicateId), duplicateId),
    null,
  )

  const missing = context()
  missing.crease_pattern.edges.pop()
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(authority(missing), missing),
    null,
  )

  const extra = context()
  extra.crease_pattern.edges.push({
    id: '40000000-0000-4000-8000-000000000007',
    start: VERTEX_IDS[0],
    end: VERTEX_IDS[2],
    kind: 'boundary',
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(authority(extra), extra),
    null,
  )

  const ambiguous = context()
  ambiguous.crease_pattern.edges.push({
    id: '40000000-0000-4000-8000-000000000007',
    start: VERTEX_IDS[1],
    end: VERTEX_IDS[0],
    kind: 'boundary',
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(authority(ambiguous), ambiguous),
    null,
  )

  const danglingNonBoundary = context()
  danglingNonBoundary.crease_pattern.edges.push({
    id: '40000000-0000-4000-8000-000000000007',
    start: VERTEX_IDS[0],
    end: '30000000-0000-4000-8000-000000000007',
    kind: 'mountain',
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(
      authority(danglingNonBoundary),
      danglingNonBoundary,
    ),
    null,
  )
})

test('unavailable authority is explicit, empty and still OCC-bound', () => {
  const snapshot = context()
  const wire = authority(snapshot)
  wire.status = 'unavailable'
  wire.entries = []
  const parsed = normalizeBoundaryLengthAuthorityV1(wire, snapshot)
  assert.equal(parsed?.status, 'unavailable')
  assert.deepEqual(parsed?.entries, [])

  wire.entries = authority(snapshot).entries
  assert.equal(normalizeBoundaryLengthAuthorityV1(wire, snapshot), null)
})

test('authority admission snapshots own data without invoking hostile values', () => {
  const snapshot = context()
  const symbolWire = authority(snapshot) as Record<PropertyKey, unknown>
  symbolWire[Symbol('future')] = true
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(symbolWire, snapshot),
    null,
  )

  let getterCalls = 0
  const accessorEntry = authority(snapshot)
  Object.defineProperty(accessorEntry.entries[0], 'length_mm', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(accessorEntry, snapshot),
    null,
  )

  const accessorArray = authority(snapshot)
  Object.defineProperty(accessorArray.entries, '0', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(accessorArray, snapshot),
    null,
  )

  const accessorEdge = context()
  Object.defineProperty(accessorEdge.crease_pattern.edges[0], 'kind', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.equal(
    normalizeBoundaryLengthAuthorityV1(authority(accessorEdge), accessorEdge),
    null,
  )
  assert.equal(getterCalls, 0)

  const sparse = authority(snapshot)
  delete sparse.entries[0]
  assert.equal(normalizeBoundaryLengthAuthorityV1(sparse, snapshot), null)
})

function context() {
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 7,
    paper: {
      boundary_vertices: [...VERTEX_IDS],
    },
    crease_pattern: {
      vertices: [
        { id: VERTEX_IDS[0], position: { x: 0, y: 0 } },
        { id: VERTEX_IDS[1], position: { x: 400, y: 0 } },
        { id: VERTEX_IDS[2], position: { x: 400, y: 200 } },
        { id: VERTEX_IDS[3], position: { x: 0, y: 200 } },
      ],
      edges: [
        {
          id: EDGE_IDS[0],
          start: VERTEX_IDS[0],
          end: VERTEX_IDS[1],
          kind: 'boundary',
        },
        {
          id: EDGE_IDS[1],
          start: VERTEX_IDS[1],
          end: VERTEX_IDS[2],
          kind: 'boundary',
        },
        {
          id: EDGE_IDS[2],
          start: VERTEX_IDS[2],
          end: VERTEX_IDS[3],
          kind: 'boundary',
        },
        {
          id: EDGE_IDS[3],
          start: VERTEX_IDS[3],
          end: VERTEX_IDS[0],
          kind: 'boundary',
        },
      ],
    },
  }
}

function authority(snapshot: ReturnType<typeof context>) {
  const lengths = [400, 200, 400, 200]
  return {
    schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
    model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1 as string,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1 as string,
    project_instance_id: snapshot.project_instance_id,
    project_id: snapshot.project_id,
    revision: snapshot.revision,
    status: 'available' as 'available' | 'unavailable',
    entries: snapshot.paper.boundary_vertices.map((start, boundaryIndex) => ({
      boundary_index: boundaryIndex,
      edge_id: EDGE_IDS[boundaryIndex],
      start_vertex_id: start,
      end_vertex_id:
        snapshot.paper.boundary_vertices[
          (boundaryIndex + 1) % snapshot.paper.boundary_vertices.length
        ],
      length_mm: lengths[boundaryIndex],
      length_bits_be: float64Bytes(lengths[boundaryIndex]),
    })),
  }
}

function float64Bytes(value: number): number[] {
  const buffer = new ArrayBuffer(8)
  new DataView(buffer).setFloat64(0, value, false)
  return [...new Uint8Array(buffer)]
}

function nextUp(value: number): number {
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value, false)
  const bits = view.getBigUint64(0, false)
  view.setBigUint64(0, bits + 1n, false)
  return view.getFloat64(0, false)
}
