import assert from 'node:assert/strict'
import test from 'node:test'

import {
  findFoldPreviewSingleAxisSweptAabb,
  type FoldPreviewContinuousIntervalAabb,
  type FoldPreviewContinuousPoint3,
} from '../src/lib/foldPreviewContinuousInterval.ts'

test('explicit Rodrigues-rotated interval samples stay inside the swept AABB', () => {
  const cases: readonly Readonly<{
    vertices: readonly FoldPreviewContinuousPoint3[]
    axisStart: FoldPreviewContinuousPoint3
    axisEnd: FoldPreviewContinuousPoint3
    span: number
  }>[] = [
    {
      vertices: [
        { x: 0, y: 2, z: 0 },
        { x: 3, y: -1, z: 4 },
        { x: -2, y: 0, z: 1 },
      ],
      axisStart: { x: 0, y: 0, z: 0 },
      axisEnd: { x: 1, y: 0, z: 0 },
      span: Math.PI / 3,
    },
    {
      vertices: [
        { x: -4, y: 3, z: 2 },
        { x: 0.5, y: -2, z: 7 },
        { x: 1, y: 4, z: -3 },
      ],
      axisStart: { x: -2, y: 0.5, z: 1 },
      axisEnd: { x: 1, y: 4, z: 3 },
      span: Math.PI,
    },
    {
      vertices: [
        { x: 5, y: -3, z: 2 },
        { x: -1, y: 8, z: -4 },
      ],
      axisStart: { x: 3, y: -2, z: 4 },
      axisEnd: { x: 3, y: -2, z: -1 },
      span: Math.PI * 2,
    },
  ]

  for (const current of cases) {
    const bounds = findFoldPreviewSingleAxisSweptAabb(
      current.vertices,
      current.axisStart,
      current.axisEnd,
      current.span,
    )
    assert.ok(bounds)
    for (const intervalFraction of [-0.5, -0.25, 0, 0.25, 0.5]) {
      for (const vertex of current.vertices) {
        assertContains(
          bounds,
          rotateByRodrigues(
            vertex,
            current.axisStart,
            current.axisEnd,
            intervalFraction * current.span,
          ),
        )
      }
    }
  }
})

test('zero span preserves the midpoint AABB apart from the numeric margin', () => {
  const vertices = [
    { x: -2, y: 1, z: 4 },
    { x: 3, y: -5, z: 0.5 },
    { x: 0, y: 0, z: 0 },
  ] as const
  const bounds = findFoldPreviewSingleAxisSweptAabb(
    vertices,
    { x: 0, y: 0, z: -1 },
    { x: 0, y: 0, z: 1 },
    0,
  )
  assert.ok(bounds)
  assert.equal(bounds.maximumChordDisplacement, 0)
  assert.equal(bounds.minX, -2 - bounds.numericalMargin)
  assert.equal(bounds.minY, -5 - bounds.numericalMargin)
  assert.equal(bounds.minZ, 0 - bounds.numericalMargin)
  assert.equal(bounds.maxX, 3 + bounds.numericalMargin)
  assert.equal(bounds.maxY, 1 + bounds.numericalMargin)
  assert.equal(bounds.maxZ, 4 + bounds.numericalMargin)
  assert.equal(Object.isFrozen(bounds), true)
})

test('off-axis radius controls a conservative global expansion', () => {
  const vertices = [
    { x: 0, y: 0, z: 0 },
    { x: 2, y: 0, z: 0 },
    { x: 10, y: 0, z: 0 },
  ] as const
  const bounds = findFoldPreviewSingleAxisSweptAabb(
    vertices,
    { x: 0, y: 0, z: -2 },
    { x: 0, y: 0, z: 2 },
    Math.PI,
  )
  assert.ok(bounds)
  assert.equal(bounds.maximumPerpendicularRadius, 10)
  assert.ok(Math.abs(
    bounds.maximumChordDisplacement - 10 * Math.SQRT2,
  ) <= Number.EPSILON * 64)

  // The near-axis vertex receives the same global maximum expansion. This is
  // deliberately less tight than a per-vertex union, but remains conservative.
  for (const vertex of vertices) {
    assertContains(bounds, rotateByRodrigues(
      vertex,
      { x: 0, y: 0, z: -2 },
      { x: 0, y: 0, z: 2 },
      Math.PI / 2,
    ))
  }
})

