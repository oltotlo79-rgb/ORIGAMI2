import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import { LENGTH_UNIT_CONTROL_TEXT } from '../src/lib/lengthUnitControlText.ts'

test('length unit control catalog is closed, deeply frozen, and preserves placeholders', () => {
  assert.deepEqual(Object.keys(LENGTH_UNIT_CONTROL_TEXT), [
    'legend', 'unit', 'millimetres', 'centimetres', 'inches',
    'paperEdgeRatio', 'referenceEdge', 'referenceEdgeAriaLabel',
    'invalidSavedReference', 'edgeOption', 'ratioNote',
    'invalidReferenceWithId', 'invalidReference', 'repairNote', 'noReference',
  ])
  assert.equal(Object.isFrozen(LENGTH_UNIT_CONTROL_TEXT), true)
  for (const text of Object.values(LENGTH_UNIT_CONTROL_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(formatLocalizedText('en', LENGTH_UNIT_CONTROL_TEXT.edgeOption, {
    index: 2, edgeId: 'edge-right', length: '200 mm',
  }), 'Edge 2 · edge-right · 200 mm')
  assert.equal(formatLocalizedText(
    'ja',
    LENGTH_UNIT_CONTROL_TEXT.invalidReferenceWithId,
    { edgeId: 'edge-x' },
  ), '保存された基準辺「edge-x」を現在の輪郭で一意に確認できません。')
})
