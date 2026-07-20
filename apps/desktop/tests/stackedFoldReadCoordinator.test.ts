import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  createStackedFoldReadCoordinator,
  type StackedFoldReadAuthority,
  type StackedFoldReadCoordinatorState,
} from '../src/lib/stackedFoldReadCoordinator.ts'
import {
  STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
  STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
  STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
  type StackedFoldReadRequest,
  type StackedFoldReadResponse,
} from '../src/lib/stackedFoldRead.ts'

const INSTANCE = '018f47a2-4b7a-7cc1-8abc-112233445566'
const PROJECT = '018f47a2-4b7a-7cc1-8abc-665544332211'

const request = (revision = 3): StackedFoldReadRequest => ({
  expectedProjectInstanceId: INSTANCE,
  expectedProjectId: PROJECT,
  expectedRevision: revision,
  first: [0, 0, 0],
  second: [1, 0, 0],
  fixedSide: 'left',
  rotationDirection: 'positive',
  requestedAngleDegrees: 180,
})

const response = (revision = 3): StackedFoldReadResponse =>
  ({
    guardModelId: STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
    proposalModelId: STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
    materialMapModelId: STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
    binding: {
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      sourceRevision: revision,
      poseGeneration: 1,
      layerOrderGeneration: 1,
    },
    support: 'no_hinge_single_face',
    crossedCells: [],
    targetFaces: [PROJECT],
    materialSegments: [{
      faceId: PROJECT,
      start: [0, 0],
      end: [1, 0],
      fixedSide: 'left',
      assignment: 'mountain',
    }],
    topologyProof: {
      targetFingerprintSha256: 'a'.repeat(64),
      targetVertexCount: 4,
      targetEdgeCount: 5,
      targetBoundaryVertexCount: 4,
      lineageRecordCount: 2,
      sourceEdgeSubdivisionCount: 4,
      expectedCreaseSubdivisionCount: 1,
      targetMaterialFaceCount: 2,
      targetHingeCount: 1,
    },
    liveGraphHingeAngles: [{
      edge: PROJECT,
      initialAngleDegrees: 0,
    }],
    endpointCollision: {
      expectedPairCount: 0,
      separatedPairCount: 0,
      touchingPairCount: 0,
      allowedPairCount: 0,
      penetratingPairCount: 0,
      indeterminatePairCount: 0,
      hasBlockingHold: false,
    },
    continuousPath: {
      modelId: 'stacked_fold_bounded_path_diagnostic_v1',
      continuousCertificateModelId: null,
      paperThicknessMm: 0,
      sampledPoseCount: 1,
      sampledNonblockingPoseCount: 1,
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
    transactionProposal: {
      transactionToken: null,
      sourceProjectId: PROJECT,
      sourceRevision: revision,
      targetRevision: revision + 1,
      sourceFingerprintSha256: 'b'.repeat(64),
      targetFingerprintSha256: 'a'.repeat(64),
      addedVertexCount: 0,
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
      scannedCells: 0,
      totalBoundaryVertices: 4,
      totalLayerRecords: 1,
      orientationTests: 1,
      exactArithmeticOperations: 1,
      maximumExactIntegerBits: 1,
      totalExactIntegerBits: 1,
      retainedCells: 0,
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
  }) as StackedFoldReadResponse

const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}

test('publishes a detached ready result only while authority remains current', async () => {
  let authority: StackedFoldReadAuthority | null = {
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 3,
  }
  const gate = deferred<StackedFoldReadResponse>()
  const states: StackedFoldReadCoordinatorState[] = []
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gate.promise,
    getAuthority: () => authority,
    onState: (state) => states.push(state),
  })
  const mutable = request()
  const result = coordinator.read(mutable)
  ;(mutable.first as number[])[0] = 99
  gate.resolve(response())
  assert.deepEqual(await result, { status: 'ready', response: response() })
  assert.equal(states[0]?.status, 'idle')
  assert.equal(states[1]?.status, 'reading')
  assert.deepEqual(
    states[1]?.status === 'reading' ? states[1].request.first : null,
    [0, 0, 0],
  )
  assert.equal(coordinator.getState().status, 'ready')
  authority = null
})

test('replacement and invalidation settle old reads without publishing stale completions', async () => {
  const gates = [
    deferred<StackedFoldReadResponse>(),
    deferred<StackedFoldReadResponse>(),
  ]
  let index = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[index++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
  })
  const first = coordinator.read(request())
  const second = coordinator.read(request())
  assert.deepEqual(await first, { status: 'cancelled', reason: 'superseded' })
  gates[0].resolve(response())
  coordinator.invalidate()
  assert.deepEqual(await second, { status: 'cancelled', reason: 'invalidated' })
  gates[1].resolve(response())
  await Promise.resolve()
  assert.equal(coordinator.getState().status, 'idle')
})

test('completion fails closed after authority drift and for forged mutation authority', async () => {
  let revision = 3
  const gates = [
    deferred<StackedFoldReadResponse>(),
    deferred<StackedFoldReadResponse>(),
  ]
  let index = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[index++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision,
    }),
  })
  const stale = coordinator.read(request())
  revision = 4
  gates[0].resolve(response())
  assert.deepEqual(await stale, {
    status: 'cancelled',
    reason: 'stale_authority',
  })

  revision = 3
  const forged = coordinator.read(request())
  gates[1].resolve({
    ...response(),
    authorizesApplyStackedFold: true,
  } as unknown as StackedFoldReadResponse)
  assert.deepEqual(await forged, {
    status: 'failed',
    reason: 'invalid_response',
  })
})

test('reentrant observer replacement owns state and disposal is terminal', async () => {
  const gates = [
    deferred<StackedFoldReadResponse>(),
    deferred<StackedFoldReadResponse>(),
  ]
  let index = 0
  let reentered = false
  let nested: Promise<unknown> | null = null
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[index++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onState(state) {
      if (state.status === 'reading' && !reentered) {
        reentered = true
        nested = coordinator.read(request())
      }
    },
  })
  const outer = coordinator.read(request())
  assert.deepEqual(await outer, { status: 'cancelled', reason: 'superseded' })
  gates[0].resolve(response())
  assert.deepEqual(await nested, { status: 'ready', response: response() })
  coordinator.dispose()
  assert.deepEqual(await coordinator.read(request()), {
    status: 'cancelled',
    reason: 'disposed',
  })
})

test('transport failures are sanitized and stale requests never invoke transport', async () => {
  let calls = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: async () => {
      calls += 1
      throw new Error('secret native detail')
    },
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
  })
  assert.deepEqual(await coordinator.read(request(2)), {
    status: 'cancelled',
    reason: 'stale_authority',
  })
  assert.equal(calls, 0)
  assert.deepEqual(await coordinator.read(request()), {
    status: 'failed',
    reason: 'native_failure',
  })
  assert.equal(calls, 1)
})

test('closed failure vocabulary preserves bounded cycle failure reasons', async () => {
  for (const reason of [
    'cycle_nonclosing',
    'cycle_path_uncertified',
    'cycle_path_unsupported',
    'cycle_path_resource_limit',
    'cycle_path_collision',
  ] as const) {
    const coordinator = createStackedFoldReadCoordinator({
      transport: async () => {
        throw { reason, secret: 'not reflected' }
      },
      getAuthority: () => ({
        projectInstanceId: INSTANCE,
        projectId: PROJECT,
        revision: 3,
      }),
    })
    assert.deepEqual(await coordinator.read(request()), { status: 'failed', reason })
  }
})
