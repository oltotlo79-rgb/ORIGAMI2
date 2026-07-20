import { invoke } from '@tauri-apps/api/core'

import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { ProjectSnapshot } from './coreClient.ts'
import { normalizeGeometricConstraintDocument } from './geometricConstraints.ts'
import { normalizeProjectLayerDocument } from './projectLayers.ts'

export const RECOVERY_SCHEMA_VERSION = 1 as const

export type RecoveryCandidateNone = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'none'
}>

export type RecoveryCandidateInvalid = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'invalid'
  recovery_id: string
}>

export type RecoveryCandidateAvailable = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'available'
  recovery_id: string
  project_id: string
  updated_at_unix_ms: number | null
}>

export type RecoveryCandidate =
  | RecoveryCandidateNone
  | RecoveryCandidateInvalid
  | RecoveryCandidateAvailable

export type RecoveryExpectedProjectBinding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

export type RecoveryDiscardedResponse = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'discarded'
}>

export type WindowCloseAuthorization = 'clean' | 'discard_confirmed'

export type PreparedWindowCloseResponse = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'prepared'
  close_prepare_id: string
  project_instance_id: string
  project_id: string
  revision: number
  authorization: WindowCloseAuthorization
}>

export type CanceledWindowCloseResponse = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: 'canceled'
  close_prepare_id: string
  project_instance_id: string
  project_id: string
  revision: number
  authorization: WindowCloseAuthorization
}>

export type WindowCloseProjectState = RecoveryExpectedProjectBinding & Readonly<{
  is_dirty: boolean
}>

export type RecoveryNativeInvoke = (
  command: string,
  args?: Readonly<Record<string, unknown>>,
) => Promise<unknown>

export type RecoveryClient = Readonly<{
  getCandidate: () => Promise<RecoveryCandidate>
  restore: (
    candidate: RecoveryCandidateAvailable,
    expected: RecoveryExpectedProjectBinding,
  ) => Promise<ProjectSnapshot>
  discard: (
    candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid,
  ) => Promise<RecoveryDiscardedResponse>
  prepareWindowClose: (
    expected: RecoveryExpectedProjectBinding,
    authorization: WindowCloseAuthorization,
  ) => Promise<PreparedWindowCloseResponse>
  cancelWindowClose: (
    prepared: PreparedWindowCloseResponse,
  ) => Promise<CanceledWindowCloseResponse>
}>

export type RecoveryClientErrorCode =
  | 'invalid_request'
  | 'native_unavailable'
  | 'invalid_response'

const ERROR_MESSAGES: Readonly<Record<RecoveryClientErrorCode, string>> = {
  invalid_request: '復旧処理の前提情報が正しくありません。',
  native_unavailable: '復旧データを処理できませんでした。',
  invalid_response: '復旧データの応答を確認できませんでした。',
}

/**
 * A deliberately redacted boundary error. Native errors, paths, document
 * contents, and malformed response values are never retained as a cause.
 */
export class RecoveryClientError extends Error {
  readonly code: RecoveryClientErrorCode

  constructor(code: RecoveryClientErrorCode) {
    super(ERROR_MESSAGES[code])
    this.name = 'RecoveryClientError'
    this.code = code
  }
}

const PROJECT_SNAPSHOT_KEYS = [
  'project_instance_id',
  'project_id',
  'name',
  'current_path',
  'revision',
  'saved_revision',
  'is_dirty',
  'paper',
  'crease_pattern',
  'instruction_timeline',
  'numeric_expressions',
  'geometric_constraints',
  'project_layers',
  'fold_model_fingerprint',
  'can_undo',
  'can_redo',
  'cutting_allowed',
] as const

const MAX_PATTERN_VERTICES = 1_000_000
const MAX_PATTERN_EDGES = 1_000_000
const MAX_BOUNDARY_VERTICES = 1_000_000
const MAX_INSTRUCTION_STEPS = 512
const MAX_INSTRUCTION_HINGES_PER_STEP = 10_000
const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/u

const defaultNativeInvoke: RecoveryNativeInvoke = (command, args) =>
  invoke<unknown>(command, args === undefined ? undefined : { ...args })

const defaultClient = createRecoveryClient()

export type WindowCloseBlocker = 'recovery' | 'core' | null

export type WindowCloseHandshakeState = {
  mounted: boolean
  in_flight: boolean
  allow_once: boolean
  close_committed: boolean
  interaction_locked: boolean
  generation: number
  attempt_generation: number | null
}

export type WindowCloseHandshakeDependencies = Readonly<{
  getBlocker: () => WindowCloseBlocker
  getProjectState: () => unknown
  confirmDiscard: () => boolean
  prepare: (
    expected: RecoveryExpectedProjectBinding,
    authorization: WindowCloseAuthorization,
  ) => Promise<PreparedWindowCloseResponse>
  cancel: (
    prepared: PreparedWindowCloseResponse,
  ) => Promise<CanceledWindowCloseResponse>
  requestClose: () => Promise<void>
  setInteractionLocked: (locked: boolean) => void
  setStatus: (message: string) => void
  reportFailure: () => void
}>

