import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createNativeStaticCollisionInspectionCoordinator,
  createNativeStaticCollisionNativeTransport,
  inspectAppliedPoseStaticCollision,
  nativeStaticCollisionPoseKey,
  NativeStaticCollisionCoordinatorError,
  NativeStaticCollisionNativeError,
} from '../src/lib/nativeStaticCollisionNative.ts'

const INSTANCE_ID = '00000000-0000-4000-8000-000000000001'
const PROJECT_ID = '00000000-0000-4000-8000-000000000002'
const FACE_A = '00000000-0000-4000-8000-000000000003'
const FACE_B = '00000000-0000-4000-8000-000000000004'
const FACE_C = '00000000-0000-4000-8000-000000000007'
const EDGE_A = '00000000-0000-4000-8000-000000000005'
const EDGE_B = '00000000-0000-4000-8000-000000000006'
const BINDING = {
  projectInstanceId: INSTANCE_ID,
  projectId: PROJECT_ID,
  revision: 7,
  poseGeneration: '11',
}
const POSE = {
  projectInstanceId: INSTANCE_ID,
  projectId: PROJECT_ID,
  revision: 7,
  fixedFaceId: FACE_A,
  completeHingeAngles: [
    { edgeId: EDGE_B, angleDegrees: 135 },
    { edgeId: EDGE_A, angleDegrees: -0 },
  ],
}
const EMPTY_PAIR_SNAPSHOT = {
  pairClassificationCounts: {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 0,
    indeterminate: 0,
    candidateExcluded: 0,
  },
  pairDiagnostics: [],
} as const
const NO_PAIR_SNAPSHOT = {
  pairClassificationCounts: null,
  pairDiagnostics: null,
} as const
const PENETRATING_PAIR_SNAPSHOT = {
  pairClassificationCounts: {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 1,
    indeterminate: 2,
    candidateExcluded: 0,
  },
  pairDiagnostics: [
    {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
      topology: 'no_shared_feature',
      evidence: 'transversal_crossing',
      policyDecision: 'penetrating',
      disposition: 'penetrating',
      strictTransversalDualGateProven: true,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: false,
    },
    {
      firstFaceId: FACE_A,
      secondFaceId: FACE_C,
      topology: 'shared_hinge_edge',
      evidence: 'boundary_line_contact',
      policyDecision: 'indeterminate',
      disposition: 'indeterminate',
      strictTransversalDualGateProven: false,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: false,
    },
    {
      firstFaceId: FACE_B,
      secondFaceId: FACE_C,
      topology: 'shared_hinge_edge',
      evidence: 'boundary_line_contact',
      policyDecision: 'indeterminate',
      disposition: 'indeterminate',
      strictTransversalDualGateProven: false,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: false,
    },
  ],
} as const
const INDETERMINATE_PAIR_SNAPSHOT = {
  pairClassificationCounts: {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 0,
    indeterminate: 1,
    candidateExcluded: 0,
  },
  pairDiagnostics: [
    {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
      topology: 'shared_hinge_edge',
      evidence: 'indeterminate',
      policyDecision: 'indeterminate',
      disposition: 'indeterminate',
      strictTransversalDualGateProven: false,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: true,
    },
  ],
} as const

test('apply uses the exact nested command contract and canonical hinge order', async () => {
  const calls: Array<readonly [string, Readonly<Record<string, unknown>> | undefined]> = []
  const transport = createNativeStaticCollisionNativeTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return { binding: BINDING }
  })

  assert.deepEqual(await transport.applyPose(POSE), BINDING)
  assert.deepEqual(calls, [[
    'apply_current_native_pose',
    {
      request: {
        expectedProjectInstanceId: INSTANCE_ID,
        expectedProjectId: PROJECT_ID,
        expectedRevision: 7,
        fixedFaceId: FACE_A,
        completeHingeAngles: [
          { edgeId: EDGE_A, angleDegrees: 0 },
          { edgeId: EDGE_B, angleDegrees: 135 },
        ],
      },
    },
  ]])
})

