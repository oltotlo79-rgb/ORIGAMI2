import assert from 'node:assert/strict'
import test from 'node:test'

import { CREATION_DIMENSION_EXPRESSION_TEXT } from '../src/lib/creationDimensionExpressionText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

test('creation dimension expression catalog is closed, frozen, and formats dimensions', () => {
  assert.deepEqual(Object.keys(CREATION_DIMENSION_EXPRESSION_TEXT), [
    'label',
    'dimensions',
    'showValue',
    'showExpression',
  ])
  assert.equal(Object.isFrozen(CREATION_DIMENSION_EXPRESSION_TEXT), true)
  for (const text of Object.values(CREATION_DIMENSION_EXPRESSION_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(
    formatLocalizedText(
      'en',
      CREATION_DIMENSION_EXPRESSION_TEXT.dimensions,
      { width: '200', height: '300' },
    ),
    '200 × 300 mm',
  )
  assert.equal(
    formatLocalizedText(
      'ja',
      CREATION_DIMENSION_EXPRESSION_TEXT.dimensions,
      { width: '{width}', height: '<height>' },
    ),
    '{width} × <height> mm',
  )
})
