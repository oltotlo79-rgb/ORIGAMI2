import type {
  FoldPreviewContinuousMotionRunnerState,
} from './foldPreviewContinuousMotionRunner'

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
  'numerical_geometry',
  'corridor_boundary',
  'non_hinge_triangle',
  'incomplete_pair_scan',
  'pair_geometry_mismatch',
  'flat_pose_penetration',
])

/**
 * Converts one terminal single-fold runner snapshot into immutable,
 * user-facing detail. Any inconsistent terminal contract returns `null`
 * instead of exposing a misleading partial explanation.
 */
export function describeFoldPreviewContinuousMotionDetail(
  state: FoldPreviewContinuousMotionRunnerState<unknown> | null,
  faceLabels: readonly FoldPreviewMotionFaceLabel[] = [],
): FoldPreviewContinuousMotionDetail | null {
  try {
    if (
      !state
      || !validAngle(state.start)
      || !validAngle(state.applied)
      || !validAngle(state.requested)
    ) return null
    const requested = state.requested
    const path = freezePath(state.start, requested)
    const labels = normalizeFaceLabels(faceLabels)

    if (state.status === 'blocked') {
      if (state.reason !== 'motion_blocked') return null
      const terminal = normalizeBlockedResult(state.result)
      if (
        !terminal
        || state.applied !== angleAt(path, terminal.certifiedSafeThrough)
      ) return null
      const blocker = normalizeBlocker(terminal.blocker)
      return blockedDetail(path, state.applied, terminal, blocker, labels)
    }

    if (state.status !== 'indeterminate' || !validReason(state.reason)) return null
    if (state.result === null) {
      return runnerFailureDetail(path, state.applied, state.reason)
    }
    const terminal = normalizeIndeterminateResult(state.result)
    if (
      !terminal
      || state.reason !== terminal.reason
      || state.applied !== angleAt(path, terminal.certifiedSafeThrough)
    ) return null
    return indeterminateDetail(path, state.applied, terminal)
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
    stats: MotionStats
  }>,
  blocker: NormalizedBlocker | null,
  labels: ReadonlyMap<string, FoldPreviewMotionFaceLabel>,
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
    : '対象面の対応を確認できません'
  const classification = blocker
    ? describeBlocker(blocker)
    : '衝突姿勢を検出しましたが、相互作用の詳細は取得できません'
  const intervalText = bracket.progress[0] === bracket.progress[1]
    ? `${formatAngle(bracket.anglesInPathOrder[0])}°`
    : `${formatAngle(bracket.anglesInPathOrder[0])}° → ${formatAngle(bracket.anglesInPathOrder[1])}°`
  const rows = freezeRows([
    userRow('開始角', `${formatAngle(path.startDegrees)}°`),
    userRow('指定角', `${formatAngle(path.requestedDegrees)}°`),
    userRow('実表示角', `${formatAngle(displayDegrees)}°`),
    userRow(
      bracket.progress[0] === bracket.progress[1]
        ? '衝突検出角度'
        : '衝突姿勢を含む探索角度範囲',
      intervalText,
    ),
    userRow('対象面ペア', faceText),
    userRow('分類', classification),
    diagnosticRow(
      '経路進捗',
      `${formatProgress(terminal.certifiedSafeThrough)} まで確認`,
    ),
    diagnosticRow(
      '内部診断コード',
      'motion_blocked',
    ),
    diagnosticRow('判定量', describeStats(terminal.stats)),
  ])
  const summaryText = rows
    .filter((row) => row.kind === 'user')
    .map((row) => `${row.label}は${row.value}`)
    .join('。')
  return Object.freeze({
    kind: 'blocked',
    title: terminal.bracket[0] === 0 && terminal.bracket[1] === 0
      ? '開始姿勢の衝突詳細'
      : '移動経路の停止詳細',
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
    userRow('開始角', `${formatAngle(path.startDegrees)}°`),
    userRow('指定角', `${formatAngle(path.requestedDegrees)}°`),
    userRow('実表示角', `${formatAngle(displayDegrees)}°`),
    userRow(
      bracket.progress[0] === bracket.progress[1]
        ? '判定不能角度'
        : '安全を確認できない角度範囲',
      intervalText,
    ),
    userRow('停止理由', describeReason(reasonCode)),
    diagnosticRow(
      '経路進捗',
      `${formatProgress(terminal.certifiedSafeThrough)} まで確認`,
    ),
    diagnosticRow('内部診断コード', reasonCode),
    diagnosticRow('判定量', describeStats(terminal.stats)),
  ])
  const summaryText = rows
    .filter((row) => row.kind === 'user')
    .map((row) => `${row.label}は${row.value}`)
    .join('。')
  return Object.freeze({
    kind: 'indeterminate',
    title: terminal.bracket[0] === 0 && terminal.bracket[1] === 0
      ? '開始姿勢の判定不能詳細'
      : '移動経路の判定停止詳細',
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
  })
}

