import type {
  FoldPreviewContinuousMotionRunnerState,
} from './foldPreviewContinuousMotionRunner'
import type {
  FoldPreviewHingeAngle,
} from './foldPreviewKinematics'
import type { Locale } from './i18n.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'

const TREE_BLOCKING_SAMPLE_VERSION =
  'tree_single_hinge_blocking_sample_v1'
const TREE_BLOCKING_SAMPLE_REQUEST_KEY_LIMIT = 8 * 1_024 * 1_024
const TREE_BLOCKING_SAMPLE_ANGLE_LIMIT = 10_000
const TREE_BLOCKING_SAMPLE_WITNESS_LIMIT = 16
const TREE_BLOCKING_SAMPLE_SUPPORT_LIMIT = 4
const TREE_BLOCKING_SAMPLE_POSITION_CANDIDATE_LIMIT = 16
const TREE_BLOCKING_SAMPLE_TRIANGLE_INDEX_LIMIT = 1_000_000
const TREE_BLOCKING_SAMPLE_UNIT_VECTOR_TOLERANCE = 1e-9
const MOTION_FACE_LABEL_LIMIT = 10_000

export type FoldPreviewMotionFaceLabel = Readonly<{
  id: string
  number: number
  label: string
}>

export type FoldPreviewMotionPath = Readonly<{
  startDegrees: number
  requestedDegrees: number
  direction: 'increasing' | 'decreasing' | 'stationary'
}>

export type FoldPreviewMotionPathBracket = Readonly<{
  progress: readonly [number, number]
  anglesInPathOrder: readonly [number, number]
}>

export type FoldPreviewMotionCertification =
  | Readonly<{ kind: 'none'; displayDegrees: number }>
  | Readonly<{ kind: 'start_point_only'; displayDegrees: number }>
  | Readonly<{
      kind: 'interval'
      throughProgress: number
      throughDegrees: number
    }>

export type FoldPreviewMotionDetailRow = Readonly<{
  label: string
  value: string
  kind: 'user' | 'diagnostic'
}>

export type FoldPreviewTreeBlockingSampleDetailContext = Readonly<{
  projectId: string
  revision: number
  fixedFaceId: string
  selectedHingeEdgeId: string
  contextKey: string
  sourcePoseRequestKey: string
  generation: number
  requestSequence: number
  collisionThickness: number
  startAngles: readonly FoldPreviewHingeAngle[]
  targetSelectedAngleDegrees: number
}>

export type FoldPreviewMotionBlockingEvidence = Readonly<{
  unsafeAnalysisDegrees: number
  firstTriangleNumber: number
  secondTriangleNumber: number
  positionCandidateCount: number
  normal: Readonly<{
    x: number
    y: number
    z: number
    uniqueness: 'unique' | 'one_of_multiple'
  }>
  escapeDistance: number
  coverage: Readonly<{
    eligiblePairCount: number
    attemptedPairCount: number
    capturedPairCount: number
    unavailablePairCount: number
    omittedByLimitCount: number
    authoritativePairScanComplete: boolean
  }>
  safety: Readonly<{
    sampleTransformsAppliedToScene: false
    scope: 'selected_triangle_prism_pair_only'
    autoApplicable: false
  }>
}>

export type FoldPreviewContinuousMotionDetail = Readonly<{
  kind: 'blocked' | 'indeterminate'
  title: string
  path: FoldPreviewMotionPath
  displayDegrees: number
  certification: FoldPreviewMotionCertification
  bracket: FoldPreviewMotionPathBracket | null
  summaryText: string
  rows: readonly FoldPreviewMotionDetailRow[]
  resultKind: 'blocked' | 'indeterminate' | 'runner_failure'
  certifiedSafeThrough: number | null
  reasonCode: string
  firstFaceNumber: number | null
  secondFaceNumber: number | null
  relation: 'hinge_adjacent' | 'non_adjacent' | null
  geometryClass: 'touching' | 'penetrating' | null
  hingeDecision:
    | 'outside_hinge_penetration'
    | 'outside_hinge_contact'
    | null
  blockingEvidence: FoldPreviewMotionBlockingEvidence | null
}>

type MotionStats = Readonly<{
  intervalTests: number
  pointTests: number
  pointCacheHits: number
  maximumDepthReached: number
}>

type NormalizedBlocker = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  geometryClass: 'touching' | 'penetrating'
  hingeDecision:
    | 'outside_hinge_penetration'
    | 'outside_hinge_contact'
    | null
}>

type BlockingEvidenceTerminal = Readonly<{
  blockingSampleTime: number
  blocker: unknown
}>

type NormalizedBlockingContext = Readonly<{
  projectId: string
  revision: number
  fixedFaceId: string
  selectedHingeEdgeId: string
  contextKey: string
  sourcePoseRequestKey: string
  generation: number
  requestSequence: number
  collisionThickness: number
  startAngles: readonly FoldPreviewHingeAngle[]
  targetSelectedAngleDegrees: number
}>

type NormalizedWitnessSample = Readonly<{
  firstFaceId: string
  secondFaceId: string
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  positionCandidateCount: number
  normal: FoldPreviewMotionBlockingEvidence['normal']
  escapeDistance: number
}>

const RUNNER_REASONS = new Set([
  'invalid_target_angle',
  'job_factory_error',
  'job_factory_returned_null',
  'job_factory_returned_malformed_job',
  'scheduler_error',
  'job_step_error',
  'malformed_job_step',
  'non_monotonic_certified_time',
  'angle_interpolation_error',
  'apply_angle_error',
  'apply_angle_rejected',
])

const CORE_REASONS = new Set([
  'point_callback_error',
  'interval_callback_error',
  'malformed_point_decision',
  'malformed_interval_decision',
  'invalid_work_budget',
  'work_limit',
  'chronology_error',
  'contradictory_interval_certificate',
  'uncertified_interval',
  'numerical_subdivision',
  'missing_target_validation',
  'invalid_interpolated_angle',
  'pose_unavailable',
  'point_collision_unavailable',
  'hinge_decision_unavailable',
  'non_adjacent_geometry_indeterminate',
  'invalid_interpolated_interval',
  'hinge_interval_numerical_margin',
  'midpoint_pose_unavailable',
  'swept_bounds_unavailable',
])

const HINGE_REASON_SUFFIXES = new Set([
  'zero_thickness',
  'missing_constraint',
  'multiple_shared_hinges',
  'pose_mismatch',
  'unsupported_flat_fold',
  'layer_offset_unmodeled',
  'numerical_geometry',
  'corridor_boundary',
  'non_hinge_triangle',
  'incomplete_pair_scan',
  'pair_geometry_mismatch',
  'flat_pose_penetration',
])

/**
 * Converts one terminal single-fold or selected-tree-hinge runner snapshot
 * into immutable,
 * user-facing detail. Any inconsistent terminal contract returns `null`
 * instead of exposing a misleading partial explanation.
 */