test('pose keys are canonical, complete, and fail closed', () => {
  const reordered = {
    ...POSE,
    completeHingeAngles: [...POSE.completeHingeAngles].reverse(),
  }
  assert.equal(nativeStaticCollisionPoseKey(POSE), nativeStaticCollisionPoseKey(reordered))
  assert.notEqual(
    nativeStaticCollisionPoseKey(POSE),
    nativeStaticCollisionPoseKey({
      ...POSE,
      completeHingeAngles: [
        { edgeId: EDGE_B, angleDegrees: 134 },
        { edgeId: EDGE_A, angleDegrees: 0 },
      ],
    }),
  )
  assert.equal(nativeStaticCollisionPoseKey({ ...POSE, projectId: 'invalid' }), null)
  assert.equal(
    nativeStaticCollisionPoseKey({
      projectInstanceId: '00000000-0000-0000-7000-000000000001',
      projectId: 'ffffffff-ffff-ffff-ffff-ffffffffffff',
      revision: 7,
      fixedFaceId: '00000000-0000-0000-0000-000000000003',
      completeHingeAngles: [
        {
          edgeId: '00000000-0000-0000-7000-000000000005',
          angleDegrees: 135,
        },
      ],
    }) === null,
    false,
  )
  assert.equal(
    nativeStaticCollisionPoseKey({
      ...POSE,
      projectId: '00000000-0000-0000-0000-000000000000',
    }),
    null,
  )
})

test('inspection accepts canonical non-nil UUIDs with arbitrary version and variant bits', async () => {
  const binding = {
    projectInstanceId: '00000000-0000-0000-7000-000000000001',
    projectId: 'ffffffff-ffff-ffff-ffff-ffffffffffff',
    revision: 7,
    poseGeneration: '11',
  }
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding,
    status: 'certified_nonblocking',
    reason: null,
    expectedUnorderedFacePairs: 0,
    provenPenetratingPairs: 0,
    firstProvenPenetratingPair: null,
    ...EMPTY_PAIR_SNAPSHOT,
  }))

  assert.deepEqual((await transport.inspect()).binding, binding)
})

test('inspection accepts only a relationally valid certified response', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'certified_nonblocking',
    reason: null,
    expectedUnorderedFacePairs: 0,
    provenPenetratingPairs: 0,
    firstProvenPenetratingPair: null,
    ...EMPTY_PAIR_SNAPSHOT,
  }))

  const result = await transport.inspect()
  assert.deepEqual(result.binding, BINDING)
  assert.equal(result.diagnostic.status, 'certified_nonblocking')
})

test('inspection preserves a canonical proven penetrating pair without raw geometry', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_zero_thickness_penetration',
    expectedUnorderedFacePairs: 3,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    ...PENETRATING_PAIR_SNAPSHOT,
  }))

  const result = await transport.inspect()
  assert.equal(result.diagnostic.reason, 'proven_zero_thickness_penetration')
  assert.deepEqual(result.diagnostic.firstProvenPenetratingPair, {
    firstFaceId: FACE_A,
    secondFaceId: FACE_B,
  })
})

test('inspection accepts a strictly bound positive-thickness penetration reason', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_positive_thickness_penetration',
    expectedUnorderedFacePairs: 3,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    ...PENETRATING_PAIR_SNAPSHOT,
  }))

  const result = await transport.inspect()
  assert.deepEqual(result.binding, BINDING)
  assert.equal(
    result.diagnostic.reason,
    'proven_positive_thickness_penetration',
  )
  assert.equal(result.diagnostic.expectedUnorderedFacePairs, 3)
  assert.equal(result.diagnostic.provenPenetratingPairs, 1)
  assert.deepEqual(result.diagnostic.firstProvenPenetratingPair, {
    firstFaceId: FACE_A,
    secondFaceId: FACE_B,
  })
})

