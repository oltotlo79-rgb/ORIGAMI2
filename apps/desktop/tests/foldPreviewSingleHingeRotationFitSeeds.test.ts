import assert from 'node:assert/strict'
import test from 'node:test'

import {
  deriveFoldPreviewSingleHingeRotationFitSeeds,
  FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS_VERSION,
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS,
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS,
  type FoldPreviewSingleHingeRotationFitInput,
} from '../src/lib/foldPreviewSingleHingeRotationFitSeeds.ts'

type Point = Readonly<{ x: number; y: number; z: number }>

const ORIGIN = Object.freeze({ x: 0, y: 0, z: 0 })
const Z_AXIS = Object.freeze({ x: 0, y: 0, z: 1 })
const UNIT_X_POINT = Object.freeze({ x: 1, y: 0, z: 0 })

test('positive and negative finite rotations recover exact angle seeds', () => {
  const axisPoint = { x: 10, y: -3, z: 5 }
  const movingPoint = { x: 12, y: -2, z: 7 }

  for (const deltaDegrees of [37, -37]) {
    const radians = degreesToRadians(deltaDegrees)
    const translation = rotationDisplacement(
      movingPoint,
      axisPoint,
      Z_AXIS,
      radians,
    )
    const result = deriveFoldPreviewSingleHingeRotationFitSeeds({
      axis: { point: axisPoint, direction: Z_AXIS },
      childRotationSign: 1,
      blockingAngleDegrees: 90,
      maximumAngleDeltaDegrees: 60,
      translation,
      movingPoints: [{ id: 'material-point', position: movingPoint }],
    })

    assert.ok(result)
    const seed = result.seeds[0]
    assert.ok(seed)
    assert.equal(seed.source, 'least_squares_stationary')
    assertClose(seed.angleDegrees, 90 + deltaDegrees)
    assertClose(seed.signedDeltaDegrees, deltaDegrees)
    assertClose(seed.signedRotationRadians, radians)
    assertClose(seed.residualSquared, 0)
    assertClose(seed.residualRms, 0)
    assertClose(seed.improvementRatio, 1)
    assertClose(
      seed.improvementSquared,
      result.baselineResidualSquared,
    )
  }
})

test('sub-hundredth-degree rotations use the stable first-harmonic roots', () => {
  const axisPoint = { x: 4, y: -2, z: 7 }
  const axisLength = Math.hypot(1, 2, 3)
  const axisDirection = {
    x: 1 / axisLength,
    y: 2 / axisLength,
    z: 3 / axisLength,
  }
  const movingPoint = { x: 11, y: -5, z: 13 }

  for (const deltaDegrees of [0.01, -0.01, 0.000_001]) {
    const radians = degreesToRadians(deltaDegrees)
    const result = deriveFoldPreviewSingleHingeRotationFitSeeds({
      axis: { point: axisPoint, direction: axisDirection },
      childRotationSign: 1,
      blockingAngleDegrees: 90,
      maximumAngleDeltaDegrees: 60,
      translation: rotationDisplacement(
        movingPoint,
        axisPoint,
        axisDirection,
        radians,
      ),
      movingPoints: [{ id: 'off-axis-point', position: movingPoint }],
    })

    assert.ok(result)
    assert.equal(result.seeds[0].source, 'least_squares_stationary')
    assertClose(
      result.seeds[0].angleDegrees,
      90 + deltaDegrees,
      1e-12,
    )
    assertClose(
      result.seeds[0].signedRotationRadians,
      radians,
      1e-12,
    )
  }

  const requestedRadians = degreesToRadians(0.01)
  const bounded = deriveFoldPreviewSingleHingeRotationFitSeeds({
    axis: { point: axisPoint, direction: axisDirection },
    childRotationSign: -1,
    blockingAngleDegrees: 90,
    maximumAngleDeltaDegrees: 0.005,
    translation: rotationDisplacement(
      movingPoint,
      axisPoint,
      axisDirection,
      requestedRadians,
    ),
    movingPoints: [{ id: 'bounded-point', position: movingPoint }],
  })
  assert.ok(bounded)
  assert.equal(bounded.seeds[0].source, 'angle_domain_minimum')
  assertClose(bounded.seeds[0].angleDegrees, 89.995)
  assertClose(bounded.seeds[0].signedRotationRadians, degreesToRadians(0.005))
})