export type WindowCloseRequestedEvent = Readonly<{
  preventDefault: () => void
}>

export type WindowCloseHandshake = Readonly<{
  handle: (event: WindowCloseRequestedEvent) => void
  dispose: () => void
}>

export const WINDOW_CLOSE_STATUS = Object.freeze({
  recoveryBlocked: '復旧データの確認が完了してから終了してください。',
  coreBlocked: '処理が完了してから終了してください。',
  cancelled: '終了をキャンセルしました。編集を続けられます。',
  preparing: '終了前の復旧データを安全に整理しています…',
  stale: '終了準備中にプロジェクトが変更されました。もう一度終了してください。',
  failed: '終了準備を完了できませんでした。アプリを開いたまま、もう一度お試しください。',
} as const)

export function getRecoveryCandidate(): Promise<RecoveryCandidate> {
  return defaultClient.getCandidate()
}

export function restoreRecoveryCandidate(
  candidate: RecoveryCandidateAvailable,
  expected: RecoveryExpectedProjectBinding,
): Promise<ProjectSnapshot> {
  return defaultClient.restore(candidate, expected)
}

export function discardRecoveryCandidate(
  candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid,
): Promise<RecoveryDiscardedResponse> {
  return defaultClient.discard(candidate)
}

export function prepareWindowClose(
  expected: RecoveryExpectedProjectBinding,
  authorization: WindowCloseAuthorization,
): Promise<PreparedWindowCloseResponse> {
  return defaultClient.prepareWindowClose(expected, authorization)
}

export function cancelWindowClosePrepare(
  prepared: PreparedWindowCloseResponse,
): Promise<CanceledWindowCloseResponse> {
  return defaultClient.cancelWindowClose(prepared)
}

export function createWindowCloseHandshakeState(): WindowCloseHandshakeState {
  return {
    mounted: false,
    in_flight: false,
    allow_once: false,
    close_committed: false,
    interaction_locked: false,
    generation: 0,
    attempt_generation: null,
  }
}

/**
 * Owns the fail-closed two-event close protocol. The first event is always
 * cancelled synchronously. Only a native prepared acknowledgement can arm
 * one subsequent event, and that permission is consumed before it is used.
 */
export function createWindowCloseHandshake(
  state: WindowCloseHandshakeState,
  dependencies: WindowCloseHandshakeDependencies,
): WindowCloseHandshake {
  state.generation += 1
  state.mounted = true
  state.allow_once = false
  state.close_committed = false

  const handle = (event: WindowCloseRequestedEvent) => {
    if (state.allow_once) {
      state.allow_once = false
      state.in_flight = false
      state.close_committed = true
      state.attempt_generation = null
      state.generation += 1
      // Native consumes the armed token and owns the bounded final clear from
      // this point. If it prevents exit on failure, autosave stays active and
      // the still-open editor must not remain permanently locked.
      setWindowCloseInteractionLocked(state, dependencies, false)
      return
    }

    // The first event in every attempt is cancelled before confirmation,
    // snapshot reads, native invocation, or any other fallible work.
    event.preventDefault()
    if (!state.mounted) return

    const blocker = readWindowCloseBlocker(dependencies)
    if (blocker === 'recovery') {
      dependencies.setStatus(WINDOW_CLOSE_STATUS.recoveryBlocked)
      return
    }
    if (blocker === 'core') {
      dependencies.setStatus(WINDOW_CLOSE_STATUS.coreBlocked)
      return
    }
    if (blocker !== null) {
      failWindowCloseAttempt(state, dependencies)
      return
    }
    if (state.in_flight) {
      dependencies.setStatus(WINDOW_CLOSE_STATUS.preparing)
      return
    }

    const project = parseWindowCloseProjectState(
      safelyGetProjectState(dependencies),
    )
    if (!project) {
      failWindowCloseAttempt(state, dependencies)
      return
    }

    let authorization: WindowCloseAuthorization = 'clean'
    if (project.is_dirty) {
      let confirmed = false
      try {
        confirmed = dependencies.confirmDiscard() === true
      } catch {
        failWindowCloseAttempt(state, dependencies)
        return
      }
      if (!confirmed) {
        dependencies.setStatus(WINDOW_CLOSE_STATUS.cancelled)
        return
      }
      authorization = 'discard_confirmed'
    }

    const expected = Object.freeze({
      project_instance_id: project.project_instance_id,
      project_id: project.project_id,
      revision: project.revision,
    })
    state.in_flight = true
    state.close_committed = false
    state.generation += 1
    const generation = state.generation
    state.attempt_generation = generation
    setWindowCloseInteractionLocked(state, dependencies, true)
    dependencies.setStatus(WINDOW_CLOSE_STATUS.preparing)

    void runWindowCloseAttempt(
      state,
      dependencies,
      generation,
      expected,
      authorization,
    )
  }

  return Object.freeze({
    handle,
    dispose() {
      if (!state.mounted) return
      state.mounted = false
      state.allow_once = false
      state.generation += 1
      if (!state.in_flight) {
        setWindowCloseInteractionLocked(state, dependencies, false)
      }
    },
  })
}

