import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { PROJECT_LAYER_PANEL_TEXT as TEXT } from '../src/lib/projectLayerPanelText.ts'

test('project layer panel catalog is closed and deeply frozen', () => {
  assert.equal(Object.keys(TEXT).length, 43)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const entry of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(entry), ['ja', 'en'])
    assert.equal(Object.isFrozen(entry), true)
  }
  assert.equal(TEXT.heading.ja, 'レイヤー')
  assert.equal(TEXT.heading.en, 'Layers')
  assert.equal(
    TEXT.unsupportedObjects.en,
    'Annotation and underlay layers can be created empty, renamed, reordered, and deleted. Editing annotation and underlay objects is not yet supported in the first release.',
  )
})

test('project layer placeholders are locale-equivalent and preserve output', () => {
  const placeholders = Object.fromEntries(
    Object.entries(TEXT).flatMap(([key, entry]) => {
      const ja = [...entry.ja.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1])
      const en = [...entry.en.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1])
      return ja.length === 0 && en.length === 0 ? [] : [[key, { ja, en }]]
    }),
  )
  assert.deepEqual(placeholders, {
    layerCount: { ja: ['count'], en: ['count'] },
    assignmentCount: { ja: ['count'], en: ['count'] },
    renameLabel: { ja: ['name'], en: ['name'] },
    presentationLabel: { ja: ['name'], en: ['name'] },
    opacityInputLabel: { ja: ['name'], en: ['name'] },
    moveUpLabel: { ja: ['name'], en: ['name'] },
    moveDownLabel: { ja: ['name'], en: ['name'] },
    assignLabel: { ja: ['name'], en: ['name'] },
    assignedLabel: { ja: ['name'], en: ['name'] },
    deleteLabel: { ja: ['name'], en: ['name'] },
    defaultDeleteLabel: { ja: ['name'], en: ['name'] },
    deleteConfirmation: {
      ja: ['name', 'count'],
      en: ['name', 'count'],
    },
  })
  assert.equal(
    formatLocalizedText('ja', TEXT.deleteConfirmation, {
      name: '補助',
      count: 3,
    }),
    'レイヤー「補助」を削除しますか？このレイヤーへ明示割当された折り線3本は既定レイヤーへ戻ります。この操作は元に戻せます。',
  )
})