test('child rotation sign reverses only the reported angle direction', () => {
  const radians = degreesToRadians(30)
  const translation = rotationDisplacement(
    UNIT_X_POINT,
    ORIGIN,
    Z_AXIS,
    radians,
  )
  const positive = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({
      childRotationSign: 1,
      translation,
    }),
  )
  const negative = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({
      childRotationSign: -1,
      translation,
    }),
  )

  assert.ok(positive && negative)
  assertClose(positive.seeds[0].angleDegrees, 120)
  assertClose(positive.seeds[0].signedDeltaDegrees, 30)
  assertClose(negative.seeds[0].angleDegrees, 60)
  assertClose(negative.seeds[0].signedDeltaDegrees, -30)
  assertClose(
    positive.seeds[0].signedRotationRadians,
    negative.seeds[0].signedRotationRadians,
  )
  assertClose(
    positive.seeds[0].residualSquared,
    negative.seeds[0].residualSquared,
  )
})

test('an accepted near-unit axis is normalized before fitting large axial offsets', () => {
  const radians = degreesToRadians(60)
  const movingPoint = { x: 1, y: 0, z: 1e9 }
  const result = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    axis: {
      point: ORIGIN,
      direction: { x: 0, y: 0, z: 1 + 5e-10 },
    },
    blockingAngleDegrees: 90,
    maximumAngleDeltaDegrees: 80,
    movingPoints: [{ id: 'large-axial-offset', position: movingPoint }],
    translation: rotationDisplacement(
      movingPoint,
      ORIGIN,
      Z_AXIS,
      radians,
    ),
  }))

  assert.ok(result)
  assertClose(result.seeds[0].angleDegrees, 150, 1e-10)
  assertClose(result.seeds[0].signedRotationRadians, radians, 1e-12)
  assertClose(result.seeds[0].residualSquared, 0, 1e-10)
})

test('multiple points expose the finite least-squares residual and improvement', () => {
  const movingPoints = [
    { id: 'near', position: { x: 1, y: 0, z: 0 } },
    { id: 'far', position: { x: 2, y: 0, z: 0 } },
  ]
  const radians = degreesToRadians(30)
  const translation = rotationDisplacement(
    { x: 1.5, y: 0, z: 0 },
    ORIGIN,
    Z_AXIS,
    radians,
  )
  const result = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({ movingPoints, translation }),
  )

  assert.ok(result)
  assert.equal(result.pointCount, 2)
  assert.equal(result.evaluatedCandidateCount, 3)
  assert.ok(result.seeds.length >= 1)
  assert.ok(result.seeds.length <= MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS)
  const seed = result.seeds[0]
  assert.equal(seed.source, 'least_squares_stationary')
  assert.ok(seed.residualSquared > 0)
  assert.ok(seed.residualSquared < result.baselineResidualSquared)
  assertClose(
    seed.residualSquared,
    independentResidualSquared(
      movingPoints.map((point) => point.position),
      ORIGIN,
      Z_AXIS,
      translation,
      seed.signedRotationRadians,
    ),
  )
  assertClose(
    result.baselineResidualSquared,
    movingPoints.length * squaredLength(translation),
  )
  assertClose(
    result.baselineResidualRms,
    Math.sqrt(result.baselineResidualSquared / movingPoints.length),
  )
  assertClose(
    seed.residualRms,
    Math.sqrt(seed.residualSquared / movingPoints.length),
  )
  assertClose(
    seed.improvementSquared,
    result.baselineResidualSquared - seed.residualSquared,
  )
  assertClose(
    seed.improvementRatio,
    seed.improvementSquared / result.baselineResidualSquared,
  )
  for (let index = 0; index < result.seeds.length; index += 1) {
    assert.equal(result.seeds[index].rank, index + 1)
    if (index > 0) {
      assert.ok(
        result.seeds[index - 1].residualSquared
          <= result.seeds[index].residualSquared,
      )
    }
  }
})

