export const GLOBAL_FLAT_FOLDABILITY_MODEL_ID =
  'convex_faces_facewise_v1' as const
export const GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID =
  'facewise_layer_order_v1' as const
export const GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS =
  '凸多角形面（切断・穴・未接続材料なし）' as const

export const GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS = Object.freeze([
  5,
  30,
  120,
] as const)
export const DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET = 30 as const

export type GlobalFlatFoldabilityTimePreset =
  (typeof GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS)[number]

export const GLOBAL_FLAT_FOLDABILITY_LIMITS = Object.freeze({
  materialFaceCount: 2_048,
  faceBoundaryVertexCount: 100_000,
  overlapFacePairCount: 500_000,
  arrangementSegmentCount: 1_000_000,
  overlapCellCount: 500_000,
  constraintCount: 5_000_000,
  searchNodeCount: 10_000_000,
  exactIntegerBitLength: 65_536,
  certificateBytes: 128 * 1024 * 1024,
  timeLimitSeconds: 300,
} as const)

export const GLOBAL_FLAT_FOLDABILITY_PROOF_FACE_LIMIT = 20
const GLOBAL_FLAT_FOLDABILITY_ELAPSED_MS_LIMIT = 24 * 60 * 60 * 1_000

export type GlobalFlatFoldabilityPhase =
  | 'capturing'
  | 'validating_local_conditions'
  | 'building_flat_embedding'
  | 'building_overlap_arrangement'
  | 'building_constraints'
  | 'propagating'
  | 'searching'
  | 'verifying_certificate'
  | 'completed'

export type GlobalFlatFoldabilityUnknownReason =
  | 'unsupported_topology'
  | 'non_convex_face'
  | 'time_limit_reached'
  | 'work_limit_reached'
  | 'exact_number_limit_reached'
  | 'overlap_arrangement_limit_reached'
  | 'constraint_limit_reached'
  | 'proof_not_completed'
  | 'local_conditions_indeterminate'

export type GlobalFlatFoldabilityProofCategory =
  | 'local_conditions_violated'
  | 'inconsistent_flat_embedding'
  | 'layer_constraints_contradictory'
  | 'exhaustive_search_no_solution'

export type GlobalFlatFoldabilityErrorCategory =
  | 'invalid_request'
  | 'snapshot_unavailable'
  | 'worker_unavailable'
  | 'result_unavailable'
  | 'internal_failure'

export type GlobalFlatFoldabilityCounts = Readonly<{
  face_count: number
  overlap_cell_count: number
  constraint_count: number
  search_node_count: number
}>

export type GlobalFlatFoldabilityProgress = Readonly<{
  model_id: typeof GLOBAL_FLAT_FOLDABILITY_MODEL_ID
  phase: GlobalFlatFoldabilityPhase
  completed_work: number
  total_work: number | null
  elapsed_ms: number
  counts: GlobalFlatFoldabilityCounts
}>

export type GlobalFlatFoldabilitySummary = Readonly<{
  model_id: typeof GLOBAL_FLAT_FOLDABILITY_MODEL_ID
  elapsed_ms: number
  counts: GlobalFlatFoldabilityCounts
}>

export type GlobalFlatFoldabilityPossibleResult = Readonly<{
  verdict: 'possible'
  summary: GlobalFlatFoldabilitySummary
  layer_order: Readonly<{
    model_id: typeof GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID
    layer_count: number
    max_ply: number
    reference_face_number: number
    layer_view_available: boolean
  }>
}>

export type GlobalFlatFoldabilityImpossibleResult = Readonly<{
  verdict: 'impossible'
  summary: GlobalFlatFoldabilitySummary
  proof: Readonly<{
    category: GlobalFlatFoldabilityProofCategory
    face_numbers: readonly number[]
  }>
}>

export type GlobalFlatFoldabilityUnknownResult = Readonly<{
  verdict: 'unknown'
  summary: GlobalFlatFoldabilitySummary
  reason: GlobalFlatFoldabilityUnknownReason
}>

export type GlobalFlatFoldabilityResult =
  | GlobalFlatFoldabilityPossibleResult
  | GlobalFlatFoldabilityImpossibleResult
  | GlobalFlatFoldabilityUnknownResult

