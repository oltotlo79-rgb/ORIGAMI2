import type {
  ProofFailureViewModel,
  UnprovenHistoryStatusCountsView,
  UnprovenHistorySummaryView,
} from './proofProgressModel.ts'
import { isSafeCount } from './proofProgressModel.ts'

const PROOF_FAILURE_KEYS = Object.freeze([
  'location',
  'outcome',
  'reason',
  'subsequentEditCount',
  'undoStepsToRevert',
] as const)

const SUMMARY_KEYS = Object.freeze(['applied', 'unappliedRedo'] as const)
const STATUS_COUNT_KEYS = Object.freeze([
  'awaitingProof',
  'proofBlocked',
  'unknownEvidenceInsufficient',
  'unknownResourceLimit',
  'unknownCancelled',
  'unknownDeadlineReached',
] as const)

const UNKNOWN_REASON_MAP = Object.freeze({
  evidence_insufficient: 'evidence_insufficient',
  resource_limit: 'resource_limit',
  cancelled: 'cancelled',
  deadline_reached: 'deadline',
} as const)

export function normalizeProofFailureViewModelV1(
  value: unknown,
): ProofFailureViewModel | null {
  const record = ownDataRecord(value, PROOF_FAILURE_KEYS)
  if (!record) return null

  const location = record.location
  if (
    location !== 'applied_trimmed_base'
    && location !== 'applied_retained_undo'
    && location !== 'unapplied_redo'
  ) return null
  if (!isSafeCount(record.subsequentEditCount)) return null
  const subsequentEditCount = record.subsequentEditCount
  const undoStepsToRevert = record.undoStepsToRevert
  if (!(undoStepsToRevert === null || (
    isSafeCount(undoStepsToRevert) && undoStepsToRevert > 0
  ))) return null

  if (
    location === 'applied_retained_undo'
      ? subsequentEditCount === Number.MAX_SAFE_INTEGER
        || undoStepsToRevert !== subsequentEditCount + 1
      : undoStepsToRevert !== null
  ) return null
  if (location === 'unapplied_redo' && subsequentEditCount !== 0) return null

  let reason: ProofFailureViewModel['reason']
  if (record.outcome === 'blocked' && record.reason === null) {
    reason = 'blocked'
  } else if (
    record.outcome === 'unknown'
    && typeof record.reason === 'string'
    && Object.hasOwn(UNKNOWN_REASON_MAP, record.reason)
  ) {
    reason = UNKNOWN_REASON_MAP[
      record.reason as keyof typeof UNKNOWN_REASON_MAP
    ]
  } else {
    return null
  }

  return Object.freeze({
    location,
    reason,
    subsequentEditCount,
    undoStepsToRevert,
  })
}

export function unprovenHistorySummaryFromSnapshotV1(
  snapshot: unknown,
): UnprovenHistorySummaryView {
  const field = optionalOwnDataValue(snapshot, 'speculativeUnprovenFolds')
  if (field.kind === 'absent') return Object.freeze({ kind: 'absent' })
  if (field.kind === 'invalid') return Object.freeze({ kind: 'unavailable' })
  const summary = ownDataRecord(field.value, SUMMARY_KEYS)
  if (!summary) return Object.freeze({ kind: 'unavailable' })
  const applied = normalizeStatusCounts(summary.applied)
  const unappliedRedo = normalizeStatusCounts(summary.unappliedRedo)
  if (!applied || !unappliedRedo) {
    return Object.freeze({ kind: 'unavailable' })
  }
  return Object.freeze({
    kind: 'known',
    applied: applied.counts,
    unappliedRedo: unappliedRedo.counts,
    appliedTotal: applied.total,
    unappliedRedoTotal: unappliedRedo.total,
  })
}

function normalizeStatusCounts(
  value: unknown,
): Readonly<{
  counts: UnprovenHistoryStatusCountsView
  total: number
}> | null {
  const record = ownDataRecord(value, STATUS_COUNT_KEYS)
  if (!record) return null
  let total = 0
  for (const key of STATUS_COUNT_KEYS) {
    const count = record[key]
    if (!isSafeCount(count) || count > Number.MAX_SAFE_INTEGER - total) {
      return null
    }
    total += count
  }
  const counts = Object.freeze({
    awaitingProof: record.awaitingProof as number,
    proofBlocked: record.proofBlocked as number,
    unknownEvidenceInsufficient: record.unknownEvidenceInsufficient as number,
    unknownResourceLimit: record.unknownResourceLimit as number,
    unknownCancelled: record.unknownCancelled as number,
    unknownDeadlineReached: record.unknownDeadlineReached as number,
  })
  return Object.freeze({ counts, total })
}

function optionalOwnDataValue(
  value: unknown,
  key: string,
):
  | Readonly<{ kind: 'present'; value: unknown }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid' }> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return Object.freeze({ kind: 'invalid' })
  }
  let descriptor: PropertyDescriptor | undefined
  try {
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      return Object.freeze({ kind: 'invalid' })
    }
    descriptor = Object.getOwnPropertyDescriptor(value, key)
  } catch {
    return Object.freeze({ kind: 'invalid' })
  }
  if (!descriptor) return Object.freeze({ kind: 'absent' })
  if (!descriptor.enumerable || !('value' in descriptor)) {
    return Object.freeze({ kind: 'invalid' })
  }
  return Object.freeze({ kind: 'present', value: descriptor.value })
}

function ownDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Record<Keys[number], unknown> | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return null
  }
  let prototype: object | null
  let keys: PropertyKey[]
  try {
    prototype = Object.getPrototypeOf(value)
    keys = Reflect.ownKeys(value)
  } catch {
    return null
  }
  if (prototype !== Object.prototype && prototype !== null) return null
  if (
    keys.length !== expectedKeys.length
    || keys.some(
      (key) => typeof key !== 'string' || !expectedKeys.includes(key),
    )
  ) return null
  const snapshot = Object.create(null) as Record<Keys[number], unknown>
  for (const key of expectedKeys) {
    let descriptor: PropertyDescriptor | undefined
    try {
      descriptor = Object.getOwnPropertyDescriptor(value, key)
    } catch {
      return null
    }
    if (
      !descriptor
      || !descriptor.enumerable
      || !('value' in descriptor)
    ) return null
    snapshot[key as Keys[number]] = descriptor.value
  }
  return snapshot
}
