import assert from 'node:assert/strict'
import test from 'node:test'

import {
  normalizeGeometricConstraintSolvePreview,
} from '../src/lib/coreClient.ts'

const TOKEN = '00000000-0000-4000-8000-000000000001'
const VERTEX = '00000000-0000-4000-8000-000000000002'
const MODEL = 'geometric_constraint_current_runtime_exact_satisfaction_v1'

function validPreview() {
  return {
    token: TOKEN,
    revision: 7,
    iterations: 3,
    maximumResidual: 1e-9,
    rank: 1,
    degreesOfFreedom: 1,
    equationCount: 2,
    conditionEstimate: 4,
    systemClassification: 'under_constrained',
    changedVertices: [{ vertexId: VERTEX, x: 12, y: 8 }],
    exactSatisfaction: {
      modelId: MODEL,
      constraintCount: 1,
      equationCount: 2,
      authorizesProjectMutation: false,
      replayableAcrossRuntimes: false,
    },
  }
}

test('strict solve preview parser accepts exact and legacy approximate responses', () => {
  const exactSource = validPreview()
  const exact = normalizeGeometricConstraintSolvePreview(exactSource)
  assert.ok(exact)
  assert.deepEqual(exact.exactSatisfaction, {
    modelId: MODEL,
    constraintCount: 1,
    equationCount: 2,
    authorizesProjectMutation: false,
    replayableAcrossRuntimes: false,
  })

  exactSource.changedVertices[0]!.x = 99
  assert.equal(exact.changedVertices[0]!.x, 12)

  const approximateSource = validPreview()
  delete (approximateSource as Partial<typeof approximateSource>).exactSatisfaction
  const approximate = normalizeGeometricConstraintSolvePreview(approximateSource)
  assert.ok(approximate)
  assert.equal(approximate.exactSatisfaction, undefined)

  const fullyDrivenBoundary = validPreview()
  fullyDrivenBoundary.rank = 2_048
  fullyDrivenBoundary.equationCount = 2_048
  fullyDrivenBoundary.degreesOfFreedom = 0
  fullyDrivenBoundary.systemClassification = 'well_constrained'
  fullyDrivenBoundary.exactSatisfaction.constraintCount = 1_024
  fullyDrivenBoundary.exactSatisfaction.equationCount = 2_048
  assert.ok(normalizeGeometricConstraintSolvePreview(fullyDrivenBoundary))

  const freeDimensionBoundary = validPreview()
  freeDimensionBoundary.rank = 0
  freeDimensionBoundary.equationCount = 512
  freeDimensionBoundary.degreesOfFreedom = 512
  freeDimensionBoundary.exactSatisfaction.constraintCount = 256
  freeDimensionBoundary.exactSatisfaction.equationCount = 512
  assert.ok(normalizeGeometricConstraintSolvePreview(freeDimensionBoundary))
})

test('strict solve preview parser rejects malformed top-level and vertex data', () => {
  const invalid = [
    { ...validPreview(), extra: true },
    { ...validPreview(), token: 'not-a-uuid' },
    { ...validPreview(), revision: -1 },
    { ...validPreview(), revision: 1.5 },
    { ...validPreview(), iterations: 33 },
    { ...validPreview(), maximumResidual: Number.NaN },
    { ...validPreview(), maximumResidual: -1 },
    { ...validPreview(), rank: -1 },
    { ...validPreview(), rank: 3 },
    { ...validPreview(), degreesOfFreedom: 0.5 },
    { ...validPreview(), degreesOfFreedom: 513 },
    { ...validPreview(), equationCount: 0 },
    { ...validPreview(), equationCount: 2_049 },
    { ...validPreview(), equationCount: Number.MAX_SAFE_INTEGER + 1 },
    { ...validPreview(), conditionEstimate: Number.POSITIVE_INFINITY },
    { ...validPreview(), systemClassification: 'solved' },
    { ...validPreview(), systemClassification: 'well_constrained' },
    { ...validPreview(), changedVertices: null },
    {
      ...validPreview(),
      changedVertices: [
        { vertexId: VERTEX, x: 1, y: 2 },
        { vertexId: VERTEX, x: 3, y: 4 },
      ],
    },
    {
      ...validPreview(),
      changedVertices: [{ vertexId: VERTEX, x: Number.NaN, y: 2 }],
    },
    {
      ...validPreview(),
      changedVertices: [{ vertexId: VERTEX, x: 1, y: 2, extra: true }],
    },
  ]
  for (const value of invalid) {
    assert.equal(normalizeGeometricConstraintSolvePreview(value), null)
  }

  const missing = validPreview() as Record<string, unknown>
  delete missing.rank
  assert.equal(normalizeGeometricConstraintSolvePreview(missing), null)

  const accessor = validPreview()
  Object.defineProperty(accessor, 'rank', {
    enumerable: true,
    get: () => 1,
  })
  assert.equal(normalizeGeometricConstraintSolvePreview(accessor), null)
})

test('strict solve preview parser rejects forged exact-satisfaction authority', () => {
  const mutateExact = (
    mutate: (exact: Record<string, unknown>) => void,
  ): unknown => {
    const value = validPreview()
    mutate(value.exactSatisfaction as unknown as Record<string, unknown>)
    return value
  }
  for (const value of [
    { ...validPreview(), exactSatisfaction: null },
    mutateExact((exact) => { exact.modelId = 'other_model' }),
    mutateExact((exact) => { exact.constraintCount = 0 }),
    mutateExact((exact) => { exact.constraintCount = 1_025 }),
    mutateExact((exact) => { exact.equationCount = 0 }),
    mutateExact((exact) => { exact.equationCount = 1 }),
    mutateExact((exact) => {
      exact.constraintCount = 3
      exact.equationCount = 2
    }),
    mutateExact((exact) => { exact.authorizesProjectMutation = true }),
    mutateExact((exact) => { exact.replayableAcrossRuntimes = true }),
    mutateExact((exact) => { exact.extra = false }),
  ]) {
    assert.equal(normalizeGeometricConstraintSolvePreview(value), null)
  }
})

test('strict solve preview parser bounds changed vertices before traversal', () => {
  const vertices = Array.from({ length: 257 }, (_, index) => ({
    vertexId:
      `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
    x: index,
    y: 0,
  }))
  assert.equal(
    normalizeGeometricConstraintSolvePreview({
      ...validPreview(),
      changedVertices: vertices,
    }),
    null,
  )
})
