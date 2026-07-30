import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
  STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
  STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
  isStackedFoldReadRequest,
  normalizeStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
  normalizeLiveHingeRegistryV1,
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
  it('admits only a canonical stale-bound live hinge registry', () => {
    const expected = {
      expectedProjectInstanceId: projectInstanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      first: [0, 0, 0] as const,
      second: [1, 0, 0] as const,
      fixedSide: 'left' as const,
      rotationDirection: 'positive' as const,
      requestedAngleDegrees: 90,
    }
    const registry = {
      version: 1,
      projectInstanceId,
      projectId,
      revision: 3,
      poseGeneration: 7,
      graphFingerprintSha256: 'a'.repeat(64),
      entries: [{ edge: faceId, initialAngleDegrees: 20 }],
      authorizesProjectMutation: false,
    } as const
    const normalized = normalizeLiveHingeRegistryV1(registry, expected)
    assert.deepEqual(normalized, registry)
    assert.notEqual(normalized, registry)
    assert.equal(Object.isFrozen(normalized), true)
    assert.equal(Object.isFrozen(normalized?.entries), true)
    assert.equal(normalizeLiveHingeRegistryV1({ ...registry, revision: 4 }, expected), null)
    assert.equal(normalizeLiveHingeRegistryV1({
      ...registry,
      entries: Array.from({ length: 65 }, () => registry.entries[0]),
    }, expected), null)
    assert.equal(normalizeLiveHingeRegistryV1({
      ...registry,
      entries: [registry.entries[0], registry.entries[0]],
    }, expected), null)
    assert.equal(normalizeLiveHingeRegistryV1({
      ...registry,
      authorizesProjectMutation: true,
    }, expected), null)
    let getterCalls = 0
    const accessor = Object.defineProperty({ ...registry }, 'entries', {
      enumerable: true,
      get() {
        getterCalls += 1
        return registry.entries
      },
    })
    assert.equal(normalizeLiveHingeRegistryV1(accessor, expected), null)
    assert.equal(getterCalls, 0)
  })

  it('admits only finite, non-degenerate, closed-enum requests', () => {
    assert.equal(isStackedFoldReadRequest(request), true)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      progressRequestId: 'stacked-fold:018f47a2-4b7a-7cc1-8abc-aabbccddeeff',
    }), true)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      progressRequestId: 'stacked fold contains spaces',
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      progressRequestId: 'x'.repeat(129),
    }), false)
    assert.equal(isStackedFoldReadRequest({ ...request, second: [0, 0, 0] }), false)
    assert.equal(isStackedFoldReadRequest({ ...request, requestedAngleDegrees: Number.NaN }), false)
    assert.equal(isStackedFoldReadRequest({ ...request, fixedSide: 'center' }), false)
    const schedule = {
      version: 1,
      entries: [{
        edge: faceId,
        uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }],
        numeratorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        requestedAngleDegrees: 90,
      }],
    } as const
    assert.equal(isStackedFoldReadRequest({ ...request, cycleScheduleV1: schedule }), true)
    const linearCandidateV1 = {
      version: 1,
      entries: [{
        edge: faceId,
        initialAngleDegrees: 20,
        requestedAngleDegrees: 40,
      }],
    } as const
    assert.equal(isStackedFoldReadRequest({ ...request, linearCandidateV1 }), true)
    const linearEntry = linearCandidateV1.entries[0]
    assert.equal(isStackedFoldReadRequest({
      ...request,
      linearCandidateV1: {
        version: 1,
        entries: Array.from({ length: 64 }, () => linearEntry),
      },
    }), true)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      linearCandidateV1: {
        version: 1,
        entries: Array.from({ length: 65 }, () => linearEntry),
      },
    }), false)
    const graph = {
      version: 1,
      states: [
        { entries: [{ edge: faceId, angleDegrees: 20 }] },
        { entries: [{ edge: faceId, angleDegrees: 40 }] },
      ],
      transitions: [{ sourceState: 0, targetState: 1 }],
      sourceState: 0,
      targetState: 1,
    } as const
    assert.equal(isStackedFoldReadRequest({
      ...request,
      certifiedPathGraphV1: graph,
    }), true)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      linearCandidateV1,
      certifiedPathGraphV1: graph,
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      certifiedPathGraphV1: {
        ...graph,
        states: Array.from({ length: 33 }, () => graph.states[0]),
      },
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      certifiedPathGraphV1: {
        ...graph,
        transitions: [
          { sourceState: 0, targetState: 1 },
          { sourceState: 0, targetState: 1 },
        ],
      },
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      cycleScheduleV1: schedule,
      linearCandidateV1,
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      cycleScheduleV1: { ...schedule, version: 2 },
    }), false)
    assert.equal(isStackedFoldReadRequest({
      ...request,
      cycleScheduleV1: {
        ...schedule,
        entries: [{
          ...schedule.entries[0],
          denominatorPowerCoefficients: [{ numerator: 1, denominator: 0 }],
        }],
      },
    }), false)
  })

  it('deeply detaches canonical requests and rejects accessor-backed input', () => {
    const mutable = {
      ...request,
      first: [...request.first],
      second: [...request.second],
      linearCandidateV1: {
        version: 1,
        entries: [{
          edge: faceId,
          initialAngleDegrees: 20,
          requestedAngleDegrees: 40,
        }],
      },
    }
    const detached = normalizeStackedFoldReadRequest(mutable)
    assert.ok(detached)
    mutable.first[0] = 99
    mutable.linearCandidateV1.entries[0]!.requestedAngleDegrees = 80
    assert.deepEqual(detached.first, [0, 0, 0])
    assert.equal(
      detached.linearCandidateV1?.entries[0]?.requestedAngleDegrees,
      40,
    )
    assert.equal(Object.isFrozen(detached), true)
    assert.equal(Object.isFrozen(detached.first), true)
    assert.equal(Object.isFrozen(detached.linearCandidateV1?.entries), true)
    assert.equal(Object.isFrozen(detached.linearCandidateV1?.entries[0]), true)

    let getterCalls = 0
    const accessor = Object.defineProperty({ ...request }, 'first', {
      enumerable: true,
      get() {
        getterCalls += 1
        return [0, 0, 0]
      },
    })
    assert.equal(normalizeStackedFoldReadRequest(accessor), null)
    assert.equal(getterCalls, 0)
    assert.equal(
      normalizeStackedFoldReadRequest(new Proxy({ ...request }, {
        ownKeys() {
          throw new Error('hostile request proxy')
        },
      })),
      null,
    )
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
      crossedCells: [{
        cellKeySha256: 'c'.repeat(64),
        bottomToTopFaces: [faceId],
        boundaryWorld: [[0, 0, 0], [1, 0, 0], [0, 0, -1]],
      }],
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
      liveGraphHingeAngles: [{
        edge: faceId,
        initialAngleDegrees: 0,
      }],
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
        paperThicknessMm: 0,
        sampledPoseCount: 2,
        sampledNonblockingPoseCount: 2,
        intervalLeafCount: 8,
        intervalPairWork: 8,
        intervalCandidateLimit: 2048,
        positiveEndpointCandidateCount: 0,
        positiveEndpointExactPairCalls: 0,
        positiveEndpointCandidateLimit: 120,
        closureRequired: false,
        closureLeafCount: 0,
        closurePairWork: 0,
        firstClosureFailureAngleDegrees: null,
        firstSampledBlockingAngleDegrees: null,
        requestedAngleDegrees: 180,
        continuousClearanceCertified: false,
        safeStopAngleDegrees: 0,
        authorizesProjectMutation: false,
      },
      certifiedPathGraph: null,
      transactionProposal: {
        applyContractVersion: 1,
        applyMode: 'none',
        transactionToken: null,
        speculativeUnprovenAvailable: false,
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
    const graphResponse = {
      ...response,
      certifiedPathGraph: {
        modelId: 'bounded_certified_pose_graph_path_v1',
        version: 1,
        sourceFingerprintSha256: '1'.repeat(64),
        targetFingerprintSha256: '2'.repeat(64),
        exploredStateCount: 2,
        evaluatedTransitionCount: 1,
        edges: [{
          sourceFingerprintSha256: '1'.repeat(64),
          targetFingerprintSha256: '2'.repeat(64),
          scheduleCertificateSha256: '3'.repeat(64),
          collisionCertificateSha256: '4'.repeat(64),
          closureCertificateSha256: '5'.repeat(64),
          hinges: [faceId],
        }],
        authorizesProjectMutation: false,
      },
    } as const
    assert.deepEqual(
      normalizeStackedFoldReadResponse(graphResponse, request),
      graphResponse,
    )
    const graphReady = {
      ...graphResponse,
      continuousPath: {
        ...graphResponse.continuousPath,
        continuousCertificateModelId:
          'stacked_fold_cycle_interval_zero_thickness_continuous_certificate_v1',
        continuousClearanceCertified: true,
      },
      transactionProposal: {
        ...graphResponse.transactionProposal,
        applyMode: 'certified',
        transactionToken: faceId,
        timelineStepCount: graphResponse.certifiedPathGraph.edges.length,
        readyForAtomicApply: true,
        failureClasses: [],
        authorizesProjectMutation: true,
      },
    } as const
    assert.deepEqual(
      normalizeStackedFoldReadResponse(graphReady, request),
      graphReady,
    )
    assert.equal(normalizeStackedFoldReadResponse({
      ...graphResponse,
      certifiedPathGraph: {
        ...graphResponse.certifiedPathGraph,
        edges: [{
          ...graphResponse.certifiedPathGraph.edges[0],
          closureCertificateSha256: 'private-path',
        }],
      },
    }, request), null)
    const ready = {
      ...response,
      continuousPath: {
        ...response.continuousPath,
        continuousCertificateModelId:
          'stacked_fold_single_hinge_zero_thickness_continuous_certificate_v1',
        continuousClearanceCertified: true,
      },
      transactionProposal: {
        ...response.transactionProposal,
        applyMode: 'certified',
        transactionToken: faceId,
        readyForAtomicApply: true,
        failureClasses: [],
        authorizesProjectMutation: true,
      },
    }
    assert.deepEqual(normalizeStackedFoldReadResponse(ready, request), ready)
    for (const modelId of [
      'stacked_fold_single_hinge_positive_thickness_continuous_certificate_v2',
      'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v2',
    ] as const) {
      const positiveReady = {
        ...ready,
        continuousPath: {
          ...ready.continuousPath,
          continuousCertificateModelId: modelId,
          paperThicknessMm: 0.1,
        },
      }
      assert.deepEqual(
        normalizeStackedFoldReadResponse(positiveReady, request),
        positiveReady,
      )
    }
    for (const retiredModelId of [
      'stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1',
      'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1',
    ] as const) {
      assert.equal(normalizeStackedFoldReadResponse({
        ...ready,
        continuousPath: {
          ...ready.continuousPath,
          continuousCertificateModelId: retiredModelId,
          paperThicknessMm: 0.1,
        },
      }, request), null)
    }
    assert.equal(
      normalizeStackedFoldReadResponse({
        ...ready,
        transactionProposal: { ...ready.transactionProposal, transactionToken: null },
      }, request),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse({
        ...ready,
        continuousPath: {
          ...ready.continuousPath,
          continuousCertificateModelId: 'forged_collective_certificate',
        },
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
      crossedCells: [{
        cellKeySha256: 'd'.repeat(64),
        bottomToTopFaces: [faceId],
        boundaryWorld: [[0, 0, 0], [1, 0, 0], [0, 0, -1]],
      }],
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
      liveGraphHingeAngles: [{
        edge: faceId,
        initialAngleDegrees: 0,
      }],
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
    const repeatedCells = Array.from({ length: 2_049 }, (_, index) => ({
      ...response.crossedCells[0],
      cellKeySha256: index.toString(16).padStart(64, '0'),
    }))
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          crossedCells: repeatedCells,
          work: { ...response.work, retainedCells: repeatedCells.length },
        },
        request,
      ),
      null,
      'the renderer must not accept an IPC-valid but unbounded cell list',
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          targetFaces: [faceId, faceId],
          materialSegments: [response.materialSegments[0], response.materialSegments[0]],
          work: { ...response.work, retainedTargetFaces: 2 },
        },
        request,
      ),
      null,
      'duplicate target faces would produce ambiguous viewer and list keys',
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          crossedCells: [response.crossedCells[0], response.crossedCells[0]],
          work: { ...response.work, retainedCells: 2 },
        },
        request,
      ),
      null,
      'duplicate cell keys would make React reuse a hostile cell tab',
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
    assert.equal(
      normalizeStackedFoldReadResponse({
        ...response,
        liveGraphHingeAngles: [
          response.liveGraphHingeAngles[0],
          response.liveGraphHingeAngles[0],
        ],
      }, request),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse({
        ...response,
        liveGraphHingeAngles: [{
          ...response.liveGraphHingeAngles[0],
          authority: true,
        }],
      }, request),
      null,
    )
    assert.equal(
      normalizeStackedFoldReadResponse(
        {
          ...response,
          continuousPath: {
            ...response.continuousPath,
            positiveEndpointCandidateCount: 121,
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
          continuousPath: {
            ...response.continuousPath,
            positiveEndpointCandidateCount: 1,
            positiveEndpointExactPairCalls: 2,
          },
        },
        request,
      ),
      null,
    )
  })
})