export type GlobalFlatFoldabilityJobDto =
  | Readonly<{
      state: 'queued'
      cancel_requested: boolean
      progress: GlobalFlatFoldabilityProgress
    }>
  | Readonly<{
      state: 'running'
      cancel_requested: boolean
      progress: GlobalFlatFoldabilityProgress
    }>
  | Readonly<{
      state: 'completed'
      result: GlobalFlatFoldabilityResult
    }>
  | Readonly<{
      state: 'cancelled'
      summary: GlobalFlatFoldabilitySummary
    }>
  | Readonly<{
      state: 'failed'
      summary: GlobalFlatFoldabilitySummary
      error_category: GlobalFlatFoldabilityErrorCategory
    }>
  | Readonly<{
      state: 'stale'
      summary: GlobalFlatFoldabilitySummary
    }>

const PHASES = new Set<GlobalFlatFoldabilityPhase>([
  'capturing',
  'validating_local_conditions',
  'building_flat_embedding',
  'building_overlap_arrangement',
  'building_constraints',
  'propagating',
  'searching',
  'verifying_certificate',
  'completed',
])

const UNKNOWN_REASONS = new Set<GlobalFlatFoldabilityUnknownReason>([
  'unsupported_topology',
  'non_convex_face',
  'time_limit_reached',
  'work_limit_reached',
  'exact_number_limit_reached',
  'overlap_arrangement_limit_reached',
  'constraint_limit_reached',
  'proof_not_completed',
  'local_conditions_indeterminate',
])

const PROOF_CATEGORIES = new Set<GlobalFlatFoldabilityProofCategory>([
  'local_conditions_violated',
  'inconsistent_flat_embedding',
  'layer_constraints_contradictory',
  'exhaustive_search_no_solution',
])

const ERROR_CATEGORIES = new Set<GlobalFlatFoldabilityErrorCategory>([
  'invalid_request',
  'snapshot_unavailable',
  'worker_unavailable',
  'result_unavailable',
  'internal_failure',
])

const JOB_STATE_KEYS = ['state'] as const
const ACTIVE_JOB_KEYS = ['state', 'cancel_requested', 'progress'] as const
const COMPLETED_JOB_KEYS = ['state', 'result'] as const
const SUMMARY_JOB_KEYS = ['state', 'summary'] as const
const FAILED_JOB_KEYS = ['state', 'summary', 'error_category'] as const
const PROGRESS_KEYS = [
  'model_id',
  'phase',
  'completed_work',
  'total_work',
  'elapsed_ms',
  'counts',
] as const
const SUMMARY_KEYS = ['model_id', 'elapsed_ms', 'counts'] as const
const COUNTS_KEYS = [
  'face_count',
  'overlap_cell_count',
  'constraint_count',
  'search_node_count',
] as const
const RESULT_VERDICT_KEYS = ['verdict'] as const
const POSSIBLE_RESULT_KEYS = ['verdict', 'summary', 'layer_order'] as const
const IMPOSSIBLE_RESULT_KEYS = ['verdict', 'summary', 'proof'] as const
const UNKNOWN_RESULT_KEYS = ['verdict', 'summary', 'reason'] as const
const LAYER_ORDER_KEYS = [
  'model_id',
  'layer_count',
  'max_ply',
  'reference_face_number',
  'layer_view_available',
] as const
const PROOF_KEYS = ['category', 'face_numbers'] as const

export function isGlobalFlatFoldabilityTimePreset(
  value: unknown,
): value is GlobalFlatFoldabilityTimePreset {
  return value === 5 || value === 30 || value === 120
}

export function normalizeGlobalFlatFoldabilityTimePreset(
  value: unknown,
): GlobalFlatFoldabilityTimePreset {
  return isGlobalFlatFoldabilityTimePreset(value)
    ? value
    : DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET
}

export function parseGlobalFlatFoldabilityJobDto(
  rawJob: unknown,
): GlobalFlatFoldabilityJobDto | null {
  try {
    const stateRecord = dataRecordContainingOnly(rawJob, JOB_STATE_KEYS)
    if (!stateRecord || typeof stateRecord.state !== 'string') return null

    switch (stateRecord.state) {
      case 'queued':
      case 'running': {
        const state = stateRecord.state
        const job = exactDataRecord(rawJob, ACTIVE_JOB_KEYS)
        if (
          !job
          || job.state !== state
          || typeof job.cancel_requested !== 'boolean'
        ) return null
        const progress = parseProgress(job.progress)
        if (!progress) return null
        if (state === 'queued' && progress.phase !== 'capturing') return null
        return Object.freeze({
          state,
          cancel_requested: job.cancel_requested,
          progress,
        })
      }
      case 'completed': {
        const job = exactDataRecord(rawJob, COMPLETED_JOB_KEYS)
        if (!job || job.state !== 'completed') return null
        const result = parseResult(job.result)
        return result
          ? Object.freeze({ state: 'completed', result })
          : null
      }
      case 'cancelled':
      case 'stale': {
        const state = stateRecord.state
        const job = exactDataRecord(rawJob, SUMMARY_JOB_KEYS)
        if (!job || job.state !== state) return null
        const summary = parseSummary(job.summary)
        return summary
          ? Object.freeze({ state, summary })
          : null
      }
      case 'failed': {
        const job = exactDataRecord(rawJob, FAILED_JOB_KEYS)
        if (
          !job
          || job.state !== 'failed'
          || !isSetMember(job.error_category, ERROR_CATEGORIES)
        ) return null
        const summary = parseSummary(job.summary)
        return summary
          ? Object.freeze({
              state: 'failed',
              summary,
              error_category: job.error_category,
            })
          : null
      }
      default:
        return null
    }
  } catch {
    return null
  }
}

