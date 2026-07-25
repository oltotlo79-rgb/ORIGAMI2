import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { PAIR_MEASUREMENT_TEXT } from '../src/lib/pairMeasurementText.ts'

test('pair measurement catalog is closed, frozen, and formats every placeholder', () => {
  assert.deepEqual(Object.keys(PAIR_MEASUREMENT_TEXT), [
    'vertexDistance',
    'unorientedEdgeAngle',
    'pending',
  ])
  assert.equal(Object.isFrozen(PAIR_MEASUREMENT_TEXT), true)
  for (const text of Object.values(PAIR_MEASUREMENT_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(
    formatLocalizedText('ja', PAIR_MEASUREMENT_TEXT.vertexDistance, {
      value: '5 mm',
    }),
    '2頂点間の距離: 5 mm',
  )
  assert.equal(
    formatLocalizedText('en', PAIR_MEASUREMENT_TEXT.unorientedEdgeAngle, {
      value: '90°',
    }),
    'Unoriented edge angle: 90°',
  )
  assert.equal(
    formatLocalizedText('en', PAIR_MEASUREMENT_TEXT.pending, {
      vertices: 1,
      lines: 2,
    }),
    'Measure: select two vertices or two edges (vertices 1/2, edges 2/2)',
  )
})
