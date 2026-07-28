import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createPostApplyProofPollingMachineV1,
  type PostApplyProofPollingStateV1,
} from '../src/lib/postApplyProofPollingMachine.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const firstJobToken = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const secondJobToken = '018f47a2-4b7a-7cc1-8abc-aabbccddeeff'

function progress(
  overrides: Readonly<Record<string, unknown>> = {},
): Readonly<Record<string, unknown>> {
  const value: Record<string, unknown> = {
    version: 1,
    jobToken: firstJobToken,
    projectInstanceId,
    projectId,
    revision: 8,
    status: 'proving',
    provenPairCount: 0,
    totalPairCount: 5,
    proofFailure: null,
    ...overrides,
  }
  if (!Object.hasOwn(overrides, 'proofFailure')) {
    value.proofFailure = value.status === 'blocked'
      ? {
          location: 'applied_retained_undo',
          outcome: 'blocked',
          reason: null,
          subsequentEditCount: 0,
          undoStepsToRevert: 1,
        }
      : null
  }
  return value
}

test('polling is single-flight and terminal progress schedules no further work', async () => {
  const first = deferred<unknown>()
  const timers: (() => void)[] = []
  let polls = 0
  const states: PostApplyProofPollingStateV1[] = []
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => {
      polls += 1
      return first.promise
    },
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState(state) {
      states.push(state)
    },
  })

  assert.equal(machine.start(progress()), true)
  assert.equal(machine.getState().status, 'polling')
  assert.equal(timers.length, 1)
  timers.shift()?.()
  assert.equal(machine.pollNow(), false)
  assert.equal(machine.pollNow(), false)
  assert.equal(polls, 1)

  first.resolve(progress({
    status: 'certified',
    provenPairCount: 5,
  }))
  await settle()
  assert.equal(machine.getState().status, 'terminal')
  assert.equal(machine.getState().generation, 1)
  assert.equal(timers.length, 0)
  assert.deepEqual(
    states.map((state) => state.status),
    ['polling', 'terminal'],
  )
})

test('a generation replacement cancels the old job and ignores its late response', async () => {
  const first = deferred<unknown>()
  const second = deferred<unknown>()
  const cancelled: unknown[] = []
  const timers: (() => void)[] = []
  let polls = 0
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => {
      polls += 1
      return polls === 1 ? first.promise : second.promise
    },
    cancel: async (request) => {
      cancelled.push(request)
    },
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })

  machine.start(progress())
  timers.shift()?.()
  machine.start(progress({
    jobToken: secondJobToken,
    provenPairCount: 0,
  }))
  await settle()
  assert.deepEqual(cancelled, [{
    version: 1,
    jobToken: firstJobToken,
    projectInstanceId,
    projectId,
    revision: 8,
  }])
  assert.equal(machine.getState().generation, 2)

  first.resolve(progress({
    status: 'certified',
    provenPairCount: 5,
  }))
  await settle()
  assert.equal(machine.getState().status, 'polling')
  assert.equal(machine.getState().generation, 2)

  timers.shift()?.()
  second.resolve(progress({
    jobToken: secondJobToken,
    status: 'blocked',
  }))
  await settle()
  assert.equal(machine.getState().status, 'terminal')
  assert.equal(
    machine.getState().status === 'terminal'
      ? machine.getState().progress.jobToken
      : null,
    secondJobToken,
  )
})

test('a late timer from an old generation cannot poll its replacement', async () => {
  const timers: (() => void)[] = []
  const polledTokens: string[] = []
  const machine = createPostApplyProofPollingMachineV1({
    poll: async (request) => {
      polledTokens.push(request.jobToken)
      return progress({
        jobToken: request.jobToken,
        status: 'blocked',
      })
    },
    cancel: async () => undefined,
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {
      // Simulate an already queued callback that cannot be removed.
    },
    onState() {},
  })
  machine.start(progress())
  const oldTimer = timers.shift()
  machine.start(progress({ jobToken: secondJobToken }))
  const newTimer = timers.shift()

  oldTimer?.()
  await settle()
  assert.deepEqual(polledTokens, [])
  assert.equal(machine.getState().generation, 2)

  newTimer?.()
  await settle()
  assert.deepEqual(polledTokens, [secondJobToken])
  assert.equal(machine.getState().status, 'terminal')
})

test('local cancel is idempotent and rejects every late response', async () => {
  const pending = deferred<unknown>()
  const timers: (() => void)[] = []
  let cancelCount = 0
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => pending.promise,
    cancel: async () => {
      cancelCount += 1
      throw new Error('best-effort native cancellation failed')
    },
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })
  machine.start(progress())
  timers.shift()?.()
  assert.equal(machine.cancel(), true)
  assert.equal(machine.cancel(), false)
  await settle()
  assert.equal(cancelCount, 1)
  assert.equal(machine.getState().status, 'cancelled')

  pending.resolve(progress({
    status: 'certified',
    provenPairCount: 5,
  }))
  await settle()
  assert.equal(machine.getState().status, 'cancelled')
})