test('axis-parallel translation and points entirely on the axis return null', () => {
  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(
      validInput({ translation: { x: 0, y: 0, z: 1 } }),
    ),
    null,
  )
  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
      translation: { x: 1, y: 0, z: 0 },
      movingPoints: [
        { id: 'axis-a', position: { x: 0, y: 0, z: -2 } },
        { id: 'axis-b', position: { x: 0, y: 0, z: 4 } },
      ],
    })),
    null,
  )
  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(
      validInput({ translation: ORIGIN }),
    ),
    null,
  )
})

test('zero and 180 degree boundaries clamp the maximum angle domain', () => {
  const positiveRadians = degreesToRadians(10)
  const atZero = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    blockingAngleDegrees: 0,
    maximumAngleDeltaDegrees: 20,
    translation: rotationDisplacement(
      UNIT_X_POINT,
      ORIGIN,
      Z_AXIS,
      positiveRadians,
    ),
  }))
  assert.ok(atZero)
  assert.deepEqual(atZero.angleDomain, {
    minimumDegrees: 0,
    maximumDegrees: 20,
  })
  assertClose(atZero.seeds[0].angleDegrees, 10)

  const negativeRadians = degreesToRadians(-10)
  const atOneEighty = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    blockingAngleDegrees: 180,
    maximumAngleDeltaDegrees: 20,
    translation: rotationDisplacement(
      UNIT_X_POINT,
      ORIGIN,
      Z_AXIS,
      negativeRadians,
    ),
  }))
  assert.ok(atOneEighty)
  assert.deepEqual(atOneEighty.angleDomain, {
    minimumDegrees: 160,
    maximumDegrees: 180,
  })
  assertClose(atOneEighty.seeds[0].angleDegrees, 170)

  const negativeZero = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    blockingAngleDegrees: -0,
    maximumAngleDeltaDegrees: 20,
    translation: rotationDisplacement(
      UNIT_X_POINT,
      ORIGIN,
      Z_AXIS,
      positiveRadians,
    ),
  }))
  assert.ok(negativeZero)
  assert.equal(Object.is(negativeZero.blockingAngleDegrees, -0), false)
  assert.equal(Object.is(negativeZero.angleDomain.minimumDegrees, -0), false)
})

test('an optimum outside maximum delta produces the correct endpoint seed', () => {
  const positiveTranslation = rotationDisplacement(
    UNIT_X_POINT,
    ORIGIN,
    Z_AXIS,
    degreesToRadians(30),
  )
  const maximum = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    maximumAngleDeltaDegrees: 5,
    translation: positiveTranslation,
  }))
  assert.ok(maximum)
  assert.equal(maximum.evaluatedCandidateCount, 2)
  assert.equal(maximum.seeds[0].source, 'angle_domain_maximum')
  assert.equal(maximum.seeds[0].angleDegrees, 95)
  assert.equal(maximum.seeds[0].signedDeltaDegrees, 5)

  const minimumWithReversedSign =
    deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
      childRotationSign: -1,
      maximumAngleDeltaDegrees: 5,
      translation: positiveTranslation,
    }))
  assert.ok(minimumWithReversedSign)
  assert.equal(
    minimumWithReversedSign.seeds[0].source,
    'angle_domain_minimum',
  )
  assert.equal(minimumWithReversedSign.seeds[0].angleDegrees, 85)
  assert.equal(minimumWithReversedSign.seeds[0].signedDeltaDegrees, -5)
  assertClose(
    minimumWithReversedSign.seeds[0].signedRotationRadians,
    degreesToRadians(5),
  )

  const tiedEndpoints = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({
      maximumAngleDeltaDegrees: 30,
      translation: { x: -2, y: 0, z: 0 },
    }),
  )
  assert.ok(tiedEndpoints)
  assert.deepEqual(
    tiedEndpoints.seeds.map((seed) => ({
      rank: seed.rank,
      source: seed.source,
      angleDegrees: seed.angleDegrees,
    })),
    [
      {
        rank: 1,
        source: 'angle_domain_minimum',
        angleDegrees: 60,
      },
      {
        rank: 2,
        source: 'angle_domain_maximum',
        angleDegrees: 120,
      },
    ],
  )
  assert.equal(
    tiedEndpoints.seeds[0].residualSquared,
    tiedEndpoints.seeds[1].residualSquared,
  )
})