export function createRecoveryClient(
  nativeInvoke: RecoveryNativeInvoke = defaultNativeInvoke,
): RecoveryClient {
  return Object.freeze({
    async getCandidate() {
      let response: unknown
      try {
        response = await nativeInvoke('get_recovery_candidate')
      } catch {
        throw new RecoveryClientError('native_unavailable')
      }
      const candidate = parseRecoveryCandidate(response)
      if (!candidate) throw new RecoveryClientError('invalid_response')
      return candidate
    },

    async restore(candidate, expected) {
      const parsedCandidate = parseRecoveryCandidate(candidate)
      const parsedExpected = parseExpectedBinding(expected)
      if (parsedCandidate?.status !== 'available' || !parsedExpected) {
        throw new RecoveryClientError('invalid_request')
      }

      const request = Object.freeze({
        schema_version: RECOVERY_SCHEMA_VERSION,
        recovery_id: parsedCandidate.recovery_id,
        expected_project_id: parsedExpected.project_id,
        expected_instance_id: parsedExpected.project_instance_id,
        expected_revision: parsedExpected.revision,
      })
      let response: unknown
      try {
        response = await nativeInvoke('restore_recovery', { request })
      } catch {
        throw new RecoveryClientError('native_unavailable')
      }
      const snapshot = parseRestoredRecoverySnapshot(
        response,
        parsedCandidate,
        parsedExpected,
      )
      if (!snapshot) throw new RecoveryClientError('invalid_response')
      return snapshot
    },

    async discard(candidate) {
      const parsedCandidate = parseRecoveryCandidate(candidate)
      if (
        !parsedCandidate
        || (parsedCandidate.status !== 'available'
          && parsedCandidate.status !== 'invalid')
      ) {
        throw new RecoveryClientError('invalid_request')
      }

      const request = Object.freeze({
        schema_version: RECOVERY_SCHEMA_VERSION,
        recovery_id: parsedCandidate.recovery_id,
      })
      let response: unknown
      try {
        response = await nativeInvoke('discard_recovery', { request })
      } catch {
        throw new RecoveryClientError('native_unavailable')
      }
      const discarded = parseDiscardedResponse(response)
      if (!discarded) throw new RecoveryClientError('invalid_response')
      return discarded
    },

    async prepareWindowClose(expected, authorization) {
      const parsedExpected = parseExpectedBinding(expected)
      if (!parsedExpected || !isWindowCloseAuthorization(authorization)) {
        throw new RecoveryClientError('invalid_request')
      }
      const request = Object.freeze({
        schema_version: RECOVERY_SCHEMA_VERSION,
        project_instance_id: parsedExpected.project_instance_id,
        project_id: parsedExpected.project_id,
        revision: parsedExpected.revision,
        authorization,
      })
      let response: unknown
      try {
        response = await nativeInvoke('prepare_window_close', { request })
      } catch {
        throw new RecoveryClientError('native_unavailable')
      }
      const prepared = parsePreparedWindowCloseResponse(
        response,
        parsedExpected,
        authorization,
      )
      if (!prepared) throw new RecoveryClientError('invalid_response')
      return prepared
    },

    async cancelWindowClose(prepared) {
      const parsedPrepared = parsePreparedWindowCloseValue(prepared)
      if (!parsedPrepared) throw new RecoveryClientError('invalid_request')
      const request = Object.freeze({
        schema_version: RECOVERY_SCHEMA_VERSION,
        close_prepare_id: parsedPrepared.close_prepare_id,
        project_instance_id: parsedPrepared.project_instance_id,
        project_id: parsedPrepared.project_id,
        revision: parsedPrepared.revision,
        authorization: parsedPrepared.authorization,
      })
      let response: unknown
      try {
        response = await nativeInvoke('cancel_window_close_prepare', { request })
      } catch {
        throw new RecoveryClientError('native_unavailable')
      }
      const canceled = parseCanceledWindowCloseResponse(
        response,
        parsedPrepared,
      )
      if (!canceled) throw new RecoveryClientError('invalid_response')
      return canceled
    },
  })
}

/**
 * Strictly admits only the versioned, data-only recovery discovery DTO.
 * Every accepted value is detached from the untrusted native object.
 */
export function parseRecoveryCandidate(value: unknown): RecoveryCandidate | null {
  try {
    const record = snapshotDataRecord(value)
    if (
      !record
      || record.schema_version !== RECOVERY_SCHEMA_VERSION
      || typeof record.status !== 'string'
    ) return null

    switch (record.status) {
      case 'none':
        return hasExactKeys(record, ['schema_version', 'status'])
          ? Object.freeze({
              schema_version: RECOVERY_SCHEMA_VERSION,
              status: 'none',
            })
          : null
      case 'invalid':
        if (
          !hasExactKeys(record, [
            'schema_version',
            'status',
            'recovery_id',
          ])
          || !isCanonicalNonNilUuid(record.recovery_id)
        ) return null
        return Object.freeze({
          schema_version: RECOVERY_SCHEMA_VERSION,
          status: 'invalid',
          recovery_id: record.recovery_id,
        })
      case 'available':
        if (
          !hasExactKeys(record, [
            'schema_version',
            'status',
            'recovery_id',
            'project_id',
            'updated_at_unix_ms',
          ])
          || !isCanonicalNonNilUuid(record.recovery_id)
          || !isCanonicalNonNilUuid(record.project_id)
          || (
            record.updated_at_unix_ms !== null
            && !isNonNegativeSafeInteger(record.updated_at_unix_ms)
          )
        ) return null
        return Object.freeze({
          schema_version: RECOVERY_SCHEMA_VERSION,
          status: 'available',
          recovery_id: record.recovery_id,
          project_id: record.project_id,
          updated_at_unix_ms: record.updated_at_unix_ms,
        })
      default:
        return null
    }
  } catch {
    return null
  }
}

