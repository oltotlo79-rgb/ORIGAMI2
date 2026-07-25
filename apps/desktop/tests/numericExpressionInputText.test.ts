import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import {
  FAILED_EVALUATION_TEXT,
  NUMERIC_EXPRESSION_ERROR_TEXT,
  NUMERIC_EXPRESSION_TEXT,
} from '../src/lib/numericExpressionInputText.ts'

test('numeric expression catalogs are closed, deeply frozen, and preserve placeholders', () => {
  assert.equal(Object.isFrozen(NUMERIC_EXPRESSION_TEXT), true)
  assert.equal(Object.isFrozen(NUMERIC_EXPRESSION_ERROR_TEXT), true)
  assert.equal(Object.isFrozen(FAILED_EVALUATION_TEXT), true)
  for (const text of [
    ...Object.values(NUMERIC_EXPRESSION_TEXT),
    ...Object.values(NUMERIC_EXPRESSION_ERROR_TEXT),
  ]) assert.equal(Object.isFrozen(text), true)
  assert.equal(formatLocalizedText('ja', NUMERIC_EXPRESSION_TEXT.source, {
    source: '200 * sqrt(2)',
  }), '式: 200 * sqrt(2)')
  assert.equal(formatLocalizedText('en', NUMERIC_EXPRESSION_TEXT.exactValue, {
    value: '282.84',
  }), 'Value: 282.84 mm')
  assert.equal(
    NUMERIC_EXPRESSION_ERROR_TEXT.invalid_response,
    FAILED_EVALUATION_TEXT,
  )
})
