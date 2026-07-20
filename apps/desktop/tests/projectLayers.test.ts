import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_PROJECT_LAYER_ID,
  MAX_LAYER_EDGE_ASSIGNMENTS,
  MAX_PROJECT_LAYERS,
  MAX_PROJECT_LAYER_INDEX_EDGES,
  normalizeProjectLayerDocument,
} from '../src/lib/projectLayers.ts'

const CREASE_LAYER_ID = '10000000-0000-4000-8000-000000000001'
const ANNOTATION_LAYER_ID = '20000000-0000-4000-8000-000000000001'
const EDGE_A = '30000000-0000-4000-8000-000000000001'
const EDGE_B = '40000000-0000-4000-8000-000000000001'
const PATTERN_EDGES = [{ id: EDGE_A }, { id: EDGE_B }] as const

test('normalizes, detaches, freezes, and JSON-round-trips the exact V1 document', () => {
  const source = validDocument()
  const roundTripped = JSON.parse(JSON.stringify(source)) as unknown
  const normalized = normalizeProjectLayerDocument(
    roundTripped,
    PATTERN_EDGES,
  )

  assert.deepEqual(normalized, source)
  assert.notEqual(normalized, roundTripped)
  assert.ok(Object.isFrozen(normalized))
  assert.ok(Object.isFrozen(normalized?.layers))
  assert.ok(Object.isFrozen(normalized?.layers[0]))
  assert.ok(Object.isFrozen(normalized?.edge_assignments))
  assert.ok(Object.isFrozen(normalized?.edge_assignments[0]))

  source.layers[1].name = 'mutated after admission'
  assert.equal(normalized?.layers[1].name, 'Details')
})

test('admits every native serde presentation subset with independent defaults', () => {
  const legacy = {
    id: DEFAULT_PROJECT_LAYER_ID,
    name: 'Crease Pattern',
    content_kind: 'crease_pattern',
  } as const
  const cases = [
    {
      record: legacy,
      expected: { visible: true, locked: false, opacity: 1 },
    },
    {
      record: { ...legacy, visible: false },
      expected: { visible: false, locked: false, opacity: 1 },
    },
    {
      record: { ...legacy, locked: true },
      expected: { visible: true, locked: true, opacity: 1 },
    },
    {
      record: { ...legacy, opacity: 0.35 },
      expected: { visible: true, locked: false, opacity: 0.35 },
    },
    {
      record: { ...legacy, visible: false, locked: true },
      expected: { visible: false, locked: true, opacity: 1 },
    },
    {
      record: { ...legacy, visible: false, opacity: 0.35 },
      expected: { visible: false, locked: false, opacity: 0.35 },
    },
    {
      record: { ...legacy, locked: true, opacity: 0.35 },
      expected: { visible: true, locked: true, opacity: 0.35 },
    },
    {
      record: {
        ...legacy,
        visible: false,
        locked: true,
        opacity: 0.35,
      },
      expected: { visible: false, locked: true, opacity: 0.35 },
    },
  ] as const

  for (const { record, expected } of cases) {
    const normalized = normalizeProjectLayerDocument({
      schema_version: 1,
      layers: [record],
      edge_assignments: [],
    }, [])
    assert.ok(normalized)
    assert.deepEqual(
      {
        visible: normalized.layers[0]?.visible,
        locked: normalized.layers[0]?.locked,
        opacity: normalized.layers[0]?.opacity,
      },
      expected,
    )
  }
})

test('rejects malformed optional presentation fields and unknown layer keys', () => {
  const legacy = {
    id: DEFAULT_PROJECT_LAYER_ID,
    name: 'Crease Pattern',
    content_kind: 'crease_pattern',
  }
  for (const record of [
    { ...legacy, visible: 'false' },
    { ...legacy, locked: 1 },
    { ...legacy, opacity: Number.NaN },
    { ...legacy, opacity: Number.POSITIVE_INFINITY },
    { ...legacy, opacity: -0 },
    { ...legacy, opacity: -0.01 },
    { ...legacy, opacity: 1.01 },
    { ...legacy, future_presentation: true },
  ]) {
    assert.equal(normalizeProjectLayerDocument({
      schema_version: 1,
      layers: [record],
      edge_assignments: [],
    }, []), null)
  }
})

