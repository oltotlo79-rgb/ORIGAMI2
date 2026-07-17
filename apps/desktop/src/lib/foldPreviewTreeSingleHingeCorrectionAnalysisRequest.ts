import type {
  FoldPreviewContinuousMotionRunnerState,
} from './foldPreviewContinuousMotionRunner.ts'
import type {
  FoldPreviewHingeAngle,
} from './foldPreviewKinematics.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'
import {
  isFoldPreviewTreeTerminalFullScanBindingAuthentic,
  isFoldPreviewTreeTerminalFullScanBindingAuthenticForModel,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS,
  type FoldPreviewTreeSingleHingeContinuousBlocker,
  type FoldPreviewTreeTerminalFullScanBinding,
} from './foldPreviewTreeSingleHingeContinuousCollision.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION =
  'tree_single_hinge_correction_analysis_policy_v1'
export const FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_REQUEST_VERSION =
  'tree_single_hinge_correction_analysis_request_v1'

const MAX_INTERVAL_TESTS = 1_000_000
const MAX_REQUEST_KEY_LENGTH = 8 * 1_024 * 1_024
const MAX_ID_LENGTH = 512
const MAX_SNAPSHOT_ARRAY_LENGTH = 1_000_000

export type FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy =
  Readonly<{
    version:
      typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION
    clearance: number
    maximumTranslation: number
    maximumAngleDeltaDegrees: number
    path: Readonly<{
      maxDepth: number
      maxIntervalTests: number
      minTimeSpan: number
      maxIntervalPairVisits: number
      maxPointTriangleTests: number
    }>
  }>

export type FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence =
  Readonly<{
    projectId: string
    revision: number
    fixedFaceId: string
    selectedHingeEdgeId: string
    contextKey: string
    sourcePoseRequestKey: string
    generation: number
    requestSequence: number
    collisionThickness: number
    targetSelectedAngleDegrees: number
  }>

export type FoldPreviewTreeSingleHingeCorrectionAnalysisRequestInput =
  Readonly<{
    sourceContext: FoldPreviewTreeMotionContext
    runnerState: FoldPreviewContinuousMotionRunnerState<
      FoldPreviewTreeSingleHingeContinuousBlocker
    >
    evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence
    policy: FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy
  }>

export type FoldPreviewTreeSingleHingeCorrectionAnalysisRequest =
  Readonly<{
    version:
      typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_REQUEST_VERSION
    kind: 'tree_single_hinge_correction_analysis_request'
    /** Detached scalar summary of the verified terminal request. */
    request: Readonly<{
      projectId: string
      revision: number
      fixedFaceId: string
      selectedHingeEdgeId: string
      contextKey: string
      sourcePoseRequestKey: string
      blockingPoseRequestKey: string
      generation: number
      requestSequence: number
      sourceSelectedAngleDegrees: number
      targetSelectedAngleDegrees: number
      blockingSelectedAngleDegrees: number
      blockingSampleTime: number
      collisionThickness: number
    }>
    /** Detached explicit bounds for all downstream analysis phases. */
    policy: FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy
    safety: Readonly<{
      analysisOnly: true
      terminalFullScanBindingAuthentic: true
      terminalRequestIdentityVerified: true
      completeRequestVectorsVerified: true
      twoBodyTranslationInputEligible: true
      freshAnalysisContextPrepared: true
      coordinatorAuthorityPrivate: true
      activeRequestLeaseBound: false
      startScenePoseMatched: false
      sceneApplied: false
      autoApplicable: false
    }>
  }>

type DataRecord = Record<string, unknown>

type AnalysisRequestAuthority = Readonly<{
  sourceContext: FoldPreviewTreeMotionContext
  context: FoldPreviewTreeMotionContext
  terminalFullScanBinding: FoldPreviewTreeTerminalFullScanBinding
}>

const analysisRequestAuthorities = new WeakMap<
  object,
  AnalysisRequestAuthority
>()

