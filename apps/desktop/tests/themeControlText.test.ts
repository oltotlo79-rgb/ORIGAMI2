import assert from 'node:assert/strict'
import test from 'node:test'

import { selectLocalizedText } from '../src/lib/i18n.ts'
import { THEME_CONTROL_TEXT } from '../src/lib/themeControlText.ts'

test('theme control catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(THEME_CONTROL_TEXT), [
    'label',
    'ariaLabel',
    'system',
    'light',
    'dark',
    'effectiveAriaLabel',
    'current',
  ])
  assert.equal(Object.isFrozen(THEME_CONTROL_TEXT), true)
  for (const text of Object.values(THEME_CONTROL_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(selectLocalizedText('ja', THEME_CONTROL_TEXT.system), 'OS設定に合わせる')
  assert.equal(selectLocalizedText('en', THEME_CONTROL_TEXT.system), 'Match OS setting')
  assert.equal(selectLocalizedText('ja', THEME_CONTROL_TEXT.current), '現在:')
  assert.equal(selectLocalizedText('en', THEME_CONTROL_TEXT.current), 'Current:')
})
