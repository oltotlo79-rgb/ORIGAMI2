import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createGeometricConstraintPresentation,
  GEOMETRIC_CONSTRAINT_SCHEMA_VERSION,
  MAX_DIRECT_CONFLICT_WITNESS_IDS,
  MAX_GEOMETRIC_CONSTRAINT_RECORDS,
  MAX_GEOMETRIC_CONSTRAINT_REFERENCES,
  normalizeGeometricConstraintDocument,
  normalizeGeometricConstraintPreflightResponse,
} from '../src/lib/geometricConstraints.ts'

const uuid = (index: number) =>
  `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`

const INSTANCE_ID = uuid(1)
const PROJECT_ID = uuid(2)
const VERTEX_1 = uuid(11)
const VERTEX_2 = uuid(12)
const VERTEX_3 = uuid(13)
const VERTEX_4 = uuid(14)
const EDGE_1 = uuid(21)
const EDGE_2 = uuid(22)
const EDGE_3 = uuid(23)
const EDGE_4 = uuid(24)
const EDGE_5 = uuid(25)
const EDGE_6 = uuid(26)
const CONSTRAINT_1 = uuid(101)
const CONSTRAINT_2 = uuid(102)
const CONSTRAINT_3 = uuid(103)

const BINDING = {
  project_instance_id: INSTANCE_ID,
  project_id: PROJECT_ID,
  revision: 7,
}

const ALL_KINDS = [
  {
    kind: 'fixed_length',
    edge: EDGE_1,
    length_mm: 10.5,
  },
  {
    kind: 'fixed_angle',
    vertex: VERTEX_1,
    first_edge: EDGE_1,
    second_edge: EDGE_2,
    angle_degrees: 45,
  },
  {
    kind: 'horizontal',
    edge: EDGE_2,
  },
  {
    kind: 'vertical',
    edge: EDGE_3,
  },
  {
    kind: 'equal_length',
    first_edge: EDGE_1,
    second_edge: EDGE_2,
  },
  {
    kind: 'parallel',
    first_edge: EDGE_2,
    second_edge: EDGE_3,
  },
  {
    kind: 'point_on_line',
    vertex: VERTEX_2,
    line_edge: EDGE_4,
  },
  {
    kind: 'mirror_symmetry',
    first_vertex: VERTEX_1,
    second_vertex: VERTEX_2,
    axis_edge: EDGE_5,
  },
  {
    kind: 'rotational_symmetry',
    center_vertex: VERTEX_1,
    source_vertex: VERTEX_2,
    target_vertex: VERTEX_3,
    angle_degrees: 120,
  },
  {
    kind: 'angle_bisector',
    vertex: VERTEX_4,
    first_edge: EDGE_1,
    second_edge: EDGE_2,
    bisector_edge: EDGE_3,
  },
  {
    kind: 'length_ratio',
    numerator_edge: EDGE_5,
    denominator_edge: EDGE_6,
    ratio: 2,
  },
] as const

const DOCUMENT = {
  schema_version: 1,
  constraints: ALL_KINDS.map((constraint, index) => ({
    id: uuid(1_000 + index),
    constraint,
  })),
}