type ExtractedTerminal = Readonly<{
  runnerState: DataRecord
  runner: Readonly<Record<string, unknown>>
  result: DataRecord
  resultFields: Readonly<Record<string, unknown>>
  blocker: DataRecord
  blockerFields: Readonly<Record<string, unknown>>
  sample: DataRecord
  sampleFields: Readonly<Record<string, unknown>>
  terminalFullScanBinding: unknown
}>

/**
 * Converts one blocked runner terminal into an analysis-only coordinator
 * request.
 *
 * The returned token exposes only detached scalar data. Its new authentic
 * context and exact binding remain in private provenance for a coordinator
 * operation implemented at this module boundary; they are not own properties,
 * cannot be serialized, and must never be copied into React state.
 */
export function prepareFoldPreviewTreeSingleHingeCorrectionAnalysisRequest(
  input: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestInput,
): FoldPreviewTreeSingleHingeCorrectionAnalysisRequest | null {
  try {
    if (!isRecord(input)) return null
    const rawSourceContext = ownDataValue(input, 'sourceContext')
    const rawRunnerState = ownDataValue(input, 'runnerState')
    const rawEvidence = ownDataValue(input, 'evidence')
    const rawPolicy = ownDataValue(input, 'policy')
    if (
      rawSourceContext === MISSING
      || rawRunnerState === MISSING
      || rawEvidence === MISSING
      || rawPolicy === MISSING
    ) return null

    const terminal = extractTerminal(rawRunnerState)
    if (!terminal) return null

    // This must remain the first operation on the raw binding. The WeakMap
    // guard neither enumerates nor reads a property of hostile input.
    if (
      !isFoldPreviewTreeTerminalFullScanBindingAuthentic(
        terminal.terminalFullScanBinding,
      )
    ) return null
    const binding = terminal.terminalFullScanBinding

    if (!isAuthenticMotionContext(rawSourceContext)) return null
    const sourceContext = rawSourceContext
    if (
      !isFoldPreviewTreeTerminalFullScanBindingAuthenticForModel(
        sourceContext.model,
        binding,
      )
    ) return null
    const evidence = snapshotEvidence(rawEvidence)
    const policy = snapshotPolicy(rawPolicy)
    if (!evidence || !policy) return null

    const terminalScalars = verifyBlockedTerminal(
      terminal,
      binding,
      evidence,
    )
    if (!terminalScalars) return null
    if (
      sourceContext.version !== 'tree_single_hinge_motion_v1'
      || sourceContext.model.projectId !== evidence.projectId
      || sourceContext.model.revision !== evidence.revision
      || sourceContext.fixedFaceId !== evidence.fixedFaceId
      || sourceContext.selectedHingeEdgeId
        !== evidence.selectedHingeEdgeId
      || sourceContext.contextKey !== evidence.contextKey
      || sourceContext.collisionThickness !== evidence.collisionThickness
      || binding.identity.projectId !== evidence.projectId
      || binding.identity.revision !== evidence.revision
      || binding.identity.fixedFaceId !== evidence.fixedFaceId
      || binding.identity.selectedHingeEdgeId
        !== evidence.selectedHingeEdgeId
      || binding.collisionThickness !== evidence.collisionThickness
    ) return null

    const context = prepareFoldPreviewTreeMotionContext({
      model: sourceContext.model,
      fixedFaceId: sourceContext.fixedFaceId,
      selectedHingeEdgeId: sourceContext.selectedHingeEdgeId,
      appliedAngles: binding.angleVectors.start,
      collisionThickness: sourceContext.collisionThickness,
      visualThickness: sourceContext.visualThickness,
    })
    if (
      !context
      || context === sourceContext
      || context.contextKey !== sourceContext.contextKey
      || !sameCompleteAngles(
        context.appliedAngles,
        binding.angleVectors.start,
      )
    ) return null

    const sourcePoseRequestKey =
      createFoldPreviewTreeSceneCollisionPoseKey(
        context.model,
        context.fixedFaceId,
        context.collisionThickness,
        context.appliedAngles,
      )
    const blockingPoseRequestKey =
      createFoldPreviewTreeSceneCollisionPoseKey(
        context.model,
        context.fixedFaceId,
        context.collisionThickness,
        binding.angleVectors.sample,
      )
    const expectedTargetAngles =
      replaceFoldPreviewTreeMotionSelectedAngle(
        context,
        evidence.targetSelectedAngleDegrees,
      )
    const expectedBlockingAngles =
      replaceFoldPreviewTreeMotionSelectedAngle(
        context,
        binding.selectedAngleDegrees,
      )
    if (
      !sourcePoseRequestKey
      || sourcePoseRequestKey !== evidence.sourcePoseRequestKey
      || sourcePoseRequestKey
        !== binding.identity.request.sourcePoseRequestKey
      || !blockingPoseRequestKey
      || blockingPoseRequestKey
        !== binding.identity.blockingPoseRequestKey
      || !expectedTargetAngles
      || !sameCompleteAngles(
        expectedTargetAngles,
        binding.angleVectors.target,
      )
      || !expectedBlockingAngles
      || !sameCompleteAngles(
        expectedBlockingAngles,
        binding.angleVectors.sample,
      )
    ) return null

    const result = deepFreeze({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_REQUEST_VERSION,
      kind: 'tree_single_hinge_correction_analysis_request',
      request: {
        projectId: evidence.projectId,
        revision: evidence.revision,
        fixedFaceId: evidence.fixedFaceId,
        selectedHingeEdgeId: evidence.selectedHingeEdgeId,
        contextKey: evidence.contextKey,
        sourcePoseRequestKey,
        blockingPoseRequestKey,
        generation: evidence.generation,
        requestSequence: evidence.requestSequence,
        sourceSelectedAngleDegrees: terminalScalars.sourceAngle,
        targetSelectedAngleDegrees:
          evidence.targetSelectedAngleDegrees,
        blockingSelectedAngleDegrees:
          binding.selectedAngleDegrees,
        blockingSampleTime: binding.blockingSampleTime,
        collisionThickness: evidence.collisionThickness,
      },
      policy,
      safety: {
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
      },
    }) satisfies FoldPreviewTreeSingleHingeCorrectionAnalysisRequest
    analysisRequestAuthorities.set(result, Object.freeze({
      sourceContext,
      context,
      terminalFullScanBinding: binding,
    }))
    return result
  } catch {
    return null
  }
}

