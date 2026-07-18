import assert from 'node:assert/strict'
import test from 'node:test'

import {
  LOCAL_FLAT_FOLDABILITY_MODEL,
  LOCAL_FLAT_FOLDABILITY_VISIBLE_ITEM_LIMIT,
  createLocalFlatFoldabilityPresentation,
  localFlatFoldabilityConditionLabel,
  localFlatFoldabilityReasonLabel,
} from '../src/lib/localFlatFoldabilityPresentation.ts'
import type {
  LocalFlatFoldabilityCondition,
  LocalFlatFoldabilityReason,
  LocalFlatFoldabilityReport,
  LocalFlatFoldabilityVertexSnapshot,
} from '../src/lib/coreClient.ts'

const MAX_DEGREE = 8

test('a successful report preserves all four vertex outcomes and fixed explanatory labels', () => {
  const vertices = [
    vertex('satisfied', 'satisfied', null, {
      foldDegree: 4,
      mountainCount: 3,
      valleyCount: 1,
      kawasaki: 'satisfied',
      maekawa: 'satisfied',
    }),
    vertex('violated', 'violated', null, {
      foldDegree: 4,
      mountainCount: 2,
      valleyCount: 2,
      kawasaki: 'satisfied',
      maekawa: 'violated',
    }),
    vertex('boundary', 'not_applicable', 'paper_boundary', {
      foldDegree: 2,
      mountainCount: 1,
      valleyCount: 1,
      kawasaki: 'not_applicable',
      maekawa: 'not_applicable',
    }),
    vertex('limit', 'indeterminate', 'fold_degree_limit', {
      foldDegree: 10,
      mountainCount: 6,
      valleyCount: 4,
      kawasaki: 'indeterminate',
      maekawa: 'satisfied',
    }),
  ]
  const presentation = createLocalFlatFoldabilityPresentation(
    analyzedReport('violated', vertices),
    vertices.map(({ vertex: id }) => id),
  )

  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.deepEqual(presentation.counts, {
    total: 4,
    applicable: 3,
    satisfied: 1,
    violated: 1,
    notApplicable: 1,
    indeterminate: 1,
  })
  assert.match(presentation.summaryText, /局所必要条件に不成立/u)
  assert.deepEqual(
    [...presentation.highlights],
    [['violated', 'violated'], ['limit', 'indeterminate']],
  )
  assert.deepEqual(
    presentation.visibleItems.map(({ vertexId }) => vertexId),
    ['violated', 'limit', 'boundary'],
  )
  assert.equal(presentation.verticesById.get('satisfied')?.ordinal, 1)
  assert.equal(localFlatFoldabilityConditionLabel('not_applicable'), '対象外')
  assert.match(
    localFlatFoldabilityReasonLabel('fold_degree_limit', MAX_DEGREE),
    /厳密計算上限（8）を超えた/u,
  )
})

test('the core successful shape remains accepted without frontend recomputation', () => {
  const vertices = [
    vertex('first', 'satisfied', null, {
      foldDegree: 6,
      mountainCount: 4,
      valleyCount: 2,
      kawasaki: 'satisfied',
      maekawa: 'satisfied',
    }),
    vertex('second', 'not_applicable', 'no_incident_fold_edges', {
      foldDegree: 0,
      mountainCount: 0,
      valleyCount: 0,
      kawasaki: 'not_applicable',
      maekawa: 'not_applicable',
    }),
    vertex('high-degree-maekawa-counterexample', 'violated', null, {
      foldDegree: 10,
      mountainCount: 5,
      valleyCount: 5,
      kawasaki: 'indeterminate',
      maekawa: 'violated',
    }),
  ]

  const presentation = createLocalFlatFoldabilityPresentation(
    analyzedReport('violated', vertices),
    ['first', 'second', 'high-degree-maekawa-counterexample'],
  )

  assert.equal(presentation.kind, 'ready')
  assert.equal(
    presentation.kind === 'ready' ? presentation.reportStatus : null,
    'violated',
  )
})

test('a structurally blocked report stays distinct from per-vertex indeterminate results', () => {
  const presentation = createLocalFlatFoldabilityPresentation({
    model: LOCAL_FLAT_FOLDABILITY_MODEL,
    max_exact_fold_degree: MAX_DEGREE,
    status: 'blocked',
    total_vertices: 0,
    applicable_vertices: 0,
    satisfied_vertices: 0,
    violated_vertices: 0,
    not_applicable_vertices: 0,
    indeterminate_vertices: 0,
    vertices: [],
  }, ['untrusted', 'untrusted'])

  assert.equal(presentation.kind, 'blocked')
  assert.match(presentation.summaryText, /前段の幾何構造に問題/u)
  assert.equal(presentation.highlights.size, 0)
})

