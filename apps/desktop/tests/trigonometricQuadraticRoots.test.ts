import assert from 'node:assert/strict'
import test from 'node:test'

import {
  enumerateTrigonometricQuadraticRoots,
  type TrigonometricQuadraticCoefficients,
} from '../src/lib/trigonometricQuadraticRoots.ts'

const ZERO = Object.freeze({
  a0: 0,
  aCos: 0,
  aSin: 0,
  aCos2: 0,
  aSin2: 0,
})

test('a nonzero constant has no roots in the closed interval', () => {
  expectRoots({ ...ZERO, a0: 7 }, [])
})

test('cosine has its one interior root', () => {
  expectRoots({ ...ZERO, aCos: 1 }, [Math.PI / 2])
})

test('sine preserves both closed-interval endpoints', () => {
  expectRoots({ ...ZERO, aSin: 1 }, [0, Math.PI])
})

test('cosine of twice the angle has both interior roots', () => {
  expectRoots(
    { ...ZERO, aCos2: 1 },
    [Math.PI / 4, 3 * Math.PI / 4],
  )
})

test('sine of twice the angle preserves endpoints and the middle root', () => {
  expectRoots(
    { ...ZERO, aSin2: 1 },
    [0, Math.PI / 2, Math.PI],
  )
})

test('one first-order sine offset produces two distinct roots', () => {
  expectRoots(
    { ...ZERO, a0: -0.5, aSin: 1 },
    [Math.PI / 6, 5 * Math.PI / 6],
  )
})

test('a quadratic in cosine finds both unequal material roots', () => {
  // (cos(theta) - 0.25) * (cos(theta) + 0.5)
  expectRoots(
    { ...ZERO, a0: 0.375, aCos: 0.25, aCos2: 0.5 },
    [Math.acos(0.25), 2 * Math.PI / 3],
  )
})

test('an interior double root is returned once', () => {
  // cos(theta)^2
  expectRoots(
    { ...ZERO, a0: 0.5, aCos2: 0.5 },
    [Math.PI / 2],
  )
})

test('double roots at either compactified endpoint remain exact', () => {
  expectRoots({ ...ZERO, a0: 1, aCos: -1 }, [0])
  expectRoots({ ...ZERO, a0: 1, aCos: 1 }, [Math.PI])
})

test('very large and very small common coefficient scales are normalized', () => {
  expectRoots({ ...ZERO, aCos: 1e300 }, [Math.PI / 2])
  expectRoots({ ...ZERO, aCos: 1e-300 }, [Math.PI / 2])
})

test('a tiny highest harmonic does not erase a well-conditioned root', () => {
  const result = success({
    ...ZERO,
    aCos: 1,
    aCos2: 1e-12,
  })
  assert.equal(result.rootsRadians.length, 1)
  assert.ok(Math.abs(result.rootsRadians[0] - Math.PI / 2) < 2e-12)
})

test('the identically zero function is rejected as infinitely ambiguous', () => {
  assert.deepEqual(enumerateTrigonometricQuadraticRoots(ZERO), {
    kind: 'rejected',
    reason: 'ambiguous',
  })
})

test('a near-double no-root case is rejected instead of guessed', () => {
  // cos(theta)^2 + 1e-10 has no mathematical root, but is deliberately
  // inside the numerical multiple-root ambiguity band.
  assert.deepEqual(enumerateTrigonometricQuadraticRoots({
    ...ZERO,
    a0: 0.5 + 1e-10,
    aCos2: 0.5,
  }), {
    kind: 'rejected',
    reason: 'ambiguous',
  })
})

test('a clearly separated positive minimum is certified as no root', () => {
  expectRoots({
    ...ZERO,
    a0: 0.5 + 1e-5,
    aCos2: 0.5,
  }, [])
})

test('non-finite and malformed coefficients fail closed as numeric', () => {
  for (const coefficients of [
    { ...ZERO, a0: Number.NaN },
    { ...ZERO, aCos: Number.POSITIVE_INFINITY },
    null,
    {},
  ]) {
    assert.deepEqual(
      enumerateTrigonometricQuadraticRoots(
        coefficients as TrigonometricQuadraticCoefficients,
      ),
      { kind: 'rejected', reason: 'numeric' },
    )
  }
})

test('success and rejection snapshots are deeply frozen', () => {
  const result = enumerateTrigonometricQuadraticRoots({
    ...ZERO,
    aSin2: 1,
  })
  assert.equal(result.kind, 'success')
  if (result.kind !== 'success') assert.fail(`unexpected ${result.reason}`)
  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.rootsRadians))

  const rejected = enumerateTrigonometricQuadraticRoots(ZERO)
  assert.ok(Object.isFrozen(rejected))
})

test('root enumeration is deterministic and remains inside its work limit', () => {
  const coefficients = {
    a0: -0.17,
    aCos: 0.41,
    aSin: -0.23,
    aCos2: 0.87,
    aSin2: 0.62,
  }
  const first = enumerateTrigonometricQuadraticRoots(coefficients)
  const second = enumerateTrigonometricQuadraticRoots(coefficients)
  assert.deepEqual(first, second)
  assert.equal(first.kind, 'success')
  if (first.kind !== 'success') assert.fail(`unexpected ${first.reason}`)
  assert.ok(first.evaluationCount > 0)
  assert.ok(first.evaluationCount <= 640)
  for (const root of first.rootsRadians) {
    assert.ok(root >= 0 && root <= Math.PI)
    assert.ok(Math.abs(evaluate(coefficients, root)) < 2e-9)
  }
})

function expectRoots(
  coefficients: TrigonometricQuadraticCoefficients,
  expected: readonly number[],
) {
  const result = success(coefficients)
  assert.equal(result.rootsRadians.length, expected.length)
  for (let index = 0; index < expected.length; index += 1) {
    assert.ok(
      Math.abs(result.rootsRadians[index] - expected[index]) < 2e-9,
      `root ${index}: ${result.rootsRadians[index]} != ${expected[index]}`,
    )
  }
}

function success(coefficients: TrigonometricQuadraticCoefficients) {
  const result = enumerateTrigonometricQuadraticRoots(coefficients)
  assert.equal(
    result.kind,
    'success',
    result.kind === 'rejected' ? result.reason : undefined,
  )
  if (result.kind !== 'success') assert.fail(`unexpected ${result.reason}`)
  return result
}

function evaluate(
  coefficients: TrigonometricQuadraticCoefficients,
  angle: number,
) {
  return coefficients.a0
    + coefficients.aCos * Math.cos(angle)
    + coefficients.aSin * Math.sin(angle)
    + coefficients.aCos2 * Math.cos(2 * angle)
    + coefficients.aSin2 * Math.sin(2 * angle)
}