/**
 * Checks only the exact opaque token issued above. It does not read a public
 * property, and a clone or serialized copy never recovers private authority.
 */
export function isFoldPreviewTreeSingleHingeCorrectionAnalysisRequestAuthentic(
  value: unknown,
): value is FoldPreviewTreeSingleHingeCorrectionAnalysisRequest {
  try {
    return typeof value === 'object'
      && value !== null
      && analysisRequestAuthorities.has(value)
  } catch {
    return false
  }
}

function extractTerminal(value: unknown): ExtractedTerminal | null {
  if (!isFrozenPlainRecord(value)) return null
  const runner = snapshotOwnDataValues(value, [
    'requested',
    'applied',
    'start',
    'status',
    'reason',
    'result',
  ])
  if (!runner || !isFrozenPlainRecord(runner.result)) return null
  const rawResult = runner.result
  const resultFields = snapshotOwnDataValues(rawResult, [
    'kind',
    'certifiedSafeThrough',
    'stopTime',
    'unsafeBracket',
    'blockingSampleTime',
    'blocker',
    'stats',
  ])
  if (!resultFields || !isFrozenPlainRecord(resultFields.blocker)) return null
  const rawBlocker = resultFields.blocker
  const blockerFields = snapshotOwnDataValues(rawBlocker, [
    'firstFaceId',
    'secondFaceId',
    'relation',
    'geometryClass',
    'blockingSample',
  ])
  if (!blockerFields || !isFrozenPlainRecord(blockerFields.blockingSample)) {
    return null
  }
  const rawSample = blockerFields.blockingSample
  const sampleFields = snapshotOwnDataValues(rawSample, [
    'version',
    'sourcePose',
    'blockingSampleTime',
    'selectedAngleDegrees',
    'collisionThickness',
    'identity',
    'angleVectors',
    'terminalFullScanBinding',
  ])
  if (!sampleFields) return null
  return {
    runnerState: value,
    runner,
    result: rawResult,
    resultFields,
    blocker: rawBlocker,
    blockerFields,
    sample: rawSample,
    sampleFields,
    terminalFullScanBinding: sampleFields.terminalFullScanBinding,
  }
}