test('unknown models, statuses, reasons, fields, and accessors fail closed', () => {
  const valid = analyzedReport('necessary_conditions_satisfied', [
    vertex('only', 'satisfied', null),
  ])
  const malformed: unknown[] = [
    { ...valid, model: 'future_model' },
    { ...valid, status: 'possible' },
    {
      ...valid,
      vertices: [{ ...valid.vertices[0], reason: 'unknown_reason' }],
    },
    { ...valid, unexpected: true },
    {
      ...valid,
      vertices: [{ ...valid.vertices[0], unexpected: true }],
    },
  ]
  const accessor = { ...valid } as Record<string, unknown>
  Object.defineProperty(accessor, 'status', {
    enumerable: true,
    get() {
      throw new Error('must not execute')
    },
  })
  malformed.push(accessor)

  for (const report of malformed) {
    assert.equal(
      createLocalFlatFoldabilityPresentation(report, ['only']).kind,
      'invalid',
    )
  }
})

test('duplicate, missing, foreign, and ambiguous current project vertices fail closed', () => {
  const first = vertex('first', 'satisfied', null)
  const second = vertex('second', 'satisfied', null)
  const valid = analyzedReport('necessary_conditions_satisfied', [first, second])
  const duplicate = analyzedReport('necessary_conditions_satisfied', [first, first])
  const foreign = analyzedReport('necessary_conditions_satisfied', [
    first,
    { ...second, vertex: 'foreign' },
  ])

  assert.equal(
    createLocalFlatFoldabilityPresentation(duplicate, ['first', 'second']).kind,
    'invalid',
  )
  assert.equal(
    createLocalFlatFoldabilityPresentation(foreign, ['first', 'second']).kind,
    'invalid',
  )
  assert.equal(
    createLocalFlatFoldabilityPresentation(valid, ['first', 'second', 'missing']).kind,
    'invalid',
  )
  assert.equal(
    createLocalFlatFoldabilityPresentation(valid, ['first', 'first']).kind,
    'invalid',
  )
})

test('counter, report-status, degree, reason, and theorem contradictions fail closed', () => {
  const satisfied = vertex('only', 'satisfied', null)
  const valid = analyzedReport('necessary_conditions_satisfied', [satisfied])
  const malformed: unknown[] = [
    { ...valid, satisfied_vertices: 0 },
    { ...valid, applicable_vertices: 0 },
    { ...valid, status: 'violated' },
    {
      ...valid,
      vertices: [{ ...satisfied, fold_degree: 6 }],
    },
    {
      ...valid,
      vertices: [{ ...satisfied, reason: 'paper_boundary' }],
    },
    {
      ...valid,
      vertices: [{ ...satisfied, maekawa: 'violated' }],
    },
    analyzedReport('indeterminate', [
      vertex('only', 'indeterminate', 'fold_degree_limit', {
        foldDegree: MAX_DEGREE,
        mountainCount: 5,
        valleyCount: 3,
        kawasaki: 'indeterminate',
        maekawa: 'satisfied',
      }),
    ]),
    analyzedReport('violated', [
      vertex('only', 'violated', null, {
        kawasaki: 'satisfied',
        maekawa: 'satisfied',
      }),
    ]),
    analyzedReport('necessary_conditions_satisfied', [
      vertex('only', 'satisfied', null, {
        foldDegree: 4,
        mountainCount: 2,
        valleyCount: 2,
        kawasaki: 'satisfied',
        maekawa: 'satisfied',
      }),
    ]),
    analyzedReport('violated', [
      vertex('only', 'violated', null, {
        foldDegree: 3,
        mountainCount: 2,
        valleyCount: 1,
        kawasaki: 'satisfied',
        maekawa: 'violated',
      }),
    ]),
    analyzedReport('not_applicable', [
      vertex('only', 'not_applicable', 'no_incident_fold_edges', {
        foldDegree: 2,
        mountainCount: 1,
        valleyCount: 1,
        kawasaki: 'not_applicable',
        maekawa: 'not_applicable',
      }),
    ]),
  ]

  for (const report of malformed) {
    assert.equal(
      createLocalFlatFoldabilityPresentation(report, ['only']).kind,
      'invalid',
    )
  }
})

