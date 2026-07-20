import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  isMeshAnimationPreviewRequest,
  isMeshAnimationSaveRequest,
  normalizeMeshAnimationPreviewResponse,
  normalizeMeshAnimationSaveResponse,
} from '../src/lib/meshAnimationExport.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const exportId = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const request = {
  expectedProjectInstanceId: projectInstanceId,
  expectedProjectId: projectId,
  expectedRevision: 9,
} as const

test('mesh animation preview request is strict and revision-bound', () => {
  assert.equal(isMeshAnimationPreviewRequest(request), true)
  assert.equal(isMeshAnimationPreviewRequest({ ...request, expectedRevision: -1 }), false)
  assert.equal(isMeshAnimationPreviewRequest({ ...request, future: true }), false)
})

test('mesh animation preview response admits only bounded native GLB metadata', () => {
  const response = {
    exportId,
    projectInstanceId,
    projectId,
    revision: 9,
    sourceFingerprint: 'a'.repeat(64),
    frameCount: 3,
    vertexCount: 16,
    triangleCount: 8,
    durationSeconds: 2.5,
    byteCount: 4096,
    mediaType: 'model/gltf-binary',
    fileExtension: 'glb',
    suggestedFileName: 'model-animation.glb',
  }
  assert.deepEqual(normalizeMeshAnimationPreviewResponse(response, request), response)
  assert.equal(normalizeMeshAnimationPreviewResponse({ ...response, revision: 10 }, request), null)
  assert.equal(normalizeMeshAnimationPreviewResponse({ ...response, frameCount: 257 }, request), null)
  assert.equal(
    normalizeMeshAnimationPreviewResponse({ ...response, durationSeconds: Number.NaN }, request),
    null,
  )
  assert.equal(
    normalizeMeshAnimationPreviewResponse({ ...response, byteCount: 64 * 1024 * 1024 + 1 }, request),
    null,
  )
  assert.equal(
    normalizeMeshAnimationPreviewResponse({ ...response, mediaType: 'application/octet-stream' }, request),
    null,
  )
})

test('mesh animation save transport is closed and response-only', () => {
  const save = {
    exportId,
    expectedProjectInstanceId: projectInstanceId,
    expectedProjectId: projectId,
    expectedRevision: 9,
    expectedSourceFingerprint: 'a'.repeat(64),
  }
  assert.equal(isMeshAnimationSaveRequest(save), true)
  assert.equal(isMeshAnimationSaveRequest({ ...save, futurePath: 'C:\\unsafe.glb' }), false)
  assert.equal(isMeshAnimationSaveRequest({ ...save, expectedSourceFingerprint: 'A'.repeat(64) }), false)
  assert.deepEqual(normalizeMeshAnimationSaveResponse({ canceled: false }), { canceled: false })
  assert.deepEqual(normalizeMeshAnimationSaveResponse({ canceled: true }), { canceled: true })
  assert.equal(normalizeMeshAnimationSaveResponse({ canceled: false, path: 'secret' }), null)
})