export function describeFoldPreviewContinuousMotionDetail(
  state: FoldPreviewContinuousMotionRunnerState<unknown> | null,
  faceLabels: readonly FoldPreviewMotionFaceLabel[] = [],
  blockingSampleContext: FoldPreviewTreeBlockingSampleDetailContext | null =
    null,
  locale: Locale = 'ja',
): FoldPreviewContinuousMotionDetail | null {
  try {
    const start = state?.start
    const applied = state?.applied
    const requested = state?.requested
    const status = state?.status
    const reason = state?.reason
    const result = state?.result
    if (
      !state
      || !validAngle(start)
      || !validAngle(applied)
      || !validAngle(requested)
    ) return null
    const path = freezePath(start, requested)
    const labels = normalizeFaceLabels(faceLabels)

    if (status === 'blocked') {
      if (reason !== 'motion_blocked') return null
      const terminal = normalizeBlockedResult(result)
      if (
        !terminal
        || applied !== angleAt(path, terminal.certifiedSafeThrough)
      ) return null
      const blocker = normalizeBlocker(terminal.blocker)
      const blockingEvidence = safeNormalizeBlockingEvidence(
        terminal,
        blocker,
        path,
        blockingSampleContext,
      )
      return blockedDetail(
        path,
        applied,
        terminal,
        blocker,
        blockingEvidence,
        labels,
        locale,
      )
    }

    if (status !== 'indeterminate' || !validReason(reason)) return null
    if (result === null) {
      return runnerFailureDetail(path, applied, reason, locale)
    }
    const terminal = normalizeIndeterminateResult(result)
    if (
      !terminal
      || reason !== terminal.reason
      || applied !== angleAt(path, terminal.certifiedSafeThrough)
    ) return null
    return indeterminateDetail(path, applied, terminal, locale)
  } catch {
    return null
  }
}

function blockedDetail(
  path: FoldPreviewMotionPath,
  displayDegrees: number,
  terminal: Readonly<{
    certifiedSafeThrough: number
    bracket: readonly [number, number]
    blocker: unknown
    blockingSampleTime: number
    stats: MotionStats
  }>,
  blocker: NormalizedBlocker | null,
  blockingEvidence: FoldPreviewMotionBlockingEvidence | null,
  labels: ReadonlyMap<string, FoldPreviewMotionFaceLabel>,
  locale: Locale,
): FoldPreviewContinuousMotionDetail {
  const bracket = pathBracket(path, terminal.bracket)
  const certification = certificationFor(
    displayDegrees,
    terminal.certifiedSafeThrough,
    terminal.bracket,
  )
  const firstFace = blocker ? labels.get(blocker.firstFaceId) ?? null : null
  const secondFace = blocker ? labels.get(blocker.secondFaceId) ?? null : null
  const faceText = firstFace && secondFace
    ? `${firstFace.label} ↔ ${secondFace.label}`
    : localized(
        locale,
        '対象面の対応を確認できません',
        'The affected faces could not be matched.',
      )
  const classification = blocker
    ? describeBlocker(blocker, locale)
    : localized(
        locale,
        '衝突姿勢を検出しましたが、相互作用の詳細は取得できません',
        'A collision pose was detected, but interaction details are unavailable.',
      )
  const intervalText = bracket.progress[0] === bracket.progress[1]
    ? `${formatAngle(bracket.anglesInPathOrder[0])}°`
    : `${formatAngle(bracket.anglesInPathOrder[0])}° → ${formatAngle(bracket.anglesInPathOrder[1])}°`
  const rows = freezeRows([
    userRow(
      localized(locale, '開始角', 'Starting angle'),
      `${formatAngle(path.startDegrees)}°`,
    ),
    userRow(
      localized(locale, '指定角', 'Requested angle'),
      `${formatAngle(path.requestedDegrees)}°`,
    ),
    userRow(
      localized(locale, '実表示角', 'Actual displayed angle'),
      `${formatAngle(displayDegrees)}°`,
    ),
    userRow(
      bracket.progress[0] === bracket.progress[1]
        ? localized(locale, '衝突検出角度', 'Collision-detection angle')
        : localized(
            locale,
            '衝突姿勢を含む探索角度範囲',
            'Search-angle range containing a collision pose',
          ),
      intervalText,
    ),
    userRow(localized(locale, '対象面ペア', 'Affected face pair'), faceText),
    userRow(localized(locale, '分類', 'Classification'), classification),
    ...(blockingEvidence
      ? describeBlockingEvidenceRows(
          blockingEvidence,
          firstFace,
          secondFace,
          locale,
        )
      : []),
    diagnosticRow(
      localized(locale, '経路進捗', 'Path progress'),
      localized(
        locale,
        `${formatProgress(terminal.certifiedSafeThrough, locale)} まで確認`,
        `Verified through ${formatProgress(terminal.certifiedSafeThrough, locale)}`,
      ),
    ),
    diagnosticRow(
      localized(locale, '内部診断コード', 'Internal diagnostic code'),
      'motion_blocked',
    ),
    diagnosticRow(
      localized(locale, '判定量', 'Check counts'),
      describeStats(terminal.stats, locale),
    ),
  ])
  const summaryText = summarizeRows(rows, locale)
  return Object.freeze({
    kind: 'blocked',
    title: terminal.bracket[0] === 0 && terminal.bracket[1] === 0
      ? localized(
          locale,
          '開始姿勢の衝突詳細',
          'Starting-pose collision details',
        )
      : localized(
          locale,
          '移動経路の停止詳細',
          'Motion-path stop details',
        ),
    path,
    displayDegrees,
    certification,
    bracket,
    summaryText,
    rows,
    resultKind: 'blocked',
    certifiedSafeThrough: terminal.certifiedSafeThrough,
    reasonCode: 'motion_blocked',
    firstFaceNumber: firstFace?.number ?? null,
    secondFaceNumber: secondFace?.number ?? null,
    relation: blocker?.relation ?? null,
    geometryClass: blocker?.geometryClass ?? null,
    hingeDecision: blocker?.hingeDecision ?? null,
    blockingEvidence,
  })
}