const DIRECT_CONFLICTS = [
  {
    conflict: {
      kind: 'different_fixed_lengths',
      edge: EDGE_1,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
  },
  {
    conflict: {
      kind: 'different_fixed_angles',
      vertex: VERTEX_1,
      first_edge: EDGE_1,
      second_edge: EDGE_2,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
  },
  {
    conflict: {
      kind: 'different_length_ratios',
      numerator_edge: EDGE_1,
      denominator_edge: EDGE_2,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
  },
  {
    conflict: {
      kind: 'horizontal_and_vertical',
      edge: EDGE_1,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
  },
  {
    conflict: {
      kind: 'equal_length_with_different_fixed_lengths',
      first_edge: EDGE_1,
      second_edge: EDGE_2,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2, CONSTRAINT_3],
  },
  {
    conflict: {
      kind: 'parallel_with_fixed_non_parallel_angle',
      first_edge: EDGE_1,
      second_edge: EDGE_2,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
  },
  {
    conflict: {
      kind: 'parallel_with_perpendicular_orientations',
      horizontal_edge: EDGE_1,
      vertical_edge: EDGE_2,
    },
    constraint_ids: [CONSTRAINT_1, CONSTRAINT_2, CONSTRAINT_3],
  },
] as const

test('normalizes, detaches, and deeply freezes all eleven document kinds', () => {
  const before = structuredClone(DOCUMENT)
  const normalized = normalizeGeometricConstraintDocument(DOCUMENT)

  assert.deepEqual(normalized, DOCUMENT)
  assert.deepEqual(DOCUMENT, before, 'normalization must not mutate its input')
  assert.notEqual(normalized, DOCUMENT)
  assert.notEqual(normalized?.constraints, DOCUMENT.constraints)
  assertDeepFrozen(normalized)
  assert.equal(GEOMETRIC_CONSTRAINT_SCHEMA_VERSION, 1)
})

test('returns fixed Japanese names and target-only summaries for all eleven kinds', () => {
  const expected = [
    ['長さを固定', `辺 ${EDGE_1}`],
    ['角度を固定', `頂点 ${VERTEX_1}／辺 ${EDGE_1}・${EDGE_2}`],
    ['水平', `辺 ${EDGE_2}`],
    ['垂直', `辺 ${EDGE_3}`],
    ['等しい長さ', `辺 ${EDGE_1}・${EDGE_2}`],
    ['平行', `辺 ${EDGE_2}・${EDGE_3}`],
    ['点を直線上に配置', `頂点 ${VERTEX_2}／直線 ${EDGE_4}`],
    [
      '線対称',
      `頂点 ${VERTEX_1}・${VERTEX_2}／対称軸 ${EDGE_5}`,
    ],
    [
      '回転対称',
      `中心 ${VERTEX_1}／対応する頂点 ${VERTEX_2}・${VERTEX_3}`,
    ],
    [
      '角の二等分',
      `頂点 ${VERTEX_4}／角の辺 ${EDGE_1}・${EDGE_2}／二等分線 ${EDGE_3}`,
    ],
    ['長さの比', `分子の辺 ${EDGE_5}／分母の辺 ${EDGE_6}`],
  ]

  const actual = DOCUMENT.constraints.map((record) =>
    createGeometricConstraintPresentation(record))
  assert.deepEqual(
    actual.map((item) => [item?.displayName, item?.targetSummary]),
    expected,
  )
  for (const item of actual) {
    assert.ok(item)
    assertDeepFrozen(item)
    assert.equal(Object.hasOwn(item, 'rawError'), false)
    assert.equal(Object.hasOwn(item, 'coordinates'), false)
    assert.equal(Object.hasOwn(item, 'position'), false)
    assert.equal(Object.hasOwn(item, 'x'), false)
    assert.equal(Object.hasOwn(item, 'y'), false)
  }
})

test('returns fixed English names and target-only summaries for all eleven kinds', () => {
  const expected = [
    ['Fixed length', `Edge ${EDGE_1}`],
    [
      'Fixed angle',
      `Vertex ${VERTEX_1} / Edges ${EDGE_1} · ${EDGE_2}`,
    ],
    ['Horizontal', `Edge ${EDGE_2}`],
    ['Vertical', `Edge ${EDGE_3}`],
    ['Equal length', `Edges ${EDGE_1} · ${EDGE_2}`],
    ['Parallel', `Edges ${EDGE_2} · ${EDGE_3}`],
    ['Point on line', `Vertex ${VERTEX_2} / Line ${EDGE_4}`],
    [
      'Mirror symmetry',
      `Vertices ${VERTEX_1} · ${VERTEX_2} / Symmetry axis ${EDGE_5}`,
    ],
    [
      'Rotational symmetry',
      `Center ${VERTEX_1} / Corresponding vertices ${VERTEX_2} · ${VERTEX_3}`,
    ],
    [
      'Angle bisector',
      `Vertex ${VERTEX_4} / Angle edges ${EDGE_1} · ${EDGE_2} / Bisector ${EDGE_3}`,
    ],
    [
      'Length ratio',
      `Numerator edge ${EDGE_5} / Denominator edge ${EDGE_6}`,
    ],
  ]

  const actual = DOCUMENT.constraints.map((record) =>
    createGeometricConstraintPresentation(record, 'en'))
  assert.deepEqual(
    actual.map((item) => [item?.displayName, item?.targetSummary]),
    expected,
  )
  for (const item of actual) {
    assert.ok(item)
    assertDeepFrozen(item)
    assert.deepEqual(
      Object.keys(item).sort(),
      ['constraintId', 'displayName', 'kind', 'targetSummary'].sort(),
    )
  }
})

test('rejects every missing or unknown object field across all eleven kinds', () => {
  const sparseConstraints = new Array(1)
  const extendedConstraints: unknown[] = []
  Object.defineProperty(extendedConstraints, 'future', {
    enumerable: false,
    value: true,
  })
  for (const malformed of [
    { constraints: [] },
    { schema_version: 1 },
    { schema_version: 0, constraints: [] },
    { schema_version: 2, constraints: [] },
    { schema_version: '1', constraints: [] },
    { schema_version: 1, constraints: {} },
    { schema_version: 1, constraints: sparseConstraints },
    { schema_version: 1, constraints: extendedConstraints },
    { ...DOCUMENT, future: true },
    {
      schema_version: 1,
      constraints: [{ constraint: ALL_KINDS[0] }],
    },
    {
      schema_version: 1,
      constraints: [{ id: CONSTRAINT_1 }],
    },
    {
      schema_version: 1,
      constraints: [{
        id: CONSTRAINT_1,
        constraint: ALL_KINDS[0],
        future: true,
      }],
    },
  ]) {
    assert.equal(normalizeGeometricConstraintDocument(malformed), null)
  }

  for (const kind of ALL_KINDS) {
    for (const key of Object.keys(kind)) {
      const constraint = { ...kind } as Record<string, unknown>
      delete constraint[key]
      assert.equal(normalizeGeometricConstraintDocument(documentOf(constraint)), null)
    }
    assert.equal(
      normalizeGeometricConstraintDocument(documentOf({ ...kind, future: true })),
      null,
    )
  }

  assert.equal(
    normalizeGeometricConstraintDocument(documentOf({ kind: 'future_constraint' })),
    null,
  )
  const symbolDocument = { ...DOCUMENT, [Symbol('private')]: true }
  assert.equal(normalizeGeometricConstraintDocument(symbolDocument), null)
})

test('requires canonical lowercase non-nil UUIDs and unique constraint IDs', () => {
  for (const id of [
    '',
    'not-an-id',
    uuid(0xabc).toUpperCase(),
    '00000000-0000-0000-0000-000000000000',
  ]) {
    assert.equal(
      normalizeGeometricConstraintDocument({
        schema_version: 1,
        constraints: [{ id, constraint: ALL_KINDS[0] }],
      }),
      null,
    )
  }
  for (const id of [
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-4000-7000-000000000001',
    'ffffffff-ffff-ffff-ffff-ffffffffffff',
  ]) {
    assert.notEqual(
      normalizeGeometricConstraintDocument({
        schema_version: 1,
        constraints: [{ id, constraint: ALL_KINDS[0] }],
      }),
      null,
      id,
    )
  }

  assert.equal(
    normalizeGeometricConstraintDocument({
      schema_version: 1,
      constraints: [
        { id: CONSTRAINT_1, constraint: ALL_KINDS[0] },
        { id: CONSTRAINT_1, constraint: ALL_KINDS[1] },
      ],
    }),
    null,
  )
  assert.equal(
    normalizeGeometricConstraintDocument(documentOf({
      ...ALL_KINDS[0],
      edge: uuid(0xdef).toUpperCase(),
    })),
    null,
  )
})

test('rejects repeated operands for every kind that requires distinct roles', () => {
  const repeated = [
    {
      ...ALL_KINDS[1],
      second_edge: ALL_KINDS[1].first_edge,
    },
    {
      ...ALL_KINDS[4],
      second_edge: ALL_KINDS[4].first_edge,
    },
    {
      ...ALL_KINDS[5],
      second_edge: ALL_KINDS[5].first_edge,
    },
    {
      ...ALL_KINDS[7],
      second_vertex: ALL_KINDS[7].first_vertex,
    },
    {
      ...ALL_KINDS[8],
      source_vertex: ALL_KINDS[8].center_vertex,
    },
    {
      ...ALL_KINDS[8],
      target_vertex: ALL_KINDS[8].source_vertex,
    },
    {
      ...ALL_KINDS[9],
      bisector_edge: ALL_KINDS[9].first_edge,
    },
    {
      ...ALL_KINDS[10],
      denominator_edge: ALL_KINDS[10].numerator_edge,
    },
  ]
  for (const constraint of repeated) {
    assert.equal(normalizeGeometricConstraintDocument(documentOf(constraint)), null)
  }
})

test('enforces finite scalar ranges at their exact open and closed boundaries', () => {
  const accepted = [
    { ...ALL_KINDS[0], length_mm: Number.MIN_VALUE },
    { ...ALL_KINDS[1], angle_degrees: -0 },
    { ...ALL_KINDS[1], angle_degrees: 180 },
    { ...ALL_KINDS[8], angle_degrees: Number.MIN_VALUE },
    { ...ALL_KINDS[8], angle_degrees: 359.999_999 },
    { ...ALL_KINDS[10], ratio: Number.MIN_VALUE },
  ]
  for (const constraint of accepted) {
    assert.ok(normalizeGeometricConstraintDocument(documentOf(constraint)))
  }
  const normalizedZero = normalizeGeometricConstraintDocument(
    documentOf({ ...ALL_KINDS[1], angle_degrees: -0 }),
  )
  assert.equal(
    Object.is(
      normalizedZero?.constraints[0]?.constraint.kind === 'fixed_angle'
        ? normalizedZero.constraints[0].constraint.angle_degrees
        : Number.NaN,
      0,
    ),
    true,
  )

  const rejected = [
    { ...ALL_KINDS[0], length_mm: 0 },
    { ...ALL_KINDS[0], length_mm: '10' },
    { ...ALL_KINDS[0], length_mm: Number.NaN },
    { ...ALL_KINDS[0], length_mm: Number.POSITIVE_INFINITY },
    { ...ALL_KINDS[1], angle_degrees: -Number.MIN_VALUE },
    { ...ALL_KINDS[1], angle_degrees: '45' },
    { ...ALL_KINDS[1], angle_degrees: 180 + Number.EPSILON * 128 },
    { ...ALL_KINDS[8], angle_degrees: 0 },
    { ...ALL_KINDS[8], angle_degrees: null },
    { ...ALL_KINDS[8], angle_degrees: 360 },
    { ...ALL_KINDS[8], angle_degrees: Number.NEGATIVE_INFINITY },
    { ...ALL_KINDS[10], ratio: 0 },
    { ...ALL_KINDS[10], ratio: false },
    { ...ALL_KINDS[10], ratio: Number.NaN },
  ]
  for (const constraint of rejected) {
    assert.equal(normalizeGeometricConstraintDocument(documentOf(constraint)), null)
  }
})

test('accepts exactly 10,000 records and 40,000 references, then rejects one more record', () => {
  const constraints = Array.from(
    { length: MAX_GEOMETRIC_CONSTRAINT_RECORDS },
    (_, index) => ({
      id: uuid(10_000 + index),
      constraint: {
        kind: 'angle_bisector',
        vertex: VERTEX_1,
        first_edge: EDGE_1,
        second_edge: EDGE_2,
        bisector_edge: EDGE_3,
      },
    }),
  )
  const atLimit = normalizeGeometricConstraintDocument({
    schema_version: 1,
    constraints,
  })
  assert.equal(atLimit?.constraints.length, MAX_GEOMETRIC_CONSTRAINT_RECORDS)
  assert.equal(
    atLimit?.constraints.length * 4,
    MAX_GEOMETRIC_CONSTRAINT_REFERENCES,
  )
  assertDeepFrozen(atLimit)

  assert.equal(
    normalizeGeometricConstraintDocument({
      schema_version: 1,
      constraints: [
        ...constraints,
        {
          id: uuid(99_999),
          constraint: ALL_KINDS[0],
        },
      ],
    }),
    null,
  )
})

test('contains hostile proxies and accessors without invoking getters', () => {
  let getterCalls = 0
  const accessor = { schema_version: 1 } as Record<string, unknown>
  Object.defineProperty(accessor, 'constraints', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('C:\\private\\secret.ori2')
    },
  })
  const throwingProxy = new Proxy({}, {
    ownKeys() {
      throw new Error('C:\\private\\secret.ori2')
    },
  })
  const hostileArray = new Proxy([], {
    ownKeys() {
      throw new Error('C:\\private\\secret.ori2')
    },
  })
  const revoked = Proxy.revocable({}, {})
  revoked.revoke()

  for (const value of [
    accessor,
    throwingProxy,
    revoked.proxy,
    { schema_version: 1, constraints: hostileArray },
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(normalizeGeometricConstraintDocument(value), null)
    })
  }
  assert.equal(getterCalls, 0)
})

test('presentation also fails closed for malformed or hostile records', () => {
  let getterCalls = 0
  const accessor = { id: CONSTRAINT_1 } as Record<string, unknown>
  Object.defineProperty(accessor, 'constraint', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  const hostile = new Proxy({}, {
    getPrototypeOf() {
      throw new Error('private')
    },
  })

  assert.equal(createGeometricConstraintPresentation(accessor), null)
  assert.equal(createGeometricConstraintPresentation(hostile), null)
  assert.equal(createGeometricConstraintPresentation({
    id: CONSTRAINT_1,
    constraint: { ...ALL_KINDS[0], raw_error: 'private' },
  }), null)
  assert.equal(getterCalls, 0)
})

test('normalizes all seven direct-conflict kinds with bounded frozen witnesses', () => {
  const raw = response({
    status: 'direct_conflict',
    conflicts: DIRECT_CONFLICTS,
  })
  const before = structuredClone(raw)
  const normalized =
    normalizeGeometricConstraintPreflightResponse(raw, BINDING)

  assert.deepEqual(normalized, raw)
  assert.deepEqual(raw, before)
  assertDeepFrozen(normalized)
  assert.equal(
    normalized?.result.status === 'direct_conflict'
      ? normalized.result.conflicts.length
      : 0,
    7,
  )
  assert.equal(MAX_DIRECT_CONFLICT_WITNESS_IDS, 3)
})

test('normalizes no-conflict and all three closed unknown reasons', () => {
  const noConflict = normalizeGeometricConstraintPreflightResponse(
    response({ status: 'no_direct_conflict' }),
    BINDING,
  )
  assert.equal(noConflict?.result.status, 'no_direct_conflict')
  assertDeepFrozen(noConflict)

  for (const reason of [
    'work_limit_exceeded',
    'solver_required_constraint_kinds',
    'invalid_document_or_geometry',
  ] as const) {
    const normalized = normalizeGeometricConstraintPreflightResponse(
      response({
        status: 'unknown',
        reason,
        unchecked_constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
      }),
      BINDING,
    )
    assert.equal(
      normalized?.result.status === 'unknown'
        ? normalized.result.reason
        : null,
      reason,
    )
    assertDeepFrozen(normalized)
  }
})

test('preflight response is bound to the exact project instance, project, and revision', () => {
  for (const raw of [
    { ...response({ status: 'no_direct_conflict' }), project_instance_id: uuid(9) },
    { ...response({ status: 'no_direct_conflict' }), project_id: uuid(9) },
    { ...response({ status: 'no_direct_conflict' }), revision: 8 },
    { ...response({ status: 'no_direct_conflict' }), revision: -1 },
    {
      ...response({ status: 'no_direct_conflict' }),
      revision: Number.MAX_SAFE_INTEGER + 1,
    },
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(raw, BINDING),
      null,
    )
  }
  for (const binding of [
    { ...BINDING, project_instance_id: 'invalid' },
    { ...BINDING, project_id: uuid(0xabc).toUpperCase() },
    { ...BINDING, revision: -1 },
    { ...BINDING, future: true },
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        response({ status: 'no_direct_conflict' }),
        binding,
      ),
      null,
    )
  }
})

test('preflight rejects unknown fields, statuses, reasons, conflict kinds, and operands', () => {
  const malformed: unknown[] = [
    { ...response({ status: 'no_direct_conflict' }), future: true },
    {
      project_instance_id: INSTANCE_ID,
      project_id: PROJECT_ID,
      revision: 7,
    },
    response({ status: 'no_direct_conflict', future: true }),
    response({ status: 'future' }),
    response({
      status: 'unknown',
      reason: 'future_reason',
      unchecked_constraint_ids: [],
    }),
    response({
      status: 'direct_conflict',
      conflicts: [],
    }),
    response({
      status: 'direct_conflict',
      conflicts: [{
        conflict: { kind: 'future_conflict', edge: EDGE_1 },
        constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
      }],
    }),
    response({
      status: 'direct_conflict',
      conflicts: [{
        conflict: {
          kind: 'different_fixed_angles',
          vertex: VERTEX_1,
          first_edge: EDGE_1,
          second_edge: EDGE_1,
        },
        constraint_ids: [CONSTRAINT_1, CONSTRAINT_2],
      }],
    }),
  ]
  for (const value of malformed) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(value, BINDING),
      null,
    )
  }

  for (const item of DIRECT_CONFLICTS) {
    for (const key of Object.keys(item.conflict)) {
      const conflict = { ...item.conflict } as Record<string, unknown>
      delete conflict[key]
      assert.equal(
        normalizeGeometricConstraintPreflightResponse(response({
          status: 'direct_conflict',
          conflicts: [{ ...item, conflict }],
        }), BINDING),
        null,
      )
    }
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(response({
        status: 'direct_conflict',
        conflicts: [{
          ...item,
          conflict: { ...item.conflict, future: true },
        }],
      }), BINDING),
      null,
    )
    for (const key of ['conflict', 'constraint_ids']) {
      const conflictRecord = { ...item } as Record<string, unknown>
      delete conflictRecord[key]
      assert.equal(
        normalizeGeometricConstraintPreflightResponse(response({
          status: 'direct_conflict',
          conflicts: [conflictRecord],
        }), BINDING),
        null,
      )
    }
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(response({
        status: 'direct_conflict',
        conflicts: [{ ...item, future: true }],
      }), BINDING),
      null,
    )
  }
})

