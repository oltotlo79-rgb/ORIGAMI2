import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import type {
  FoldPreviewContinuousMotionResult,
} from '../src/lib/foldPreviewContinuousMotion.ts'
import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'
import {
  FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION,
  FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_REQUEST_VERSION,
  isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic,
  prepareFoldPreviewTreeSingleHingeCorrectionAnalysisRequest,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisRequestInput,
} from '../src/lib/foldPreviewTreeSingleHingeCorrectionAnalysisRequest.ts'
import {
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  type FoldPreviewTreeSingleHingeContinuousAnalyzer,
  type FoldPreviewTreeSingleHingeContinuousBlocker,
  type FoldPreviewTreeTerminalFullScanBinding,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  type FoldPreviewTreeMotionContext,
} from '../src/lib/foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'

const COLLISION_THICKNESS = 0.02
const TARGET_ANGLE = 120
const GENERATION = 7
const REQUEST_SEQUENCE = 11

const POLICY = Object.freeze({
  version:
    FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION,
  clearance: 0.005,
  maximumTranslation: 0.01,
  maximumAngleDeltaDegrees: 30,
  path: Object.freeze({
    maxDepth: 18,
    maxIntervalTests: 10_000,
    minTimeSpan: 2 ** -22,
    maxIntervalPairVisits: 1_000_000,
    maxPointTriangleTests: 1_000_000,
  }),
}) satisfies FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy

type BlockedResult = Extract<
  FoldPreviewContinuousMotionResult<
    FoldPreviewTreeSingleHingeContinuousBlocker
  >,
  { kind: 'blocked' }
>

type Fixture = Readonly<{
  sourceContext: FoldPreviewTreeMotionContext
  sourcePoseRequestKey: string
  result: BlockedResult
  sample: NonNullable<
    NonNullable<BlockedResult['blocker']>['blockingSample']
  >
  binding: FoldPreviewTreeTerminalFullScanBinding
  runnerState: FoldPreviewContinuousMotionRunnerState<
    FoldPreviewTreeSingleHingeContinuousBlocker
  >
  evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence
}>

test('a genuine blocked terminal becomes a frozen internal analysis request', () => {
  const fixture = correctionFixture()
  const input = createInput(fixture)
  const request =
    prepareFoldPreviewTreeSingleHingeCorrectionAnalysisRequest(input)
  assert.ok(request)

  assert.equal(
    request.version,
    FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_REQUEST_VERSION,
  )
  assert.equal(
    request.kind,
    'tree_single_hinge_correction_analysis_request',
  )
  assert.equal(fixture.sourceContext.selectedAngleDegrees, 5)
  assert.deepEqual(request.request, {
    projectId: 'stationary-branch-project',
    revision: 1,
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    contextKey: fixture.sourceContext.contextKey,
    sourcePoseRequestKey: fixture.sourcePoseRequestKey,
    blockingPoseRequestKey:
      fixture.binding.identity.blockingPoseRequestKey,
    generation: GENERATION,
    requestSequence: REQUEST_SEQUENCE,
    sourceSelectedAngleDegrees: 0,
    targetSelectedAngleDegrees: TARGET_ANGLE,
    blockingSelectedAngleDegrees:
      fixture.binding.selectedAngleDegrees,
    blockingSampleTime: fixture.binding.blockingSampleTime,
    collisionThickness: COLLISION_THICKNESS,
  })
  assert.deepEqual(request.policy, POLICY)
  assert.notStrictEqual(request.policy, input.policy)
  assert.notStrictEqual(request.policy.path, input.policy.path)
  assert.notStrictEqual(request.request, input.evidence)
  assert.deepEqual(request.safety, {
    analysisOnly: true,
    terminalFullScanBindingAuthentic: true,
    terminalRequestIdentityVerified: true,
    completeRequestVectorsVerified: true,
    twoBodyTranslationInputEligible: true,
    freshAnalysisContextPrepared: true,
    coordinatorAuthorityPrivate: true,
    activeRequestLeaseBound: false,
    startScenePoseMatched: false,
    sceneApplied: false,
    autoApplicable: false,
  })
  assertDeeplyFrozen(request)
  assert.equal(
    isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(request),
    true,
  )
  assert.equal(
    isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
      structuredClone(request),
    ),
    false,
  )
  assert.equal(
    isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic({
      ...request,
    }),
    false,
  )
  assert.equal(
    isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
      Object.create(request),
    ),
    false,
  )

  let tokenPropertyReads = 0
  const hostileToken = new Proxy(request, {
    get() {
      tokenPropertyReads += 1
      throw new Error('request token property read')
    },
    getOwnPropertyDescriptor() {
      tokenPropertyReads += 1
      throw new Error('request token descriptor read')
    },
    ownKeys() {
      tokenPropertyReads += 1
      throw new Error('request token ownKeys read')
    },
  })
  const revokedToken = Proxy.revocable(request, {})
  revokedToken.revoke()
  assert.doesNotThrow(() => {
    assert.equal(
      isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
        hostileToken,
      ),
      false,
    )
    assert.equal(
      isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
        revokedToken.proxy,
      ),
      false,
    )
  })
  assert.equal(tokenPropertyReads, 0)
  for (const value of [
    null,
    undefined,
    false,
    0,
    '',
    Symbol('request'),
    0n,
  ]) {
    assert.equal(
      isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
        value,
      ),
      false,
    )
  }
})