test('point order is deterministic while duplicate identities fail closed', () => {
  const movingPoints = [
    { id: 'z-point', position: { x: 3, y: 1, z: 0 } },
    { id: 'a-point', position: { x: 1, y: -1, z: 0 } },
    { id: 'm-point', position: { x: 2, y: 0.5, z: 1 } },
  ]
  const forward = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({ movingPoints }),
  )
  const reverse = deriveFoldPreviewSingleHingeRotationFitSeeds(
    validInput({ movingPoints: [...movingPoints].reverse() }),
  )
  assert.ok(forward && reverse)
  assert.deepEqual(reverse, forward)

  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
      movingPoints: [
        { id: 'duplicate', position: UNIT_X_POINT },
        { id: 'duplicate', position: { x: 2, y: 0, z: 0 } },
      ],
    })),
    null,
  )

  const coincident = deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
    movingPoints: [
      { id: 'first', position: UNIT_X_POINT },
      { id: 'second', position: UNIT_X_POINT },
    ],
  }))
  assert.ok(coincident)
  assert.equal(coincident.pointCount, 2)
})

test('result metadata is deterministic, detached, and deeply frozen', () => {
  const mutableTranslation = rotationDisplacement(
    UNIT_X_POINT,
    ORIGIN,
    Z_AXIS,
    degreesToRadians(20),
  ) as { x: number; y: number; z: number }
  const input = validInput({ translation: mutableTranslation })
  const first = deriveFoldPreviewSingleHingeRotationFitSeeds(input)
  const second = deriveFoldPreviewSingleHingeRotationFitSeeds(input)
  assert.ok(first && second)
  assert.deepEqual(second, first)
  assert.notStrictEqual(second, first)
  mutableTranslation.x = 123
  assert.notEqual(first.translation.x, mutableTranslation.x)
  assert.equal(
    first.version,
    FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS_VERSION,
  )
  assert.equal(first.kind, 'unverified_single_hinge_rotation_fit_seeds')
  assert.deepEqual(first.analysis, {
    method: 'bounded_finite_rotation_least_squares_v1',
    objective: 'moving_material_points_match_common_translation',
    maximumPointCount: MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS,
    maximumSeedCount: MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS,
  })
  assert.deepEqual(first.safety, {
    modelIdentityBound: false,
    collisionConstraintsRevalidated: false,
    legalCorrectionPoseGenerated: false,
    staticCandidateRevalidated: false,
    continuousCandidatePathCertified: false,
    autoApplicable: false,
  })
  assertDeeplyFrozen(first)
})