function verifyBlockedTerminal(
  terminal: ExtractedTerminal,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence,
): Readonly<{ sourceAngle: number }> | null {
  const {
    runner,
    result,
    resultFields,
    blocker,
    blockerFields,
    sample,
    sampleFields,
  } = terminal
  const certifiedSafeThrough = resultFields.certifiedSafeThrough
  const blockingSampleTime = resultFields.blockingSampleTime
  const unsafeBracket = snapshotBracket(resultFields.unsafeBracket)
  const stats = snapshotStats(resultFields.stats)
  if (
    runner.status !== 'blocked'
    || runner.reason !== 'motion_blocked'
    || runner.result !== result
    || resultFields.kind !== 'blocked'
    || !validNonTerminalTime(certifiedSafeThrough)
    || resultFields.stopTime !== certifiedSafeThrough
    || !unsafeBracket
    || unsafeBracket[0] !== certifiedSafeThrough
    || unsafeBracket[1] !== blockingSampleTime
    || !validUnitTime(blockingSampleTime)
    || !stats
    || resultFields.blocker !== blocker
    || blockerFields.relation !== 'non_adjacent'
    || blockerFields.blockingSample !== sample
    || terminal.terminalFullScanBinding !== binding
    || sampleFields.terminalFullScanBinding !== binding
    || sampleFields.version !== 'tree_single_hinge_blocking_sample_v1'
    || sampleFields.sourcePose !== 'blocking_evaluate_point_pose'
    || sampleFields.blockingSampleTime !== blockingSampleTime
    || binding.blockingSampleTime !== blockingSampleTime
    || sampleFields.selectedAngleDegrees !== binding.selectedAngleDegrees
    || sampleFields.collisionThickness !== binding.collisionThickness
    || !sameCompleteAngleVectorSets(
      sampleFields.angleVectors,
      binding.angleVectors,
    )
  ) return null

  const sourceAngle = selectedAngle(
    binding.angleVectors.start,
    evidence.selectedHingeEdgeId,
  )
  const targetAngle = selectedAngle(
    binding.angleVectors.target,
    evidence.selectedHingeEdgeId,
  )
  const blockingAngle = selectedAngle(
    binding.angleVectors.sample,
    evidence.selectedHingeEdgeId,
  )
  if (
    sourceAngle === null
    || targetAngle !== evidence.targetSelectedAngleDegrees
    || blockingAngle !== binding.selectedAngleDegrees
    || runner.start !== sourceAngle
    || runner.requested !== targetAngle
    || runner.applied !== interpolate(
      sourceAngle,
      targetAngle,
      certifiedSafeThrough,
    )
    || binding.selectedAngleDegrees !== interpolate(
      sourceAngle,
      targetAngle,
      blockingSampleTime,
    )
    || !sameIdentity(
      sampleFields.identity,
      binding,
      evidence,
    )
    || !bindingMatchesRequestEvidence(binding, evidence)
    || !eligibleTwoBodyBinding(binding)
  ) return null
  return Object.freeze({ sourceAngle })
}

function sameIdentity(
  value: unknown,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence,
) {
  if (!isRecord(value)) return false
  const identity = snapshotOwnDataValues(value, [
    'projectId',
    'revision',
    'revisionBinding',
    'fixedFaceId',
    'selectedHingeEdgeId',
    'request',
  ])
  if (!identity) return false
  return identity.projectId === binding.identity.projectId
    && identity.projectId === evidence.projectId
    && identity.revision === binding.identity.revision
    && identity.revision === evidence.revision
    && identity.revisionBinding === 'project_response_source_equal_v1'
    && identity.fixedFaceId === binding.identity.fixedFaceId
    && identity.fixedFaceId === evidence.fixedFaceId
    && identity.selectedHingeEdgeId
      === binding.identity.selectedHingeEdgeId
    && identity.selectedHingeEdgeId === evidence.selectedHingeEdgeId
    && sameRequestIdentity(identity.request, binding, evidence)
}