/**
 * Validates replacement semantics in addition to the complete native snapshot
 * envelope. A recovery must keep the persisted project ID, issue a fresh
 * runtime instance ID, and open as a dirty, pathless project.
 */
export function parseRestoredRecoverySnapshot(
  value: unknown,
  candidate: unknown,
  expected: unknown,
): ProjectSnapshot | null {
  try {
    const parsedCandidate = parseRecoveryCandidate(candidate)
    const parsedExpected = parseExpectedBinding(expected)
    const record = exactDataRecord(value, PROJECT_SNAPSHOT_KEYS)
    if (
      parsedCandidate?.status !== 'available'
      || !parsedExpected
      || !record
      || !isCanonicalNonNilUuid(record.project_instance_id)
      || record.project_instance_id === parsedExpected.project_instance_id
      || record.project_id !== parsedCandidate.project_id
      || typeof record.name !== 'string'
      || record.current_path !== null
      || record.revision !== 0
      || !isNullableRevision(record.saved_revision)
      || record.saved_revision !== null
      || record.is_dirty !== true
      || record.can_undo !== false
      || record.can_redo !== false
      || typeof record.cutting_allowed !== 'boolean'
      || typeof record.fold_model_fingerprint !== 'string'
      || !FINGERPRINT_PATTERN.test(record.fold_model_fingerprint)
    ) return null

    const paper = parsePaper(record.paper)
    const creasePattern = parseCreasePattern(record.crease_pattern)
    const instructionTimeline = parseInstructionTimeline(
      record.instruction_timeline,
    )
    const numericExpressions = parseNumericExpressions(
      record.numeric_expressions,
    )
    const geometricConstraints = normalizeGeometricConstraintDocument(
      record.geometric_constraints,
    )
    const projectLayers = normalizeProjectLayerDocument(
      record.project_layers,
      creasePattern?.edges ?? [],
    )
    if (
      !paper
      || !creasePattern
      || !instructionTimeline
      || !numericExpressions
      || !geometricConstraints
      || !projectLayers
      || paper.cutting_allowed !== record.cutting_allowed
    ) return null

    return Object.freeze({
      project_instance_id: record.project_instance_id,
      project_id: parsedCandidate.project_id,
      name: record.name,
      current_path: null,
      revision: 0,
      saved_revision: null,
      is_dirty: true,
      paper,
      crease_pattern: creasePattern,
      instruction_timeline: instructionTimeline,
      numeric_expressions: numericExpressions,
      geometric_constraints: geometricConstraints,
      project_layers: projectLayers,
      fold_model_fingerprint: record.fold_model_fingerprint,
      can_undo: false,
      can_redo: false,
      cutting_allowed: record.cutting_allowed,
    })
  } catch {
    return null
  }
}

/**
 * Strictly admits the pathless project snapshot returned by expanded-folder
 * I/O. Unlike recovery restore, this accepts any valid revision/dirty state,
 * but it never accepts a native filesystem path.
 */
export function parsePathlessProjectSnapshot(
  value: unknown,
): ProjectSnapshot | null {
  try {
    const record = exactDataRecord(value, PROJECT_SNAPSHOT_KEYS)
    if (
      !record
      || !isCanonicalNonNilUuid(record.project_instance_id)
      || !isCanonicalNonNilUuid(record.project_id)
      || typeof record.name !== 'string'
      || record.current_path !== null
      || !isNonNegativeSafeInteger(record.revision)
      || !isNullableRevision(record.saved_revision)
      || typeof record.is_dirty !== 'boolean'
      || typeof record.can_undo !== 'boolean'
      || typeof record.can_redo !== 'boolean'
      || typeof record.cutting_allowed !== 'boolean'
      || typeof record.fold_model_fingerprint !== 'string'
      || !FINGERPRINT_PATTERN.test(record.fold_model_fingerprint)
    ) return null

    const paper = parsePaper(record.paper)
    const creasePattern = parseCreasePattern(record.crease_pattern)
    const instructionTimeline = parseInstructionTimeline(
      record.instruction_timeline,
    )
    const numericExpressions = parseNumericExpressions(
      record.numeric_expressions,
    )
    const geometricConstraints = normalizeGeometricConstraintDocument(
      record.geometric_constraints,
    )
    const projectLayers = normalizeProjectLayerDocument(
      record.project_layers,
      creasePattern?.edges ?? [],
    )
    if (
      !paper
      || !creasePattern
      || !instructionTimeline
      || !numericExpressions
      || !geometricConstraints
      || !projectLayers
      || paper.cutting_allowed !== record.cutting_allowed
    ) return null

    return Object.freeze({
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      name: record.name,
      current_path: null,
      revision: record.revision,
      saved_revision: record.saved_revision,
      is_dirty: record.is_dirty,
      paper,
      crease_pattern: creasePattern,
      instruction_timeline: instructionTimeline,
      numeric_expressions: numericExpressions,
      geometric_constraints: geometricConstraints,
      project_layers: projectLayers,
      fold_model_fingerprint: record.fold_model_fingerprint,
      can_undo: record.can_undo,
      can_redo: record.can_redo,
      cutting_allowed: record.cutting_allowed,
    })
  } catch {
    return null
  }
}

