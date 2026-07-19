import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createRecoveryAutosaveStatusClient,
  createRecoveryAutosaveStatusPoller,
  parseRecoveryAutosaveStatus,
  RECOVERY_AUTOSAVE_STATUS_POLL_INTERVAL_MS,
  RecoveryAutosaveStatusClientError,
  type RecoveryAutosaveMonitorView,
  type RecoveryAutosavePollingClock,
  type RecoveryAutosaveStatus,
} from '../src/lib/recoveryAutosaveStatusClient.ts'

const PENDING: RecoveryAutosaveStatus = {
  schema_version: 1,
  status: 'pending_first_attempt',
  transition_id: 0,
}
const FAILED: RecoveryAutosaveStatus = {
  schema_version: 1,
  status: 'persistence_failed',
  transition_id: 1,
}
const OPERATIONAL: RecoveryAutosaveStatus = {
  schema_version: 1,
  status: 'operational',
  transition_id: 2,
}
const EXHAUSTED: RecoveryAutosaveStatus = {
  schema_version: 1,
  status: 'persistence_failed',
  transition_id: 0xffff_ffff,
}

test('admits only exact and semantically valid autosave health DTOs', () => {
  for (const valid of [PENDING, FAILED, OPERATIONAL, EXHAUSTED]) {
    const parsed = parseRecoveryAutosaveStatus(valid)
    assert.deepEqual(parsed, valid)
    assert.notEqual(parsed, valid)
    assert.equal(Object.isFrozen(parsed), true)
  }

  for (const invalid of [
    null,
    [],
    { ...PENDING, schema_version: 2 },
    { ...PENDING, status: 'future' },
    { ...PENDING, transition_id: 1 },
    { ...FAILED, transition_id: 0 },
    { ...FAILED, transition_id: -0 },
    { ...FAILED, transition_id: -1 },
    { ...FAILED, transition_id: 1.5 },
    { ...FAILED, transition_id: Number.NaN },
    { ...FAILED, transition_id: 0x1_0000_0000 },
    { ...FAILED, path: 'C:\\private\\recovery.ori2' },
    { ...FAILED, error: 'permission denied' },
    Object.create({ ...FAILED }),
  ]) {
    assert.equal(parseRecoveryAutosaveStatus(invalid), null)
  }
})

test('status admission rejects accessors, symbols, and hostile proxies without reading values', () => {
  let getterCalls = 0
  const accessor = {
    schema_version: 1,
    status: 'persistence_failed',
  }
  Object.defineProperty(accessor, 'transition_id', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 1
    },
  })
  assert.equal(parseRecoveryAutosaveStatus(accessor), null)
  assert.equal(getterCalls, 0)

  const symbolStatus = { ...FAILED }
  Object.defineProperty(symbolStatus, Symbol('private'), {
    enumerable: true,
    value: 'hidden',
  })
  assert.equal(parseRecoveryAutosaveStatus(symbolStatus), null)
  assert.equal(parseRecoveryAutosaveStatus(new Proxy({}, {
    getPrototypeOf() {
      throw new Error('C:\\private\\proxy')
    },
  })), null)
  const revocable = Proxy.revocable({ ...FAILED }, {})
  revocable.revoke()
  assert.equal(parseRecoveryAutosaveStatus(revocable.proxy), null)
})

test('client invokes the read-only command without arguments and redacts native failures', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createRecoveryAutosaveStatusClient(async (command) => {
    calls.push([command, undefined])
    return FAILED
  })
  assert.deepEqual(await client.getStatus(), FAILED)
  assert.deepEqual(calls, [['get_recovery_autosave_status', undefined]])

  const failing = createRecoveryAutosaveStatusClient(async () => {
    throw new Error('C:\\private\\recovery\\slot.ori2')
  })
  await assert.rejects(failing.getStatus(), (error: unknown) => {
    assert.ok(error instanceof RecoveryAutosaveStatusClientError)
    assert.equal(error.code, 'native_unavailable')
    assert.doesNotMatch(error.message, /private|slot|ori2/iu)
    assert.equal('cause' in error, false)
    return true
  })

  const malformed = createRecoveryAutosaveStatusClient(async () => ({
    ...FAILED,
    path: 'C:\\private\\slot.ori2',
  }))
  await assert.rejects(malformed.getStatus(), (error: unknown) => {
    assert.ok(error instanceof RecoveryAutosaveStatusClientError)
    assert.equal(error.code, 'invalid_response')
    assert.doesNotMatch(error.message, /private|slot|ori2/iu)
    return true
  })
})

