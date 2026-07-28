import type { ProjectSnapshot } from '../src/lib/coreClient.ts'

export const SPECULATIVE_INSTANCE_ID =
  '018f47a2-4b7a-7cc1-8abc-112233445566'
export const SPECULATIVE_PROJECT_ID =
  '018f47a2-4b7a-7cc1-8abc-665544332211'
export const SPECULATIVE_TOKEN =
  '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

export const SPECULATIVE_EXPECTED_READ = Object.freeze({
  expectedProjectInstanceId: SPECULATIVE_INSTANCE_ID,
  expectedProjectId: SPECULATIVE_PROJECT_ID,
  expectedRevision: 3,
  requestedAngleDegrees: 180,
})

export function makeSpeculativeSnapshot(
  speculativeUnprovenFolds?: unknown,
): ProjectSnapshot {
  return {
    project_instance_id: SPECULATIVE_INSTANCE_ID,
    project_id: SPECULATIVE_PROJECT_ID,
    revision: 3,
    ...(speculativeUnprovenFolds === undefined
      ? {}
      : { speculativeUnprovenFolds }),
  } as ProjectSnapshot
}

export function makeSpeculativeStackedFoldResponse(): any {
  return {
    guardModelId: 'native_flat_stacked_fold_read_guard_v1',
    proposalModelId: 'native_linear_stacked_fold_read_proposal_v1',
    materialMapModelId: 'native_flat_stacked_fold_material_map_v1',
    binding: {
      projectInstanceId: SPECULATIVE_INSTANCE_ID,
      projectId: SPECULATIVE_PROJECT_ID,
      sourceRevision: 3,
      poseGeneration: 1,
      layerOrderGeneration: 1,
    },
    support: 'bit_exact_flat_endpoint_tree',
    crossedCells: [{
      cellKeySha256: 'c'.repeat(64),
      bottomToTopFaces: [SPECULATIVE_PROJECT_ID],
      boundaryWorld: [
        [0, 0, 0],
        [20, 0, 0],
        [20, 0, -10],
        [0, 0, -10],
      ],
    }],
    targetFaces: [SPECULATIVE_PROJECT_ID],
    materialSegments: [{
      faceId: SPECULATIVE_PROJECT_ID,
      start: [1, 2],
      end: [3, 4],
      fixedSide: 'left',
      assignment: 'mountain',
    }],
    topologyProof: {
      targetFingerprintSha256: 'a'.repeat(64),
      targetVertexCount: 5,
      targetEdgeCount: 6,
      targetBoundaryVertexCount: 4,
      lineageRecordCount: 2,
      sourceEdgeSubdivisionCount: 1,
      expectedCreaseSubdivisionCount: 1,
      targetMaterialFaceCount: 3,
      targetHingeCount: 2,
    },
    liveGraphHingeAngles: [
      { edge: SPECULATIVE_INSTANCE_ID, initialAngleDegrees: 0 },
      { edge: SPECULATIVE_PROJECT_ID, initialAngleDegrees: 0 },
    ],
    endpointCollision: {
      expectedPairCount: 3,
      separatedPairCount: 0,
      touchingPairCount: 0,
      allowedPairCount: 3,
      penetratingPairCount: 0,
      indeterminatePairCount: 0,
      hasBlockingHold: false,
    },
    continuousPath: {
      modelId: 'stacked_fold_bounded_path_diagnostic_v1',
      continuousCertificateModelId: null,
      paperThicknessMm: 0.1,
      sampledPoseCount: 2,
      sampledNonblockingPoseCount: 2,
      intervalLeafCount: 8,
      intervalPairWork: 8,
      intervalCandidateLimit: 2048,
      positiveEndpointCandidateCount: 64,
      positiveEndpointExactPairCalls: 0,
      positiveEndpointCandidateLimit: 120,
      closureRequired: false,
      closureLeafCount: 0,
      closurePairWork: 0,
      firstClosureFailureAngleDegrees: null,
      firstSampledBlockingAngleDegrees: null,
      requestedAngleDegrees: 180,
      continuousClearanceCertified: false,
      safeStopAngleDegrees: 180,
      authorizesProjectMutation: false,
    },
    certifiedPathGraph: null,
    flatEndpointLayerOrder: {
      applicable: true,
      certified: true,
      materialFaceCount: 3,
      overlapCellCount: 1,
    },
    transactionProposal: {
      applyContractVersion: 1,
      applyMode: 'speculative_unproven',
      transactionToken: SPECULATIVE_TOKEN,
      speculativeUnprovenAvailable: true,
      sourceProjectId: SPECULATIVE_PROJECT_ID,
      sourceRevision: 3,
      targetRevision: 4,
      sourceFingerprintSha256: 'b'.repeat(64),
      targetFingerprintSha256: 'a'.repeat(64),
      readyForAtomicApply: false,
      failureClasses: ['continuous_path_uncertified'],
      authorizesProjectMutation: false,
      addedVertexCount: 1,
      addedEdgeCount: 2,
      mountainCreaseCount: 1,
      valleyCreaseCount: 0,
      timelineStepCount: 1,
      timelineCompleteHingeAngleCount: 2,
      requestedAngleDegrees: 180,
    },
    work: {
      scannedCells: 0,
      totalBoundaryVertices: 4,
      totalLayerRecords: 2,
      orientationTests: 1,
      exactArithmeticOperations: 1,
      maximumExactIntegerBits: 64,
      totalExactIntegerBits: 64,
      retainedCells: 1,
      retainedTargetFaces: 1,
    },
    authorizesProjectMutation: false,
    authorizesApplyStackedFold: false,
  }
}

export function makeCertifiedStackedFoldResponse(): any {
  const response = makeSpeculativeStackedFoldResponse()
  response.continuousPath.continuousCertificateModelId =
    'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v2'
  response.continuousPath.continuousClearanceCertified = true
  response.transactionProposal.applyMode = 'certified'
  response.transactionProposal.speculativeUnprovenAvailable = false
  response.transactionProposal.readyForAtomicApply = true
  response.transactionProposal.failureClasses = []
  response.transactionProposal.authorizesProjectMutation = true
  return response
}

export const EMPTY_UNPROVEN_COUNTS = Object.freeze({
  awaitingProof: 0,
  proofBlocked: 0,
  unknownEvidenceInsufficient: 0,
  unknownResourceLimit: 0,
  unknownCancelled: 0,
  unknownDeadlineReached: 0,
})
