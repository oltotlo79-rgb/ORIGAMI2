import { invoke } from '@tauri-apps/api/core'

import { RECOVERY_SCHEMA_VERSION } from './recoveryClient.ts'

export const RECOVERY_AUTOSAVE_STATUS_POLL_INTERVAL_MS = 5_000 as const
const MAX_TRANSITION_ID = 0xffff_ffff

export type RecoveryAutosaveHealthStatus =
  | 'pending_first_attempt'
  | 'operational'
  | 'persistence_failed'

export type RecoveryAutosaveStatus = Readonly<{
  schema_version: typeof RECOVERY_SCHEMA_VERSION
  status: RecoveryAutosaveHealthStatus
  transition_id: number
}>

export type RecoveryAutosaveMonitorView =
  | Readonly<{ kind: 'inactive' }>
  | Readonly<{ kind: 'checking' }>
  | Readonly<{ kind: 'monitor_unavailable' }>
  | Readonly<{
      kind: 'pending_first_attempt'
      transition_id: number
    }>
  | Readonly<{
      kind: 'operational'
      transition_id: number
      recovered: boolean
    }>
  | Readonly<{
      kind: 'persistence_failed'
      transition_id: number
    }>

export type RecoveryAutosaveStatusNativeInvoke = (
  command: string,
) => Promise<unknown>

export type RecoveryAutosaveStatusClient = Readonly<{
  getStatus: () => Promise<RecoveryAutosaveStatus>
}>

export type RecoveryAutosavePollingClock = Readonly<{
  setInterval: (callback: () => void, delayMs: number) => unknown
  clearInterval: (handle: unknown) => void
}>

export type RecoveryAutosaveStatusPoller = Readonly<{
  start: () => void
  refresh: () => void
  dispose: () => void
}>

export type RecoveryAutosaveStatusPollerOptions = Readonly<{
  nativeAvailable: boolean
  onChange: (view: RecoveryAutosaveMonitorView) => void
  client?: RecoveryAutosaveStatusClient
  clock?: RecoveryAutosavePollingClock
}>

export class RecoveryAutosaveStatusClientError extends Error {
  readonly code: 'native_unavailable' | 'invalid_response'

  constructor(code: 'native_unavailable' | 'invalid_response') {
    super(
      code === 'native_unavailable'
        ? 'The automatic recovery status is unavailable.'
        : 'The automatic recovery status response is invalid.',
    )
    this.name = 'RecoveryAutosaveStatusClientError'
    this.code = code
  }
}

const defaultNativeInvoke: RecoveryAutosaveStatusNativeInvoke = (command) =>
  invoke<unknown>(command)

const defaultClient = createRecoveryAutosaveStatusClient()

const defaultClock: RecoveryAutosavePollingClock = Object.freeze({
  setInterval: (callback, delayMs) => globalThis.setInterval(callback, delayMs),
  clearInterval: (handle) => {
    globalThis.clearInterval(
      handle as ReturnType<typeof globalThis.setInterval>,
    )
  },
})

export function createRecoveryAutosaveStatusClient(
  nativeInvoke: RecoveryAutosaveStatusNativeInvoke = defaultNativeInvoke,
): RecoveryAutosaveStatusClient {
  return Object.freeze({
    async getStatus() {
      let response: unknown
      try {
        response = await nativeInvoke('get_recovery_autosave_status')
      } catch {
        throw new RecoveryAutosaveStatusClientError('native_unavailable')
      }
      const status = parseRecoveryAutosaveStatus(response)
      if (!status) {
        throw new RecoveryAutosaveStatusClientError('invalid_response')
      }
      return status
    },
  })
}

/**
 * Strictly admits the anonymous, data-only health DTO. Accessors, symbols,
 * custom prototypes, unknown keys, and invalid semantic combinations are
 * rejected before any value is retained.
 */
export function parseRecoveryAutosaveStatus(
  value: unknown,
): RecoveryAutosaveStatus | null {
  try {
    const record = exactDataRecord(value, [
      'schema_version',
      'status',
      'transition_id',
    ])
    if (
      !record
      || record.schema_version !== RECOVERY_SCHEMA_VERSION
      || !isRecoveryAutosaveHealthStatus(record.status)
      || !isTransitionId(record.transition_id)
      || (
        record.status === 'pending_first_attempt'
        && record.transition_id !== 0
      )
      || (
        record.status !== 'pending_first_attempt'
        && record.transition_id === 0
      )
    ) return null

    return Object.freeze({
      schema_version: RECOVERY_SCHEMA_VERSION,
      status: record.status,
      transition_id: record.transition_id,
    })
  } catch {
    return null
  }
}

/**
 * Owns one renderer lifecycle of cached-status polling. At most one command
 * can be in flight, and a disposed lifecycle can never publish a late result.
 */
