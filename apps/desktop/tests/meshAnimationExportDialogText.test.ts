import assert from 'node:assert/strict'
import test from 'node:test'
import { MESH_ANIMATION_EXPORT_DIALOG_TEXT } from '../src/lib/meshAnimationExportDialogText.ts'

test('mesh animation export dialog catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(MESH_ANIMATION_EXPORT_DIALOG_TEXT), ['ja', 'en'])
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT), true)
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja), true)
  assert.equal(Object.isFrozen(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en), true)
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.processing, 'Processing…')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja.numberLocale, 'ja-JP')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.numberLocale, 'en-US')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja.vertices, '頂点')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.vertices, 'vertices')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja.triangles, '三角形')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.triangles, 'triangles')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja.bytes, 'バイト')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.bytes, 'bytes')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.ja.noticePlaceholder, '\u00a0')
  assert.equal(MESH_ANIMATION_EXPORT_DIALOG_TEXT.en.noticePlaceholder, '\u00a0')
})