test('rejects every identity, name, kind, assignment, and canonical-order drift', () => {
  const base = validDocument()
  const invalid = [
    null,
    { ...validDocument(), future: true },
    { ...validDocument(), schema_version: 2 },
    { ...validDocument(), layers: [] },
    {
      ...validDocument(),
      layers: [base.layers[1]],
    },
    {
      ...validDocument(),
      layers: [base.layers[0], base.layers[0]],
    },
    withLayer(0, { content_kind: 'annotation' }),
    withLayer(0, { id: '00000000-0000-0000-0000-000000000000' }),
    withLayer(1, { id: 'ABCDEF00-0000-4000-8000-000000000001' }),
    withLayer(1, { name: '   ' }),
    withLayer(1, { name: `bad\u0085name` }),
    withLayer(1, { name: '😀'.repeat(121) }),
    withLayer(1, { content_kind: 'future' }),
    withAssignment(0, { future: true }),
    withAssignment(0, {
      edge: 'ABCDEF00-0000-4000-8000-000000000001',
    }),
    withAssignment(0, { layer: DEFAULT_PROJECT_LAYER_ID }),
    withAssignment(0, {
      layer: '50000000-0000-4000-8000-000000000001',
    }),
    withAssignment(0, { layer: ANNOTATION_LAYER_ID }),
    withAssignment(0, {
      edge: '50000000-0000-4000-8000-000000000001',
    }),
    {
      ...validDocument(),
      edge_assignments: [
        { edge: EDGE_B, layer: CREASE_LAYER_ID },
        { edge: EDGE_A, layer: CREASE_LAYER_ID },
      ],
    },
    {
      ...validDocument(),
      edge_assignments: [
        { edge: EDGE_A, layer: CREASE_LAYER_ID },
        { edge: EDGE_A, layer: CREASE_LAYER_ID },
      ],
    },
  ]

  for (const [index, value] of invalid.entries()) {
    assert.equal(
      normalizeProjectLayerDocument(value, PATTERN_EDGES),
      null,
      `invalid fixture ${index}`,
    )
  }
})

test('enforces inclusive layer and non-relaxable assignment/index ceilings', () => {
  const maximumLayers = [
    validDocument().layers[0],
    ...Array.from({ length: MAX_PROJECT_LAYERS - 1 }, (_, index) => ({
      id: `00000000-0000-4000-8000-${(index + 2).toString(16).padStart(12, '0')}`,
      name: `Layer ${index + 2}`,
      content_kind: 'underlay' as const,
    })),
  ]
  assert.ok(normalizeProjectLayerDocument({
    schema_version: 1,
    layers: maximumLayers,
    edge_assignments: [],
  }, []))
  assert.equal(normalizeProjectLayerDocument({
    schema_version: 1,
    layers: [...maximumLayers, {
      id: '00000000-0000-4000-8000-000000000200',
      name: 'One over',
      content_kind: 'underlay',
    }],
    edge_assignments: [],
  }, []), null)

  assert.equal(normalizeProjectLayerDocument({
    ...validDocument(),
    edge_assignments: new Array(MAX_LAYER_EDGE_ASSIGNMENTS + 1),
  }, PATTERN_EDGES), null)
  assert.equal(normalizeProjectLayerDocument(validDocument(), new Array(
    MAX_PROJECT_LAYER_INDEX_EDGES + 1,
  ).fill(PATTERN_EDGES[0])), null)
  assert.equal(normalizeProjectLayerDocument(
    validDocument(),
    [PATTERN_EDGES[0], PATTERN_EDGES[0]],
  ), null)
})

test('accepts the exact Unicode scalar name limit and rejects hostile values fail-closed', () => {
  assert.ok(normalizeProjectLayerDocument(
    withLayer(1, { name: '😀'.repeat(120) }),
    PATTERN_EDGES,
  ))

  let getterCalls = 0
  const accessor = validDocument()
  Object.defineProperty(accessor, 'layers', {
    enumerable: true,
    get() {
      getterCalls += 1
      return []
    },
  })
  assert.equal(
    normalizeProjectLayerDocument(accessor, PATTERN_EDGES),
    null,
  )
  assert.equal(getterCalls, 0)

  const proxy = new Proxy({}, {
    ownKeys() {
      throw new Error('private layer path')
    },
  })
  assert.equal(
    normalizeProjectLayerDocument({
      ...validDocument(),
      layers: [proxy],
    }, PATTERN_EDGES),
    null,
  )

  const edgeWithAccessor = { id: EDGE_A }
  Object.defineProperty(edgeWithAccessor, 'id', {
    enumerable: true,
    get() {
      throw new Error('must not execute')
    },
  })
  assert.equal(
    normalizeProjectLayerDocument(validDocument(), [edgeWithAccessor]),
    null,
  )
})

function validDocument() {
  return {
    schema_version: 1,
    layers: [
      {
        id: DEFAULT_PROJECT_LAYER_ID,
        name: 'Crease Pattern',
        content_kind: 'crease_pattern',
        visible: true,
        locked: false,
        opacity: 1,
      },
      {
        id: CREASE_LAYER_ID,
        name: 'Details',
        content_kind: 'crease_pattern',
        visible: true,
        locked: false,
        opacity: 1,
      },
      {
        id: ANNOTATION_LAYER_ID,
        name: 'Notes',
        content_kind: 'annotation',
        visible: true,
        locked: false,
        opacity: 1,
      },
    ],
    edge_assignments: [
      { edge: EDGE_A, layer: CREASE_LAYER_ID },
      { edge: EDGE_B, layer: CREASE_LAYER_ID },
    ],
  }
}

function withLayer(index: number, replacement: Record<string, unknown>) {
  const document = validDocument()
  document.layers[index] = {
    ...document.layers[index],
    ...replacement,
  } as typeof document.layers[number]
  return document
}

function withAssignment(
  index: number,
  replacement: Record<string, unknown>,
) {
  const document = validDocument()
  document.edge_assignments[index] = {
    ...document.edge_assignments[index],
    ...replacement,
  } as typeof document.edge_assignments[number]
  return document
}
