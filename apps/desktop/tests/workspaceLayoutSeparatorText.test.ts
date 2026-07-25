import assert from 'node:assert/strict'
import test from 'node:test'
import { WORKSPACE_LAYOUT_SEPARATOR_TEXT } from '../src/lib/workspaceLayoutSeparatorText.ts'

test('workspace layout separator catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(WORKSPACE_LAYOUT_SEPARATOR_TEXT), [
    'editorLabel', 'inspectorLabel', 'timelineLabel',
  ])
  assert.equal(Object.isFrozen(WORKSPACE_LAYOUT_SEPARATOR_TEXT), true)
  for (const text of Object.values(WORKSPACE_LAYOUT_SEPARATOR_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(WORKSPACE_LAYOUT_SEPARATOR_TEXT.timelineLabel, {
    ja: '折り手順パネルの高さを変更',
    en: 'Resize instruction timeline panel',
  })
})