function runnerFailureDetail(
  path: FoldPreviewMotionPath,
  displayDegrees: number,
  rawReason: string,
): FoldPreviewContinuousMotionDetail {
  const reasonCode = knownReasonCode(rawReason)
  const rows = freezeRows([
    userRow('開始角', `${formatAngle(path.startDegrees)}°`),
    userRow('指定角', `${formatAngle(path.requestedDegrees)}°`),
    userRow('保持中の表示角', `${formatAngle(displayDegrees)}°`),
    userRow('停止理由', describeReason(reasonCode)),
    diagnosticRow('内部診断コード', reasonCode),
  ])
  const summaryText = rows
    .filter((row) => row.kind === 'user')
    .map((row) => `${row.label}は${row.value}`)
    .join('。')
  return Object.freeze({
    kind: 'indeterminate',
    title: '移動経路を開始できない理由',
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
  })
}

function normalizeBlockedResult(result: unknown) {
  if (!result || typeof result !== 'object') return null
  const record = result as Record<string, unknown>
  if (
    record.kind !== 'blocked'
    || !validNonTerminalTime(record.certifiedSafeThrough)
    || record.stopTime !== record.certifiedSafeThrough
    || !validBracket(record.unsafeBracket)
    || record.unsafeBracket[0] !== record.certifiedSafeThrough
    || !validUnitTime(record.blockingSampleTime)
    || record.blockingSampleTime !== record.unsafeBracket[1]
  ) return null
  const stats = normalizeStats(record.stats)
  if (!stats) return null
  return Object.freeze({
    certifiedSafeThrough: record.certifiedSafeThrough,
    bracket: freezeBracket(record.unsafeBracket),
    blocker: Object.hasOwn(record, 'blocker') ? record.blocker : null,
    stats,
  })
}

function normalizeIndeterminateResult(result: unknown) {
  if (!result || typeof result !== 'object') return null
  const record = result as Record<string, unknown>
  if (
    record.kind !== 'indeterminate'
    || !validNonTerminalTime(record.certifiedSafeThrough)
    || record.stopTime !== record.certifiedSafeThrough
    || !validBracket(record.unresolvedBracket)
    || record.unresolvedBracket[0] !== record.certifiedSafeThrough
    || !validReason(record.reason)
  ) return null
  const stats = normalizeStats(record.stats)
  if (!stats) return null
  return Object.freeze({
    certifiedSafeThrough: record.certifiedSafeThrough,
    bracket: freezeBracket(record.unresolvedBracket),
    reason: record.reason,
    stats,
  })
}

function normalizeBlocker(value: unknown): NormalizedBlocker | null {
  if (!value || typeof value !== 'object') return null
  const blocker = value as Record<string, unknown>
  if (
    !validId(blocker.firstFaceId)
    || !validId(blocker.secondFaceId)
    || blocker.firstFaceId === blocker.secondFaceId
  ) return null
  if (blocker.relation === 'non_adjacent') {
    if (
      (blocker.geometryClass !== 'touching'
        && blocker.geometryClass !== 'penetrating')
      || Object.hasOwn(blocker, 'hingeDecisionKind')
    ) return null
    return Object.freeze({
      firstFaceId: blocker.firstFaceId,
      secondFaceId: blocker.secondFaceId,
      relation: 'non_adjacent',
      geometryClass: blocker.geometryClass,
      hingeDecision: null,
    })
  }
  if (blocker.relation !== 'hinge_adjacent') return null
  if (
    blocker.hingeDecisionKind === 'outside_hinge_penetration'
    && blocker.geometryClass === 'penetrating'
  ) {
    return Object.freeze({
      firstFaceId: blocker.firstFaceId,
      secondFaceId: blocker.secondFaceId,
      relation: 'hinge_adjacent',
      geometryClass: 'penetrating',
      hingeDecision: 'outside_hinge_penetration',
    })
  }
  if (
    blocker.hingeDecisionKind === 'outside_hinge_contact'
    && blocker.geometryClass === 'touching'
  ) {
    return Object.freeze({
      firstFaceId: blocker.firstFaceId,
      secondFaceId: blocker.secondFaceId,
      relation: 'hinge_adjacent',
      geometryClass: 'touching',
      hingeDecision: 'outside_hinge_contact',
    })
  }
  return null
}