test('non-finite, invalid, and overflowing inputs fail closed', () => {
  const invalidInputs: unknown[] = [
    null,
    [],
    { ...validInput(), axis: null },
    {
      ...validInput(),
      axis: { point: ORIGIN, direction: { x: 0, y: 0, z: 0 } },
    },
    {
      ...validInput(),
      axis: { point: ORIGIN, direction: { x: 0, y: 0, z: 2 } },
    },
    {
      ...validInput(),
      axis: {
        point: { x: Number.POSITIVE_INFINITY, y: 0, z: 0 },
        direction: Z_AXIS,
      },
    },
    { ...validInput(), childRotationSign: 0 },
    { ...validInput(), blockingAngleDegrees: Number.NaN },
    { ...validInput(), blockingAngleDegrees: -1 },
    { ...validInput(), blockingAngleDegrees: 181 },
    { ...validInput(), maximumAngleDeltaDegrees: 0 },
    { ...validInput(), maximumAngleDeltaDegrees: -1 },
    { ...validInput(), maximumAngleDeltaDegrees: 181 },
    {
      ...validInput(),
      maximumAngleDeltaDegrees: Number.POSITIVE_INFINITY,
    },
    {
      ...validInput(),
      translation: { x: Number.NaN, y: 0, z: 0 },
    },
    { ...validInput(), movingPoints: [] },
    {
      ...validInput(),
      movingPoints: [{ id: '', position: UNIT_X_POINT }],
    },
    {
      ...validInput(),
      movingPoints: [{
        id: 'x'.repeat(513),
        position: UNIT_X_POINT,
      }],
    },
    {
      ...validInput(),
      movingPoints: [{
        id: 'non-finite',
        position: { x: Number.NEGATIVE_INFINITY, y: 0, z: 0 },
      }],
    },
    {
      ...validInput(),
      axis: {
        point: { x: -Number.MAX_VALUE, y: 0, z: 0 },
        direction: Z_AXIS,
      },
      movingPoints: [{
        id: 'subtraction-overflow',
        position: { x: Number.MAX_VALUE, y: 0, z: 0 },
      }],
    },
    {
      ...validInput(),
      movingPoints: [{
        id: 'square-overflow',
        position: { x: 1e308, y: 0, z: 0 },
      }],
    },
    {
      ...validInput(),
      translation: { x: 1e154, y: 0, z: 0 },
      movingPoints: [
        { id: 'sum-overflow-a', position: UNIT_X_POINT },
        {
          id: 'sum-overflow-b',
          position: { x: 1, y: 0, z: 1 },
        },
      ],
    },
  ]

  for (const [index, invalid] of invalidInputs.entries()) {
    assert.equal(
      deriveFoldPreviewSingleHingeRotationFitSeeds(
        invalid as FoldPreviewSingleHingeRotationFitInput,
      ),
      null,
      `invalid case ${index}`,
    )
  }
})

test('oversized point arrays are rejected before indexed access', () => {
  let lengthReads = 0
  let indexReads = 0
  const oversized = new Proxy([], {
    get(target, property, receiver) {
      if (property === 'length') {
        lengthReads += 1
        return MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS + 1
      }
      if (typeof property === 'string' && /^\d+$/u.test(property)) {
        indexReads += 1
        throw new Error('oversized point index must not be read')
      }
      return Reflect.get(target, property, receiver)
    },
  })

  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(validInput({
      movingPoints:
        oversized as FoldPreviewSingleHingeRotationFitInput['movingPoints'],
    })),
    null,
  )
  assert.equal(lengthReads, 1)
  assert.equal(indexReads, 0)
})

test('public records and arrays are snapshotted once and hostile proxies fail closed', () => {
  const radians = degreesToRadians(20)
  const guardedAxisPoint = oneReadProxy({ ...ORIGIN })
  const guardedAxisDirection = oneReadProxy({ ...Z_AXIS })
  const guardedTranslation = oneReadProxy(rotationDisplacement(
    UNIT_X_POINT,
    ORIGIN,
    Z_AXIS,
    radians,
  ))
  const guardedPosition = oneReadProxy({ ...UNIT_X_POINT })
  const guardedMovingPoint = oneReadProxy({
    id: 'guarded-point',
    position: guardedPosition.value,
  })
  const guardedMovingPoints = oneReadProxy([guardedMovingPoint.value])
  const guardedAxis = oneReadProxy({
    point: guardedAxisPoint.value,
    direction: guardedAxisDirection.value,
  })
  const guardedInput = oneReadProxy({
    axis: guardedAxis.value,
    childRotationSign: 1 as const,
    blockingAngleDegrees: 90,
    maximumAngleDeltaDegrees: 60,
    translation: guardedTranslation.value,
    movingPoints: guardedMovingPoints.value,
  })

  const result = deriveFoldPreviewSingleHingeRotationFitSeeds(
    guardedInput.value,
  )
  assert.ok(result)
  guardedInput.assertOnlyRead([
    'axis',
    'childRotationSign',
    'blockingAngleDegrees',
    'maximumAngleDeltaDegrees',
    'translation',
    'movingPoints',
  ])
  guardedAxis.assertOnlyRead(['point', 'direction'])
  guardedAxisPoint.assertOnlyRead(['x', 'y', 'z'])
  guardedAxisDirection.assertOnlyRead(['x', 'y', 'z'])
  guardedTranslation.assertOnlyRead(['x', 'y', 'z'])
  guardedMovingPoints.assertOnlyRead(['length', '0'])
  guardedMovingPoint.assertOnlyRead(['id', 'position'])
  guardedPosition.assertOnlyRead(['x', 'y', 'z'])

  const throwing = new Proxy(validInput(), {
    get() {
      throw new Error('hostile input')
    },
  })
  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(throwing),
    null,
  )
  const revocable = Proxy.revocable(validInput(), {})
  revocable.revoke()
  assert.equal(
    deriveFoldPreviewSingleHingeRotationFitSeeds(revocable.proxy),
    null,
  )
})

