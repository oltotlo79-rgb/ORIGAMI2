import assert from 'node:assert/strict'
import test from 'node:test'

import {
  canonicalizeIntegerPolarPolygonV1,
  INTEGER_POLAR_POLYGON_MODEL_ID_V1,
} from '../src/lib/integerPolarPolygon.ts'

test('integer polar model handles origin and axes without transcendental calls', () => {
  assert.equal(
    INTEGER_POLAR_POLYGON_MODEL_ID_V1,
    'ori_integer_polar_polygon_half_plane_cross_radius_xy_v1',
  )
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(
      [[0, 0], [2, 0], [0, 2], [-2, 0], [0, -2]],
      false,
    ),
    [[-2, 0], [0, -2], [0, 0], [2, 0], [0, 2]],
  )
})

test('canonical order is independent of input order in both orientations', () => {
  const points = [[8, -6], [-12, -4], [-7, 8], [10, 5]] as const
  const reversed = [...points].reverse()
  const counterclockwise = [[-12, -4], [8, -6], [10, 5], [-7, 8]]
  const clockwise = [[-12, -4], [-7, 8], [10, 5], [8, -6]]
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(points, false),
    counterclockwise,
  )
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(reversed, false),
    counterclockwise,
  )
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(points, true),
    clockwise,
  )
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(reversed, true),
    clockwise,
  )
})

test('same-ray cross zero uses radius before coordinate tie-breaks', () => {
  const result = canonicalizeIntegerPolarPolygonV1(
    [[1, 0], [2, 0], [-3, 1], [0, -1]],
    false,
  )
  assert.deepEqual(result, [[-3, 1], [0, -1], [1, 0], [2, 0]])
})

test('maximum supported coordinates remain inside exact integer arithmetic', () => {
  assert.deepEqual(
    canonicalizeIntegerPolarPolygonV1(
      [
        [-100_000, -100_000],
        [100_000, -100_000],
        [100_000, 100_000],
        [-100_000, 100_000],
      ],
      false,
    ),
    [
      [-100_000, -100_000],
      [100_000, -100_000],
      [100_000, 100_000],
      [-100_000, 100_000],
    ],
  )
})

test('duplicates and malformed numeric inputs fail closed', () => {
  for (const points of [
    [[0, 0], [0, 0], [1, 1]],
    [[0, 0], [1.5, 0], [0, 1]],
    [[0, 0], [Number.NaN, 0], [0, 1]],
    [[0, 0], [Number.POSITIVE_INFINITY, 0], [0, 1]],
    [[0, 0], [100_001, 0], [0, 1]],
    [[0, 0], [1, 0]],
    Array.from({ length: 17 }, (_, index) => [index, index + 1]),
  ]) {
    assert.equal(
      canonicalizeIntegerPolarPolygonV1(points as Array<[number, number]>, false),
      null,
    )
  }
})