function normalizeFaceLabels(
  labels: readonly FoldPreviewMotionFaceLabel[],
): ReadonlyMap<string, FoldPreviewMotionFaceLabel> {
  if (!Array.isArray(labels)) return new Map()
  const result = new Map<string, FoldPreviewMotionFaceLabel>()
  const ambiguous = new Set<string>()
  for (const label of labels) {
    if (
      !label
      || !validId(label.id)
      || !Number.isSafeInteger(label.number)
      || label.number <= 0
      || typeof label.label !== 'string'
      || label.label.length === 0
      || label.label.length > 80
    ) continue
    if (result.has(label.id) || ambiguous.has(label.id)) {
      result.delete(label.id)
      ambiguous.add(label.id)
      continue
    }
    result.set(label.id, Object.freeze({ ...label }))
  }
  return result
}

function normalizeStats(value: unknown): MotionStats | null {
  if (!value || typeof value !== 'object') return null
  const stats = value as Record<string, unknown>
  if (
    !validCount(stats.intervalTests, 1_000_000)
    || !validCount(stats.pointTests, 1_000_002)
    || !validCount(stats.pointCacheHits, 2_000_002)
    || !validCount(stats.maximumDepthReached, 52)
  ) return null
  return Object.freeze({
    intervalTests: stats.intervalTests,
    pointTests: stats.pointTests,
    pointCacheHits: stats.pointCacheHits,
    maximumDepthReached: stats.maximumDepthReached,
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

function describeBlocker(blocker: NormalizedBlocker) {
  if (blocker.relation === 'non_adjacent') {
    return blocker.geometryClass === 'penetrating'
      ? '非隣接面間の体積貫通'
      : '非隣接面間の境界接触'
  }
  return blocker.hingeDecision === 'outside_hinge_penetration'
    ? '共有ヒンジの許容領域外で体積貫通'
    : '共有ヒンジの許容領域外で境界接触'
}

function describeReason(reasonCode: string) {
  if (reasonCode === 'work_limit' || reasonCode === 'uncertified_interval') {
    return '計算上限内で経路区間の安全を確認できませんでした'
  }
  if (
    reasonCode.startsWith('hinge_')
    || reasonCode === 'hinge_decision_unavailable'
    || reasonCode === 'non_adjacent_geometry_indeterminate'
  ) return '接触モデルまたは数値境界を確定できませんでした'
  if (
    reasonCode.includes('numerical')
    || reasonCode.includes('interpolated')
    || reasonCode === 'swept_bounds_unavailable'
    || reasonCode === 'midpoint_pose_unavailable'
    || reasonCode === 'pose_unavailable'
    || reasonCode === 'point_collision_unavailable'
  ) return '数値計算の安全条件を満たせませんでした'
  if (
    reasonCode.startsWith('job_factory_')
    || reasonCode === 'scheduler_error'
  ) return '現在の入力では経路判定を開始できませんでした'
  if (
    reasonCode === 'job_step_error'
    || reasonCode === 'malformed_job_step'
    || reasonCode === 'non_monotonic_certified_time'
    || reasonCode === 'chronology_error'
    || reasonCode === 'contradictory_interval_certificate'
  ) return '経路判定結果の整合性を確認できませんでした'
  if (reasonCode === 'unclassified') {
    return '未分類の内部理由により経路の安全を確定できませんでした'
  }
  return '経路の安全を確定できませんでした'
}

function knownReasonCode(value: string) {
  if (RUNNER_REASONS.has(value) || CORE_REASONS.has(value)) return value
  if (
    value.startsWith('hinge_')
    && HINGE_REASON_SUFFIXES.has(value.slice('hinge_'.length))
  ) return value
  return 'unclassified'
}

function describeStats(stats: MotionStats) {
  return `区間 ${stats.intervalTests}・姿勢点 ${stats.pointTests}・再利用 ${stats.pointCacheHits}・最大深さ ${stats.maximumDepthReached}`
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

function validBracket(value: unknown): value is readonly [number, number] {
  return Array.isArray(value)
    && value.length === 2
    && validUnitTime(value[0])
    && validUnitTime(value[1])
    && value[0] <= value[1]
    && (value[0] < value[1] || value[0] === 0)
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

function validReason(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
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

function formatProgress(value: number) {
  return `${(value * 100).toLocaleString('ja-JP', {
    maximumFractionDigits: 3,
  })}%`
}