test('positive-volume shared-hinge proof uses the general material-penetration reason', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_positive_thickness_penetration',
    expectedUnorderedFacePairs: 1,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    pairClassificationCounts: {
      separated: 0,
      touching: 0,
      allowed: 0,
      penetrating: 1,
      indeterminate: 0,
      candidateExcluded: 0,
    },
    pairDiagnostics: [{
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
      topology: 'shared_hinge_edge',
      evidence: 'positive_volume_overlap',
      policyDecision: 'penetrating',
      disposition: 'penetrating',
      strictTransversalDualGateProven: false,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: true,
    }],
  }))

  const diagnostic = (await transport.inspect()).diagnostic
  assert.equal(
    diagnostic.reason,
    'proven_positive_thickness_penetration',
  )
  assert.equal(diagnostic.pairDiagnostics?.[0]?.evidence, 'positive_volume_overlap')
  assert.equal(
    diagnostic.pairDiagnostics?.[0]?.sharedHingeSolidClassified,
    true,
  )
})

test('positive-thickness penetration rejects every malformed relational contract', async () => {
  const valid = {
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_positive_thickness_penetration',
    expectedUnorderedFacePairs: 3,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    ...PENETRATING_PAIR_SNAPSHOT,
  }
  const malformed = [
    { ...valid, binding: null },
    { ...valid, status: 'certified_nonblocking' },
    { ...valid, expectedUnorderedFacePairs: null },
    { ...valid, expectedUnorderedFacePairs: 0 },
    { ...valid, provenPenetratingPairs: null },
    { ...valid, provenPenetratingPairs: 0 },
    { ...valid, provenPenetratingPairs: 4 },
    { ...valid, firstProvenPenetratingPair: null },
    {
      ...valid,
      firstProvenPenetratingPair: {
        firstFaceId: FACE_B,
        secondFaceId: FACE_A,
      },
    },
    {
      ...valid,
      firstProvenPenetratingPair: {
        firstFaceId: FACE_A,
        secondFaceId: FACE_A,
      },
    },
    {
      ...valid,
      firstProvenPenetratingPair: {
        firstFaceId: 'not-a-face-id',
        secondFaceId: FACE_B,
      },
    },
    { ...valid, reason: 'proven_positive_volume_overlap' },
    { ...valid, rawGeometry: 'private' },
  ]

  for (const value of malformed) {
    const transport = createNativeStaticCollisionNativeTransport(() => value)
    await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
  }
})

test('inspection preserves every canonical pair classification and proof marker', async () => {
  const transport = createNativeStaticCollisionNativeTransport(
    () => penetratingWireResponse(),
  )

  const diagnostic = (await transport.inspect()).diagnostic
  assert.deepEqual(
    diagnostic.pairClassificationCounts,
    PENETRATING_PAIR_SNAPSHOT.pairClassificationCounts,
  )
  assert.deepEqual(
    diagnostic.pairDiagnostics,
    PENETRATING_PAIR_SNAPSHOT.pairDiagnostics,
  )
  assert.equal(Object.isFrozen(diagnostic.pairClassificationCounts), true)
  assert.equal(Object.isFrozen(diagnostic.pairDiagnostics), true)
  assert.equal(
    Object.isFrozen(diagnostic.pairDiagnostics?.[0]),
    true,
  )
})

test('independent exact proof rejects unnormalized raw evidence', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_zero_thickness_penetration',
    expectedUnorderedFacePairs: 1,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    pairClassificationCounts: {
      separated: 0,
      touching: 0,
      allowed: 0,
      penetrating: 1,
      indeterminate: 0,
      candidateExcluded: 0,
    },
    pairDiagnostics: [{
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
      topology: 'no_shared_feature',
      evidence: 'indeterminate',
      policyDecision: 'indeterminate',
      disposition: 'penetrating',
      strictTransversalDualGateProven: true,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: false,
      sharedHingeSolidClassified: false,
    }],
  }))

  await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
})