test('odd degrees above the exact cap retain core precedence instead of becoming indeterminate', () => {
  const presentation = createLocalFlatFoldabilityPresentation(
    analyzedReport('violated', [
      vertex('only', 'violated', null, {
        foldDegree: 9,
        mountainCount: 5,
        valleyCount: 4,
        kawasaki: 'violated',
        maekawa: 'violated',
      }),
    ]),
    ['only'],
  )

  assert.equal(presentation.kind, 'ready')
  assert.equal(
    presentation.kind === 'ready' ? presentation.reportStatus : null,
    'violated',
  )
})

test('the visible list is capped and prioritizes violations, uncertainty, boundary, and cuts', () => {
  const vertices: LocalFlatFoldabilityVertexSnapshot[] = []
  for (let index = 0; index < 25; index += 1) {
    vertices.push(vertex(`isolated-${index}`, 'not_applicable', 'no_incident_fold_edges', {
      foldDegree: 0,
      mountainCount: 0,
      valleyCount: 0,
      kawasaki: 'not_applicable',
      maekawa: 'not_applicable',
    }))
  }
  vertices.push(vertex('cut', 'not_applicable', 'cut_incident', {
    foldDegree: 2,
    mountainCount: 1,
    valleyCount: 1,
    kawasaki: 'not_applicable',
    maekawa: 'not_applicable',
  }))
  vertices.push(vertex('boundary', 'not_applicable', 'paper_boundary', {
    foldDegree: 2,
    mountainCount: 1,
    valleyCount: 1,
    kawasaki: 'not_applicable',
    maekawa: 'not_applicable',
  }))

  const presentation = createLocalFlatFoldabilityPresentation(
    analyzedReport('not_applicable', vertices),
    vertices.map(({ vertex: id }) => id),
  )

  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.equal(presentation.visibleItems.length, LOCAL_FLAT_FOLDABILITY_VISIBLE_ITEM_LIMIT)
  assert.deepEqual(
    presentation.visibleItems.slice(0, 2).map(({ vertexId }) => vertexId),
    ['boundary', 'cut'],
  )
  assert.equal(presentation.hiddenItemCount, 7)
  assert.equal(presentation.counts.notApplicable, 27)
})

test('ten thousand project vertices are validated in one bounded presentation pass', () => {
  const vertices = Array.from({ length: 10_000 }, (_, index) =>
    vertex(`vertex-${index}`, 'satisfied', null))
  const presentation = createLocalFlatFoldabilityPresentation(
    analyzedReport('necessary_conditions_satisfied', vertices),
    vertices.map(({ vertex: id }) => id),
  )

  assert.equal(presentation.kind, 'ready')
  assert.equal(presentation.verticesById.size, 10_000)
  assert.equal(presentation.highlights.size, 0)
  assert.equal(presentation.visibleItems.length, 0)
})

function vertex(
  id: string,
  verdict: LocalFlatFoldabilityCondition,
  reason: LocalFlatFoldabilityReason,
  overrides: Partial<LocalFlatFoldabilityVertexSnapshot> & {
    foldDegree?: number
    mountainCount?: number
    valleyCount?: number
  } = {},
): LocalFlatFoldabilityVertexSnapshot {
  const foldDegree = overrides.foldDegree ?? 4
  const mountainCount = overrides.mountainCount ?? 3
  const valleyCount = overrides.valleyCount ?? 1
  return {
    vertex: id,
    fold_degree: foldDegree,
    mountain_count: mountainCount,
    valley_count: valleyCount,
    verdict,
    reason,
    kawasaki: overrides.kawasaki ?? verdict,
    maekawa: overrides.maekawa ?? verdict,
  }
}

function analyzedReport(
  status: Exclude<LocalFlatFoldabilityReport['status'], 'blocked'>,
  vertices: LocalFlatFoldabilityVertexSnapshot[],
): LocalFlatFoldabilityReport {
  const count = (verdict: LocalFlatFoldabilityCondition) =>
    vertices.filter((vertex) => vertex.verdict === verdict).length
  const satisfied = count('satisfied')
  const violated = count('violated')
  const notApplicable = count('not_applicable')
  const indeterminate = count('indeterminate')
  return {
    model: LOCAL_FLAT_FOLDABILITY_MODEL,
    max_exact_fold_degree: MAX_DEGREE,
    status,
    total_vertices: vertices.length,
    applicable_vertices: satisfied + violated + indeterminate,
    satisfied_vertices: satisfied,
    violated_vertices: violated,
    not_applicable_vertices: notApplicable,
    indeterminate_vertices: indeterminate,
    vertices,
  }
}