function parseDiscardedResponse(
  value: unknown,
): RecoveryDiscardedResponse | null {
  try {
    const record = exactDataRecord(value, ['schema_version', 'status'])
    if (
      !record
      || record.schema_version !== RECOVERY_SCHEMA_VERSION
      || record.status !== 'discarded'
    ) return null
    return Object.freeze({
      schema_version: RECOVERY_SCHEMA_VERSION,
      status: 'discarded',
    })
  } catch {
    return null
  }
}

export function parsePreparedWindowCloseResponse(
  value: unknown,
  expected: unknown,
  authorization: unknown,
): PreparedWindowCloseResponse | null {
  try {
    const parsedExpected = parseExpectedBinding(expected)
    const prepared = parsePreparedWindowCloseValue(value)
    if (
      !parsedExpected
      || !isWindowCloseAuthorization(authorization)
      || !prepared
      || prepared.project_instance_id !== parsedExpected.project_instance_id
      || prepared.project_id !== parsedExpected.project_id
      || prepared.revision !== parsedExpected.revision
      || prepared.authorization !== authorization
    ) return null
    return prepared
  } catch {
    return null
  }
}

function parsePreparedWindowCloseValue(
  value: unknown,
): PreparedWindowCloseResponse | null {
  try {
    const record = exactDataRecord(value, [
      'schema_version',
      'status',
      'close_prepare_id',
      'project_instance_id',
      'project_id',
      'revision',
      'authorization',
    ])
    if (
      !record
      || record.schema_version !== RECOVERY_SCHEMA_VERSION
      || record.status !== 'prepared'
      || !isCanonicalNonNilUuid(record.close_prepare_id)
      || !isCanonicalNonNilUuid(record.project_instance_id)
      || !isCanonicalNonNilUuid(record.project_id)
      || !isNonNegativeSafeInteger(record.revision)
      || !isWindowCloseAuthorization(record.authorization)
    ) return null
    return Object.freeze({
      schema_version: RECOVERY_SCHEMA_VERSION,
      status: 'prepared',
      close_prepare_id: record.close_prepare_id,
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
      authorization: record.authorization,
    })
  } catch {
    return null
  }
}

export function parseCanceledWindowCloseResponse(
  value: unknown,
  prepared: unknown,
): CanceledWindowCloseResponse | null {
  try {
    const parsedPrepared = parsePreparedWindowCloseValue(prepared)
    const record = exactDataRecord(value, [
      'schema_version',
      'status',
      'close_prepare_id',
      'project_instance_id',
      'project_id',
      'revision',
      'authorization',
    ])
    if (
      !parsedPrepared
      || !record
      || record.schema_version !== RECOVERY_SCHEMA_VERSION
      || record.status !== 'canceled'
      || record.close_prepare_id !== parsedPrepared.close_prepare_id
      || record.project_instance_id !== parsedPrepared.project_instance_id
      || record.project_id !== parsedPrepared.project_id
      || record.revision !== parsedPrepared.revision
      || record.authorization !== parsedPrepared.authorization
    ) return null
    return Object.freeze({
      schema_version: RECOVERY_SCHEMA_VERSION,
      status: 'canceled',
      close_prepare_id: parsedPrepared.close_prepare_id,
      project_instance_id: parsedPrepared.project_instance_id,
      project_id: parsedPrepared.project_id,
      revision: parsedPrepared.revision,
      authorization: parsedPrepared.authorization,
    })
  } catch {
    return null
  }
}

function parseExpectedBinding(
  value: unknown,
): RecoveryExpectedProjectBinding | null {
  try {
    const record = exactDataRecord(value, [
      'project_instance_id',
      'project_id',
      'revision',
    ])
    if (
      !record
      || !isCanonicalNonNilUuid(record.project_instance_id)
      || !isCanonicalNonNilUuid(record.project_id)
      || !isNonNegativeSafeInteger(record.revision)
    ) return null
    return Object.freeze({
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
    })
  } catch {
    return null
  }
}

function parseWindowCloseProjectState(
  value: unknown,
): WindowCloseProjectState | null {
  try {
    const record = exactDataRecord(value, [
      'project_instance_id',
      'project_id',
      'revision',
      'is_dirty',
    ])
    if (
      !record
      || !isCanonicalNonNilUuid(record.project_instance_id)
      || !isCanonicalNonNilUuid(record.project_id)
      || !isNonNegativeSafeInteger(record.revision)
      || typeof record.is_dirty !== 'boolean'
    ) return null
    return Object.freeze({
      project_instance_id: record.project_instance_id,
      project_id: record.project_id,
      revision: record.revision,
      is_dirty: record.is_dirty,
    })
  } catch {
    return null
  }
}