function indeterminateDetail(
  path: FoldPreviewMotionPath,
  displayDegrees: number,
  terminal: Readonly<{
    certifiedSafeThrough: number
    bracket: readonly [number, number]
    reason: string
    stats: MotionStats
  }>,
  locale: Locale,
): FoldPreviewContinuousMotionDetail {
  const bracket = pathBracket(path, terminal.bracket)
  const certification = certificationFor(
    displayDegrees,
    terminal.certifiedSafeThrough,
    terminal.bracket,
  )
  const reasonCode = knownReasonCode(terminal.reason)
  const intervalText = bracket.progress[0] === bracket.progress[1]
    ? `${formatAngle(bracket.anglesInPathOrder[0])}°`
    : `${formatAngle(bracket.anglesInPathOrder[0])}° → ${formatAngle(bracket.anglesInPathOrder[1])}°`
  const rows = freezeRows([
    userRow(
      localized(locale, '開始角', 'Starting angle'),
      `${formatAngle(path.startDegrees)}°`,
    ),
    userRow(
      localized(locale, '指定角', 'Requested angle'),
      `${formatAngle(path.requestedDegrees)}°`,
    ),
    userRow(
      localized(locale, '実表示角', 'Actual displayed angle'),
      `${formatAngle(displayDegrees)}°`,
    ),
    userRow(
      bracket.progress[0] === bracket.progress[1]
        ? localized(locale, '判定不能角度', 'Indeterminate angle')
        : localized(
            locale,
            '安全を確認できない角度範囲',
            'Angle range whose safety could not be verified',
          ),
      intervalText,
    ),
    userRow(
      localized(locale, '停止理由', 'Stop reason'),
      describeReason(reasonCode, locale),
    ),
    diagnosticRow(
      localized(locale, '経路進捗', 'Path progress'),
      localized(
        locale,
        `${formatProgress(terminal.certifiedSafeThrough, locale)} まで確認`,
        `Verified through ${formatProgress(terminal.certifiedSafeThrough, locale)}`,
      ),
    ),
    diagnosticRow(
      localized(locale, '内部診断コード', 'Internal diagnostic code'),
      reasonCode,
    ),
    diagnosticRow(
      localized(locale, '判定量', 'Check counts'),
      describeStats(terminal.stats, locale),
    ),
  ])
  const summaryText = summarizeRows(rows, locale)
  return Object.freeze({
    kind: 'indeterminate',
    title: terminal.bracket[0] === 0 && terminal.bracket[1] === 0
      ? localized(
          locale,
          '開始姿勢の判定不能詳細',
          'Starting-pose indeterminate details',
        )
      : localized(
          locale,
          '移動経路の判定停止詳細',
          'Motion-path indeterminate stop details',
        ),
    path,
    displayDegrees,
    certification,
    bracket,
    summaryText,
    rows,
    resultKind: 'indeterminate',
    certifiedSafeThrough: terminal.certifiedSafeThrough,
    reasonCode,
    firstFaceNumber: null,
    secondFaceNumber: null,
    relation: null,
    geometryClass: null,
    hingeDecision: null,
    blockingEvidence: null,
  })
}

function runnerFailureDetail(
  path: FoldPreviewMotionPath,
  displayDegrees: number,
  rawReason: string,
  locale: Locale,
): FoldPreviewContinuousMotionDetail {
  const reasonCode = knownReasonCode(rawReason)
  const rows = freezeRows([
    userRow(
      localized(locale, '開始角', 'Starting angle'),
      `${formatAngle(path.startDegrees)}°`,
    ),
    userRow(
      localized(locale, '指定角', 'Requested angle'),
      `${formatAngle(path.requestedDegrees)}°`,
    ),
    userRow(
      localized(locale, '保持中の表示角', 'Held displayed angle'),
      `${formatAngle(displayDegrees)}°`,
    ),
    userRow(
      localized(locale, '停止理由', 'Stop reason'),
      describeReason(reasonCode, locale),
    ),
    diagnosticRow(
      localized(locale, '内部診断コード', 'Internal diagnostic code'),
      reasonCode,
    ),
  ])
  const summaryText = summarizeRows(rows, locale)
  return Object.freeze({
    kind: 'indeterminate',
    title: localized(
      locale,
      '移動経路を開始できない理由',
      'Why the motion path could not start',
    ),
    path,
    displayDegrees,
    certification: Object.freeze({
      kind: 'none',
      displayDegrees,
    }),
    bracket: null,
    summaryText,
    rows,
    resultKind: 'runner_failure',
    certifiedSafeThrough: null,
    reasonCode,
    firstFaceNumber: null,
    secondFaceNumber: null,
    relation: null,
    geometryClass: null,
    hingeDecision: null,
    blockingEvidence: null,
  })
}

function normalizeBlockedResult(result: unknown) {
  if (!result || typeof result !== 'object') return null
  const record = result as Record<string, unknown>
  const kind = record.kind
  const certifiedSafeThrough = record.certifiedSafeThrough
  const stopTime = record.stopTime
  const unsafeBracket = snapshotBracket(record.unsafeBracket)
  const blockingSampleTime = record.blockingSampleTime
  if (
    kind !== 'blocked'
    || !validNonTerminalTime(certifiedSafeThrough)
    || stopTime !== certifiedSafeThrough
    || !unsafeBracket
    || unsafeBracket[0] !== certifiedSafeThrough
    || !validUnitTime(blockingSampleTime)
    || blockingSampleTime !== unsafeBracket[1]
  ) return null
  const stats = normalizeStats(record.stats)
  if (!stats) return null
  return Object.freeze({
    certifiedSafeThrough,
    bracket: unsafeBracket,
    blockingSampleTime,
    blocker: Object.hasOwn(record, 'blocker') ? record.blocker : null,
    stats,
  })
}

function normalizeIndeterminateResult(result: unknown) {
  if (!result || typeof result !== 'object') return null
  const record = result as Record<string, unknown>
  const kind = record.kind
  const certifiedSafeThrough = record.certifiedSafeThrough
  const stopTime = record.stopTime
  const unresolvedBracket = snapshotBracket(record.unresolvedBracket)
  const reason = record.reason
  if (
    kind !== 'indeterminate'
    || !validNonTerminalTime(certifiedSafeThrough)
    || stopTime !== certifiedSafeThrough
    || !unresolvedBracket
    || unresolvedBracket[0] !== certifiedSafeThrough
    || !validReason(reason)
  ) return null
  const stats = normalizeStats(record.stats)
  if (!stats) return null
  return Object.freeze({
    certifiedSafeThrough,
    bracket: unresolvedBracket,
    reason,
    stats,
  })
}

function normalizeBlocker(value: unknown): NormalizedBlocker | null {
  if (!value || typeof value !== 'object') return null
  const blocker = value as Record<string, unknown>
  const firstFaceId = blocker.firstFaceId
  const secondFaceId = blocker.secondFaceId
  const relation = blocker.relation
  const geometryClass = blocker.geometryClass
  const hasHingeDecisionKind = Object.hasOwn(
    blocker,
    'hingeDecisionKind',
  )
  const hingeDecisionKind = hasHingeDecisionKind
    ? blocker.hingeDecisionKind
    : undefined
  if (
    !validId(firstFaceId)
    || !validId(secondFaceId)
    || firstFaceId === secondFaceId
  ) return null
  if (relation === 'non_adjacent') {
    if (
      (geometryClass !== 'touching'
        && geometryClass !== 'penetrating')
      || hasHingeDecisionKind
    ) return null
    return Object.freeze({
      firstFaceId,
      secondFaceId,
      relation: 'non_adjacent',
      geometryClass,
      hingeDecision: null,
    })
  }
  if (relation !== 'hinge_adjacent') return null
  if (
    hingeDecisionKind === 'outside_hinge_penetration'
    && geometryClass === 'penetrating'
  ) {
    return Object.freeze({
      firstFaceId,
      secondFaceId,
      relation: 'hinge_adjacent',
      geometryClass: 'penetrating',
      hingeDecision: 'outside_hinge_penetration',
    })
  }
  if (
    hingeDecisionKind === 'outside_hinge_contact'
    && geometryClass === 'touching'
  ) {
    return Object.freeze({
      firstFaceId,
      secondFaceId,
      relation: 'hinge_adjacent',
      geometryClass: 'touching',
      hingeDecision: 'outside_hinge_contact',
    })
  }
  return null
}

