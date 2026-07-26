export const PROOF_PROGRESS_STATES = Object.freeze([
  'proving',
  'certified',
  'blocked',
  'evidence_insufficient',
  'resource_limit',
  'cancelled',
  'deadline',
  'stale',
] as const)

export type ProofProgressState = (typeof PROOF_PROGRESS_STATES)[number]

export const UNPROVEN_HISTORY_STATUS_KEYS = Object.freeze([
  'awaitingProof',
  'proofBlocked',
  'unknownEvidenceInsufficient',
  'unknownResourceLimit',
  'unknownCancelled',
  'unknownDeadlineReached',
] as const)

export type UnprovenHistoryStatus =
  (typeof UNPROVEN_HISTORY_STATUS_KEYS)[number]

export type UnprovenHistoryStatusCountsView = Readonly<
  Record<UnprovenHistoryStatus, number>
>

export type UnprovenHistorySummaryView =
  | Readonly<{
      kind: 'known'
      applied: UnprovenHistoryStatusCountsView
      unappliedRedo: UnprovenHistoryStatusCountsView
      appliedTotal: number
      unappliedRedoTotal: number
    }>
  | Readonly<{ kind: 'unavailable' }>
  | Readonly<{ kind: 'absent' }>

export type ProofFailureLocation =
  | 'applied_trimmed_base'
  | 'applied_retained_undo'
  | 'unapplied_redo'

export type ProofFailureReason =
  | 'blocked'
  | 'evidence_insufficient'
  | 'resource_limit'
  | 'cancelled'
  | 'deadline'

export type ProofFailureViewModel = Readonly<{
  location: ProofFailureLocation
  reason: ProofFailureReason
  subsequentEditCount: number
  undoStepsToRevert: number | null
}>

export type ProofProgressPanelModel = Readonly<{
  status: ProofProgressState | null
  provenPairCount: number
  totalPairCount: number | null
  unprovenHistory: UnprovenHistorySummaryView
  speculativeApplyAvailable: boolean
  proofFailure: ProofFailureViewModel | null
}>

export function isProofProgressState(value: unknown): value is ProofProgressState {
  return PROOF_PROGRESS_STATES.some((state) => state === value)
}

export function failClosedProofProgressState(value: unknown): ProofProgressState {
  return isProofProgressState(value) ? value : 'evidence_insufficient'
}

export function isSafeCount(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && Number(value) >= 0
    && !Object.is(value, -0)
}