test('browser-disabled polling never invokes native code or installs a timer', () => {
  let calls = 0
  const clock = new FakeClock()
  const views: RecoveryAutosaveMonitorView[] = []
  const poller = createRecoveryAutosaveStatusPoller({
    nativeAvailable: false,
    client: {
      async getStatus() {
        calls += 1
        return PENDING
      },
    },
    clock,
    onChange: (view) => views.push(view),
  })
  poller.start()
  poller.refresh()
  clock.tick()
  poller.dispose()
  assert.equal(calls, 0)
  assert.deepEqual(clock.delays, [])
  assert.deepEqual(views, [])
})

test('polling is immediate, fixed at five seconds, single-flight, and StrictMode disposal rejects late results', async () => {
  const pending = deferred<RecoveryAutosaveStatus>()
  let calls = 0
  const clock = new FakeClock()
  const views: RecoveryAutosaveMonitorView[] = []
  const poller = createRecoveryAutosaveStatusPoller({
    nativeAvailable: true,
    client: {
      getStatus() {
        calls += 1
        return pending.promise
      },
    },
    clock,
    onChange: (view) => views.push(view),
  })

  poller.start()
  assert.equal(calls, 1)
  assert.deepEqual(clock.delays, [
    RECOVERY_AUTOSAVE_STATUS_POLL_INTERVAL_MS,
  ])
  clock.tick()
  poller.refresh()
  assert.equal(calls, 1, 'one native request must own the poller at a time')

  poller.dispose()
  pending.resolve(FAILED)
  await flushPromises()
  assert.deepEqual(views, [{ kind: 'checking' }])
  assert.equal(clock.clearCalls, 1)
})

test('poller suppresses repeats and stale responses, announces recovery, and fails closed on contradiction', async () => {
  const responses: Array<RecoveryAutosaveStatus | Error> = [
    FAILED,
    FAILED,
    OPERATIONAL,
    FAILED,
    { ...FAILED, transition_id: 2 },
  ]
  const clock = new FakeClock()
  const views: RecoveryAutosaveMonitorView[] = []
  const poller = createRecoveryAutosaveStatusPoller({
    nativeAvailable: true,
    client: {
      async getStatus() {
        const response = responses.shift()
        if (!response) throw new Error('missing test response')
        if (response instanceof Error) throw response
        return response
      },
    },
    clock,
    onChange: (view) => views.push(view),
  })

  poller.start()
  await flushPromises()
  for (let index = 0; index < 4; index += 1) {
    clock.tick()
    await flushPromises()
  }
  poller.dispose()

  assert.deepEqual(views, [
    { kind: 'checking' },
    { kind: 'persistence_failed', transition_id: 1 },
    { kind: 'operational', transition_id: 2, recovered: true },
    { kind: 'monitor_unavailable' },
  ])
})

test('a transient monitoring failure is replaced by the next valid cached status', async () => {
  let attempt = 0
  const clock = new FakeClock()
  const views: RecoveryAutosaveMonitorView[] = []
  const poller = createRecoveryAutosaveStatusPoller({
    nativeAvailable: true,
    client: {
      async getStatus() {
        attempt += 1
        if (attempt === 1) throw new Error('C:\\private\\failure')
        return PENDING
      },
    },
    clock,
    onChange: (view) => views.push(view),
  })
  poller.start()
  await flushPromises()
  clock.tick()
  await flushPromises()
  poller.dispose()
  assert.deepEqual(views, [
    { kind: 'checking' },
    { kind: 'monitor_unavailable' },
    { kind: 'pending_first_attempt', transition_id: 0 },
  ])
})

class FakeClock implements RecoveryAutosavePollingClock {
  readonly delays: number[] = []
  clearCalls = 0
  private callback: (() => void) | null = null

  setInterval(callback: () => void, delayMs: number): unknown {
    this.callback = callback
    this.delays.push(delayMs)
    return 1
  }

  clearInterval(_handle: unknown): void {
    this.clearCalls += 1
    this.callback = null
  }

  tick(): void {
    this.callback?.()
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

async function flushPromises(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}