test('watertight zero-thickness shared-hinge boundary proof is accepted', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'evidence_unavailable',
    expectedUnorderedFacePairs: 1,
    provenPenetratingPairs: null,
    firstProvenPenetratingPair: null,
    pairClassificationCounts: {
      separated: 0,
      touching: 0,
      allowed: 1,
      penetrating: 0,
      indeterminate: 0,
      candidateExcluded: 0,
    },
    pairDiagnostics: [{
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
      topology: 'shared_hinge_edge',
      evidence: 'shared_feature_contact',
      policyDecision: 'requires_hinge_model',
      disposition: 'allowed',
      strictTransversalDualGateProven: false,
      wholeFaceOverlapProven: false,
      sharedHingeBoundaryContactProven: true,
      sharedHingeSolidClassified: false,
    }],
  }))

  const diagnostic = (await transport.inspect()).diagnostic
  assert.equal(
    diagnostic.pairDiagnostics?.[0]?.sharedHingeBoundaryContactProven,
    true,
  )
  assert.equal(diagnostic.pairDiagnostics?.[0]?.disposition, 'allowed')
})

test('pair snapshot rejects unknown keys, inconsistent counts, and noncanonical coverage', async () => {
  const base = penetratingWireResponse()
  const first = base.pairDiagnostics[0]
  const second = base.pairDiagnostics[1]
  const third = base.pairDiagnostics[2]
  const malformed = [
    {
      ...base,
      unexpectedPairPayload: true,
    },
    {
      ...base,
      pairClassificationCounts: {
        ...base.pairClassificationCounts,
        rawTriangleCount: 9,
      },
    },
    {
      ...base,
      pairDiagnostics: [
        { ...first, rawGeometry: [0, 1, 2] },
        second,
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          evidence: 'separated',
          policyDecision: 'separated',
        },
        second,
        third,
      ],
    },
    {
      ...base,
      pairClassificationCounts: {
        ...base.pairClassificationCounts,
        penetrating: 0,
      },
    },
    {
      ...base,
      pairClassificationCounts: {
        ...base.pairClassificationCounts,
        penetrating: 0,
        indeterminate: 3,
      },
    },
    {
      ...base,
      pairClassificationCounts: {
        ...base.pairClassificationCounts,
        candidateExcluded: 1,
      },
    },
    {
      ...base,
      pairDiagnostics: [first, second],
    },
    {
      ...base,
      pairDiagnostics: [first, first, third],
    },
    {
      ...base,
      pairDiagnostics: [second, first, third],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          firstFaceId: FACE_B,
          secondFaceId: FACE_A,
        },
        second,
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          firstFaceId: '00000000-0000-4000-8000-00000000000A',
          secondFaceId: '00000000-0000-4000-8000-00000000000b',
        },
        second,
        third,
      ],
    },
  ]

  for (const value of malformed) {
    const transport = createNativeStaticCollisionNativeTransport(() => value)
    await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
  }
})

test('pair snapshot rejects policy, disposition, and proof-provenance contradictions', async () => {
  const base = penetratingWireResponse()
  const first = base.pairDiagnostics[0]
  const second = base.pairDiagnostics[1]
  const third = base.pairDiagnostics[2]
  const malformed = [
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          policyDecision: 'touching',
        },
        second,
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          strictTransversalDualGateProven: false,
        },
        second,
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          disposition: 'indeterminate',
        },
        second,
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        first,
        {
          ...second,
          sharedHingeSolidClassified: true,
        },
        third,
      ],
    },
    {
      ...base,
      pairDiagnostics: [
        {
          ...first,
          topology: 'shared_vertex',
          sharedHingeSolidClassified: true,
          strictTransversalDualGateProven: false,
        },
        second,
        third,
      ],
    },
    {
      ...base,
      firstProvenPenetratingPair: {
        firstFaceId: FACE_A,
        secondFaceId: FACE_C,
      },
    },
  ]

  for (const value of malformed) {
    const transport = createNativeStaticCollisionNativeTransport(() => value)
    await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
  }
})

test('inspection rejects the retired transversal-only wire contract', async () => {
  const transport = createNativeStaticCollisionNativeTransport(() => ({
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_transversal_penetration',
    expectedUnorderedFacePairs: 3,
    provenTransversalPairs: 1,
    firstProvenTransversalPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
  }))

  await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
})