function safelyGetProjectState(
  dependencies: WindowCloseHandshakeDependencies,
): unknown {
  try {
    return dependencies.getProjectState()
  } catch {
    return null
  }
}

function readWindowCloseBlocker(
  dependencies: WindowCloseHandshakeDependencies,
): WindowCloseBlocker | 'invalid' {
  try {
    const blocker = dependencies.getBlocker()
    return blocker === null || blocker === 'recovery' || blocker === 'core'
      ? blocker
      : 'invalid'
  } catch {
    return 'invalid'
  }
}

function sameExpectedProject(
  current: WindowCloseProjectState,
  expected: RecoveryExpectedProjectBinding,
): boolean {
  return current.project_instance_id === expected.project_instance_id
    && current.project_id === expected.project_id
    && current.revision === expected.revision
}

async function runWindowCloseAttempt(
  state: WindowCloseHandshakeState,
  dependencies: WindowCloseHandshakeDependencies,
  generation: number,
  expected: RecoveryExpectedProjectBinding,
  authorization: WindowCloseAuthorization,
): Promise<void> {
  let prepared: PreparedWindowCloseResponse
  try {
    prepared = await dependencies.prepare(expected, authorization)
  } catch {
    finishWindowCloseAttempt(
      state,
      dependencies,
      generation,
      WINDOW_CLOSE_STATUS.failed,
      true,
    )
    return
  }

  if (!ownsWindowCloseAttempt(state, generation)) {
    const canceled = await cancelPreparedWindowClose(dependencies, prepared)
    if (!canceled) dependencies.reportFailure()
    if (!state.close_committed) {
      finishWindowCloseAttempt(state, dependencies, generation, null, false)
    }
    return
  }

  const current = parseWindowCloseProjectState(
    safelyGetProjectState(dependencies),
  )
  if (
    !current
    || !sameExpectedProject(current, expected)
    || (authorization === 'clean' && current.is_dirty)
    || readWindowCloseBlocker(dependencies) !== null
  ) {
    const canceled = await cancelPreparedWindowClose(dependencies, prepared)
    if (!canceled) dependencies.reportFailure()
    finishWindowCloseAttempt(
      state,
      dependencies,
      generation,
      WINDOW_CLOSE_STATUS.stale,
      false,
    )
    return
  }

  state.allow_once = true
  try {
    await dependencies.requestClose()
  } catch {
    if (state.close_committed) return
    state.allow_once = false
    await cancelPreparedWindowClose(dependencies, prepared)
    finishWindowCloseAttempt(
      state,
      dependencies,
      generation,
      WINDOW_CLOSE_STATUS.failed,
      true,
    )
    return
  }

  if (state.close_committed) return
  state.allow_once = false
  await cancelPreparedWindowClose(dependencies, prepared)
  finishWindowCloseAttempt(
    state,
    dependencies,
    generation,
    WINDOW_CLOSE_STATUS.failed,
    true,
  )
}

async function cancelPreparedWindowClose(
  dependencies: WindowCloseHandshakeDependencies,
  prepared: PreparedWindowCloseResponse,
): Promise<boolean> {
  try {
    await dependencies.cancel(prepared)
    return true
  } catch {
    // Prepare only arms a short-lived token; it neither clears recovery nor
    // stops autosave. A failed best-effort cancel therefore remains safe to
    // unlock, and the token expires natively without recovery-data loss.
    return false
  }
}

function ownsWindowCloseAttempt(
  state: WindowCloseHandshakeState,
  generation: number,
): boolean {
  return state.mounted
    && state.in_flight
    && state.generation === generation
}

function finishWindowCloseAttempt(
  state: WindowCloseHandshakeState,
  dependencies: WindowCloseHandshakeDependencies,
  generation: number,
  status: string | null,
  reportFailure: boolean,
): void {
  // A canceled old lifecycle may finish after a newly mounted lifecycle has
  // started. It may revoke its own native token, but it must never unlock or
  // mutate a newer attempt (ABA guard).
  if (state.attempt_generation !== generation) return
  state.allow_once = false
  state.in_flight = false
  state.attempt_generation = null
  setWindowCloseInteractionLocked(state, dependencies, false)
  if (!state.mounted || state.generation !== generation) return
  if (reportFailure) dependencies.reportFailure()
  if (status !== null) dependencies.setStatus(status)
}

function setWindowCloseInteractionLocked(
  state: WindowCloseHandshakeState,
  dependencies: WindowCloseHandshakeDependencies,
  locked: boolean,
): void {
  if (state.interaction_locked === locked) return
  dependencies.setInteractionLocked(locked)
  state.interaction_locked = locked
}

function failWindowCloseAttempt(
  state: WindowCloseHandshakeState,
  dependencies: WindowCloseHandshakeDependencies,
): void {
  state.allow_once = false
  state.in_flight = false
  dependencies.reportFailure()
  dependencies.setStatus(WINDOW_CLOSE_STATUS.failed)
}

