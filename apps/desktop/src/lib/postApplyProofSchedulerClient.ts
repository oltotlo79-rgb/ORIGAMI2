import { invoke } from '@tauri-apps/api/core'

import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { ProofFailureViewModel } from './proofProgressModel.ts'
import { normalizeProofFailureViewModelV1 } from './speculativeUnprovenWire.ts'

/**
 * Provisional v1 transport boundary. Native command names and field casing
 * remain isolated in this adapter while the rest of the UI uses typed values.
 */
export const POST_APPLY_PROOF_PROTOCOL_VERSION_V1 = 1 as const

export const POST_APPLY_PROOF_STATUSES_V1 = Object.freeze([
  'proving',
  'certified',
  'blocked',
  'unknown_evidence_insufficient',
  'unknown_resource_limit',
  'unknown_cancelled',
  'unknown_deadline_reached',
  'stale',
] as const)

export type PostApplyProofStatusV1 =
  (typeof POST_APPLY_PROOF_STATUSES_V1)[number]

export type StartPostApplyProofJobRequestV1 = Readonly<{
  version: typeof POST_APPLY_PROOF_PROTOCOL_VERSION_V1
  projectInstanceId: string
  projectId: string
  revision: number
}>

export type PostApplyProofJobRequestV1 =
  StartPostApplyProofJobRequestV1
  & Readonly<{ jobToken: string }>

export type RevertPostApplyProofFailureRequestV1 = Readonly<{
  version: typeof POST_APPLY_PROOF_PROTOCOL_VERSION_V1
  projectInstanceId: string
  projectId: string
  expectedRevision: number
  jobToken: string
  expectedLocation: ProofFailureViewModel['location']
  expectedOutcome: 'blocked' | 'unknown'
  expectedReason:
    | 'evidence_insufficient'
    | 'resource_limit'
    | 'cancelled'
    | 'deadline_reached'
    | null
  expectedSubsequentEditCount: number
  expectedUndoStepsToRevert: number | null
  explicitConfirmation: true
}>

/**
 * Intentionally coarse progress. Apart from the required authority binding,
 * this wire view contains no entity IDs, geometry, proof paths, or errors.
 */
export type PostApplyProofProgressV1 =
  PostApplyProofJobRequestV1
  & Readonly<{
    status: PostApplyProofStatusV1
    provenPairCount: number
    totalPairCount: number
    proofFailure: ProofFailureViewModel | null
  }>

export type PostApplyProofSchedulerClientErrorReason =
  | 'invalid_request'
  | 'invalid_response'
  | 'transport_failure'

export class PostApplyProofSchedulerClientError extends Error {
  readonly reason: PostApplyProofSchedulerClientErrorReason

  constructor(reason: PostApplyProofSchedulerClientErrorReason) {
    super(`post-Apply proof scheduler ${reason}`)
    this.name = 'PostApplyProofSchedulerClientError'
    this.reason = reason
  }
}

const START_REQUEST_KEYS = Object.freeze([
  'version',
  'projectInstanceId',
  'projectId',
  'revision',
] as const)

const JOB_REQUEST_KEYS = Object.freeze([
  ...START_REQUEST_KEYS,
  'jobToken',
] as const)

const PROGRESS_KEYS = Object.freeze([
  ...JOB_REQUEST_KEYS,
  'status',
  'provenPairCount',
  'totalPairCount',
  'proofFailure',
] as const)

const NORMALIZED_PROGRESS_V1 = new WeakSet<object>()

const REVERT_REQUEST_KEYS = Object.freeze([
  'version',
  'projectInstanceId',
  'projectId',
  'expectedRevision',
  'jobToken',
  'expectedLocation',
  'expectedOutcome',
  'expectedReason',
  'expectedSubsequentEditCount',
  'expectedUndoStepsToRevert',
  'explicitConfirmation',
] as const)

export function normalizeStartPostApplyProofJobRequestV1(
  value: unknown,
): StartPostApplyProofJobRequestV1 | null {
  const record = ownDataRecord(value, START_REQUEST_KEYS)
  if (!record || !hasValidBinding(record)) return null
  return Object.freeze({
    version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    revision: record.revision,
  })
}