function validInput(
  overrides: Partial<FoldPreviewSingleHingeRotationFitInput> = {},
): FoldPreviewSingleHingeRotationFitInput {
  return {
    axis: { point: ORIGIN, direction: Z_AXIS },
    childRotationSign: 1,
    blockingAngleDegrees: 90,
    maximumAngleDeltaDegrees: 60,
    translation: rotationDisplacement(
      UNIT_X_POINT,
      ORIGIN,
      Z_AXIS,
      degreesToRadians(30),
    ),
    movingPoints: [{ id: 'point', position: UNIT_X_POINT }],
    ...overrides,
  }
}

function rotationDisplacement(
  point: Point,
  axisPoint: Point,
  axisDirection: Point,
  radians: number,
): Point {
  const offset = subtract(point, axisPoint)
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  const axial = dot(axisDirection, offset)
  const tangent = cross(axisDirection, offset)
  const rotatedOffset = {
    x: offset.x * cosine
      + tangent.x * sine
      + axisDirection.x * axial * (1 - cosine),
    y: offset.y * cosine
      + tangent.y * sine
      + axisDirection.y * axial * (1 - cosine),
    z: offset.z * cosine
      + tangent.z * sine
      + axisDirection.z * axial * (1 - cosine),
  }
  return {
    x: rotatedOffset.x - offset.x,
    y: rotatedOffset.y - offset.y,
    z: rotatedOffset.z - offset.z,
  }
}

function independentResidualSquared(
  points: readonly Point[],
  axisPoint: Point,
  axisDirection: Point,
  translation: Point,
  radians: number,
) {
  let result = 0
  for (const point of points) {
    const displacement = rotationDisplacement(
      point,
      axisPoint,
      axisDirection,
      radians,
    )
    const error = subtract(displacement, translation)
    result += squaredLength(error)
  }
  return result
}

function squaredLength(point: Point) {
  return point.x * point.x + point.y * point.y + point.z * point.z
}

function subtract(first: Point, second: Point): Point {
  return {
    x: first.x - second.x,
    y: first.y - second.y,
    z: first.z - second.z,
  }
}

function cross(first: Point, second: Point): Point {
  return {
    x: first.y * second.z - first.z * second.y,
    y: first.z * second.x - first.x * second.z,
    z: first.x * second.y - first.y * second.x,
  }
}

function dot(first: Point, second: Point) {
  return first.x * second.x + first.y * second.y + first.z * second.z
}

function degreesToRadians(degrees: number) {
  return degrees * Math.PI / 180
}

function assertClose(actual: number, expected: number, tolerance = 1e-12) {
  const difference = Math.abs(actual - expected)
  const scale = Math.max(1, Math.abs(actual), Math.abs(expected))
  assert.ok(
    difference <= tolerance * scale,
    `${actual} is not close to ${expected}`,
  )
}

function assertDeeplyFrozen(value: unknown, seen = new Set<object>()) {
  if (typeof value !== 'object' || value === null || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const property of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[property],
      seen,
    )
  }
}

function oneReadProxy<T extends object>(target: T) {
  const reads = new Map<PropertyKey, number>()
  const value = new Proxy(target, {
    get(source, property, receiver) {
      const next = (reads.get(property) ?? 0) + 1
      reads.set(property, next)
      if (next > 1) {
        throw new Error(`property reread: ${String(property)}`)
      }
      return Reflect.get(source, property, receiver)
    },
  })
  return {
    value,
    assertOnlyRead(expected: readonly PropertyKey[]) {
      assert.deepEqual([...reads.keys()], [...expected])
      for (const property of expected) {
        assert.equal(reads.get(property), 1, String(property))
      }
    },
  }
}
