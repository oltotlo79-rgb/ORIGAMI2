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
const ORPHANED_TOKEN = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

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
    certifiedPathGraph: null,
    transactionProposal: {
      applyContractVersion: 1,
      applyMode: 'none',
      transactionToken: null,
      speculativeUnprovenAvailable: false,
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
      failureClasses: [
        'continuous_path_uncertified',
        'target_layer_order_unavailable',
      ],
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

const tokenizedResponse = (revision = 3): StackedFoldReadResponse => {
  const value = response(revision)
  return {
    ...value,
    flatEndpointLayerOrder: {
      applicable: true,
      certified: true,
      materialFaceCount: 2,
      overlapCellCount: 0,
    },
    transactionProposal: {
      ...value.transactionProposal,
      applyMode: 'speculative_unproven',
      transactionToken: ORPHANED_TOKEN,
      speculativeUnprovenAvailable: true,
      failureClasses: ['continuous_path_uncertified'],
    },
  }
}

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
  const transported: StackedFoldReadRequest[] = []
  const coordinator = createStackedFoldReadCoordinator({
    transport: (value) => {
      transported.push(value)
      return gate.promise
    },
    getAuthority: () => authority,
    onState: (state) => states.push(state),
  })
  const mutable = {
    ...request(),
    first: [0, 0, 0] as [number, number, number],
    second: [1, 0, 0] as [number, number, number],
    linearCandidateV1: {
      version: 1 as const,
      entries: [{
        edge: PROJECT,
        initialAngleDegrees: 20,
        requestedAngleDegrees: 40,
      }],
    },
  }
  const result = coordinator.read(mutable)
  mutable.first[0] = 99
  mutable.linearCandidateV1.entries[0]!.requestedAngleDegrees = 80
  assert.equal(
    transported[0]?.linearCandidateV1?.entries[0]?.requestedAngleDegrees,
    40,
  )
  assert.equal(Object.isFrozen(transported[0]), true)
  assert.equal(Object.isFrozen(transported[0]?.linearCandidateV1?.entries[0]), true)
  gate.resolve(response())
  assert.deepEqual(await result, { status: 'ready', response: response() })
  assert.equal(states[0]?.status, 'reading')
  assert.deepEqual(
    states[0]?.status === 'reading' ? states[0].request.first : null,
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

test('reports a strictly normalized late ready response once after invalidation', async () => {
  const gate = deferred<unknown>()
  const orphaned: StackedFoldReadResponse[] = []
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gate.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onOrphanedReadyResponse: (value) => orphaned.push(value),
  })
  const result = coordinator.read(request())
  coordinator.invalidate()
  coordinator.invalidate()
  assert.deepEqual(await result, {
    status: 'cancelled',
    reason: 'invalidated',
  })

  gate.resolve(tokenizedResponse())
  await Promise.resolve()
  assert.deepEqual(orphaned, [tokenizedResponse()])
  assert.equal(orphaned[0]?.transactionProposal.transactionToken, ORPHANED_TOKEN)
})

test('reports a late ready response after disposal without reviving authority', async () => {
  const gate = deferred<unknown>()
  const orphaned: StackedFoldReadResponse[] = []
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gate.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onOrphanedReadyResponse: (value) => orphaned.push(value),
  })
  const result = coordinator.read(request())
  coordinator.dispose()
  assert.deepEqual(await result, { status: 'cancelled', reason: 'disposed' })

  gate.resolve(tokenizedResponse())
  await Promise.resolve()
  assert.deepEqual(orphaned, [tokenizedResponse()])
  assert.equal(coordinator.getState().status, 'idle')
  assert.deepEqual(await coordinator.read(request()), {
    status: 'cancelled',
    reason: 'disposed',
  })
})

test('reports only the superseded late ready response and not the active ready response', async () => {
  const gates = [deferred<unknown>(), deferred<unknown>()]
  const orphaned: StackedFoldReadResponse[] = []
  let index = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[index++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onOrphanedReadyResponse: (value) => orphaned.push(value),
  })
  const superseded = coordinator.read(request())
  const active = coordinator.read(request())
  assert.deepEqual(await superseded, {
    status: 'cancelled',
    reason: 'superseded',
  })

  gates[0]!.resolve(tokenizedResponse())
  await Promise.resolve()
  assert.deepEqual(orphaned, [tokenizedResponse()])
  gates[1]!.resolve(tokenizedResponse())
  assert.deepEqual(await active, {
    status: 'ready',
    response: tokenizedResponse(),
  })
  assert.equal(orphaned.length, 1)
})

test('ignores invalid late responses and isolates orphan callback failures', async () => {
  const gates = [deferred<unknown>(), deferred<unknown>()]
  let index = 0
  let callbackCalls = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[index++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onOrphanedReadyResponse: () => {
      callbackCalls += 1
      throw new Error('cleanup failed')
    },
  })
  const invalidated = coordinator.read(request())
  coordinator.invalidate()
  assert.deepEqual(await invalidated, {
    status: 'cancelled',
    reason: 'invalidated',
  })
  gates[0]!.resolve({ ...tokenizedResponse(), unexpected: true })
  await Promise.resolve()
  assert.equal(callbackCalls, 0)

  const disposed = coordinator.read(request())
  coordinator.dispose()
  assert.deepEqual(await disposed, { status: 'cancelled', reason: 'disposed' })
  gates[1]!.resolve(tokenizedResponse())
  await Promise.resolve()
  assert.equal(callbackCalls, 1)
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

test('a hostile authority callback cannot start transport or escape the coordinator', async () => {
  let calls = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: async () => {
      calls += 1
      return response()
    },
    getAuthority: () => {
      throw new Error('hostile authority callback')
    },
  })
  assert.deepEqual(await coordinator.read(request()), {
    status: 'cancelled',
    reason: 'stale_authority',
  })
  assert.equal(calls, 0)
})

