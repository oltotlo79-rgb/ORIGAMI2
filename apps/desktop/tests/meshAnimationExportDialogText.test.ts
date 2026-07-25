import assert from 'node:assert/strict'
import test from 'node:test'
import { MESH_ANIMATION_EXPORT_DIALOG_TEXT } from '../src/lib/meshAnimationExportDialogText.ts'

test('mesh animation export dialog catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(MESH_ANIMATION_EXPORT_DIALOG_TEXT), ['ja', 'en'])
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT), true)
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja), true)
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en), true)
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.processing, 'Processing…')
})