function parseProgress(rawProgress: unknown): GlobalFlatFoldabilityProgress | null {
  const progress = exactDataRecord(rawProgress, PROGRESS_KEYS)
  if (
    !progress
    || progress.model_id !== GLOBAL_FLAT_FOLDABILITY_MODEL_ID
    || !isSetMember(progress.phase, PHASES)
    || !isBoundedCount(
      progress.completed_work,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.searchNodeCount,
    )
    || !isNullableBoundedCount(
      progress.total_work,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.searchNodeCount,
    )
    || !isBoundedCount(
      progress.elapsed_ms,
      GLOBAL_FLAT_FOLDABILITY_ELAPSED_MS_LIMIT,
    )
  ) return null
  if (
    progress.total_work !== null
    && progress.completed_work > progress.total_work
  ) return null
  const counts = parseCounts(progress.counts)
  return counts
    ? Object.freeze({
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        phase: progress.phase,
        completed_work: progress.completed_work,
        total_work: progress.total_work,
        elapsed_ms: progress.elapsed_ms,
        counts,
      })
    : null
}

function parseSummary(rawSummary: unknown): GlobalFlatFoldabilitySummary | null {
  const summary = exactDataRecord(rawSummary, SUMMARY_KEYS)
  if (
    !summary
    || summary.model_id !== GLOBAL_FLAT_FOLDABILITY_MODEL_ID
    || !isBoundedCount(
      summary.elapsed_ms,
      GLOBAL_FLAT_FOLDABILITY_ELAPSED_MS_LIMIT,
    )
  ) return null
  const counts = parseCounts(summary.counts)
  return counts
    ? Object.freeze({
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        elapsed_ms: summary.elapsed_ms,
        counts,
      })
    : null
}

function parseCounts(rawCounts: unknown): GlobalFlatFoldabilityCounts | null {
  const counts = exactDataRecord(rawCounts, COUNTS_KEYS)
  if (
    !counts
    || !isBoundedCount(
      counts.face_count,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.materialFaceCount,
    )
    || !isBoundedCount(
      counts.overlap_cell_count,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.overlapCellCount,
    )
    || !isBoundedCount(
      counts.constraint_count,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.constraintCount,
    )
    || !isBoundedCount(
      counts.search_node_count,
      GLOBAL_FLAT_FOLDABILITY_LIMITS.searchNodeCount,
    )
  ) return null
  return Object.freeze({
    face_count: counts.face_count,
    overlap_cell_count: counts.overlap_cell_count,
    constraint_count: counts.constraint_count,
    search_node_count: counts.search_node_count,
  })
}

function parseResult(rawResult: unknown): GlobalFlatFoldabilityResult | null {
  const verdictRecord = dataRecordContainingOnly(rawResult, RESULT_VERDICT_KEYS)
  if (!verdictRecord || typeof verdictRecord.verdict !== 'string') return null
  switch (verdictRecord.verdict) {
    case 'possible':
      return parsePossibleResult(rawResult)
    case 'impossible':
      return parseImpossibleResult(rawResult)
    case 'unknown':
      return parseUnknownResult(rawResult)
    default:
      return null
  }
}