test('preflight witnesses are canonical, duplicate-free, and bounded to three IDs', () => {
  const invalidWitnesses = [
    [],
    [CONSTRAINT_1],
    [CONSTRAINT_1, CONSTRAINT_1],
    [CONSTRAINT_2, CONSTRAINT_1],
    [CONSTRAINT_1, CONSTRAINT_2, CONSTRAINT_3, uuid(104)],
    [CONSTRAINT_1, uuid(0xabc).toUpperCase()],
  ]
  for (const constraintIds of invalidWitnesses) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(response({
        status: 'direct_conflict',
        conflicts: [{
          conflict: {
            kind: 'different_fixed_lengths',
            edge: EDGE_1,
          },
          constraint_ids: constraintIds,
        }],
      }), BINDING),
      null,
    )
  }

  assert.equal(
    normalizeGeometricConstraintPreflightResponse(response({
      status: 'unknown',
      reason: 'invalid_document_or_geometry',
      unchecked_constraint_ids: [CONSTRAINT_2, CONSTRAINT_1],
    }), BINDING),
    null,
  )
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(response({
      status: 'unknown',
      reason: 'invalid_document_or_geometry',
      unchecked_constraint_ids: [CONSTRAINT_1, CONSTRAINT_1],
    }), BINDING),
    null,
  )
})