function safeNormalizeBlockingEvidence(
  terminal: BlockingEvidenceTerminal,
  blocker: NormalizedBlocker | null,
  path: FoldPreviewMotionPath,
  context: FoldPreviewTreeBlockingSampleDetailContext | null,
): FoldPreviewMotionBlockingEvidence | null {
  try {
    return normalizeBlockingEvidence(terminal, blocker, path, context)
  } catch {
    // Unsafe-pose explanation is optional. Never weaken or hide the block
    // when hostile, stale, or malformed evidence cannot be explained.
    return null
  }
}

function normalizeBlockingEvidence(
  terminal: BlockingEvidenceTerminal,
  blocker: NormalizedBlocker | null,
  path: FoldPreviewMotionPath,
  rawContext: FoldPreviewTreeBlockingSampleDetailContext | null,
): FoldPreviewMotionBlockingEvidence | null {
  if (!blocker || blocker.relation !== 'non_adjacent') return null
  const context = snapshotBlockingContext(rawContext)
  if (
    !context
    || context.targetSelectedAngleDegrees !== path.requestedDegrees
    || !validUnitTime(terminal.blockingSampleTime)
  ) return null
  const selectedStart = context.startAngles.find(
    (angle) => angle.edgeId === context.selectedHingeEdgeId,
  )?.angleDegrees
  if (selectedStart !== path.startDegrees) return null

  const rawBlocker = terminal.blocker
  if (!isRecord(rawBlocker) || Array.isArray(rawBlocker)) return null
  const rawFirstFaceId = rawBlocker.firstFaceId
  const rawSecondFaceId = rawBlocker.secondFaceId
  const rawRelation = rawBlocker.relation
  const rawGeometryClass = rawBlocker.geometryClass
  if (
    rawFirstFaceId !== blocker.firstFaceId
    || rawSecondFaceId !== blocker.secondFaceId
    || rawRelation !== blocker.relation
    || rawGeometryClass !== blocker.geometryClass
    || !Object.hasOwn(rawBlocker, 'blockingSample')
  ) return null

  const rawSample = rawBlocker.blockingSample
  if (!isRecord(rawSample) || Array.isArray(rawSample)) return null
  const blockingSampleTime = rawSample.blockingSampleTime
  const selectedAngleDegrees = rawSample.selectedAngleDegrees
  const expectedSelectedAngle = angleAt(path, terminal.blockingSampleTime)
  if (
    rawSample.version !== TREE_BLOCKING_SAMPLE_VERSION
    || rawSample.sourcePose !== 'blocking_evaluate_point_pose'
    || blockingSampleTime !== terminal.blockingSampleTime
    || selectedAngleDegrees !== expectedSelectedAngle
    || rawSample.collisionThickness !== context.collisionThickness
    || !identityMatchesContext(rawSample.identity, context)
    || !angleVectorsMatchContext(
      rawSample.angleVectors,
      context,
      expectedSelectedAngle,
    )
    || !validBlockingFaceTransforms(
      rawSample.faceTransforms,
      blocker.firstFaceId,
      blocker.secondFaceId,
    )
  ) return null

  const witnesses = normalizeWitnessSamples(rawSample.witnessSamples)
  if (!witnesses) return null
  const coverage = normalizeWitnessCoverage(
    rawSample.witnessCoverage,
    witnesses.length,
  )
  if (!coverage) return null
  const expectedPrimaryIndex = witnesses.findIndex(
    (witness) =>
      witness.firstFaceId === blocker.firstFaceId
      && witness.secondFaceId === blocker.secondFaceId
      && witness.geometryClass === blocker.geometryClass,
  )
  const primaryWitnessIndex = rawSample.primaryWitnessIndex
  if (
    expectedPrimaryIndex < 0
    || !Number.isSafeInteger(primaryWitnessIndex)
    || primaryWitnessIndex !== expectedPrimaryIndex
  ) return null
  const primary = witnesses[expectedPrimaryIndex]
  if (!primary) return null

  return Object.freeze({
    unsafeAnalysisDegrees: selectedAngleDegrees,
    firstTriangleNumber: primary.firstTriangleIndex + 1,
    secondTriangleNumber: primary.secondTriangleIndex + 1,
    positionCandidateCount: primary.positionCandidateCount,
    normal: primary.normal,
    escapeDistance: primary.escapeDistance,
    coverage,
    safety: Object.freeze({
      sampleTransformsAppliedToScene: false,
      scope: 'selected_triangle_prism_pair_only',
      autoApplicable: false,
    }),
  })
}

function snapshotBlockingContext(
  value: FoldPreviewTreeBlockingSampleDetailContext | null,
): NormalizedBlockingContext | null {
  if (!isRecord(value) || Array.isArray(value)) return null
  const projectId = value.projectId
  const revision = value.revision
  const fixedFaceId = value.fixedFaceId
  const selectedHingeEdgeId = value.selectedHingeEdgeId
  const contextKey = value.contextKey
  const sourcePoseRequestKey = value.sourcePoseRequestKey
  const generation = value.generation
  const requestSequence = value.requestSequence
  const collisionThickness = value.collisionThickness
  const targetSelectedAngleDegrees = value.targetSelectedAngleDegrees
  const startAngles = snapshotAngleVector(value.startAngles)
  if (
    !validEvidenceId(projectId)
    || !validRevision(revision)
    || !validEvidenceId(fixedFaceId)
    || !validEvidenceId(selectedHingeEdgeId)
    || !validBoundedKey(contextKey)
    || !validBoundedKey(sourcePoseRequestKey)
    || !validGeneration(generation)
    || !validRequestSequence(requestSequence)
    || !validCollisionThickness(collisionThickness)
    || !validAngle(targetSelectedAngleDegrees)
    || !startAngles
    || startAngles.filter(
      (angle) => angle.edgeId === selectedHingeEdgeId,
    ).length !== 1
  ) return null
  const recomputedSourcePoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      {
        projectId,
        revision,
        kind: 'fold_graph',
      },
      fixedFaceId,
      collisionThickness,
      startAngles,
    )
  if (recomputedSourcePoseRequestKey !== sourcePoseRequestKey) return null
  return Object.freeze({
    projectId,
    revision,
    fixedFaceId,
    selectedHingeEdgeId,
    contextKey,
    sourcePoseRequestKey,
    generation,
    requestSequence,
    collisionThickness,
    startAngles,
    targetSelectedAngleDegrees,
  })
}

