import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createPostApplyProofSchedulerCoordinatorV1,
  type PostApplyProofSchedulerViewStateV1,
} from '../src/lib/postApplyProofSchedulerCoordinator.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const firstJobToken = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const secondJobToken = '018f47a2-4b7a-7cc1-8abc-aabbccddeeff'

const binding = Object.freeze({
  version: 1 as const,
  projectInstanceId,
  projectId,
  revision: 8,
})

function progress(overrides: Readonly<Record<string, unknown>> = {}) {
  const value: Record<string, unknown> = {
    ...binding,
    jobToken: firstJobToken,
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

test('an exact terminal start is published without polling or mutation authority', async () => {
  let polls = 0
  const states: PostApplyProofSchedulerViewStateV1[] = []
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: async () => progress({
        status: 'certified',
        provenPairCount: 5,
      }),
      poll: async () => {
        polls += 1
        return progress()
      },
      cancel: async () => undefined,
    },
    onState(state) {
      states.push(state)
    },
  })

  assert.equal(coordinator.start(binding), true)
  assert.deepEqual(coordinator.getState(), { kind: 'starting' })
  await settle()
  assert.equal(coordinator.getState().kind, 'progress')
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.status
      : null,
    'certified',
  )
  assert.equal(polls, 0)
  assert.deepEqual(states.map((state) => state.kind), [
    'starting',
    'progress',
  ])
})

test('later edits preserve a pending start while project identity drift cancels it', async () => {
  const initial = deferred<unknown>()
  const cancelled: unknown[] = []
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: async () => initial.promise,
      poll: async () => progress(),
      cancel: async (request) => {
        cancelled.push(request)
      },
    },
    onState() {},
  })

  coordinator.start(binding)
  assert.equal(coordinator.observeAuthority({
    ...binding,
    revision: 9,
  }), false)
  assert.deepEqual(coordinator.getState(), { kind: 'starting' })
  assert.equal(coordinator.observeAuthority({
    ...binding,
    revision: 9,
    projectId: '018f47a2-4b7a-7cc1-8abc-001122334455',
  }), true)
  assert.deepEqual(coordinator.getState(), { kind: 'idle' })

  initial.resolve(progress())
  await settle()
  assert.deepEqual(cancelled, [{
    ...binding,
    jobToken: firstJobToken,
  }])
  assert.deepEqual(coordinator.getState(), { kind: 'idle' })
})

test('later revisions refresh a terminal failure report without replacing its job', async () => {
  const polled: unknown[] = []
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: async () => progress({ status: 'blocked' }),
      poll: async (request) => {
        polled.push(request)
        return progress({
          status: 'blocked',
          proofFailure: {
            location: 'applied_retained_undo',
            outcome: 'blocked',
            reason: null,
            subsequentEditCount: 1,
            undoStepsToRevert: 2,
          },
        })
      },
      cancel: async () => undefined,
    },
    onState() {},
  })

  coordinator.start(binding)
  await settle()
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.proofFailure?.subsequentEditCount
      : null,
    0,
  )
  assert.equal(coordinator.observeAuthority({ ...binding, revision: 9 }), false)
  await settle()
  assert.deepEqual(polled, [{ ...binding, jobToken: firstJobToken }])
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.proofFailure?.subsequentEditCount
      : null,
    1,
  )
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.proofFailure?.undoStepsToRevert
      : null,
    2,
  )
})

test('a replacement start cancels the old job and rejects its late response', async () => {
  const first = deferred<unknown>()
  const cancelled: unknown[] = []
  let starts = 0
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: async () => {
        starts += 1
        return starts === 1
          ? first.promise
          : progress({
              jobToken: secondJobToken,
              status: 'blocked',
            })
      },
      poll: async () => progress(),
      cancel: async (request) => {
        cancelled.push(request)
      },
    },
    onState() {},
  })

  coordinator.start(binding)
  coordinator.start(binding)
  await settle()
  assert.equal(coordinator.getState().kind, 'progress')
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.jobToken
      : null,
    secondJobToken,
  )

  first.resolve(progress())
  await settle()
  assert.deepEqual(cancelled, [{
    ...binding,
    jobToken: firstJobToken,
  }])
  assert.equal(
    coordinator.getState().kind === 'progress'
      ? coordinator.getState().progress.jobToken
      : null,
    secondJobToken,
  )
})

test('transport and invalid responses collapse to one fixed unavailable state', async () => {
  const secret = 'C:\\private\\native-proof-error.json'
  const responses: (() => Promise<unknown>)[] = [
    async () => {
      throw new Error(secret)
    },
    async () => progress({ projectId: projectInstanceId }),
    async () => ({ ...progress(), geometry: [1, 2, 3] }),
  ]
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: () => responses.shift()!(),
      poll: async () => progress(),
      cancel: async () => undefined,
    },
    onState() {},
  })

  for (const _response of [0, 1, 2]) {
    coordinator.start(binding)
    await settle()
    assert.deepEqual(coordinator.getState(), { kind: 'unavailable' })
    assert.equal(JSON.stringify(coordinator.getState()).includes(secret), false)
  }
})

test('invalid authority fails closed before transport and dispose is idempotent', () => {
  let starts = 0
  const coordinator = createPostApplyProofSchedulerCoordinatorV1({
    client: {
      start: async () => {
        starts += 1
        return progress()
      },
      poll: async () => progress(),
      cancel: async () => undefined,
    },
    onState() {},
  })
  assert.equal(coordinator.start({ ...binding, revision: -0 }), false)
  assert.deepEqual(coordinator.getState(), { kind: 'unavailable' })
  assert.equal(starts, 0)
  coordinator.dispose()
  coordinator.dispose()
  assert.equal(coordinator.start(binding), false)
  assert.deepEqual(coordinator.getState(), { kind: 'idle' })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((accept) => {
    resolve = accept
  })
  return { promise, resolve }
}

async function settle() {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}