function sameRequestIdentity(
  value: unknown,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence,
) {
  if (!isRecord(value)) return false
  const request = snapshotOwnDataValues(value, [
    'contextKey',
    'sourcePoseRequestKey',
    'generation',
    'requestSequence',
  ])
  if (!request) return false
  return request.contextKey === binding.identity.request.contextKey
    && request.contextKey === evidence.contextKey
    && request.sourcePoseRequestKey
      === binding.identity.request.sourcePoseRequestKey
    && request.sourcePoseRequestKey === evidence.sourcePoseRequestKey
    && request.generation === binding.identity.request.generation
    && request.generation === evidence.generation
    && request.requestSequence
      === binding.identity.request.requestSequence
    && request.requestSequence === evidence.requestSequence
}

function bindingMatchesRequestEvidence(
  binding: FoldPreviewTreeTerminalFullScanBinding,
  evidence: FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence,
) {
  const request = binding.identity.request
  return binding.version
      === 'tree_single_hinge_terminal_full_scan_binding_v1'
    && binding.sourcePose === 'blocking_evaluate_point_pose'
    && binding.requestIdentityBound === true
    && binding.identity.revisionBinding
      === 'project_response_source_equal_v1'
    && binding.identity.projectId === evidence.projectId
    && binding.identity.revision === evidence.revision
    && binding.identity.fixedFaceId === evidence.fixedFaceId
    && binding.identity.selectedHingeEdgeId
      === evidence.selectedHingeEdgeId
    && request.contextKey === evidence.contextKey
    && request.sourcePoseRequestKey === evidence.sourcePoseRequestKey
    && request.generation === evidence.generation
    && request.requestSequence === evidence.requestSequence
    && binding.collisionThickness === evidence.collisionThickness
}

function eligibleTwoBodyBinding(
  binding: FoldPreviewTreeTerminalFullScanBinding,
) {
  const safety = binding.safety
  return binding.evidence.kind === 'complete'
    && binding.evidence.witnessSamples.length > 0
    && safety.nonAdjacentScopeOnly === true
    && safety.hingeAdjacentPairsIncluded === false
    && safety.allWitnessesCrossPartition === true
    && safety.sameBodyWitnessCount === 0
    && safety.twoBodyTranslationInputEligible === true
    && safety.wholeSceneConstraintsRepresented === false
    && safety.legalCorrectionPoseGenerated === false
    && safety.staticCandidateRevalidated === false
    && safety.continuousCandidatePathCertified === false
    && safety.autoApplicable === false
}

function snapshotEvidence(
  value: unknown,
): FoldPreviewTreeSingleHingeCorrectionAnalysisRequestEvidence | null {
  if (!isRecord(value)) return null
  const snapshot = snapshotOwnDataValues(value, [
    'projectId',
    'revision',
    'fixedFaceId',
    'selectedHingeEdgeId',
    'contextKey',
    'sourcePoseRequestKey',
    'generation',
    'requestSequence',
    'collisionThickness',
    'targetSelectedAngleDegrees',
  ])
  if (!snapshot) return null
  const {
    projectId,
    revision,
    fixedFaceId,
    selectedHingeEdgeId,
    contextKey,
    sourcePoseRequestKey,
    generation,
    requestSequence,
    collisionThickness,
    targetSelectedAngleDegrees,
  } = snapshot
  if (
    !validId(projectId)
    || !validRevision(revision)
    || !validId(fixedFaceId)
    || !validId(selectedHingeEdgeId)
    || !validKey(contextKey)
    || !validKey(sourcePoseRequestKey)
    || !validGeneration(generation)
    || !validRequestSequence(requestSequence)
    || !validPositive(collisionThickness)
    || !validAngle(targetSelectedAngleDegrees)
  ) return null
  return deepFreeze({
    projectId,
    revision,
    fixedFaceId,
    selectedHingeEdgeId,
    contextKey,
    sourcePoseRequestKey,
    generation,
    requestSequence,
    collisionThickness,
    targetSelectedAngleDegrees,
  })
}