test('authority and request accessors are rejected without invoking getters', async () => {
  let transportCalls = 0
  let requestGetterCalls = 0
  const requestWithAccessor = Object.defineProperty(
    { ...request() },
    'first',
    {
      enumerable: true,
      get() {
        requestGetterCalls += 1
        return [0, 0, 0]
      },
    },
  ) as StackedFoldReadRequest
  const requestCoordinator = createStackedFoldReadCoordinator({
    transport: async () => {
      transportCalls += 1
      return response()
    },
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
  })
  assert.deepEqual(await requestCoordinator.read(requestWithAccessor), {
    status: 'failed',
    reason: 'invalid_response',
  })
  assert.equal(requestGetterCalls, 0)
  assert.equal(transportCalls, 0)

  let authorityGetterCalls = 0
  const authorityCoordinator = createStackedFoldReadCoordinator({
    transport: async () => {
      transportCalls += 1
      return response()
    },
    getAuthority: () => Object.defineProperty({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
    }, 'revision', {
      enumerable: true,
      get() {
        authorityGetterCalls += 1
        return 3
      },
    }) as StackedFoldReadAuthority,
  })
  assert.deepEqual(await authorityCoordinator.read(request()), {
    status: 'cancelled',
    reason: 'stale_authority',
  })
  assert.equal(authorityGetterCalls, 0)
  assert.equal(transportCalls, 0)
})

test('replacement never exposes a reentrant transient idle ownership gap', async () => {
  const gates = [
    deferred<StackedFoldReadResponse>(),
    deferred<StackedFoldReadResponse>(),
  ]
  let transportCalls = 0
  let idleCalls = 0
  let nested: Promise<unknown> | null = null
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gates[transportCalls++]!.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onState(state) {
      if (state.status === 'idle') {
        idleCalls += 1
        nested = coordinator.read(request())
      }
    },
  })
  const first = coordinator.read(request())
  const second = coordinator.read(request())
  assert.deepEqual(await first, {
    status: 'cancelled',
    reason: 'superseded',
  })
  assert.equal(idleCalls, 0)
  assert.equal(nested, null)
  assert.equal(transportCalls, 2)
  gates[1]!.resolve(response())
  assert.deepEqual(await second, { status: 'ready', response: response() })
  gates[0]!.resolve(response())
})

test('authority drift sanitizes native rejection instead of publishing stale failure', async () => {
  let revision = 3
  const gate = deferred<unknown>()
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => gate.promise,
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision,
    }),
  })
  const result = coordinator.read(request())
  revision = 4
  gate.reject({ reason: 'cycle_path_collision' })
  assert.deepEqual(await result, {
    status: 'cancelled',
    reason: 'stale_authority',
  })
  assert.equal(coordinator.getState().status, 'idle')
})

test('dispose is terminal before observers can reenter', async () => {
  const gate = deferred<StackedFoldReadResponse>()
  let transportCalls = 0
  let idleCalls = 0
  const coordinator = createStackedFoldReadCoordinator({
    transport: () => {
      transportCalls += 1
      return gate.promise
    },
    getAuthority: () => ({
      projectInstanceId: INSTANCE,
      projectId: PROJECT,
      revision: 3,
    }),
    onState(state) {
      if (state.status === 'idle') {
        idleCalls += 1
        void coordinator.read(request())
      }
    },
  })
  const result = coordinator.read(request())
  coordinator.dispose()
  assert.deepEqual(await result, { status: 'cancelled', reason: 'disposed' })
  assert.equal(idleCalls, 0)
  assert.equal(transportCalls, 1)
  assert.deepEqual(await coordinator.read(request()), {
    status: 'cancelled',
    reason: 'disposed',
  })
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

test('native failure classification reads only one own data reason field', async () => {
  const readFailure = async (error: unknown) => {
    const coordinator = createStackedFoldReadCoordinator({
      transport: async () => {
        throw error
      },
      getAuthority: () => ({
        projectInstanceId: INSTANCE,
        projectId: PROJECT,
        revision: 3,
      }),
    })
    return coordinator.read(request())
  }

  let getterCalls = 0
  const accessor = Object.defineProperty({}, 'reason', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('secret native detail')
    },
  })
  assert.deepEqual(await readFailure(accessor), {
    status: 'failed',
    reason: 'native_failure',
  })
  assert.equal(getterCalls, 0)
  assert.deepEqual(await readFailure(Object.create({
    reason: 'cycle_path_collision',
  })), {
    status: 'failed',
    reason: 'native_failure',
  })

  let proxyGetCalls = 0
  const proxied = new Proxy({ reason: 'cycle_path_cancelled' }, {
    get() {
      proxyGetCalls += 1
      throw new Error('secret native detail')
    },
  })
  assert.deepEqual(await readFailure(proxied), {
    status: 'failed',
    reason: 'cycle_path_cancelled',
  })
  assert.equal(proxyGetCalls, 0)

  const revocable = Proxy.revocable({ reason: 'cycle_nonclosing' }, {})
  revocable.revoke()
  assert.deepEqual(await readFailure(revocable.proxy), {
    status: 'failed',
    reason: 'native_failure',
  })
})