test('mismatched and hostile poll responses fail closed without reading wire fields', async () => {
  const timers: (() => void)[] = []
  const requestedKeys: PropertyKey[] = []
  const hostile = new Proxy(progress(), {
    get(_target, key) {
      requestedKeys.push(key)
      throw new Error('must not be read')
    },
  })
  const responses: unknown[] = [
    progress({ revision: 9 }),
    hostile,
  ]
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => responses.shift(),
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })

  machine.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: 1,
    reason: 'invalid_response',
  })

  machine.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: 2,
    reason: 'transport_failure',
  })
  // Promise resolution must query `then`; no wire field is ever requested.
  assert.deepEqual(requestedKeys, ['then'])
})

test('same-job progress keeps total fixed, proven monotone, and certification complete', async () => {
  const timers: (() => void)[] = []
  const responses: unknown[] = [
    progress({ provenPairCount: 1 }),
    progress({ totalPairCount: 6 }),
    progress({ status: 'certified', provenPairCount: 4 }),
  ]
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => responses.shift(),
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })

  machine.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: 1,
    reason: 'invalid_response',
  })

  machine.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: 2,
    reason: 'invalid_response',
  })

  machine.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: 3,
    reason: 'invalid_response',
  })
})

test('invalid replacement, transport failure, and timer failure remain fixed and redacted', async () => {
  const secret = 'C:\\private\\post-proof.json'
  const timerFailure = createPostApplyProofPollingMachineV1({
    poll: async () => progress(),
    setTimer() {
      throw new Error(secret)
    },
    clearTimer() {},
    onState() {
      throw new Error('observational callback')
    },
  })
  assert.equal(timerFailure.start(progress()), true)
  assert.deepEqual(timerFailure.getState(), {
    status: 'failed',
    generation: 1,
    reason: 'scheduler_failure',
  })
  assert.equal(JSON.stringify(timerFailure.getState()).includes(secret), false)

  const timers: (() => void)[] = []
  const transportFailure = createPostApplyProofPollingMachineV1({
    poll: async () => {
      throw new Error(secret)
    },
    cancel: async () => undefined,
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })
  transportFailure.start(progress())
  timers.shift()?.()
  await settle()
  assert.deepEqual(transportFailure.getState(), {
    status: 'failed',
    generation: 1,
    reason: 'transport_failure',
  })
  assert.equal(JSON.stringify(transportFailure.getState()).includes(secret), false)

  assert.equal(transportFailure.start(progress()), true)
  assert.equal(transportFailure.start({
    ...progress(),
    authorizesProjectMutation: true,
  }), false)
  assert.deepEqual(transportFailure.getState(), {
    status: 'failed',
    generation: 3,
    reason: 'invalid_job',
  })
})

test('polling states expose progress only and never infer mutation or revert authority', () => {
  const machine = createPostApplyProofPollingMachineV1({
    poll: async () => progress(),
    setTimer() {
      return 1 as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })
  machine.start(progress())
  const serialized = JSON.stringify(machine.getState())
  assert.equal(serialized.includes('authorizesProjectMutation'), false)
  assert.equal(serialized.includes('revert'), false)
  machine.dispose()
  assert.equal(machine.getState().status, 'idle')
  assert.equal(machine.start(progress()), false)
})

test('generation exhaustion invalidates the current run without reuse or overflow', async () => {
  const timers: (() => void)[] = []
  let pollCount = 0
  let cancelCount = 0
  const machine = createPostApplyProofPollingMachineV1({
    initialGeneration: Number.MAX_SAFE_INTEGER - 1,
    poll: async () => {
      pollCount += 1
      return progress()
    },
    cancel: async () => {
      cancelCount += 1
    },
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState() {},
  })
  assert.equal(machine.start(progress()), true)
  assert.equal(machine.getState().generation, Number.MAX_SAFE_INTEGER)
  assert.equal(machine.start(progress({ jobToken: secondJobToken })), false)
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: Number.MAX_SAFE_INTEGER,
    reason: 'generation_exhausted',
  })
  timers.shift()?.()
  await settle()
  assert.equal(pollCount, 0)
  assert.equal(cancelCount, 1)
  assert.equal(machine.cancel(), false)
  assert.deepEqual(machine.getState(), {
    status: 'failed',
    generation: Number.MAX_SAFE_INTEGER,
    reason: 'generation_exhausted',
  })
})

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((accept, deny) => {
    resolve = accept
    reject = deny
  })
  return { promise, resolve, reject }
}

function settle() {
  return new Promise<void>((resolve) => setImmediate(resolve))
}
