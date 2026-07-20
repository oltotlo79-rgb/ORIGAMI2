import assert from 'node:assert/strict'
import test from 'node:test'
import {
  normalizeFold3dFrameSelection,
  normalizeFold3dFramesPicker,
  normalizeFold3dPoseCompatibility,
} from '../src/lib/fold3dFrames.ts'

const id = '00000000-0000-4000-8000-000000000001'

test('strict FOLD 3D picker parser admits bounded metadata only', () => {
  const parsed = normalizeFold3dFramesPicker({
    canceled: false,
    preview: {
      token: id,
      projectInstanceId: id,
      projectId: id,
      revision: 2,
      frameCount: 1,
      frames: [{ index: 0, parent: null, inherits: true, vertexCount: 3 }],
      authorizesProjectImport: false,
    },
  })
  assert.equal(parsed?.preview?.frames[0]?.vertexCount, 3)
  assert.equal(normalizeFold3dFramesPicker({
    canceled: false,
    preview: { ...parsed?.preview, coordinates: [[0, 0, 0]] },
  }), null)
})

test('pose compatibility parser keeps geometry mutation authority inert', () => {
  const value = {
    token: id,
    frameIndex: 1,
    hingeCount: 2,
    sourceFingerprint: 'cd'.repeat(32),
    authorizesProjectGeometryMutation: false,
    requiresExplicitApply: true,
  }
  assert.ok(normalizeFold3dPoseCompatibility(value))
  assert.equal(normalizeFold3dPoseCompatibility({
    ...value,
    authorizesProjectGeometryMutation: true,
  }), null)
})

test('selection parser accepts only bounded PNG and inert authorities', () => {
  const value = {
    token: id,
    frameIndex: 0,
    vertexCount: 3,
    sourceSha256Hex: 'ab'.repeat(32),
    previewImageDataUrl: 'data:image/png;base64,iVBORw0KGgo=',
    previewWidth: 512,
    previewHeight: 384,
    renderCoordinatesExposed: false,
    authorizesProjectImport: false,
    authorizesAppliedPose: false,
    authorizesInstructionTimeline: false,
  }
  assert.ok(normalizeFold3dFrameSelection(value))
  assert.equal(normalizeFold3dFrameSelection({
    ...value,
    authorizesProjectImport: true,
  }), null)
  assert.equal(normalizeFold3dFrameSelection({
    ...value,
    previewImageDataUrl: 'data:image/svg+xml,<svg/>',
  }), null)
})
