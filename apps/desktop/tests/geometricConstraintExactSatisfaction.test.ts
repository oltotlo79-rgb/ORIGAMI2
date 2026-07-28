import assert from 'node:assert/strict'
import test from 'node:test'

import {
  GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID,
  MAX_GEOMETRIC_CONSTRAINT_RECORDS,
  normalizeGeometricConstraintPreflightResponse,
} from '../src/lib/geometricConstraints.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'

const uuid = (index: number) =>
  `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`

const BINDING = Object.freeze({
  project_instance_id: uuid(1),
  project_id: uuid(2),
  revision: 7,
})

function response(result: unknown) {
  return {
    ...BINDING,
    result,
  }
}

function exactResult(
  constraintCount: number,
  equationCount: number,
  replayableAcrossRuntimes = true,
) {
  return {
    status: 'proven_satisfiable',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    constraint_count: constraintCount,
    equation_count: equationCount,
    authorizes_project_mutation: false,
    replayable_across_runtimes: replayableAcrossRuntimes,
  }
}

test('exact satisfiability DTO accepts both replay scopes and inclusive count boundaries', () => {
  for (const result of [
    exactResult(1, 1),
    exactResult(1, 2, false),
    exactResult(
      MAX_GEOMETRIC_CONSTRAINT_RECORDS,
      MAX_GEOMETRIC_CONSTRAINT_RECORDS * 2,
    ),
  ]) {
    const normalized = normalizeGeometricConstraintPreflightResponse(
      response(result),
      BINDING,
    )
    assert.deepEqual(normalized?.result, result)
    assert.equal(Object.isFrozen(normalized), true)
    assert.equal(Object.isFrozen(normalized?.result), true)
  }
})

test('exact satisfiability DTO rejects malformed model and every numeric boundary violation', () => {
  const base = exactResult(2, 3)
  const invalid = [
    { ...base, model_id: 'geometric_constraint_binary64_exact_satisfaction_v2' },
    { ...base, model_id: '' },
    { ...base, transcendental_model_id: 'forged_model' },
    { ...base, transcendental_model_id: '' },
    { ...base, constraint_count: 0 },
    { ...base, constraint_count: -1 },
    { ...base, constraint_count: 1.5 },
    { ...base, constraint_count: Number.NaN },
    { ...base, constraint_count: Number.POSITIVE_INFINITY },
    { ...base, constraint_count: Number.MAX_SAFE_INTEGER + 1 },
    { ...base, constraint_count: MAX_GEOMETRIC_CONSTRAINT_RECORDS + 1 },
    { ...base, equation_count: 0 },
    { ...base, equation_count: 1 },
    { ...base, equation_count: 2.5 },
    { ...base, equation_count: Number.NaN },
    { ...base, equation_count: Number.NEGATIVE_INFINITY },
    { ...base, equation_count: Number.MAX_SAFE_INTEGER + 1 },
    { ...base, equation_count: 5 },
    { ...base, authorizes_project_mutation: true },
    { ...base, authorizes_project_mutation: 'false' },
    { ...base, replayable_across_runtimes: 'false' },
    { ...base, future: true },
    {
      status: base.status,
      model_id: base.model_id,
      transcendental_model_id: base.transcendental_model_id,
      constraint_count: base.constraint_count,
    },
    {
      status: base.status,
      model_id: base.model_id,
      transcendental_model_id: base.transcendental_model_id,
      equation_count: base.equation_count,
    },
    {
      status: base.status,
      model_id: base.model_id,
      constraint_count: base.constraint_count,
      equation_count: base.equation_count,
      authorizes_project_mutation: base.authorizes_project_mutation,
      replayable_across_runtimes: base.replayable_across_runtimes,
    },
  ]

  for (const result of invalid) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(response(result), BINDING),
      null,
    )
  }
})

test('exact satisfiability DTO rejects inherited symbol non-enumerable and accessor fields', () => {
  const inherited = Object.assign(Object.create({
    status: 'proven_satisfiable',
  }), {
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    constraint_count: 1,
    equation_count: 1,
    authorizes_project_mutation: false,
    replayable_across_runtimes: false,
  })
  const symbol = {
    ...exactResult(1, 1),
    [Symbol('private')]: true,
  }
  const nonEnumerable = exactResult(1, 1)
  Object.defineProperty(nonEnumerable, 'equation_count', {
    enumerable: false,
    value: 1,
  })
  let getterCalls = 0
  const accessor = Object.create(null) as Record<string, unknown>
  Object.defineProperties(accessor, {
    status: {
      enumerable: true,
      value: 'proven_satisfiable',
    },
    model_id: {
      enumerable: true,
      value:
        GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID,
    },
    transcendental_model_id: {
      enumerable: true,
      value: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    },
    constraint_count: {
      enumerable: true,
      value: 1,
    },
    equation_count: {
      enumerable: true,
      get() {
        getterCalls += 1
        throw new Error('must not be read')
      },
    },
    authorizes_project_mutation: {
      enumerable: true,
      value: false,
    },
    replayable_across_runtimes: {
      enumerable: true,
      value: false,
    },
  })
  const hostileProxy = new Proxy({}, {
    getPrototypeOf() {
      throw new Error('must fail closed')
    },
  })

  for (const result of [
    inherited,
    symbol,
    nonEnumerable,
    accessor,
    hostileProxy,
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        normalizeGeometricConstraintPreflightResponse(
          response(result),
          BINDING,
        ),
        null,
      )
    })
  }
  assert.equal(getterCalls, 0)
})

test('exact satisfiability DTO accepts a null-prototype own-data record', () => {
  const result = Object.assign(Object.create(null), exactResult(11, 14))
  const normalized = normalizeGeometricConstraintPreflightResponse(
    response(result),
    BINDING,
  )
  assert.deepEqual(normalized?.result, exactResult(11, 14))
})