test('apply and inspection must bind to the exact same native pose generation', async () => {
  let call = 0
  const transport = createNativeStaticCollisionNativeTransport((command) => {
    call += 1
    if (command === 'apply_current_native_pose') return { binding: BINDING }
    return {
      binding: { ...BINDING, poseGeneration: '12' },
      status: 'certified_nonblocking',
      reason: null,
      expectedUnorderedFacePairs: 0,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: null,
      ...EMPTY_PAIR_SNAPSHOT,
    }
  })

  await assert.rejects(
    inspectAppliedPoseStaticCollision(transport, POSE),
    NativeStaticCollisionNativeError,
  )
  assert.equal(call, 2)
})

test('unavailable pose authority is explicit but cannot satisfy apply-and-inspect', async () => {
  let call = 0
  const transport = createNativeStaticCollisionNativeTransport((command) => {
    call += 1
    if (command === 'apply_current_native_pose') return { binding: BINDING }
    return {
      binding: null,
      status: 'unavailable',
      reason: 'pose_authority_unavailable',
      expectedUnorderedFacePairs: null,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
      ...NO_PAIR_SNAPSHOT,
    }
  })

  await assert.rejects(
    inspectAppliedPoseStaticCollision(transport, POSE),
    NativeStaticCollisionNativeError,
  )
  assert.equal(call, 2)
})

test('malformed and contradictory DTOs fail closed', async () => {
  const malformed = [
    {
      binding: BINDING,
      status: 'certified_nonblocking',
      reason: null,
      expectedUnorderedFacePairs: null,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: null,
      ...EMPTY_PAIR_SNAPSHOT,
    },
    {
      binding: BINDING,
      status: 'blocking',
      reason: 'proven_zero_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: {
        firstFaceId: FACE_A,
        secondFaceId: FACE_B,
      },
      ...PENETRATING_PAIR_SNAPSHOT,
    },
    {
      binding: BINDING,
      status: 'blocking',
      reason: 'evidence_unavailable',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
      ...INDETERMINATE_PAIR_SNAPSHOT,
      rawGeometry: 'private',
    },
    {
      binding: { ...BINDING, poseGeneration: '18446744073709551616' },
      status: 'blocking',
      reason: 'resource_limit_exceeded',
      expectedUnorderedFacePairs: null,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
      ...NO_PAIR_SNAPSHOT,
    },
  ]
  for (const value of malformed) {
    const transport = createNativeStaticCollisionNativeTransport(() => value)
    await assert.rejects(transport.inspect(), NativeStaticCollisionNativeError)
  }
})

test('invalid pose requests are rejected before native invocation', async () => {
  let calls = 0
  const transport = createNativeStaticCollisionNativeTransport(() => {
    calls += 1
    return { binding: BINDING }
  })
  const invalid = [
    { ...POSE, projectInstanceId: 'not-an-id' },
    {
      ...POSE,
      projectId: '00000000-0000-0000-0000-000000000000',
    },
    { ...POSE, revision: -1 },
    { ...POSE, fixedFaceId: 'not-an-id' },
    {
      ...POSE,
      completeHingeAngles: [
        { edgeId: EDGE_A, angleDegrees: 1 },
        { edgeId: EDGE_A, angleDegrees: 2 },
      ],
    },
    {
      ...POSE,
      completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 181 }],
    },
  ]
  for (const value of invalid) {
    await assert.rejects(transport.applyPose(value), NativeStaticCollisionNativeError)
  }
  assert.equal(calls, 0)
})

test('hostile response objects and raw native errors are contained', async () => {
  const hostile = new Proxy({}, {
    ownKeys() {
      throw new Error('C:\\private\\secret.ori')
    },
  })
  const hostileTransport = createNativeStaticCollisionNativeTransport(() => hostile)
  await assert.rejects(hostileTransport.inspect(), (error: unknown) => (
    error instanceof NativeStaticCollisionNativeError
    && !String(error).includes('secret.ori')
  ))

  const throwingTransport = createNativeStaticCollisionNativeTransport(() => {
    throw new Error('C:\\private\\secret.ori')
  })
  await assert.rejects(throwingTransport.applyPose(POSE), (error: unknown) => (
    error instanceof NativeStaticCollisionNativeError
    && !String(error).includes('secret.ori')
  ))
})