test('unknown preflight ID arrays accept their exact ceiling and reject one more', () => {
  const ids = Array.from(
    { length: MAX_GEOMETRIC_CONSTRAINT_RECORDS },
    (_, index) => uuid(20_000 + index),
  )
  const atLimit = normalizeGeometricConstraintPreflightResponse(response({
    status: 'unknown',
    reason: 'invalid_document_or_geometry',
    unchecked_constraint_ids: ids,
  }), BINDING)
  assert.equal(
    atLimit?.result.status === 'unknown'
      ? atLimit.result.unchecked_constraint_ids.length
      : 0,
    MAX_GEOMETRIC_CONSTRAINT_RECORDS,
  )
  assertDeepFrozen(atLimit)

  assert.equal(
    normalizeGeometricConstraintPreflightResponse(response({
      status: 'unknown',
      reason: 'invalid_document_or_geometry',
      unchecked_constraint_ids: [...ids, uuid(99_999)],
    }), BINDING),
    null,
  )
})

test('preflight hostile proxies and nested accessors fail closed without mutation or throws', () => {
  let getterCalls = 0
  const result = Object.create(null) as Record<string, unknown>
  Object.defineProperty(result, 'status', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  const hostile = new Proxy({}, {
    ownKeys() {
      throw new Error('private')
    },
  })
  const raw = response({ status: 'no_direct_conflict' })
  const before = structuredClone(raw)

  for (const value of [
    hostile,
    response(result),
    response({
      status: 'direct_conflict',
      conflicts: new Proxy([], {
        ownKeys() {
          throw new Error('private')
        },
      }),
    }),
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        normalizeGeometricConstraintPreflightResponse(value, BINDING),
        null,
      )
    })
  }
  assert.equal(getterCalls, 0)
  assert.ok(normalizeGeometricConstraintPreflightResponse(raw, BINDING))
  assert.deepEqual(raw, before)
})

function documentOf(constraint: unknown) {
  return {
    schema_version: 1,
    constraints: [{ id: CONSTRAINT_1, constraint }],
  }
}

function response(result: unknown) {
  return {
    ...BINDING,
    result,
  }
}

function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (value === null || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value))) {
    if ('value' in descriptor) assertDeepFrozen(descriptor.value, seen)
  }
}