function snapshotAngleVector(value: unknown) {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length === 0
    || length > TREE_BLOCKING_SAMPLE_ANGLE_LIMIT
  ) return null
  const seenEdgeIds = new Set<string>()
  const result: FoldPreviewHingeAngle[] = []
  for (let index = 0; index < length; index += 1) {
    const rawAngle = value[index]
    if (!isRecord(rawAngle) || Array.isArray(rawAngle)) return null
    const edgeId = rawAngle.edgeId
    const angleDegrees = rawAngle.angleDegrees
    if (
      !validEvidenceId(edgeId)
      || seenEdgeIds.has(edgeId)
      || !validAngle(angleDegrees)
    ) return null
    seenEdgeIds.add(edgeId)
    result.push(Object.freeze({ edgeId, angleDegrees }))
  }
  return Object.freeze(result)
}

function identityMatchesContext(
  value: unknown,
  context: NormalizedBlockingContext,
) {
  if (!isRecord(value) || Array.isArray(value)) return false
  const request = value.request
  if (!isRecord(request) || Array.isArray(request)) return false
  return value.projectId === context.projectId
    && value.revision === context.revision
    && value.revisionBinding === 'project_response_source_equal_v1'
    && value.fixedFaceId === context.fixedFaceId
    && value.selectedHingeEdgeId === context.selectedHingeEdgeId
    && request.contextKey === context.contextKey
    && request.sourcePoseRequestKey === context.sourcePoseRequestKey
    && request.generation === context.generation
    && request.requestSequence === context.requestSequence
}

function angleVectorsMatchContext(
  value: unknown,
  context: NormalizedBlockingContext,
  selectedSampleAngle: number,
) {
  if (!isRecord(value) || Array.isArray(value)) return false
  const start = snapshotAngleVector(value.start)
  const target = snapshotAngleVector(value.target)
  const sample = snapshotAngleVector(value.sample)
  if (
    !start
    || !target
    || !sample
    || start.length !== context.startAngles.length
    || target.length !== context.startAngles.length
    || sample.length !== context.startAngles.length
  ) return false
  const startByEdgeId = angleVectorMap(start)
  const targetByEdgeId = angleVectorMap(target)
  const sampleByEdgeId = angleVectorMap(sample)
  if (!startByEdgeId || !targetByEdgeId || !sampleByEdgeId) return false
  for (const expectedStart of context.startAngles) {
    const targetAngle = expectedStart.edgeId === context.selectedHingeEdgeId
      ? context.targetSelectedAngleDegrees
      : expectedStart.angleDegrees
    const sampleAngle = expectedStart.edgeId === context.selectedHingeEdgeId
      ? selectedSampleAngle
      : expectedStart.angleDegrees
    if (
      startByEdgeId.get(expectedStart.edgeId)
        !== expectedStart.angleDegrees
      || targetByEdgeId.get(expectedStart.edgeId) !== targetAngle
      || sampleByEdgeId.get(expectedStart.edgeId) !== sampleAngle
    ) return false
  }
  return true
}

function angleVectorMap(
  angles: readonly FoldPreviewHingeAngle[],
): ReadonlyMap<string, number> | null {
  const result = new Map<string, number>()
  for (let index = 0; index < angles.length; index += 1) {
    const angle = angles[index]
    if (!angle || result.has(angle.edgeId)) return null
    result.set(angle.edgeId, angle.angleDegrees)
  }
  return result.size === angles.length ? result : null
}

function validBlockingFaceTransforms(
  value: unknown,
  firstFaceId: string,
  secondFaceId: string,
) {
  if (!Array.isArray(value)) return false
  const length = value.length
  return length === 2
    && validBlockingFaceTransform(value[0], firstFaceId)
    && validBlockingFaceTransform(value[1], secondFaceId)
}

function validBlockingFaceTransform(
  value: unknown,
  expectedFaceId: string,
) {
  if (
    !isRecord(value)
    || Array.isArray(value)
    || value.faceId !== expectedFaceId
  ) return false
  const elements = value.elements
  if (!Array.isArray(elements)) return false
  const length = elements.length
  if (length !== 16) return false
  for (let index = 0; index < length; index += 1) {
    const element = elements[index]
    if (typeof element !== 'number' || !Number.isFinite(element)) {
      return false
    }
  }
  return true
}

function normalizeWitnessSamples(
  value: unknown,
): readonly NormalizedWitnessSample[] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length > TREE_BLOCKING_SAMPLE_WITNESS_LIMIT
  ) return null
  const result: NormalizedWitnessSample[] = []
  for (let index = 0; index < length; index += 1) {
    const rawSample = value[index]
    const sample = normalizeWitnessSample(rawSample)
    if (!sample) return null
    result.push(sample)
  }
  return Object.freeze(result)
}

function normalizeWitnessSample(
  value: unknown,
): NormalizedWitnessSample | null {
  if (!isRecord(value) || Array.isArray(value)) return null
  const firstFaceId = value.firstFaceId
  const secondFaceId = value.secondFaceId
  const firstTriangleIndex = value.firstTriangleIndex
  const secondTriangleIndex = value.secondTriangleIndex
  const geometryClass = value.geometryClass
  if (
    !validId(firstFaceId)
    || !validId(secondFaceId)
    || firstFaceId === secondFaceId
    || value.relation !== 'non_adjacent'
    || !validTriangleIndex(firstTriangleIndex)
    || !validTriangleIndex(secondTriangleIndex)
    || (geometryClass !== 'touching' && geometryClass !== 'penetrating')
  ) return null
  const witness = normalizeTrianglePrismWitness(
    value.witness,
    geometryClass,
  )
  if (!witness) return null
  return Object.freeze({
    firstFaceId,
    secondFaceId,
    firstTriangleIndex,
    secondTriangleIndex,
    geometryClass,
    positionCandidateCount: witness.positionCandidateCount,
    normal: witness.normal,
    escapeDistance: witness.escapeDistance,
  })
}