test('latest-only coordinator serializes exact work and coalesces queued poses', async () => {
  type InspectionControl = Readonly<{
    binding: typeof BINDING
    resolve(value: unknown): void
  }>
  const controls: InspectionControl[] = []
  const appliedAngles: number[] = []
  let generation = 10
  let lastBinding = BINDING
  let activeInspections = 0
  let maximumActiveInspections = 0
  const transport = {
    async applyPose(pose: typeof POSE) {
      appliedAngles.push(pose.completeHingeAngles[0]?.angleDegrees ?? -1)
      generation += 1
      lastBinding = { ...BINDING, poseGeneration: String(generation) }
      return lastBinding
    },
    inspect() {
      const binding = lastBinding
      activeInspections += 1
      maximumActiveInspections = Math.max(
        maximumActiveInspections,
        activeInspections,
      )
      return new Promise((resolve) => {
        controls.push({
          binding,
          resolve(value) {
            activeInspections -= 1
            resolve(value)
          },
        })
      })
    },
  }
  const coordinator =
    createNativeStaticCollisionInspectionCoordinator(transport)
  const poseA = {
    ...POSE,
    completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 10 }],
  }
  const poseB = {
    ...POSE,
    completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 20 }],
  }
  const poseC = {
    ...POSE,
    completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 30 }],
  }

  const first = coordinator.inspectLatest(poseA)
  await nextMicrotask()
  assert.equal(controls.length, 1)
  const waiting = coordinator.inspectLatest(poseB)
  const latest = coordinator.inspectLatest(poseC)
  await assert.rejects(first, isCoordinatorCategory('superseded'))
  await assert.rejects(waiting, isCoordinatorCategory('superseded'))
  assert.deepEqual(appliedAngles, [10])
  assert.equal(maximumActiveInspections, 1)

  controls[0]?.resolve(certifiedInspection(controls[0].binding))
  await nextMicrotask()
  await nextMicrotask()
  assert.deepEqual(appliedAngles, [10, 30], 'the waiting 20-degree pose is never applied')
  assert.equal(controls.length, 2)
  assert.equal(maximumActiveInspections, 1)
  controls[1]?.resolve({
    binding: controls[1].binding,
    diagnostic: {
      status: 'blocking',
      reason: 'evidence_unavailable',
      expectedUnorderedFacePairs: 1,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
    },
  })

  const latestDiagnostic = await latest
  assert.equal(latestDiagnostic.status, 'blocking')
  assert.equal(latestDiagnostic.reason, 'evidence_unavailable')
  assert.equal(maximumActiveInspections, 1)
  assert.equal(activeInspections, 0)
})

test('an active old result cannot settle the latest pose promise', async () => {
  const inspections: Array<{
    binding: typeof BINDING
    resolve(value: unknown): void
  }> = []
  let generation = 20
  let binding = BINDING
  const transport = {
    async applyPose() {
      generation += 1
      binding = { ...BINDING, poseGeneration: String(generation) }
      return binding
    },
    inspect() {
      const captured = binding
      return new Promise((resolve) => {
        inspections.push({ binding: captured, resolve })
      })
    },
  }
  const coordinator =
    createNativeStaticCollisionInspectionCoordinator(transport)
  const oldPromise = coordinator.inspectLatest(POSE)
  await nextMicrotask()
  const latestPromise = coordinator.inspectLatest({
    ...POSE,
    completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 91 }],
  })
  let latestSettled = false
  void latestPromise.finally(() => {
    latestSettled = true
  })
  await assert.rejects(oldPromise, isCoordinatorCategory('superseded'))

  inspections[0]?.resolve(certifiedInspection(inspections[0].binding))
  await nextMicrotask()
  await nextMicrotask()
  assert.equal(latestSettled, false)
  assert.equal(inspections.length, 2)
  inspections[1]?.resolve({
    binding: inspections[1].binding,
    diagnostic: {
      status: 'blocking',
      reason: 'resource_limit_exceeded',
      expectedUnorderedFacePairs: null,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
    },
  })
  assert.equal((await latestPromise).reason, 'resource_limit_exceeded')
})