export function createRecoveryAutosaveStatusPoller(
  options: RecoveryAutosaveStatusPollerOptions,
): RecoveryAutosaveStatusPoller {
  const client = options.client ?? defaultClient
  const clock = options.clock ?? defaultClock
  let started = false
  let disposed = false
  let inFlight = false
  let lifecycle = 0
  let intervalHandle: unknown = null
  let lastNative: RecoveryAutosaveStatus | null = null
  let currentView: RecoveryAutosaveMonitorView = Object.freeze({
    kind: 'inactive',
  })
  let observedPersistenceFailure = false

  const publish = (view: RecoveryAutosaveMonitorView) => {
    if (sameMonitorView(currentView, view)) return
    currentView = view
    try {
      options.onChange(view)
    } catch {
      // A renderer callback cannot break ownership or expose native details.
    }
  }

  const accept = (status: RecoveryAutosaveStatus) => {
    if (lastNative) {
      if (status.transition_id < lastNative.transition_id) return
      if (
        status.transition_id === lastNative.transition_id
        && status.status !== lastNative.status
      ) {
        publish(Object.freeze({ kind: 'monitor_unavailable' }))
        return
      }
      if (
        status.transition_id === lastNative.transition_id
        && currentView.kind !== 'monitor_unavailable'
      ) return
    }

    const recovered = status.status === 'operational'
      && observedPersistenceFailure
    if (status.status === 'persistence_failed') {
      observedPersistenceFailure = true
    } else if (status.status === 'operational') {
      observedPersistenceFailure = false
    }
    lastNative = status
    publish(viewFromNativeStatus(status, recovered))
  }

  const refresh = () => {
    if (
      !options.nativeAvailable
      || !started
      || disposed
      || inFlight
    ) return
    inFlight = true
    const owner = lifecycle
    void client.getStatus().then((status) => {
      if (!ownsLifecycle(owner)) return
      accept(status)
    }).catch(() => {
      if (!ownsLifecycle(owner)) return
      publish(Object.freeze({ kind: 'monitor_unavailable' }))
    }).finally(() => {
      if (ownsLifecycle(owner)) inFlight = false
    })
  }

  const ownsLifecycle = (owner: number) =>
    started && !disposed && lifecycle === owner

  return Object.freeze({
    start() {
      if (!options.nativeAvailable || started || disposed) return
      started = true
      lifecycle += 1
      publish(Object.freeze({ kind: 'checking' }))
      refresh()
      try {
        intervalHandle = clock.setInterval(
          refresh,
          RECOVERY_AUTOSAVE_STATUS_POLL_INTERVAL_MS,
        )
      } catch {
        publish(Object.freeze({ kind: 'monitor_unavailable' }))
      }
    },
    refresh,
    dispose() {
      if (disposed) return
      disposed = true
      started = false
      lifecycle += 1
      if (intervalHandle !== null) {
        try {
          clock.clearInterval(intervalHandle)
        } catch {
          // Disposal remains final even if a hostile clock rejects cleanup.
        }
        intervalHandle = null
      }
    },
  })
}

function viewFromNativeStatus(
  status: RecoveryAutosaveStatus,
  recovered: boolean,
): RecoveryAutosaveMonitorView {
  switch (status.status) {
    case 'pending_first_attempt':
      return Object.freeze({
        kind: 'pending_first_attempt',
        transition_id: status.transition_id,
      })
    case 'operational':
      return Object.freeze({
        kind: 'operational',
        transition_id: status.transition_id,
        recovered,
      })
    case 'persistence_failed':
      return Object.freeze({
        kind: 'persistence_failed',
        transition_id: status.transition_id,
      })
  }
}

function sameMonitorView(
  left: RecoveryAutosaveMonitorView,
  right: RecoveryAutosaveMonitorView,
): boolean {
  if (left.kind !== right.kind) return false
  if (
    left.kind === 'inactive'
    || left.kind === 'checking'
    || left.kind === 'monitor_unavailable'
  ) return true
  if (
    right.kind === 'inactive'
    || right.kind === 'checking'
    || right.kind === 'monitor_unavailable'
  ) return false
  if (left.transition_id !== right.transition_id) return false
  return left.kind !== 'operational'
    || (
      right.kind === 'operational'
      && left.recovered === right.recovered
    )
}

function isRecoveryAutosaveHealthStatus(
  value: unknown,
): value is RecoveryAutosaveHealthStatus {
  return value === 'pending_first_attempt'
    || value === 'operational'
    || value === 'persistence_failed'
}

function isTransitionId(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && !Object.is(value, -0)
    && value >= 0
    && value <= MAX_TRANSITION_ID
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const actualKeys = Reflect.ownKeys(descriptors)
  if (
    actualKeys.length !== keys.length
    || actualKeys.some((key) => typeof key !== 'string')
    || keys.some((key) => !Object.hasOwn(descriptors, key))
  ) return null

  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}