function normalizeTrianglePrismWitness(
  value: unknown,
  expectedGeometryClass: 'touching' | 'penetrating',
) {
  if (!isRecord(value) || Array.isArray(value)) return null
  const numericalMargin = value.numericalMargin
  const escapeDistance = value.escapeDistance
  const toleratedGap = value.toleratedGap
  if (
    value.algorithm !== 'triangle_prism_sat_witness_v1'
    || value.geometryClass !== expectedGeometryClass
    || !validNonNegativeFinite(numericalMargin)
    || !validNonNegativeFinite(escapeDistance)
    || !validNonNegativeFinite(toleratedGap)
    || toleratedGap > numericalMargin
    || (
      expectedGeometryClass === 'penetrating'
      && (escapeDistance <= 0 || toleratedGap !== 0)
    )
  ) return null

  const rawNormal = value.normal
  if (!isRecord(rawNormal) || Array.isArray(rawNormal)) return null
  const normalVector = snapshotPoint(rawNormal.vector)
  const normalLength = normalVector
    ? Math.hypot(normalVector.x, normalVector.y, normalVector.z)
    : Number.NaN
  const uniqueness = rawNormal.uniqueness
  if (
    !normalVector
    || !Number.isFinite(normalLength)
    || Math.abs(normalLength - 1)
      > TREE_BLOCKING_SAMPLE_UNIT_VECTOR_TOLERANCE
    || rawNormal.convention !== 'moves_second_away_from_first'
    || (uniqueness !== 'unique' && uniqueness !== 'one_of_multiple')
  ) return null

  const firstSupport = snapshotPoints(
    value.firstSupport,
    TREE_BLOCKING_SAMPLE_SUPPORT_LIMIT,
  )
  const secondSupport = snapshotPoints(
    value.secondSupport,
    TREE_BLOCKING_SAMPLE_SUPPORT_LIMIT,
  )
  const rawPositionRegion = value.positionRegion
  if (
    !firstSupport
    || !secondSupport
    || !isRecord(rawPositionRegion)
    || Array.isArray(rawPositionRegion)
    || rawPositionRegion.kind !== 'support_midpoint_hull_v1'
    || rawPositionRegion.sourcePose !== 'analyzed_input_pose'
  ) return null
  const positionCandidates = snapshotPoints(
    rawPositionRegion.generators,
    TREE_BLOCKING_SAMPLE_POSITION_CANDIDATE_LIMIT,
  )
  const expectedCandidateCount = firstSupport.length * secondSupport.length
  if (
    !positionCandidates
    || positionCandidates.length !== expectedCandidateCount
  ) return null
  let candidateIndex = 0
  for (const firstPoint of firstSupport) {
    for (const secondPoint of secondSupport) {
      const candidate = positionCandidates[candidateIndex]
      if (
        !candidate
        || candidate.x !== firstPoint.x / 2 + secondPoint.x / 2
        || candidate.y !== firstPoint.y / 2 + secondPoint.y / 2
        || candidate.z !== firstPoint.z / 2 + secondPoint.z / 2
      ) return null
      candidateIndex += 1
    }
  }

  const rawHint = value.localSeparationHint
  if (
    !isRecord(rawHint)
    || Array.isArray(rawHint)
    || rawHint.distance !== escapeDistance
    || rawHint.scope !== 'selected_triangle_prism_pair_only'
    || rawHint.autoApplicable !== false
  ) return null
  const translation = snapshotPoint(rawHint.translation)
  if (
    !translation
    || translation.x !== normalVector.x * escapeDistance
    || translation.y !== normalVector.y * escapeDistance
    || translation.z !== normalVector.z * escapeDistance
  ) return null

  return Object.freeze({
    positionCandidateCount: positionCandidates.length,
    normal: Object.freeze({
      x: normalVector.x,
      y: normalVector.y,
      z: normalVector.z,
      uniqueness,
    }),
    escapeDistance,
  })
}

function snapshotPoints(
  value: unknown,
  maximum: number,
) {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length === 0
    || length > maximum
  ) return null
  const result: Readonly<{ x: number; y: number; z: number }>[] = []
  for (let index = 0; index < length; index += 1) {
    const rawPoint = value[index]
    const point = snapshotPoint(rawPoint)
    if (!point) return null
    result.push(point)
  }
  return Object.freeze(result)
}

function snapshotPoint(value: unknown) {
  if (!isRecord(value) || Array.isArray(value)) return null
  const x = value.x
  const y = value.y
  const z = value.z
  return typeof x === 'number'
    && Number.isFinite(x)
    && typeof y === 'number'
    && Number.isFinite(y)
    && typeof z === 'number'
    && Number.isFinite(z)
    ? Object.freeze({ x, y, z })
    : null
}

function normalizeWitnessCoverage(
  value: unknown,
  capturedPairCount: number,
): FoldPreviewMotionBlockingEvidence['coverage'] | null {
  if (!isRecord(value) || Array.isArray(value)) return null
  const eligiblePairCount = value.eligiblePairCount
  const attemptedPairCount = value.attemptedPairCount
  const unavailablePairCount = value.unavailablePairCount
  const omittedByLimitCount = value.omittedByLimitCount
  const authoritativePairScanComplete = value.authoritativePairScanComplete
  if (
    value.scope
      !== 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1'
    || !validSafeCount(eligiblePairCount, 1_000_000)
    || !validSafeCount(attemptedPairCount, 1_000_000)
    || attemptedPairCount > TREE_BLOCKING_SAMPLE_WITNESS_LIMIT
    || !validSafeCount(unavailablePairCount, 1_000_000)
    || !validSafeCount(omittedByLimitCount, 1_000_000)
    || eligiblePairCount !== attemptedPairCount + omittedByLimitCount
    || attemptedPairCount !== capturedPairCount + unavailablePairCount
    || typeof authoritativePairScanComplete !== 'boolean'
  ) return null
  return Object.freeze({
    eligiblePairCount,
    attemptedPairCount,
    capturedPairCount,
    unavailablePairCount,
    omittedByLimitCount,
    authoritativePairScanComplete,
  })
}

function normalizeFaceLabels(
  labels: readonly FoldPreviewMotionFaceLabel[],
): ReadonlyMap<string, FoldPreviewMotionFaceLabel> {
  if (!Array.isArray(labels)) return new Map()
  const length = labels.length
  if (
    !Number.isSafeInteger(length)
    || length > MOTION_FACE_LABEL_LIMIT
  ) return new Map()
  const result = new Map<string, FoldPreviewMotionFaceLabel>()
  const ambiguous = new Set<string>()
  for (let index = 0; index < length; index += 1) {
    const label = labels[index]
    const id = label?.id
    const number = label?.number
    const text = label?.label
    if (
      !label
      || !validId(id)
      || !Number.isSafeInteger(number)
      || (number as number) <= 0
      || typeof text !== 'string'
      || text.length === 0
      || text.length > 80
    ) continue
    if (result.has(id) || ambiguous.has(id)) {
      result.delete(id)
      ambiguous.add(id)
      continue
    }
    result.set(id, Object.freeze({
      id,
      number: number as number,
      label: text,
    }))
  }
  return result
}