test('coordinator contains raw failures and supports retry and later requests', async () => {
  let attempt = 0
  let binding = BINDING
  const appliedAngles: number[] = []
  const transport = {
    async applyPose(pose: typeof POSE) {
      appliedAngles.push(Math.max(
        ...pose.completeHingeAngles.map((angle) => angle.angleDegrees),
      ))
      binding = {
        ...BINDING,
        poseGeneration: String(Number(binding.poseGeneration) + 1),
      }
      return binding
    },
    async inspect() {
      attempt += 1
      if (attempt === 1) throw new Error('C:\\private\\secret.ori')
      return certifiedInspection(binding)
    },
  }
  const coordinator =
    createNativeStaticCollisionInspectionCoordinator(transport)

  await assert.rejects(
    coordinator.inspectLatest(POSE),
    (error: unknown) => (
      isCoordinatorCategory('native_unavailable')(error)
      && !String(error).includes('secret.ori')
    ),
  )
  assert.equal((await coordinator.retry()).status, 'certified_nonblocking')

  const nextPose = {
    ...POSE,
    completeHingeAngles: [{ edgeId: EDGE_A, angleDegrees: 45 }],
  }
  assert.equal(
    (await coordinator.inspectLatest(nextPose)).status,
    'certified_nonblocking',
  )
  assert.deepEqual(appliedAngles, [135, 135, 45])
})

test('coordinator rejects invalid, superseded, and disposed work without raw data', async () => {
  let nativeCalls = 0
  const transport = {
    async applyPose() {
      nativeCalls += 1
      return BINDING
    },
    async inspect() {
      nativeCalls += 1
      return certifiedInspection(BINDING)
    },
  }
  const coordinator =
    createNativeStaticCollisionInspectionCoordinator(transport)

  await assert.rejects(
    coordinator.retry(),
    isCoordinatorCategory('invalid_request'),
  )
  await assert.rejects(
    coordinator.inspectLatest({ ...POSE, projectId: 'C:\\secret.ori' }),
    (error: unknown) => (
      isCoordinatorCategory('invalid_request')(error)
      && !String(error).includes('secret.ori')
    ),
  )
  assert.equal(nativeCalls, 0)

  coordinator.dispose()
  await assert.rejects(
    coordinator.inspectLatest(POSE),
    isCoordinatorCategory('disposed'),
  )
  await assert.rejects(coordinator.retry(), isCoordinatorCategory('disposed'))
  assert.equal(nativeCalls, 0)
})

function certifiedInspection(binding: typeof BINDING) {
  return {
    binding,
    diagnostic: {
      status: 'certified_nonblocking' as const,
      reason: null,
      expectedUnorderedFacePairs: 0,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: null,
    },
  }
}

function isCoordinatorCategory(
  category: NativeStaticCollisionCoordinatorError['category'],
) {
  return (error: unknown) => (
    error instanceof NativeStaticCollisionCoordinatorError
    && error.category === category
  )
}

async function nextMicrotask() {
  await Promise.resolve()
}

function penetratingWireResponse() {
  return {
    binding: BINDING,
    status: 'blocking',
    reason: 'proven_positive_thickness_penetration',
    expectedUnorderedFacePairs: 3,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: FACE_A,
      secondFaceId: FACE_B,
    },
    pairClassificationCounts: {
      ...PENETRATING_PAIR_SNAPSHOT.pairClassificationCounts,
    },
    pairDiagnostics: PENETRATING_PAIR_SNAPSHOT.pairDiagnostics.map(
      (pair) => ({ ...pair }),
    ),
  } as const
}