function snapshotPolicy(
  value: unknown,
): FoldPreviewTreeSingleHingeCorrectionAnalysisPolicy | null {
  if (!isRecord(value)) return null
  const snapshot = snapshotOwnDataValues(value, [
    'version',
    'clearance',
    'maximumTranslation',
    'maximumAngleDeltaDegrees',
    'path',
  ])
  if (
    !snapshot
    || snapshot.version
      !== FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION
    || !validPositive(snapshot.clearance)
    || !validPositive(snapshot.maximumTranslation)
    || snapshot.clearance > snapshot.maximumTranslation
    || !validPositiveAngleDelta(snapshot.maximumAngleDeltaDegrees)
    || !isRecord(snapshot.path)
  ) return null
  const path = snapshotOwnDataValues(snapshot.path, [
    'maxDepth',
    'maxIntervalTests',
    'minTimeSpan',
    'maxIntervalPairVisits',
    'maxPointTriangleTests',
  ])
  if (!path) return null
  if (
    !Number.isSafeInteger(path.maxDepth)
    || (path.maxDepth as number) < 0
    || (path.maxDepth as number) > 52
    || !Number.isSafeInteger(path.maxIntervalTests)
    || (path.maxIntervalTests as number) <= 0
    || (path.maxIntervalTests as number) > MAX_INTERVAL_TESTS
    || typeof path.minTimeSpan !== 'number'
    || !Number.isFinite(path.minTimeSpan)
    || path.minTimeSpan <= 0
    || path.minTimeSpan > 1
    || !Number.isSafeInteger(path.maxIntervalPairVisits)
    || (path.maxIntervalPairVisits as number) <= 0
    || (path.maxIntervalPairVisits as number)
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
    || !Number.isSafeInteger(path.maxPointTriangleTests)
    || (path.maxPointTriangleTests as number) <= 0
    || (path.maxPointTriangleTests as number)
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
  ) return null
  return deepFreeze({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_POLICY_VERSION,
    clearance: snapshot.clearance,
    maximumTranslation: snapshot.maximumTranslation,
    maximumAngleDeltaDegrees: snapshot.maximumAngleDeltaDegrees,
    path: {
      maxDepth: path.maxDepth as number,
      maxIntervalTests: path.maxIntervalTests as number,
      minTimeSpan: path.minTimeSpan,
      maxIntervalPairVisits: path.maxIntervalPairVisits as number,
      maxPointTriangleTests: path.maxPointTriangleTests as number,
    },
  })
}

function isAuthenticMotionContext(
  value: unknown,
): value is FoldPreviewTreeMotionContext {
  return replaceFoldPreviewTreeMotionSelectedAngle(
    value as FoldPreviewTreeMotionContext,
    0,
  ) !== null
}

function sameCompleteAngleVectorSets(
  value: unknown,
  expected: FoldPreviewTreeTerminalFullScanBinding['angleVectors'],
) {
  if (!isRecord(value)) return false
  const vectors = snapshotOwnDataValues(
    value,
    ['start', 'target', 'sample'],
  )
  if (!vectors) return false
  return sameCompleteAngles(vectors.start, expected.start)
    && sameCompleteAngles(vectors.target, expected.target)
    && sameCompleteAngles(vectors.sample, expected.sample)
}

function sameCompleteAngles(
  first: unknown,
  second: readonly FoldPreviewHingeAngle[],
) {
  const firstValues = snapshotArrayValues(first, second.length)
  if (!firstValues) return false
  const byEdgeId = new Map<string, number>()
  for (const value of firstValues) {
    if (!isRecord(value)) return false
    const angle = snapshotOwnDataValues(
      value,
      ['edgeId', 'angleDegrees'],
    )
    if (!angle) return false
    const edgeId = angle.edgeId
    const angleDegrees = angle.angleDegrees
    if (
      !validId(edgeId)
      || !validAngle(angleDegrees)
      || byEdgeId.has(edgeId)
    ) return false
    byEdgeId.set(edgeId, angleDegrees)
  }
  if (byEdgeId.size !== second.length) return false
  const seen = new Set<string>()
  for (const value of second) {
    if (
      !validId(value.edgeId)
      || !validAngle(value.angleDegrees)
      || seen.has(value.edgeId)
      || byEdgeId.get(value.edgeId) !== value.angleDegrees
    ) return false
    seen.add(value.edgeId)
  }
  return true
}

