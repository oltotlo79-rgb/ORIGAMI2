import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  isMeshAnimationPreviewRequest,
  normalizeMeshAnimationPreviewResponse,
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
