import assert from 'node:assert/strict'
import test from 'node:test'
import { ANNOTATION_PANEL_TEXT } from '../src/lib/annotationPanelText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import { LANGUAGE_CONTROL_TEXT } from '../src/lib/languageControlText.ts'
import { UNDERLAY_PANEL_TEXT } from '../src/lib/underlayPanelText.ts'

for (const [name, catalog] of [
  ['language control', LANGUAGE_CONTROL_TEXT],
  ['underlay panel', UNDERLAY_PANEL_TEXT],
  ['annotation panel', ANNOTATION_PANEL_TEXT],
] as const) {
  test(`${name} catalog is closed and deeply frozen`, () => {
    assert.equal(Object.isFrozen(catalog), true)
    for (const text of Object.values(catalog)) assert.equal(Object.isFrozen(text), true)
  })
}

test('small panel catalogs preserve labels and the underlay index placeholder', () => {
  assert.deepEqual(LANGUAGE_CONTROL_TEXT.label, {
    ja: '表示言語',
    en: 'Display language',
  })
  assert.equal(formatLocalizedText('en', UNDERLAY_PANEL_TEXT.item, {
    index: 3,
  }), 'Underlay 3')
  assert.deepEqual(ANNOTATION_PANEL_TEXT.edit, {
    ja: '注釈編集',
    en: 'Edit annotation',
  })
})