function parsePossibleResult(
  rawResult: unknown,
): GlobalFlatFoldabilityPossibleResult | null {
  const result = exactDataRecord(rawResult, POSSIBLE_RESULT_KEYS)
  if (!result || result.verdict !== 'possible') return null
  const summary = parseSummary(result.summary)
  const layerOrder = exactDataRecord(result.layer_order, LAYER_ORDER_KEYS)
  if (
    !summary
    || summary.counts.face_count === 0
    || !layerOrder
    || layerOrder.model_id !== GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID
    || !isPositiveBoundedCount(
      layerOrder.layer_count,
      summary.counts.face_count,
    )
    || layerOrder.layer_count !== summary.counts.face_count
    || !isPositiveBoundedCount(
      layerOrder.max_ply,
      layerOrder.layer_count,
    )
    || !isPositiveBoundedCount(
      layerOrder.reference_face_number,
      summary.counts.face_count,
    )
    || typeof layerOrder.layer_view_available !== 'boolean'
  ) return null
  return Object.freeze({
    verdict: 'possible',
    summary,
    layer_order: Object.freeze({
      model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
      layer_count: layerOrder.layer_count,
      max_ply: layerOrder.max_ply,
      reference_face_number: layerOrder.reference_face_number,
      layer_view_available: layerOrder.layer_view_available,
    }),
  })
}

function parseImpossibleResult(
  rawResult: unknown,
): GlobalFlatFoldabilityImpossibleResult | null {
  const result = exactDataRecord(rawResult, IMPOSSIBLE_RESULT_KEYS)
  if (!result || result.verdict !== 'impossible') return null
  const summary = parseSummary(result.summary)
  const proof = exactDataRecord(result.proof, PROOF_KEYS)
  if (
    !summary
    || !proof
    || !isSetMember(proof.category, PROOF_CATEGORIES)
    || !Array.isArray(proof.face_numbers)
    || proof.face_numbers.length === 0
    || proof.face_numbers.length > GLOBAL_FLAT_FOLDABILITY_PROOF_FACE_LIMIT
  ) return null
  const faceNumbers: number[] = []
  let previousFaceNumber = 0
  for (const faceNumber of proof.face_numbers) {
    if (
      !isPositiveBoundedCount(faceNumber, summary.counts.face_count)
      || faceNumber <= previousFaceNumber
    ) return null
    previousFaceNumber = faceNumber
    faceNumbers.push(faceNumber)
  }
  return Object.freeze({
    verdict: 'impossible',
    summary,
    proof: Object.freeze({
      category: proof.category,
      face_numbers: Object.freeze(faceNumbers),
    }),
  })
}

function parseUnknownResult(
  rawResult: unknown,
): GlobalFlatFoldabilityUnknownResult | null {
  const result = exactDataRecord(rawResult, UNKNOWN_RESULT_KEYS)
  if (
    !result
    || result.verdict !== 'unknown'
    || !isSetMember(result.reason, UNKNOWN_REASONS)
  ) return null
  const summary = parseSummary(result.summary)
  return summary
    ? Object.freeze({
        verdict: 'unknown',
        summary,
        reason: result.reason,
      })
    : null
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): { [Key in Keys[number]]: unknown } | null {
  if (!isPlainDataObject(value)) return null
  const ownKeys = Reflect.ownKeys(value)
  if (ownKeys.length !== expectedKeys.length) return null
  const expected = new Set<string>(expectedKeys)
  const result: Record<string, unknown> = {}
  for (const key of ownKeys) {
    if (typeof key !== 'string' || !expected.has(key)) return null
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return null
    result[key] = descriptor.value
  }
  return result as { [Key in Keys[number]]: unknown }
}

function dataRecordContainingOnly<const Keys extends readonly string[]>(
  value: unknown,
  requiredKeys: Keys,
): { [Key in Keys[number]]: unknown } | null {
  if (!isPlainDataObject(value)) return null
  const required = new Set<string>(requiredKeys)
  const result: Record<string, unknown> = {}
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string') return null
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return null
    if (required.has(key)) result[key] = descriptor.value
  }
  for (const key of requiredKeys) {
    if (!Object.prototype.hasOwnProperty.call(result, key)) return null
  }
  return result as { [Key in Keys[number]]: unknown }
}

function isPlainDataObject(
  value: unknown,
): value is Record<PropertyKey, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return false
  }
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function isSetMember<const Value extends string>(
  value: unknown,
  values: ReadonlySet<Value>,
): value is Value {
  return typeof value === 'string' && values.has(value as Value)
}

function isBoundedCount(value: unknown, maximum: number): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
    && value <= maximum
}

function isPositiveBoundedCount(
  value: unknown,
  maximum: number,
): value is number {
  return isBoundedCount(value, maximum) && value > 0
}

function isNullableBoundedCount(
  value: unknown,
  maximum: number,
): value is number | null {
  return value === null || isBoundedCount(value, maximum)
}