export function normalizePostApplyProofJobRequestV1(
  value: unknown,
): PostApplyProofJobRequestV1 | null {
  const record = ownDataRecord(value, JOB_REQUEST_KEYS)
  if (
    !record
    || !hasValidBinding(record)
    || !isCanonicalNonNilUuid(record.jobToken)
  ) return null
  return Object.freeze({
    version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    revision: record.revision,
    jobToken: record.jobToken,
  })
}

export function normalizePostApplyProofProgressV1(
  value: unknown,
): PostApplyProofProgressV1 | null {
  if (
    typeof value === 'object'
    && value !== null
    && NORMALIZED_PROGRESS_V1.has(value)
  ) {
    return value as PostApplyProofProgressV1
  }
  const record = ownDataRecord(value, PROGRESS_KEYS)
  if (
    !record
    || !hasValidBinding(record)
    || !isCanonicalNonNilUuid(record.jobToken)
    || !isPostApplyProofStatusV1(record.status)
    || !isSafeCount(record.provenPairCount)
    || !isSafeCount(record.totalPairCount)
    || record.totalPairCount === 0
    || record.provenPairCount > record.totalPairCount
    || (
      record.status === 'certified'
        ? record.provenPairCount !== record.totalPairCount
        : record.provenPairCount !== 0
    )
  ) return null
  const proofFailure = record.proofFailure === null
    ? null
    : normalizeProofFailureViewModelV1(record.proofFailure)
  if (terminalFailureReasonV1(record.status) === null) {
    if (record.proofFailure !== null) return null
  } else if (
    !proofFailure
    || proofFailure.reason !== terminalFailureReasonV1(record.status)
  ) {
    return null
  }
  const progress = Object.freeze({
    version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    revision: record.revision,
    jobToken: record.jobToken,
    status: record.status,
    provenPairCount: record.provenPairCount,
    totalPairCount: record.totalPairCount,
    proofFailure,
  })
  NORMALIZED_PROGRESS_V1.add(progress)
  return progress
}

export function normalizeRevertPostApplyProofFailureRequestV1(
  value: unknown,
): RevertPostApplyProofFailureRequestV1 | null {
  const record = ownDataRecord(value, REVERT_REQUEST_KEYS)
  if (
    !record
    || record.version !== POST_APPLY_PROOF_PROTOCOL_VERSION_V1
    || !isCanonicalNonNilUuid(record.projectInstanceId)
    || !isCanonicalNonNilUuid(record.projectId)
    || !isSafeCount(record.expectedRevision)
    || !isCanonicalNonNilUuid(record.jobToken)
    || record.expectedLocation !== 'applied_retained_undo'
    || !isSafeCount(record.expectedSubsequentEditCount)
    || !isSafeCount(record.expectedUndoStepsToRevert)
    || record.expectedUndoStepsToRevert === 0
    || record.expectedUndoStepsToRevert > 0xffff_ffff
    || record.expectedSubsequentEditCount === Number.MAX_SAFE_INTEGER
    || record.expectedUndoStepsToRevert
      !== record.expectedSubsequentEditCount + 1
    || record.explicitConfirmation !== true
  ) return null
  if (
    record.expectedOutcome === 'blocked'
      ? record.expectedReason !== null
      : record.expectedOutcome !== 'unknown'
        || !isNativeUnknownReasonV1(record.expectedReason)
  ) return null
  const expectedOutcome = record.expectedOutcome === 'blocked'
    ? 'blocked'
    : 'unknown'
  const expectedReason = expectedOutcome === 'blocked'
    ? null
    : record.expectedReason as Exclude<
        RevertPostApplyProofFailureRequestV1['expectedReason'],
        null
      >
  return Object.freeze({
    version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    expectedRevision: record.expectedRevision,
    jobToken: record.jobToken,
    expectedLocation: record.expectedLocation,
    expectedOutcome,
    expectedReason,
    expectedSubsequentEditCount: record.expectedSubsequentEditCount,
    expectedUndoStepsToRevert: record.expectedUndoStepsToRevert,
    explicitConfirmation: true,
  })
}

