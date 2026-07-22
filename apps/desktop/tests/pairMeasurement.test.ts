import assert from 'node:assert/strict'
import test from 'node:test'
import { advanceMeasurementPair, measureUnorientedEdgeAngle, measureVertexPair, retainMeasurementPair } from '../src/lib/pairMeasurement.ts'

test('pair measurements are finite, ID-distinct, and edge-direction independent', () => {
  assert.equal(measureVertexPair({ id: 'a', x: 0, y: 0 }, { id: 'b', x: 3, y: 4 }), 5)
  const x = { id: 'x', x1: 0, y1: 0, x2: 1, y2: 0 }
  assert.equal(measureUnorientedEdgeAngle(x, { id: 'y', x1: 0, y1: 1, x2: 0, y2: 0 }), 90)
  assert.equal(measureUnorientedEdgeAngle(x, { id: 'z', x1: 1, y1: 0, x2: 0, y2: 0 }), 0)
  assert.equal(measureUnorientedEdgeAngle(x, { id: 'd', x1: 0, y1: 0, x2: 0, y2: 0 }), null)
  assert.equal(measureVertexPair({ id: 'a', x: 0, y: 0 }, { id: 'a', x: 1, y: 1 }), null)
})

test('stale and duplicate measurement selections are removed after deletion', () => {
  assert.deepEqual(retainMeasurementPair(['a', 'gone', 'a'], new Set(['a'])), ['a'])
})

test('non-finite and overflowing geometry fails closed', () => {
  assert.equal(measureVertexPair({ id: 'a', x: 0, y: 0 }, { id: 'b', x: Infinity, y: 0 }), null)
  assert.equal(measureVertexPair({ id: 'a', x: -1e308, y: 0 }, { id: 'b', x: 1e308, y: 0 }), null)
  assert.equal(measureUnorientedEdgeAngle(
    { id: 'large-a', x1: 0, y1: 0, x2: 1e200, y2: 0 },
    { id: 'large-b', x1: 0, y1: 0, x2: 0, y2: 1e200 },
  ), 90)
  assert.equal(measureUnorientedEdgeAngle(
    { id: 'tiny-a', x1: 0, y1: 0, x2: 1e-300, y2: 0 },
    { id: 'tiny-b', x1: 0, y1: 0, x2: -1e-300, y2: 0 },
  ), 0)
})

test('pair selection toggles, caps at two, and restarts deterministically', () => {
  assert.deepEqual(advanceMeasurementPair([], 'a'), ['a'])
  assert.deepEqual(advanceMeasurementPair(['a'], 'b'), ['a', 'b'])
  assert.deepEqual(advanceMeasurementPair(['a', 'b'], 'c'), ['c'])
  assert.deepEqual(advanceMeasurementPair(['a', 'b'], 'a'), ['b'])
})