function normalizeStats(value: unknown): MotionStats | null {
  if (!value || typeof value !== 'object') return null
  const stats = value as Record<string, unknown>
  const intervalTests = stats.intervalTests
  const pointTests = stats.pointTests
  const pointCacheHits = stats.pointCacheHits
  const maximumDepthReached = stats.maximumDepthReached
  if (
    !validCount(intervalTests, 1_000_000)
    || !validCount(pointTests, 1_000_002)
    || !validCount(pointCacheHits, 2_000_002)
    || !validCount(maximumDepthReached, 52)
  ) return null
  return Object.freeze({
    intervalTests,
    pointTests,
    pointCacheHits,
    maximumDepthReached,
  })
}

function certificationFor(
  displayDegrees: number,
  certifiedSafeThrough: number,
  bracket: readonly [number, number],
): FoldPreviewMotionCertification {
  if (certifiedSafeThrough > 0) {
    return Object.freeze({
      kind: 'interval',
      throughProgress: certifiedSafeThrough,
      throughDegrees: displayDegrees,
    })
  }
  if (bracket[1] > 0) {
    return Object.freeze({
      kind: 'start_point_only',
      displayDegrees,
    })
  }
  return Object.freeze({ kind: 'none', displayDegrees })
}

function pathBracket(
  path: FoldPreviewMotionPath,
  progress: readonly [number, number],
): FoldPreviewMotionPathBracket {
  const anglesInPathOrder: readonly [number, number] = Object.freeze([
    angleAt(path, progress[0]),
    angleAt(path, progress[1]),
  ])
  return Object.freeze({
    progress: freezeBracket(progress),
    anglesInPathOrder,
  })
}

function freezePath(
  startDegrees: number,
  requestedDegrees: number,
): FoldPreviewMotionPath {
  return Object.freeze({
    startDegrees,
    requestedDegrees,
    direction: requestedDegrees > startDegrees
      ? 'increasing'
      : requestedDegrees < startDegrees
        ? 'decreasing'
        : 'stationary',
  })
}

function describeBlockingEvidenceRows(
  evidence: FoldPreviewMotionBlockingEvidence,
  firstFace: FoldPreviewMotionFaceLabel | null,
  secondFace: FoldPreviewMotionFaceLabel | null,
  locale: Locale,
): readonly FoldPreviewMotionDetailRow[] {
  const trianglePair = firstFace && secondFace
    ? localized(
        locale,
        `${firstFace.label}の三角形 ${evidence.firstTriangleNumber} ↔ ${secondFace.label}の三角形 ${evidence.secondTriangleNumber}`,
        `${firstFace.label} triangle ${evidence.firstTriangleNumber} ↔ ${secondFace.label} triangle ${evidence.secondTriangleNumber}`,
      )
    : localized(
        locale,
        `第1面側の三角形 ${evidence.firstTriangleNumber} ↔ 第2面側の三角形 ${evidence.secondTriangleNumber}`,
        `First-face triangle ${evidence.firstTriangleNumber} ↔ second-face triangle ${evidence.secondTriangleNumber}`,
      )
  const normalUniqueness = evidence.normal.uniqueness === 'unique'
    ? localized(locale, '一意', 'unique')
    : localized(locale, '同率候補の1つ', 'one of tied candidates')
  const coverage = evidence.coverage
  const scanStatus = coverage.authoritativePairScanComplete
    ? localized(locale, '全ペア走査完了', 'all-pair scan complete')
    : localized(locale, '早期停止を含む', 'includes early stop')
  return [
    userRow(
      localized(locale, '解析行列の扱い', 'Analysis-matrix handling'),
      localized(
        locale,
        '保存した危険側の面行列は3D表示の更新に使用していません',
        'The stored unsafe-side face matrices were not used to update the 3D view.',
      ),
    ),
    userRow(
      localized(locale, '危険解析角度', 'Unsafe analysis angle'),
      `${formatAngle(evidence.unsafeAnalysisDegrees)}°`,
    ),
    userRow(
      localized(locale, '局所三角形ペア', 'Local triangle pair'),
      trianglePair,
    ),
    userRow(
      localized(locale, '位置候補数', 'Position candidates'),
      localized(
        locale,
        `${evidence.positionCandidateCount}点`,
        `${evidence.positionCandidateCount} points`,
      ),
    ),
    userRow(
      localized(locale, '局所分離方向', 'Local separation direction'),
      localized(
        locale,
        `(${formatEvidenceNumber(evidence.normal.x)}, ${formatEvidenceNumber(evidence.normal.y)}, ${formatEvidenceNumber(evidence.normal.z)})・${normalUniqueness}`,
        `(${formatEvidenceNumber(evidence.normal.x)}, ${formatEvidenceNumber(evidence.normal.y)}, ${formatEvidenceNumber(evidence.normal.z)}) · ${normalUniqueness}`,
      ),
    ),
    userRow(
      localized(locale, '局所分離距離', 'Local separation distance'),
      localized(
        locale,
        `${formatEvidenceNumber(evidence.escapeDistance)}（3Dモデル座標）`,
        `${formatEvidenceNumber(evidence.escapeDistance)} (3D model coordinates)`,
      ),
    ),
    userRow(
      localized(locale, '証拠の範囲', 'Evidence scope'),
      localized(
        locale,
        '選択した三角柱1組だけの局所候補です',
        'This is a local candidate for only the selected pair of triangular prisms.',
      ),
    ),
    userRow(
      localized(locale, '自動適用可否', 'Automatic application'),
      localized(
        locale,
        'この局所分離方向・距離は自動適用できません',
        'This local separation direction and distance cannot be applied automatically.',
      ),
    ),
    diagnosticRow(
      localized(locale, '証拠取得範囲', 'Evidence collection coverage'),
      localized(
        locale,
        `${scanStatus}・対象 ${coverage.eligiblePairCount}・試行 ${coverage.attemptedPairCount}・取得 ${coverage.capturedPairCount}・導出不可 ${coverage.unavailablePairCount}・上限省略 ${coverage.omittedByLimitCount}`,
        `${scanStatus} · eligible ${coverage.eligiblePairCount} · attempted ${coverage.attemptedPairCount} · captured ${coverage.capturedPairCount} · unavailable ${coverage.unavailablePairCount} · omitted by limit ${coverage.omittedByLimitCount}`,
      ),
    ),
  ]
}

function describeBlocker(blocker: NormalizedBlocker, locale: Locale) {
  if (blocker.relation === 'non_adjacent') {
    return blocker.geometryClass === 'penetrating'
      ? localized(
          locale,
          '非隣接面間の体積貫通',
          'Volumetric penetration between non-adjacent faces',
        )
      : localized(
          locale,
          '非隣接面間の境界接触',
          'Boundary contact between non-adjacent faces',
        )
  }
  return blocker.hingeDecision === 'outside_hinge_penetration'
    ? localized(
        locale,
        '共有ヒンジの許容領域外で体積貫通',
        'Volumetric penetration outside the shared-hinge allowance',
      )
    : localized(
        locale,
        '共有ヒンジの許容領域外で境界接触',
        'Boundary contact outside the shared-hinge allowance',
      )
}