export function createRevertPostApplyProofFailureRequestV1(
  progress: PostApplyProofProgressV1,
  expectedRevision: number,
  failure: ProofFailureViewModel,
): RevertPostApplyProofFailureRequestV1 | null {
  if (
    progress.proofFailure === null
    || !sameProofFailure(progress.proofFailure, failure)
  ) return null
  const expectedOutcome = failure.reason === 'blocked' ? 'blocked' : 'unknown'
  const expectedReason = nativeUnknownReasonV1(failure.reason)
  return normalizeRevertPostApplyProofFailureRequestV1({
    version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
    projectInstanceId: progress.projectInstanceId,
    projectId: progress.projectId,
    expectedRevision,
    jobToken: progress.jobToken,
    expectedLocation: failure.location,
    expectedOutcome,
    expectedReason,
    expectedSubsequentEditCount: failure.subsequentEditCount,
    expectedUndoStepsToRevert: failure.undoStepsToRevert,
    explicitConfirmation: true,
  })
}

function terminalFailureReasonV1(
  status: PostApplyProofStatusV1,
): ProofFailureViewModel['reason'] | null {
  switch (status) {
    case 'blocked':
      return 'blocked'
    case 'unknown_evidence_insufficient':
      return 'evidence_insufficient'
    case 'unknown_resource_limit':
      return 'resource_limit'
    case 'unknown_cancelled':
      return 'cancelled'
    case 'unknown_deadline_reached':
      return 'deadline'
    case 'proving':
    case 'certified':
    case 'stale':
    default:
      return null
  }
}

export function isPostApplyProofStatusV1(
  value: unknown,
): value is PostApplyProofStatusV1 {
  return POST_APPLY_PROOF_STATUSES_V1.some((status) => status === value)
}

type InvokeCommand = (
  command: string,
  args?: Readonly<Record<string, unknown>>,
) => Promise<unknown>

export type PostApplyProofSchedulerClientV1 = Readonly<{
  start(
    request: StartPostApplyProofJobRequestV1,
  ): Promise<PostApplyProofProgressV1>
  poll(request: PostApplyProofJobRequestV1): Promise<PostApplyProofProgressV1>
  cancel(request: PostApplyProofJobRequestV1): Promise<void>
  revert(request: RevertPostApplyProofFailureRequestV1): Promise<number>
}>

export function createPostApplyProofSchedulerClientV1(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): PostApplyProofSchedulerClientV1 {
  const start = async (
    request: StartPostApplyProofJobRequestV1,
  ): Promise<PostApplyProofProgressV1> => {
    const normalized = normalizeStartPostApplyProofJobRequestV1(request)
    if (!normalized) throw clientError('invalid_request')
    const raw = await invokeRedacted(
      invokeCommand,
      'start_post_apply_proof_job_v1',
      normalized,
    )
    const progress = normalizePostApplyProofProgressV1(raw)
    if (!progress || !sameProjectBinding(progress, normalized)) {
      throw clientError('invalid_response')
    }
    return progress
  }

  const poll = async (
    request: PostApplyProofJobRequestV1,
  ): Promise<PostApplyProofProgressV1> => {
    const normalized = normalizePostApplyProofJobRequestV1(request)
    if (!normalized) throw clientError('invalid_request')
    const raw = await invokeRedacted(
      invokeCommand,
      'poll_post_apply_proof_job_v1',
      normalized,
    )
    const progress = normalizePostApplyProofProgressV1(raw)
    if (!progress || !sameJobBinding(progress, normalized)) {
      throw clientError('invalid_response')
    }
    return progress
  }

  const cancel = async (
    request: PostApplyProofJobRequestV1,
  ): Promise<void> => {
    const normalized = normalizePostApplyProofJobRequestV1(request)
    if (!normalized) throw clientError('invalid_request')
    await invokeRedacted(
      invokeCommand,
      'cancel_post_apply_proof_job_v1',
      normalized,
    )
  }

  const revert = async (
    request: RevertPostApplyProofFailureRequestV1,
  ): Promise<number> => {
    const normalized = normalizeRevertPostApplyProofFailureRequestV1(request)
    if (!normalized) throw clientError('invalid_request')
    const revision = await invokeRedacted(
      invokeCommand,
      'revert_post_apply_proof_failure_v1',
      normalized,
    )
    if (!isSafeCount(revision)) throw clientError('invalid_response')
    return revision
  }

  return Object.freeze({ start, poll, cancel, revert })
}