test('the displacement and bounds expand monotonically up to one full turn', () => {
  const vertices = [
    { x: -3, y: 2, z: 1 },
    { x: 4, y: -1, z: 5 },
  ] as const
  const axisStart = { x: 0, y: -2, z: 0 } as const
  const axisEnd = { x: 0, y: 3, z: 0 } as const
  const spans = [0, Math.PI / 6, Math.PI / 2, Math.PI, Math.PI * 2]
  const results = spans.map((span) =>
    findFoldPreviewSingleAxisSweptAabb(
      vertices,
      axisStart,
      axisEnd,
      span,
    ))
  assert.ok(results.every(Boolean))
  for (let index = 1; index < results.length; index += 1) {
    const previous = results[index - 1]
    const current = results[index]
    assert.ok(previous && current)
    assert.ok(current.maximumChordDisplacement >= previous.maximumChordDisplacement)
    assert.ok(current.minX <= previous.minX)
    assert.ok(current.minY <= previous.minY)
    assert.ok(current.minZ <= previous.minZ)
    assert.ok(current.maxX >= previous.maxX)
    assert.ok(current.maxY >= previous.maxY)
    assert.ok(current.maxZ >= previous.maxZ)
  }
})

test('negative and positive spans of the same magnitude have identical bounds', () => {
  const vertices = [{ x: 2, y: 3, z: 4 }] as const
  const common = [
    vertices,
    { x: -1, y: 0, z: 0 },
    { x: 1, y: 2, z: 3 },
  ] as const
  const positive = findFoldPreviewSingleAxisSweptAabb(...common, 1.25)
  const negative = findFoldPreviewSingleAxisSweptAabb(...common, -1.25)
  assert.ok(positive && negative)
  assert.deepEqual(negative, positive)
})

test('invalid, degenerate, excessive, and unrepresentable inputs fail closed', () => {
  const validVertex = { x: 1, y: 2, z: 3 } as const
  const origin = { x: 0, y: 0, z: 0 } as const
  const xAxis = { x: 1, y: 0, z: 0 } as const
  assert.equal(findFoldPreviewSingleAxisSweptAabb([], origin, xAxis, 0), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [{ x: Number.NaN, y: 0, z: 0 }],
    origin,
    xAxis,
    0,
  ), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [validVertex],
    origin,
    origin,
    0,
  ), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [validVertex],
    origin,
    { x: Number.MIN_VALUE, y: 0, z: 0 },
    0,
  ), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [validVertex],
    origin,
    xAxis,
    Number.NaN,
  ), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [validVertex],
    origin,
    xAxis,
    Math.PI * 2 + Number.EPSILON * 8,
  ), null)
  assert.equal(findFoldPreviewSingleAxisSweptAabb(
    [{ x: Number.MAX_VALUE, y: 0, z: 0 }],
    { x: -Number.MAX_VALUE, y: 0, z: 0 },
    { x: Number.MAX_VALUE, y: 0, z: 0 },
    Math.PI,
  ), null)
})

function assertContains(
  bounds: FoldPreviewContinuousIntervalAabb,
  point: FoldPreviewContinuousPoint3,
) {
  assert.ok(point.x >= bounds.minX && point.x <= bounds.maxX)
  assert.ok(point.y >= bounds.minY && point.y <= bounds.maxY)
  assert.ok(point.z >= bounds.minZ && point.z <= bounds.maxZ)
}

function rotateByRodrigues(
  point: FoldPreviewContinuousPoint3,
  axisStart: FoldPreviewContinuousPoint3,
  axisEnd: FoldPreviewContinuousPoint3,
  angle: number,
): FoldPreviewContinuousPoint3 {
  const axisX = axisEnd.x - axisStart.x
  const axisY = axisEnd.y - axisStart.y
  const axisZ = axisEnd.z - axisStart.z
  const length = Math.hypot(axisX, axisY, axisZ)
  const unitX = axisX / length
  const unitY = axisY / length
  const unitZ = axisZ / length
  const relativeX = point.x - axisStart.x
  const relativeY = point.y - axisStart.y
  const relativeZ = point.z - axisStart.z
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  const oneMinusCosine = 1 - cosine
  const dot = unitX * relativeX + unitY * relativeY + unitZ * relativeZ
  const crossX = unitY * relativeZ - unitZ * relativeY
  const crossY = unitZ * relativeX - unitX * relativeZ
  const crossZ = unitX * relativeY - unitY * relativeX
  return {
    x: axisStart.x
      + relativeX * cosine
      + crossX * sine
      + unitX * dot * oneMinusCosine,
    y: axisStart.y
      + relativeY * cosine
      + crossY * sine
      + unitY * dot * oneMinusCosine,
    z: axisStart.z
      + relativeZ * cosine
      + crossZ * sine
      + unitZ * dot * oneMinusCosine,
  }
}
