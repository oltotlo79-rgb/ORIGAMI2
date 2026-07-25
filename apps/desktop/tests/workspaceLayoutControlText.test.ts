import assert from 'node:assert/strict'
import test from 'node:test'

import { WORKSPACE_LAYOUT_CONTROL_TEXT } from '../src/lib/workspaceLayoutControlText.ts'

test('workspace layout control catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(WORKSPACE_LAYOUT_CONTROL_TEXT), [
    'summary',
    'groupAriaLabel',
    'swapPanels',
    'movePropertiesLeft',
    'movePropertiesRight',
    'reset',
    'outputAriaLabel',
    'properties',
    'timeline',
  ])
  assert.equal(Object.isFrozen(WORKSPACE_LAYOUT_CONTROL_TEXT), true)
  for (const text of Object.values(WORKSPACE_LAYOUT_CONTROL_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(WORKSPACE_LAYOUT_CONTROL_TEXT.groupAriaLabel, {
    ja: '作業レイアウト',
    en: 'Workspace layout',
  })
  assert.deepEqual(WORKSPACE_LAYOUT_CONTROL_TEXT.outputAriaLabel, {
    ja: '現在の作業レイアウト',
    en: 'Current workspace layout',
  })
})