test('request evidence, runner scalars, times, and complete vectors are exact', () => {
  const fixture = correctionFixture()
  const time = fixture.result.blockingSampleTime
  const alteredSampleVectors = Object.freeze({
    ...fixture.sample.angleVectors,
    target: Object.freeze(
      fixture.sample.angleVectors.target.map((angle) =>
        Object.freeze({
          ...angle,
          angleDegrees: angle.edgeId === 'frozen'
            ? angle.angleDegrees - 1
            : angle.angleDegrees,
        })),
    ),
  })
  const mismatchedStates = [
    replaceRunnerState(fixture, { status: 'clear' }),
    replaceRunnerState(fixture, { start: 1 }),
    replaceRunnerState(fixture, { requested: TARGET_ANGLE - 1 }),
    replaceRunnerState(fixture, {
      applied: fixture.runnerState.applied + 0.001,
    }),
    replaceTerminalResult(fixture, {
      blockingSampleTime: Math.min(1, time + 0.001),
    }),
    replaceBlockingSample(fixture, {
      blockingSampleTime: Math.min(1, time + 0.001),
    }),
    replaceBlockingSample(fixture, {
      angleVectors: alteredSampleVectors,
    }),
  ]
  for (const runnerState of mismatchedStates) {
    assert.equal(
      prepareUnknown({
        ...createInput(fixture),
        runnerState,
      }),
      null,
    )
  }

  const staleEvidence = [
    { projectId: 'other-project' },
    { revision: 2 },
    { fixedFaceId: 'moving' },
    { selectedHingeEdgeId: 'frozen' },
    { contextKey: `${fixture.evidence.contextKey}:stale` },
    {
      sourcePoseRequestKey:
        `${fixture.evidence.sourcePoseRequestKey}:stale`,
    },
    { generation: GENERATION + 1 },
    { requestSequence: REQUEST_SEQUENCE + 1 },
    { collisionThickness: COLLISION_THICKNESS + 0.001 },
    { targetSelectedAngleDegrees: TARGET_ANGLE - 1 },
  ]
  for (const evidenceOverride of staleEvidence) {
    assert.equal(
      prepareUnknown({
        ...createInput(fixture),
        evidence: {
          ...fixture.evidence,
          ...evidenceOverride,
        },
      }),
      null,
    )
  }

  const staleContext = prepareFoldPreviewTreeMotionContext({
    model: stationaryBranchCollisionModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    appliedAngles: [
      { edgeId: 'selected', angleDegrees: 0 },
      { edgeId: 'frozen', angleDegrees: 80 },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(staleContext)
  assert.equal(
    prepareUnknown({
      ...createInput(fixture),
      sourceContext: staleContext,
    }),
    null,
  )

  const equivalentContext = prepareFoldPreviewTreeMotionContext({
    model: fixture.sourceContext.model,
    fixedFaceId: fixture.sourceContext.fixedFaceId,
    selectedHingeEdgeId: fixture.sourceContext.selectedHingeEdgeId,
    appliedAngles: fixture.sourceContext.appliedAngles,
    collisionThickness: fixture.sourceContext.collisionThickness,
    visualThickness: fixture.sourceContext.visualThickness,
  })
  assert.ok(equivalentContext)
  assert.equal(equivalentContext.contextKey, fixture.sourceContext.contextKey)
  assert.notStrictEqual(equivalentContext, fixture.sourceContext)
  assert.notStrictEqual(
    equivalentContext.model,
    fixture.sourceContext.model,
  )
  assert.equal(
    prepareUnknown({
      ...createInput(fixture),
      sourceContext: equivalentContext,
    }),
    null,
  )
})

test('clones, wrappers, non-blocked snapshots, and malformed inputs fail closed', () => {
  const fixture = correctionFixture()
  const input = createInput(fixture)
  const clonedContext = structuredClone(fixture.sourceContext)
  const clonedBinding = structuredClone(fixture.binding)
  const spreadBinding = Object.freeze({ ...fixture.binding })
  const inheritedBinding = Object.create(fixture.binding)

  assert.equal(
    prepareUnknown({ ...input, sourceContext: clonedContext }),
    null,
  )
  for (const binding of [
    clonedBinding,
    spreadBinding,
    inheritedBinding,
  ]) {
    assert.equal(
      prepareUnknown({
        ...input,
        runnerState: replaceTerminalBinding(fixture, binding),
      }),
      null,
    )
  }
  assert.equal(prepareUnknown(structuredClone(input)), null)
  assert.equal(
    prepareUnknown({
      ...input,
      runnerState: Object.freeze({
        ...fixture.runnerState,
        result: null,
      }),
    }),
    null,
  )
  for (const value of [
    null,
    undefined,
    false,
    0,
    '',
    Symbol('request'),
    [],
    {},
    { ...input, evidence: null },
    { ...input, policy: null },
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(prepareUnknown(value), null)
    })
  }
})

test('hostile and revoked Proxy inputs fail closed before binding reads', () => {
  const fixture = correctionFixture()
  const input = createInput(fixture)
  let bindingPropertyReads = 0
  const hostileBinding = new Proxy(fixture.binding, {
    get() {
      bindingPropertyReads += 1
      throw new Error('binding property read')
    },
    getOwnPropertyDescriptor() {
      bindingPropertyReads += 1
      throw new Error('binding descriptor read')
    },
    ownKeys() {
      bindingPropertyReads += 1
      throw new Error('binding ownKeys read')
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(
      prepareUnknown({
        ...input,
        runnerState: replaceTerminalBinding(
          fixture,
          hostileBinding,
        ),
      }),
      null,
    )
  })
  assert.equal(bindingPropertyReads, 0)

  const revoked = Proxy.revocable(fixture.binding, {})
  revoked.revoke()
  assert.doesNotThrow(() => {
    assert.equal(
      prepareUnknown({
        ...input,
        runnerState: replaceTerminalBinding(
          fixture,
          revoked.proxy,
        ),
      }),
      null,
    )
  })

  const hostileInput = new Proxy(input, {
    getOwnPropertyDescriptor() {
      throw new Error('input descriptor read')
    },
  })
  const hostileEvidence = new Proxy(fixture.evidence, {
    getOwnPropertyDescriptor() {
      throw new Error('evidence descriptor read')
    },
  })
  const hostileContext = new Proxy(fixture.sourceContext, {
    get() {
      throw new Error('context property read')
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(prepareUnknown(hostileInput), null)
    assert.equal(
      prepareUnknown({ ...input, evidence: hostileEvidence }),
      null,
    )
    assert.equal(
      prepareUnknown({ ...input, sourceContext: hostileContext }),
      null,
    )
  })
})

test('outer records are snapshotted from data descriptors without Proxy gets', () => {
  const fixture = correctionFixture()
  let propertyGets = 0
  const withoutGets = <T extends object>(value: T): T =>
    new Proxy(value, {
      get() {
        propertyGets += 1
        return propertyGets % 2 === 0
          ? 'stateful-even-value'
          : 'stateful-odd-value'
      },
    })

  const sample = withoutGets(Object.freeze({ ...fixture.sample }))
  const blocker = withoutGets(Object.freeze({
    ...fixture.result.blocker,
    blockingSample: sample,
  }))
  const result = withoutGets(Object.freeze({
    ...fixture.result,
    blocker,
  }))
  const runnerState = withoutGets(Object.freeze({
    ...fixture.runnerState,
    result,
  }))
  const evidence = withoutGets(fixture.evidence)
  const path = withoutGets(POLICY.path)
  const policy = withoutGets(Object.freeze({
    ...POLICY,
    path,
  }))

  const request = prepareUnknown({
    sourceContext: fixture.sourceContext,
    runnerState,
    evidence,
    policy,
  })
  assert.ok(request)
  assert.equal(propertyGets, 0)
  assert.equal(request.request.generation, GENERATION)
  assert.deepEqual(request.policy, POLICY)
})

test('wrong array lengths reject before any indexed descriptor is read', () => {
  const fixture = correctionFixture()
  let bracketIndexReads = 0
  const oversizedBracket = new Proxy([0, 0.5, 1], {
    getOwnPropertyDescriptor(target, property) {
      if (typeof property === 'string' && /^\d+$/u.test(property)) {
        bracketIndexReads += 1
        throw new Error('bracket index should not be read')
      }
      return Reflect.getOwnPropertyDescriptor(target, property)
    },
  })
  assert.equal(
    prepareUnknown({
      ...createInput(fixture),
      runnerState: replaceTerminalResult(fixture, {
        unsafeBracket: oversizedBracket,
      }),
    }),
    null,
  )
  assert.equal(bracketIndexReads, 0)

  let angleIndexReads = 0
  const oversizedAngles = new Proxy(
    [...fixture.sample.angleVectors.target, {
      edgeId: 'extra',
      angleDegrees: 0,
    }],
    {
      getOwnPropertyDescriptor(target, property) {
        if (typeof property === 'string' && /^\d+$/u.test(property)) {
          angleIndexReads += 1
          throw new Error('angle index should not be read')
        }
        return Reflect.getOwnPropertyDescriptor(target, property)
      },
    },
  )
  assert.equal(
    prepareUnknown({
      ...createInput(fixture),
      runnerState: replaceBlockingSample(fixture, {
        angleVectors: Object.freeze({
          ...fixture.sample.angleVectors,
          target: oversizedAngles,
        }),
      }),
    }),
    null,
  )
  assert.equal(angleIndexReads, 0)
})

test('all explicit policy bounds fail closed and are detached on success', () => {
  const fixture = correctionFixture()
  const invalidPolicies = [
    { ...POLICY, version: 'other-policy' },
    { ...POLICY, clearance: 0 },
    { ...POLICY, maximumTranslation: Number.POSITIVE_INFINITY },
    { ...POLICY, clearance: 0.02, maximumTranslation: 0.01 },
    { ...POLICY, maximumAngleDeltaDegrees: 181 },
    { ...POLICY, path: { ...POLICY.path, maxDepth: 53 } },
    { ...POLICY, path: { ...POLICY.path, maxIntervalTests: 0 } },
    { ...POLICY, path: { ...POLICY.path, minTimeSpan: 0 } },
    {
      ...POLICY,
      path: { ...POLICY.path, maxIntervalPairVisits: 1_000_001 },
    },
    {
      ...POLICY,
      path: { ...POLICY.path, maxPointTriangleTests: 1_000_001 },
    },
  ]
  for (const policy of invalidPolicies) {
    assert.equal(
      prepareUnknown({ ...createInput(fixture), policy }),
      null,
    )
  }

  const mutablePolicy = {
    ...POLICY,
    path: { ...POLICY.path },
  }
  const mutableEvidence = { ...fixture.evidence }
  const request = prepareUnknown({
    ...createInput(fixture),
    evidence: mutableEvidence,
    policy: mutablePolicy,
  })
  assert.ok(request)
  mutablePolicy.clearance = 0.001
  mutablePolicy.path.maxDepth = 1
  mutableEvidence.generation += 1
  assert.equal(request.policy.clearance, POLICY.clearance)
  assert.equal(request.policy.path.maxDepth, POLICY.path.maxDepth)
  assert.equal(request.request.generation, GENERATION)
})

test('the internal request exposes no scene or runtime application capability', () => {
  const fixture = correctionFixture()
  const request =
    prepareFoldPreviewTreeSingleHingeCorrectionAnalysisRequest(
      createInput(fixture),
    )
  assert.ok(request)

  for (const forbiddenKey of [
    'context',
    'terminalFullScanBinding',
    'runnerState',
    'runtimeState',
    'runnerToken',
    'applicationToken',
    'sceneCommand',
    'applyAngle',
    'applyPose',
  ]) {
    assert.equal(
      Object.hasOwn(request, forbiddenKey),
      false,
      `top-level request exposed ${forbiddenKey}`,
    )
  }
  assert.equal(containsFunction(request), false)
  const serialized = JSON.stringify(request)
  assert.doesNotMatch(serialized, /terminal_full_scan_binding/iu)
  assert.doesNotMatch(serialized, /"angleVectors"|"faces"|"hinges"/u)

  const source = readFileSync(
    new URL(
      '../src/lib/foldPreviewTreeSingleHingeCorrectionAnalysisRequest.ts',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(
    source,
    /remain in private provenance for a coordinator\s+\* operation/iu,
  )
  assert.doesNotMatch(
    source,
    /import\s+\{[^}]*\b(?:apply|commit|dispatch|execute)[A-Z][^}]*\}\s+from/iu,
  )
  assert.match(
    source,
    /if \(Object\.isFrozen\(object\) \|\| seen\.has\(object\)\) return value/u,
  )
  assert.match(
    source,
    /Reads every untrusted field exactly once from its own data descriptor/u,
  )
  assert.match(
    source,
    /analysisRequestAuthorities = new WeakMap/u,
  )
})

function correctionFixture(): Fixture {
  const model = stationaryBranchCollisionModel()
  const sourceContext = prepareFoldPreviewTreeMotionContext({
    model,
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    // The selected magnitude is intentionally not the next request's start.
    appliedAngles: [
      { edgeId: 'selected', angleDegrees: 5 },
      { edgeId: 'frozen', angleDegrees: 90 },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(sourceContext)
  const startAngles = [
    { edgeId: 'selected', angleDegrees: 0 },
    { edgeId: 'frozen', angleDegrees: 90 },
  ] as const
  const sourcePoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      sourceContext.model,
      sourceContext.fixedFaceId,
      sourceContext.collisionThickness,
      startAngles,
    )
  assert.ok(sourcePoseRequestKey)
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    sourceContext.model,
    sourceContext.fixedFaceId,
    sourceContext.selectedHingeEdgeId,
  )
  assert.ok(analyzer)
  const job = analyzer.createJob(
    startAngles,
    TARGET_ANGLE,
    COLLISION_THICKNESS,
    {
      maxDepth: POLICY.path.maxDepth,
      maxIntervalTests: POLICY.path.maxIntervalTests,
      minTimeSpan: POLICY.path.minTimeSpan,
      requestIdentity: {
        contextKey: sourceContext.contextKey,
        sourcePoseRequestKey,
        generation: GENERATION,
        requestSequence: REQUEST_SEQUENCE,
      },
    },
  )
  assert.ok(job)
  const terminal = run(job)
  assert.equal(terminal.kind, 'blocked', JSON.stringify(terminal))
  if (terminal.kind !== 'blocked') {
    throw new Error('expected a blocked terminal')
  }
  const sample = terminal.blocker?.blockingSample
  const binding = sample?.terminalFullScanBinding
  assert.ok(sample)
  assert.ok(binding)
  const sourceAngle = selectedAngle(binding.angleVectors.start)
  const targetAngle = selectedAngle(binding.angleVectors.target)
  assert.notEqual(sourceAngle, null)
  assert.equal(targetAngle, TARGET_ANGLE)
  const runnerState = Object.freeze({
    requested: TARGET_ANGLE,
    applied: sourceAngle
      + (
        TARGET_ANGLE - sourceAngle
      ) * terminal.certifiedSafeThrough,
    start: sourceAngle,
    status: 'blocked' as const,
    reason: 'motion_blocked',
    result: terminal,
  })
  const evidence = Object.freeze({
    projectId: sourceContext.model.projectId,
    revision: sourceContext.model.revision,
    fixedFaceId: sourceContext.fixedFaceId,
    selectedHingeEdgeId: sourceContext.selectedHingeEdgeId,
    contextKey: sourceContext.contextKey,
    sourcePoseRequestKey,
    generation: GENERATION,
    requestSequence: REQUEST_SEQUENCE,
    collisionThickness: COLLISION_THICKNESS,
    targetSelectedAngleDegrees: TARGET_ANGLE,
  })
  return Object.freeze({
    sourceContext,
    sourcePoseRequestKey,
    result: terminal,
    sample,
    binding,
    runnerState,
    evidence,
  })
}

function createInput(
  fixture: Fixture,
): FoldPreviewTreeSingleHingeCorrectionAnalysisRequestInput {
  return {
    sourceContext: fixture.sourceContext,
    runnerState: fixture.runnerState,
    evidence: fixture.evidence,
    policy: POLICY,
  }
}

function replaceRunnerState(
  fixture: Fixture,
  overrides: Record<string, unknown>,
) {
  return Object.freeze({
    ...fixture.runnerState,
    ...overrides,
  })
}

function replaceTerminalResult(
  fixture: Fixture,
  overrides: Record<string, unknown>,
) {
  return Object.freeze({
    ...fixture.runnerState,
    result: Object.freeze({
      ...fixture.result,
      ...overrides,
    }),
  })
}

function replaceBlockingSample(
  fixture: Fixture,
  overrides: Record<string, unknown>,
) {
  const sample = Object.freeze({
    ...fixture.sample,
    ...overrides,
  })
  const blocker = Object.freeze({
    ...fixture.result.blocker,
    blockingSample: sample,
  })
  return Object.freeze({
    ...fixture.runnerState,
    result: Object.freeze({
      ...fixture.result,
      blocker,
    }),
  })
}

function replaceTerminalBinding(
  fixture: Fixture,
  terminalFullScanBinding: unknown,
) {
  return replaceBlockingSample(fixture, {
    terminalFullScanBinding,
  })
}

function prepareUnknown(value: unknown) {
  return prepareFoldPreviewTreeSingleHingeCorrectionAnalysisRequest(
    value as FoldPreviewTreeSingleHingeCorrectionAnalysisRequestInput,
  )
}

function run(
  job: NonNullable<
    ReturnType<FoldPreviewTreeSingleHingeContinuousAnalyzer['createJob']>
  >,
) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(32)
    if (result.kind !== 'pending') return result
  }
  throw new Error('tree single-hinge continuous job did not terminate')
}

function selectedAngle(
  angles: readonly Readonly<{
    edgeId: string
    angleDegrees: number
  }>[],
) {
  return angles.find((angle) => angle.edgeId === 'selected')
    ?.angleDegrees ?? null
}

function assertDeeplyFrozen(
  value: unknown,
  seen = new Set<object>(),
) {
  if (typeof value !== 'object' || value === null || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const key of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
}

function containsFunction(
  value: unknown,
  seen = new Set<object>(),
): boolean {
  if (typeof value === 'function') return true
  if (typeof value !== 'object' || value === null || seen.has(value)) {
    return false
  }
  seen.add(value)
  return Reflect.ownKeys(value).some((key) =>
    containsFunction(
      (value as Record<PropertyKey, unknown>)[key],
      seen,
    ))
}

function stationaryBranchCollisionModel(): FoldGraphPreviewModel {
  const movingAxisStart = { vertexId: 'ma', x: 0.25, z: 0 }
  const movingAxisEnd = { vertexId: 'mb', x: 0.25, z: 1 }
  const obstacleAxisStart = { vertexId: 'oa', x: 0, z: 0 }
  const obstacleAxisEnd = { vertexId: 'ob', x: 0, z: 1 }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [
      movingAxisStart,
      obstacleAxisStart,
      obstacleAxisEnd,
      movingAxisEnd,
    ],
  }
  const moving: FoldPreviewFaceModel = {
    id: 'moving',
    polygon: [
      movingAxisEnd,
      { vertexId: 'moving-top-right', x: 0.75, z: 1 },
      { vertexId: 'moving-bottom-right', x: 0.75, z: 0 },
      movingAxisStart,
    ],
  }
  const obstacle: FoldPreviewFaceModel = {
    id: 'obstacle',
    polygon: [
      obstacleAxisStart,
      { vertexId: 'obstacle-bottom-left', x: -0.5, z: 0 },
      { vertexId: 'obstacle-top-left', x: -0.5, z: 1 },
      obstacleAxisEnd,
    ],
  }
  const selected: FoldPreviewHingeModel = {
    edgeId: 'selected',
    leftFaceId: 'root',
    rightFaceId: 'moving',
    start: movingAxisStart,
    end: movingAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const frozen: FoldPreviewHingeModel = {
    edgeId: 'frozen',
    leftFaceId: 'root',
    rightFaceId: 'obstacle',
    start: obstacleAxisStart,
    end: obstacleAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'stationary-branch-project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.125, y: 0.5 },
    worldBounds: {
      minX: -0.5,
      minZ: 0,
      maxX: 0.75,
      maxZ: 1,
    },
    faces: [root, moving, obstacle],
    hinges: [selected, frozen],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'moving',
          hinge: selected,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'root',
          childFaceId: 'obstacle',
          hinge: frozen,
          childRotationSign: -1,
        },
      ],
    },
  }
}