function isWindowCloseAuthorization(
  value: unknown,
): value is WindowCloseAuthorization {
  return value === 'clean' || value === 'discard_confirmed'
}

function parsePaper(value: unknown): ProjectSnapshot['paper'] | null {
  const record = exactDataRecord(value, [
    'boundary_vertices',
    'thickness_mm',
    'length_display_unit',
    'cutting_allowed',
    'front',
    'back',
  ])
  if (
    !record
    || typeof record.thickness_mm !== 'number'
    || !Number.isFinite(record.thickness_mm)
    || record.thickness_mm < 0
    || typeof record.cutting_allowed !== 'boolean'
  ) return null
  const boundarySource = snapshotExactArray(
    record.boundary_vertices,
    MAX_BOUNDARY_VERTICES,
  )
  if (!boundarySource) return null
  const boundaryVertices: string[] = []
  for (const vertex of boundarySource) {
    if (!isCanonicalNonNilUuid(vertex)) return null
    boundaryVertices.push(vertex)
  }
  const lengthDisplayUnit = parseLengthDisplayUnit(record.length_display_unit)
  const front = parsePaperAppearance(record.front)
  const back = parsePaperAppearance(record.back)
  if (!lengthDisplayUnit || !front || !back) return null
  return {
    boundary_vertices: boundaryVertices,
    thickness_mm: normalizeZero(record.thickness_mm),
    length_display_unit: lengthDisplayUnit,
    cutting_allowed: record.cutting_allowed,
    front,
    back,
  }
}

function parseLengthDisplayUnit(
  value: unknown,
): ProjectSnapshot['paper']['length_display_unit'] | null {
  if (value === 'mm' || value === 'cm' || value === 'inch') return value
  const outer = exactDataRecord(value, ['paper_edge_ratio'])
  const ratio = outer
    ? exactDataRecord(outer.paper_edge_ratio, ['reference_edge'])
    : null
  return ratio && isCanonicalNonNilUuid(ratio.reference_edge)
    ? { paper_edge_ratio: { reference_edge: ratio.reference_edge } }
    : null
}

function parsePaperAppearance(
  value: unknown,
): ProjectSnapshot['paper']['front'] | null {
  const record = exactDataRecord(value, ['color', 'texture_asset'])
  if (
    !record
    || (
      record.texture_asset !== null
      && !isCanonicalNonNilUuid(record.texture_asset)
    )
  ) return null
  const color = exactDataRecord(record.color, [
    'red',
    'green',
    'blue',
    'alpha',
  ])
  if (
    !color
    || !isByte(color.red)
    || !isByte(color.green)
    || !isByte(color.blue)
    || !isByte(color.alpha)
  ) return null
  return {
    color: {
      red: color.red,
      green: color.green,
      blue: color.blue,
      alpha: color.alpha,
    },
    texture_asset: record.texture_asset,
  }
}

function parseCreasePattern(
  value: unknown,
): ProjectSnapshot['crease_pattern'] | null {
  const record = exactDataRecord(value, ['vertices', 'edges'])
  if (!record) return null
  const vertexSource = snapshotExactArray(
    record.vertices,
    MAX_PATTERN_VERTICES,
  )
  const edgeSource = snapshotExactArray(record.edges, MAX_PATTERN_EDGES)
  if (!vertexSource || !edgeSource) return null

  const vertices: ProjectSnapshot['crease_pattern']['vertices'] = []
  for (const value of vertexSource) {
    const vertex = exactDataRecord(value, ['id', 'position'])
    const position = vertex
      ? exactDataRecord(vertex.position, ['x', 'y'])
      : null
    if (
      !vertex
      || !isCanonicalNonNilUuid(vertex.id)
      || !position
      || !isFiniteNumber(position.x)
      || !isFiniteNumber(position.y)
    ) return null
    vertices.push({
      id: vertex.id,
      position: {
        x: normalizeZero(position.x),
        y: normalizeZero(position.y),
      },
    })
  }

  const edges: ProjectSnapshot['crease_pattern']['edges'] = []
  for (const value of edgeSource) {
    const edge = exactDataRecord(value, [
      'id',
      'start',
      'end',
      'kind',
    ])
    if (
      !edge
      || !isCanonicalNonNilUuid(edge.id)
      || !isCanonicalNonNilUuid(edge.start)
      || !isCanonicalNonNilUuid(edge.end)
      || !isEdgeKind(edge.kind)
    ) return null
    edges.push({
      id: edge.id,
      start: edge.start,
      end: edge.end,
      kind: edge.kind,
    })
  }
  return { vertices, edges }
}

