import assert from 'node:assert/strict'
import test from 'node:test'
import {
  advanceFoldPreviewMeasurementIds,
  measureWorldFaceNormalAngleDegrees,
  measureWorldVertexDistanceMm,
  resolveMidsurfaceVertexSample,
} from '../src/lib/foldPreviewMeasurement.ts'

test('current-world vertex distance converts back to millimetres', () => {
  assert.equal(measureWorldVertexDistanceMm(
    { x: 0, y: 0, z: 0 }, { x: 0.3, y: 0.4, z: 0 }, 0.1,
  ), 5)
  assert.equal(measureWorldVertexDistanceMm(
    { x: 0, y: 0, z: 0 }, { x: Infinity, y: 0, z: 0 }, 1,
  ), null)
})

test('planar and pivoted single-fold registries sample the midsurface, not raised markers', () => {
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
  assert.deepEqual(resolveMidsurfaceVertexSample([
    { x: 2, z: 3, offsetX: 0, offsetZ: 0, matrix: identity },
  ], 4), { x: 2, y: 0, z: 3 })
  const pivotTranslation = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1]
  assert.deepEqual(resolveMidsurfaceVertexSample([
    { x: 2, z: 0, offsetX: -1, offsetZ: 0, matrix: pivotTranslation },
    { x: 2, z: 0, offsetX: 0, offsetZ: 0, matrix: identity },
  ], 4), { x: 2, y: 0, z: 0 })
  const folded = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
  const first = resolveMidsurfaceVertexSample([
    { x: 1, z: 0, offsetX: 0, offsetZ: 0, matrix: folded },
  ], 4)!
  assert.equal(measureWorldVertexDistanceMm(
    { x: 0, y: 0, z: 0 }, first, 1,
  ), 1)
  assert.equal(resolveMidsurfaceVertexSample([
    { x: 0, z: 0, offsetX: 0, offsetZ: 0, matrix: identity },
    { x: 1, z: 0, offsetX: 0, offsetZ: 0, matrix: identity },
  ], 4), null)
  assert.deepEqual(resolveMidsurfaceVertexSample([
    { x: 1e308, z: 0, offsetX: 0, offsetZ: 0, matrix: identity },
    { x: 1e308, z: 0, offsetX: 0, offsetZ: 0, matrix: identity },
  ], 1e308), { x: 1e308, y: 0, z: 0 })
})

test('face-normal angle preserves the full zero-to-180 degree range', () => {
  assert.equal(measureWorldFaceNormalAngleDegrees(
    { x: 0, y: 1, z: 0 }, { x: 1, y: 0, z: 0 },
  ), 90)
  assert.equal(measureWorldFaceNormalAngleDegrees(
    { x: 0, y: 1, z: 0 }, { x: 0, y: -2, z: 0 },
  ), 180)
  assert.equal(measureWorldFaceNormalAngleDegrees(
    { x: 0, y: 0, z: 0 }, { x: 0, y: 1, z: 0 },
  ), null)
})

test('measurement slots toggle and restart after two IDs', () => {
  assert.deepEqual(advanceFoldPreviewMeasurementIds([], 'a'), ['a'])
  assert.deepEqual(advanceFoldPreviewMeasurementIds(['a'], 'b'), ['a', 'b'])
  assert.deepEqual(advanceFoldPreviewMeasurementIds(['a', 'b'], 'c'), ['c'])
  assert.deepEqual(advanceFoldPreviewMeasurementIds(['a', 'b'], 'a'), ['b'])
})