function describeReason(reasonCode: string, locale: Locale) {
  if (reasonCode === 'hinge_layer_offset_unmodeled') {
    return localized(
      locale,
      '紙厚による層ずらしを初版では再現しないため、この角度以降は安全を判定できませんでした',
      'Safety beyond this angle could not be determined because the first release does not reproduce thickness-driven layer offsets.',
    )
  }
  if (reasonCode === 'work_limit' || reasonCode === 'uncertified_interval') {
    return localized(
      locale,
      '計算上限内で経路区間の安全を確認できませんでした',
      'The path interval could not be verified as safe within the calculation limits.',
    )
  }
  if (
    reasonCode.startsWith('hinge_')
    || reasonCode === 'hinge_decision_unavailable'
    || reasonCode === 'non_adjacent_geometry_indeterminate'
  ) {
    return localized(
      locale,
      '接触モデルまたは数値境界を確定できませんでした',
      'The contact model or numerical boundary could not be resolved.',
    )
  }
  if (
    reasonCode.includes('numerical')
    || reasonCode.includes('interpolated')
    || reasonCode === 'swept_bounds_unavailable'
    || reasonCode === 'midpoint_pose_unavailable'
    || reasonCode === 'pose_unavailable'
    || reasonCode === 'point_collision_unavailable'
  ) {
    return localized(
      locale,
      '数値計算の安全条件を満たせませんでした',
      'The numerical safety conditions could not be satisfied.',
    )
  }
  if (
    reasonCode.startsWith('job_factory_')
    || reasonCode === 'scheduler_error'
  ) {
    return localized(
      locale,
      '現在の入力では経路判定を開始できませんでした',
      'The path check could not start with the current input.',
    )
  }
  if (
    reasonCode === 'job_step_error'
    || reasonCode === 'malformed_job_step'
    || reasonCode === 'non_monotonic_certified_time'
    || reasonCode === 'chronology_error'
    || reasonCode === 'contradictory_interval_certificate'
  ) {
    return localized(
      locale,
      '経路判定結果の整合性を確認できませんでした',
      'The consistency of the path-check result could not be verified.',
    )
  }
  if (reasonCode === 'unclassified') {
    return localized(
      locale,
      '未分類の内部理由により経路の安全を確定できませんでした',
      'Path safety could not be determined because of an unclassified internal reason.',
    )
  }
  return localized(
    locale,
    '経路の安全を確定できませんでした',
    'Path safety could not be determined.',
  )
}

function knownReasonCode(value: string) {
  if (RUNNER_REASONS.has(value) || CORE_REASONS.has(value)) return value
  if (
    value.startsWith('hinge_')
    && HINGE_REASON_SUFFIXES.has(value.slice('hinge_'.length))
  ) return value
  return 'unclassified'
}

function describeStats(stats: MotionStats, locale: Locale) {
  return localized(
    locale,
    `区間 ${stats.intervalTests}・姿勢点 ${stats.pointTests}・再利用 ${stats.pointCacheHits}・最大深さ ${stats.maximumDepthReached}`,
    `intervals ${stats.intervalTests} · pose points ${stats.pointTests} · cache hits ${stats.pointCacheHits} · maximum depth ${stats.maximumDepthReached}`,
  )
}

function angleAt(path: FoldPreviewMotionPath, progress: number) {
  return path.startDegrees
    + (path.requestedDegrees - path.startDegrees) * progress
}

function freezeBracket(
  bracket: readonly [number, number],
): readonly [number, number] {
  return Object.freeze([bracket[0], bracket[1]])
}

function snapshotBracket(
  value: unknown,
): readonly [number, number] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (length !== 2) return null
  const lower = value[0]
  const upper = value[1]
  return validUnitTime(lower)
    && validUnitTime(upper)
    && lower <= upper
    && (lower < upper || lower === 0)
    ? Object.freeze([lower, upper])
    : null
}

function freezeRows(
  rows: readonly FoldPreviewMotionDetailRow[],
): readonly FoldPreviewMotionDetailRow[] {
  return Object.freeze(rows.map((row) => Object.freeze(row)))
}

function userRow(label: string, value: string): FoldPreviewMotionDetailRow {
  return { label, value, kind: 'user' }
}

function diagnosticRow(label: string, value: string): FoldPreviewMotionDetailRow {
  return { label, value, kind: 'diagnostic' }
}

function summarizeRows(
  rows: readonly FoldPreviewMotionDetailRow[],
  locale: Locale,
) {
  const userRows = rows.filter((row) => row.kind === 'user')
  return locale === 'en'
    ? userRows.map((row) => `${row.label}: ${row.value}`).join('. ')
    : userRows.map((row) => `${row.label}は${row.value}`).join('。')
}

function localized(locale: Locale, ja: string, en: string) {
  return locale === 'en' ? en : ja
}

function validNonTerminalTime(value: unknown): value is number {
  return validUnitTime(value) && value < 1
}

function validUnitTime(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
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

function validCollisionThickness(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function validNonNegativeFinite(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
}

function validReason(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function validEvidenceId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= 512
    && value.trim().length > 0
}

function validBoundedKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= TREE_BLOCKING_SAMPLE_REQUEST_KEY_LIMIT
}

function validId(value: unknown): value is string {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > 128
  ) return false
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code <= 31 || code === 127) return false
  }
  return true
}

function validTriangleIndex(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= 0
    && (value as number) < TREE_BLOCKING_SAMPLE_TRIANGLE_INDEX_LIMIT
}

function validSafeCount(
  value: unknown,
  maximum: number,
): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= 0
    && (value as number) <= maximum
}

function validCount(value: unknown, maximum: number): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= 0
    && (value as number) <= maximum
}

function formatAngle(value: number) {
  const rounded = Math.round(value * 1_000_000) / 1_000_000
  return Number.isInteger(rounded)
    ? String(rounded)
    : rounded.toFixed(6).replace(/0+$/u, '').replace(/\.$/u, '')
}

function formatEvidenceNumber(value: number) {
  if (value === 0) return '0'
  const absolute = Math.abs(value)
  if (absolute < 0.000001 || absolute >= 1_000_000) {
    return value.toExponential(6)
      .replace(/\.?0+e/u, 'e')
      .replace(/e\+/u, 'e')
  }
  const rounded = Math.round(value * 1_000_000) / 1_000_000
  return Number.isInteger(rounded)
    ? String(rounded)
    : rounded.toFixed(6).replace(/0+$/u, '').replace(/\.$/u, '')
}

function formatProgress(value: number, locale: Locale) {
  return `${(value * 100).toLocaleString(
    locale === 'en' ? 'en-US' : 'ja-JP',
    {
    maximumFractionDigits: 3,
    },
  )}%`
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