function parseInstructionTimeline(
  value: unknown,
): ProjectSnapshot['instruction_timeline'] | null {
  const timeline = exactDataRecord(value, ['steps'])
  const stepSource = timeline
    ? snapshotExactArray(timeline.steps, MAX_INSTRUCTION_STEPS)
    : null
  if (!stepSource) return null
  const steps: ProjectSnapshot['instruction_timeline']['steps'][number][] = []
  for (const value of stepSource) {
    const step = exactDataRecord(value, [
      'id',
      'title',
      'description',
      'caution',
      'duration_ms',
      'pose',
    ])
    if (
      !step
      || !isCanonicalNonNilUuid(step.id)
      || typeof step.title !== 'string'
      || typeof step.description !== 'string'
      || typeof step.caution !== 'string'
      || !isNonNegativeSafeInteger(step.duration_ms)
    ) return null
    const pose = exactDataRecord(step.pose, [
      'model',
      'source_model_fingerprint',
      'fixed_face',
      'hinge_angles',
    ])
    if (
      !pose
      || (
        pose.model !== 'absolute_hinge_angles_v1'
        && pose.model !== 'declarative_only_v1'
      )
      || typeof pose.source_model_fingerprint !== 'string'
      || !FINGERPRINT_PATTERN.test(pose.source_model_fingerprint)
      || (
        pose.fixed_face !== null
        && !isCanonicalNonNilUuid(pose.fixed_face)
      )
    ) return null
    const hingeSource = snapshotExactArray(
      pose.hinge_angles,
      MAX_INSTRUCTION_HINGES_PER_STEP,
    )
    if (!hingeSource) return null
    const hingeAngles: Array<{ edge: string; angle_degrees: number }> = []
    for (const value of hingeSource) {
      const hinge = exactDataRecord(value, ['edge', 'angle_degrees'])
      if (
        !hinge
        || !isCanonicalNonNilUuid(hinge.edge)
        || !isFiniteNumber(hinge.angle_degrees)
        || hinge.angle_degrees < 0
        || hinge.angle_degrees > 180
      ) return null
      hingeAngles.push({
        edge: hinge.edge,
        angle_degrees: normalizeZero(hinge.angle_degrees),
      })
    }
    if (
      pose.model === 'declarative_only_v1'
      && (pose.fixed_face !== null || hingeAngles.length !== 0)
    ) return null
    steps.push({
      id: step.id,
      title: step.title,
      description: step.description,
      caution: step.caution,
      duration_ms: step.duration_ms,
      pose: {
        model: pose.model,
        source_model_fingerprint: pose.source_model_fingerprint,
        fixed_face: pose.fixed_face,
        hinge_angles: hingeAngles,
      },
    })
  }
  return { steps }
}

function parseNumericExpressions(
  value: unknown,
): NonNullable<ProjectSnapshot['numeric_expressions']> | null {
  const record = snapshotDataRecord(value)
  if (!record) return null
  if (hasExactKeys(record, [])) return {}
  if (!hasExactKeys(record, ['rectangular_paper_creation'])) return null
  const rectangular = exactDataRecord(record.rectangular_paper_creation, [
    'schema_version',
    'width_source',
    'height_source',
    'adopted_width_mm',
    'adopted_height_mm',
  ])
  if (
    !rectangular
    || rectangular.schema_version !== 1
    || typeof rectangular.width_source !== 'string'
    || typeof rectangular.height_source !== 'string'
    || !isPositiveFiniteNumber(rectangular.adopted_width_mm)
    || !isPositiveFiniteNumber(rectangular.adopted_height_mm)
  ) return null
  return {
    rectangular_paper_creation: {
      schema_version: 1,
      width_source: rectangular.width_source,
      height_source: rectangular.height_source,
      adopted_width_mm: rectangular.adopted_width_mm,
      adopted_height_mm: rectangular.adopted_height_mm,
    },
  }
}

function snapshotDataRecord(
  value: unknown,
): Record<string, unknown> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key !== 'string') return null
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotDataRecord(value)
  return record && hasExactKeys(record, keys)
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function hasExactKeys(
  record: Readonly<Record<string, unknown>>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(record)
  return actual.length === expected.length
    && expected.every((key) => Object.hasOwn(record, key))
}

function snapshotExactArray(
  value: unknown,
  maximum: number,
): unknown[] | null {
  if (!Array.isArray(value)) return null
  const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
    Record<PropertyKey, PropertyDescriptor>
  const keys = Reflect.ownKeys(descriptors)
  if (keys.some((key) => typeof key !== 'string')) return null
  const lengthDescriptor = descriptors.length
  if (
    !lengthDescriptor
    || !('value' in lengthDescriptor)
    || lengthDescriptor.enumerable
    || !Number.isSafeInteger(lengthDescriptor.value)
    || lengthDescriptor.value < 0
    || lengthDescriptor.value > maximum
    || keys.length !== lengthDescriptor.value + 1
  ) return null
  const result: unknown[] = []
  for (let index = 0; index < lengthDescriptor.value; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    result.push(descriptor.value)
  }
  return result
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
    && !Object.is(value, -0)
}

function isNullableRevision(value: unknown): value is number | null {
  return value === null || isNonNegativeSafeInteger(value)
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function isPositiveFiniteNumber(value: unknown): value is number {
  return isFiniteNumber(value) && value > 0
}

function isByte(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isInteger(value)
    && value >= 0
    && value <= 255
}

function isEdgeKind(value: unknown): value is string {
  return value === 'mountain'
    || value === 'valley'
    || value === 'auxiliary'
    || value === 'boundary'
    || value === 'cut'
}

function normalizeZero(value: number): number {
  return Object.is(value, -0) ? 0 : value
}