function selectedAngle(
  angles: readonly FoldPreviewHingeAngle[],
  selectedHingeEdgeId: string,
) {
  const matches = angles.filter(
    (angle) => angle.edgeId === selectedHingeEdgeId,
  )
  return matches.length === 1 && validAngle(matches[0].angleDegrees)
    ? matches[0].angleDegrees
    : null
}

function snapshotStats(value: unknown) {
  if (!isRecord(value)) return null
  const stats = snapshotOwnDataValues(value, [
    'intervalTests',
    'pointTests',
    'pointCacheHits',
    'maximumDepthReached',
  ])
  if (
    !stats
    || !validCount(stats.intervalTests)
    || !validCount(stats.pointTests)
    || !validCount(stats.pointCacheHits)
    || !validCount(stats.maximumDepthReached)
  ) return null
  return stats
}

function snapshotBracket(
  value: unknown,
): readonly [number, number] | null {
  const values = snapshotArrayValues(value, 2)
  if (
    !values
    || !validUnitTime(values[0])
    || !validUnitTime(values[1])
    || values[0] > values[1]
    || (values[0] === values[1] && values[0] !== 0)
  ) return null
  return Object.freeze([values[0], values[1]])
}

function interpolate(start: number, target: number, time: number) {
  const value = start + (target - start) * time
  return Number.isFinite(value) ? value : null
}

const MISSING = Symbol('missing')

function ownDataValue(
  value: object,
  key: PropertyKey,
): unknown | typeof MISSING {
  const descriptor = Object.getOwnPropertyDescriptor(value, key)
  return descriptor && Object.hasOwn(descriptor, 'value')
    ? descriptor.value
    : MISSING
}

/**
 * Reads every untrusted field exactly once from its own data descriptor.
 * Callers use only this detached scalar/reference table after this boundary.
 */
function snapshotOwnDataValues(
  value: object,
  keys: readonly PropertyKey[],
) {
  const snapshot: Record<PropertyKey, unknown> = Object.create(null)
  for (const key of keys) {
    const field = ownDataValue(value, key)
    if (field === MISSING) return null
    snapshot[key] = field
  }
  return Object.freeze(snapshot)
}

function snapshotArrayValues(
  value: unknown,
  expectedLength?: number,
): readonly unknown[] | null {
  if (!Array.isArray(value)) return null
  const rawLength = ownDataValue(value, 'length')
  if (
    !Number.isSafeInteger(rawLength)
    || (rawLength as number) < 0
    || (rawLength as number) > MAX_SNAPSHOT_ARRAY_LENGTH
    || (expectedLength !== undefined && rawLength !== expectedLength)
  ) return null
  const length = rawLength as number
  const snapshot: unknown[] = []
  for (let index = 0; index < length; index += 1) {
    const field = ownDataValue(value, String(index))
    if (field === MISSING) return null
    snapshot.push(field)
  }
  return Object.freeze(snapshot)
}

function isFrozenPlainRecord(value: unknown): value is DataRecord {
  if (!isRecord(value) || !Object.isFrozen(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_ID_LENGTH
    && value.trim().length > 0
}

function validKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_REQUEST_KEY_LENGTH
}

function validRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validGeneration(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validRequestSequence(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0
}

function validCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validPositive(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function validPositiveAngleDelta(value: unknown): value is number {
  return validPositive(value) && value <= 180
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validUnitTime(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
}

function validNonTerminalTime(value: unknown): value is number {
  return validUnitTime(value) && value < 1
}

function isRecord(value: unknown): value is DataRecord {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

function deepFreeze<T>(value: T, seen = new WeakSet<object>()): T {
  if (typeof value !== 'object' || value === null) return value
  const object = value as object
  // Authentic contexts and terminal bindings are already deeply frozen.
  // Skipping them prevents a second traversal of their bounded large models
  // and terminal witness evidence.
  if (Object.isFrozen(object) || seen.has(object)) return value
  seen.add(object)
  for (const key of Reflect.ownKeys(object)) {
    deepFreeze(
      (object as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
  return Object.freeze(value)
}
