import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  isStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
} from '../src/lib/stackedFoldRead.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const request = {
  expectedProjectInstanceId: projectInstanceId,
  expectedProjectId: projectId,
  expectedRevision: 3,
  first: [0, 0, 0],
  second: [10, 0, 0],
  fixedSide: 'left',
  rotationDirection: 'positive',
  requestedAngleDegrees: 180,
} as const

describe('stacked-fold read boundary', () => {
  it('admits only finite, non-degenerate, closed-enum requests', () => {
    assert.equal(isStackedFoldReadRequest(request), true)
    assert.equal(isStackedFoldReadRequest({ ...request, second: [0, 0, 0] }), false)
    assert.equal(isStackedFoldReadRequest({ ...request, requestedAngleDegrees: Number.NaN }), false)
    assert.equal(isStackedFoldReadRequest({ ...request, fixedSide: 'center' }), false)
  })

  it('accepts a read-only response bound to the requested project revision', () => {
    const response = {
      guardModelId: 'guard-v1',
      proposalModelId: 'proposal-v1',
      materialMapModelId: 'material-v1',
      binding: {
        projectInstanceId,
        projectId,
        sourceRevision: 3,
        poseGeneration: 7,
        layerOrderGeneration: 8,
      },
      support: 'bit_exact_flat_endpoint_tree',
      crossedCells: [],
      targetFaces: [],
      materialSegments: [],
      topologyProof: { targetFingerprintSha256: 'a'.repeat(64) },
      endpointCollision: { hasBlockingHold: false },
      work: { scannedCells: 1 },
      authorizesProjectMutation: false,
      authorizesApplyStackedFold: false,
      flatEndpointLayerOrder: {
        applicable: true,
        certified: true,
        materialFaceCount: 3,
        overlapCellCount: 1,
      },
    }
    assert.deepEqual(normalizeStackedFoldReadResponse(response, request), response)
  })

  it('fails closed on stale authority, mutation authority, and contradictory layer order', () => {
    const response = {
      guardModelId: 'guard-v1',
      proposalModelId: 'proposal-v1',
      materialMapModelId: 'material-v1',
      binding: {
        projectInstanceId,
        projectId,
        sourceRevision: 3,
        poseGeneration: 8,
        layerOrderGeneration: 9,
      },
      support: 'no_hinge_single_face',
      crossedCells: [],
      targetFaces: [],
      materialSegments: [],
      topologyProof: { targetFingerprintSha256: 'b'.repeat(64) },
      endpointCollision: { hasBlockingHold: false },
      work: { scannedCells: 0 },
      authorizesProjectMutation: false,
      authorizesApplyStackedFold: false,
      flatEndpointLayerOrder: {
        applicable: false,
        certified: false,
        materialFaceCount: 0,
        overlapCellCount: 0,
      },
    }
    assert.equal(
      normalizeStackedFoldReadResponse(
        { ...response, binding: { ...response.binding, sourceRevision: 4 } },
        request,
      ),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        { ...response, authorizesApplyStackedFold: true },
        request,
      ),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          flatEndpointLayerOrder: {
            applicable: false,
            certified: true,
            materialFaceCount: 1,
            overlapCellCount: 1,
          },
        },
        request,
      ),
      null,
    )
  })
})
