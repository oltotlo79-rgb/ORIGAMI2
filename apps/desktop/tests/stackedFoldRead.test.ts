import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
  STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
  STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
  isStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
} from '../src/lib/stackedFoldRead.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const faceId = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
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
      guardModelId: STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
      proposalModelId: STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
      materialMapModelId: STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
      binding: {
        projectInstanceId,
        projectId,
        sourceRevision: 3,
        poseGeneration: 7,
        layerOrderGeneration: 8,
      },
      support: 'bit_exact_flat_endpoint_tree',
      crossedCells: [{ cellKeySha256: 'c'.repeat(64), bottomToTopFaces: [faceId] }],
      targetFaces: [faceId],
      materialSegments: [
        {
          faceId,
          start: [0, 0],
          end: [10, 0],
          fixedSide: 'left',
          assignment: 'mountain',
        },
      ],
      topologyProof: {
        targetFingerprintSha256: 'a'.repeat(64),
        targetVertexCount: 5,
        targetEdgeCount: 5,
        targetBoundaryVertexCount: 4,
        lineageRecordCount: 1,
        sourceEdgeSubdivisionCount: 4,
        expectedCreaseSubdivisionCount: 1,
        targetMaterialFaceCount: 2,
        targetHingeCount: 1,
      },
      endpointCollision: {
        expectedPairCount: 1,
        separatedPairCount: 0,
        touchingPairCount: 0,
        allowedPairCount: 1,
        penetratingPairCount: 0,
        indeterminatePairCount: 0,
        hasBlockingHold: false,
      },
      continuousPath: {
        modelId: 'stacked_fold_bounded_path_diagnostic_v1',
        continuousCertificateModelId: null,
        sampledPoseCount: 2,
        sampledNonblockingPoseCount: 2,
        firstSampledBlockingAngleDegrees: null,
        requestedAngleDegrees: 180,
        continuousClearanceCertified: false,
        safeStopAngleDegrees: 180,
        authorizesProjectMutation: false,
      },
      transactionProposal: {
        transactionToken: null,
        sourceProjectId: projectId,
        sourceRevision: 3,
        targetRevision: 4,
        sourceFingerprintSha256: 'e'.repeat(64),
        targetFingerprintSha256: 'a'.repeat(64),
        addedVertexCount: 1,
        addedEdgeCount: 1,
        mountainCreaseCount: 1,
        valleyCreaseCount: 0,
        timelineStepCount: 1,
        timelineCompleteHingeAngleCount: 1,
        requestedAngleDegrees: 180,
        readyForAtomicApply: false,
        failureClasses: ['continuous_path_uncertified'],
        authorizesProjectMutation: false,
      },
      work: {
        scannedCells: 1,
        totalBoundaryVertices: 4,
        totalLayerRecords: 1,
        orientationTests: 1,
        exactArithmeticOperations: 1,
        maximumExactIntegerBits: 64,
        totalExactIntegerBits: 64,
        retainedCells: 1,
        retainedTargetFaces: 1,
      },
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
    const ready = {
      ...response,
      continuousPath: {
        ...response.continuousPath,
        continuousClearanceCertified: true,
      },
      transactionProposal: {
        ...response.transactionProposal,
        transactionToken: faceId,
        readyForAtomicApply: true,
        failureClasses: [],
        authorizesProjectMutation: true,
      },
    }
    assert.deepEqual(normalizeStackedFoldReadResponse(ready, request), ready)
    assert.equal(
      normalizeStackedFoldReadResponse({
        ...ready,
        transactionProposal: { ...ready.transactionProposal, transactionToken: null },
      }, request),
      null,
    )
  })

  it('fails closed on stale authority, mutation authority, and contradictory layer order', () => {
    const response = {
      guardModelId: STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
      proposalModelId: STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
      materialMapModelId: STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
      binding: {
        projectInstanceId,
        projectId,
        sourceRevision: 3,
        poseGeneration: 8,
        layerOrderGeneration: 9,
      },
      support: 'no_hinge_single_face',
      crossedCells: [{ cellKeySha256: 'd'.repeat(64), bottomToTopFaces: [faceId] }],
      targetFaces: [faceId],
      materialSegments: [
        {
          faceId,
          start: [0, 0],
          end: [10, 0],
          fixedSide: 'left',
          assignment: 'mountain',
        },
      ],
      topologyProof: {
        targetFingerprintSha256: 'b'.repeat(64),
        targetVertexCount: 5,
        targetEdgeCount: 5,
        targetBoundaryVertexCount: 4,
        lineageRecordCount: 1,
        sourceEdgeSubdivisionCount: 4,
        expectedCreaseSubdivisionCount: 1,
        targetMaterialFaceCount: 2,
        targetHingeCount: 1,
      },
      endpointCollision: {
        expectedPairCount: 1,
        separatedPairCount: 0,
        touchingPairCount: 0,
        allowedPairCount: 1,
        penetratingPairCount: 0,
        indeterminatePairCount: 0,
        hasBlockingHold: false,
      },
      work: {
        scannedCells: 1,
        totalBoundaryVertices: 4,
        totalLayerRecords: 1,
        orientationTests: 1,
        exactArithmeticOperations: 1,
        maximumExactIntegerBits: 64,
        totalExactIntegerBits: 64,
        retainedCells: 1,
        retainedTargetFaces: 1,
      },
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
      normalizeStackedFoldReadResponse({ ...response, guardModelId: 'future-guard-v2' }, request),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse({ ...response, futureAuthority: false }, request),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          binding: { ...response.binding, futureGeneration: 10 },
        },
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
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          crossedCells: [{ ...response.crossedCells[0], cellKeySha256: 'not-a-hash' }],
        },
        request,
      ),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          materialSegments: [
            { ...response.materialSegments[0], end: response.materialSegments[0].start },
          ],
        },
        request,
      ),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          endpointCollision: { ...response.endpointCollision, penetratingPairCount: 1 },
        },
        request,
      ),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        { ...response, work: { ...response.work, retainedTargetFaces: 2 } },
        request,
      ),
      null,
    )
  })
})