const defaultClient = createPostApplyProofSchedulerClientV1()

export const startPostApplyProofJobV1 = defaultClient.start
export const pollPostApplyProofJobV1 = defaultClient.poll
export const cancelPostApplyProofJobV1 = defaultClient.cancel
export const revertPostApplyProofFailureV1 = defaultClient.revert

function hasValidBinding(
  record: Readonly<Record<string, unknown>>,
): record is Readonly<{
  version: 1
  projectInstanceId: string
  projectId: string
  revision: number
}> {
  return record.version === POST_APPLY_PROOF_PROTOCOL_VERSION_V1
    && isCanonicalNonNilUuid(record.projectInstanceId)
    && isCanonicalNonNilUuid(record.projectId)
    && isSafeCount(record.revision)
}

function isSafeCount(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && Number(value) >= 0
    && !Object.is(value, -0)
}

function sameProjectBinding(
  left: StartPostApplyProofJobRequestV1,
  right: StartPostApplyProofJobRequestV1,
): boolean {
  return left.version === right.version
    && left.projectInstanceId === right.projectInstanceId
    && left.projectId === right.projectId
    && left.revision === right.revision
}

export function samePostApplyProofJobBindingV1(
  left: PostApplyProofJobRequestV1,
  right: PostApplyProofJobRequestV1,
): boolean {
  return sameProjectBinding(left, right)
    && left.jobToken === right.jobToken
}

function sameJobBinding(
  left: PostApplyProofJobRequestV1,
  right: PostApplyProofJobRequestV1,
): boolean {
  return samePostApplyProofJobBindingV1(left, right)
}

async function invokeRedacted(
  invokeCommand: InvokeCommand,
  command: string,
  request:
    | StartPostApplyProofJobRequestV1
    | PostApplyProofJobRequestV1
    | RevertPostApplyProofFailureRequestV1,
): Promise<unknown> {
  try {
    return await invokeCommand(command, Object.freeze({ request }))
  } catch {
    throw clientError('transport_failure')
  }
}

function isNativeUnknownReasonV1(
  value: unknown,
): value is Exclude<
  RevertPostApplyProofFailureRequestV1['expectedReason'],
  null
> {
  return value === 'evidence_insufficient'
    || value === 'resource_limit'
    || value === 'cancelled'
    || value === 'deadline_reached'
}

function nativeUnknownReasonV1(
  reason: ProofFailureViewModel['reason'],
): RevertPostApplyProofFailureRequestV1['expectedReason'] {
  switch (reason) {
    case 'evidence_insufficient':
    case 'resource_limit':
    case 'cancelled':
      return reason
    case 'deadline':
      return 'deadline_reached'
    case 'blocked':
    default:
      return null
  }
}

function sameProofFailure(
  left: ProofFailureViewModel,
  right: ProofFailureViewModel,
): boolean {
  return left.location === right.location
    && left.reason === right.reason
    && left.subsequentEditCount === right.subsequentEditCount
    && left.undoStepsToRevert === right.undoStepsToRevert
}

function clientError(
  reason: PostApplyProofSchedulerClientErrorReason,
): PostApplyProofSchedulerClientError {
  return new PostApplyProofSchedulerClientError(reason)
}

function ownDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Record<Keys[number], unknown> | null {
  if (typeof value !== 'object' || value === null) return null
  try {
    if (Array.isArray(value)) return null
  } catch {
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
