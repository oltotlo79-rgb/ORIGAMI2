import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createGlobalFlatFoldabilityCoordinator,
  GLOBAL_FLAT_FOLDABILITY_POLL_INTERVAL_MS,
  type GlobalFlatFoldabilityCoordinator,
  type GlobalFlatFoldabilityCoordinatorOptions,
  type GlobalFlatFoldabilityCoordinatorState,
  type GlobalFlatFoldabilityTimeoutScheduler,
  type GlobalFlatFoldabilityTransport,
} from '../src/lib/globalFlatFoldabilityCoordinator.ts'
import {
  GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  type GlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityPhase,
} from '../src/lib/globalFlatFoldability.ts'
import { GlobalFlatFoldabilityNativeError } from '../src/lib/globalFlatFoldabilityNative.ts'

const FIRST_CONTEXT = Object.freeze({
  projectInstanceId: '018f47d1-5ca0-75b1-a53a-c579f39f9660',
  projectId: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
  revision: 7,
  foldModelFingerprint: 'a'.repeat(64),
})
const SECOND_CONTEXT = Object.freeze({
  projectInstanceId: '018f47d1-5ca0-75b1-a53a-c579f39f9664',
  projectId: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
  revision: 8,
  foldModelFingerprint: 'b'.repeat(64),
})
const FIRST_JOB_ID = '018f47d1-5ca0-75b1-a53a-c579f39f9662'
const SECOND_JOB_ID = '018f47d1-5ca0-75b1-a53a-c579f39f9663'

test('start binds one preset and revision, then polls full DTOs to one terminal', async () => {
  const scheduler = manualScheduler()
  const beginCalls: Array<Readonly<{
    context: Readonly<{ projectId: string; revision: number }>
    timeLimitMs: number
  }>> = []
  const pollCalls: string[] = []
  const states: GlobalFlatFoldabilityCoordinatorState[] = []
  const pollJobs = [
    runningJob('building_constraints', 12, 1_000),
    possibleJob(20, 2_000),
  ]
  const transport = transportFixture({
    begin: async (context, timeLimitMs) => {
      beginCalls.push({ context, timeLimitMs })
      return { jobId: FIRST_JOB_ID, job: queuedJob() }
    },
    poll: async (jobId) => {
      pollCalls.push(jobId)
      return pollJobs.shift()
    },
  })
  const coordinator = requiredCoordinator({
    transport,
    scheduler,
    onState: (state) => states.push(state),
  })

  assert.equal(
    coordinator.start(FIRST_CONTEXT, 1 as 5),
    false,
    'non-preset values are rejected without replacing a run',
  )
  assert.equal(
    coordinator.start(
      { ...FIRST_CONTEXT, revision: -1 },
      30,
    ),
    false,
  )
  assert.equal(coordinator.start(FIRST_CONTEXT, 30), true)
  assert.equal(coordinator.getState().generation, 1)
  assert.equal(coordinator.getState().job?.state, 'queued')
  assert.deepEqual(beginCalls, [{
    context: FIRST_CONTEXT,
    timeLimitMs: 30_000,
  }])
  assert.notEqual(beginCalls[0]?.context, FIRST_CONTEXT)
  assert.ok(Object.isFrozen(beginCalls[0]?.context))

  await settlePromises()
  assert.equal(scheduler.pendingCount(), 1)
  assert.deepEqual(scheduler.delays(), [
    GLOBAL_FLAT_FOLDABILITY_POLL_INTERVAL_MS,
  ])

  scheduler.runNext()
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'running')
  assert.equal(activeWork(coordinator.getState()), 12)
  assert.equal(scheduler.pendingCount(), 1)

  scheduler.runNext()
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'completed')
  assert.equal(completedVerdict(coordinator.getState()), 'possible')
  assert.equal(scheduler.pendingCount(), 0)
  assert.deepEqual(pollCalls, [FIRST_JOB_ID, FIRST_JOB_ID])
  assert.ok(states.every(Object.isFrozen))
})

test('a newer start cancels the old generation and ignores late begin and poll callbacks', async () => {
  const scheduler = manualScheduler()
  const firstBegin = deferred<Readonly<{
    jobId: string
    job: GlobalFlatFoldabilityJobDto
  }>>()
  const secondBegin = deferred<Readonly<{
    jobId: string
    job: GlobalFlatFoldabilityJobDto
  }>>()
  const latePoll = deferred<unknown>()
  const cancelCalls: string[] = []
  let beginIndex = 0
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: () => {
        beginIndex += 1
        return beginIndex === 1 ? firstBegin.promise : secondBegin.promise
      },
      poll: () => latePoll.promise,
      cancel: async (jobId) => {
        cancelCalls.push(jobId)
      },
    }),
    scheduler,
    onState: () => undefined,
  })

  assert.equal(coordinator.start(FIRST_CONTEXT, 5), true)
  assert.equal(coordinator.start(SECOND_CONTEXT, 120), true)
  assert.equal(coordinator.getState().generation, 2)

  firstBegin.resolve({ jobId: FIRST_JOB_ID, job: queuedJob() })
  await settlePromises()
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
  assert.equal(coordinator.getState().generation, 2)
  assert.equal(coordinator.getState().job?.state, 'queued')

  secondBegin.resolve({ jobId: SECOND_JOB_ID, job: queuedJob() })
  await settlePromises()
  const oldTimer = scheduler.onlyPendingHandle()
  scheduler.runNext()
  await settlePromises()
  assert.equal(scheduler.pendingCount(), 0)

  assert.equal(coordinator.start(FIRST_CONTEXT, 30), true)
  assert.equal(coordinator.getState().generation, 3)
  latePoll.resolve(possibleJob(10, 2_000))
  await settlePromises()
  scheduler.force(oldTimer)
  await settlePromises()
  assert.equal(coordinator.getState().generation, 3)
  assert.equal(coordinator.getState().job?.state, 'queued')
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID, SECOND_JOB_ID])
})

test('phase, work, total, counts and elapsed time may only advance', async () => {
  const previous = runningJob('searching', 100, 1_000, {
    face_count: 4,
    overlap_cell_count: 10,
    constraint_count: 20,
    search_node_count: 30,
  }, 1_000)
  const regressions = [
    runningJob('propagating', 100, 1_000, previous.progress.counts, 1_000),
    runningJob('searching', 99, 1_000, previous.progress.counts, 1_000),
    runningJob('verifying_certificate', 99, 1_000, previous.progress.counts, null),
    runningJob('searching', 100, 999, previous.progress.counts, 1_000),
    runningJob('searching', 100, 1_000, {
      ...previous.progress.counts,
      face_count: 3,
    }, 1_000),
    runningJob('searching', 100, 1_000, {
      ...previous.progress.counts,
      overlap_cell_count: 9,
    }, 1_000),
    runningJob('searching', 100, 1_000, {
      ...previous.progress.counts,
      constraint_count: 19,
    }, 1_000),
    runningJob('searching', 100, 1_000, {
      ...previous.progress.counts,
      search_node_count: 29,
    }, 1_000),
    runningJob('searching', 100, 1_000, previous.progress.counts, null),
    queuedJob(),
  ]

  for (const regression of regressions) {
    const scheduler = manualScheduler()
    const cancelCalls: string[] = []
    const coordinator = requiredCoordinator({
      transport: transportFixture({
        begin: async () => ({ jobId: FIRST_JOB_ID, job: previous }),
        poll: async () => regression,
        cancel: async (jobId) => {
          cancelCalls.push(jobId)
        },
      }),
      scheduler,
      onState: () => undefined,
    })
    coordinator.start(FIRST_CONTEXT, 30)
    await settlePromises()
    scheduler.runNext()
    await settlePromises()
    const job = coordinator.getState().job
    assert.equal(job?.state, 'failed')
    assert.equal(
      job?.state === 'failed' ? job.error_category : null,
      'result_unavailable',
    )
    assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
    assert.equal(scheduler.pendingCount(), 0)
  }
})

test('a phase advance keeps cumulative work but may replace its phase-local total', async () => {
  const previous = runningJob('building_constraints', 500, 1_000, {
    face_count: 4,
    overlap_cell_count: 10,
    constraint_count: 20,
    search_node_count: 30,
  }, 500)
  const next = runningJob(
    'searching',
    500,
    1_001,
    previous.progress.counts,
    null,
  )
  const scheduler = manualScheduler()
  const cancelCalls: string[] = []
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({ jobId: FIRST_JOB_ID, job: previous }),
      poll: async () => next,
      cancel: async (jobId) => {
        cancelCalls.push(jobId)
      },
    }),
    scheduler,
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  scheduler.runNext()
  await settlePromises()

  const job = coordinator.getState().job
  assert.equal(job?.state, 'running')
  assert.equal(
    job?.state === 'running' ? job.progress.phase : null,
    'searching',
  )
  assert.equal(
    job?.state === 'running' ? job.progress.completed_work : null,
    500,
  )
  assert.deepEqual(cancelCalls, [])
  assert.equal(scheduler.pendingCount(), 1)
})

test('malformed and rejected transport data become closed failures without raw text', async () => {
  const privateValue =
    'C:\\Users\\alice\\秘密の作品.ori; point=(12.3,45.6); internal_id=7'
  for (const poll of [
    async () => ({
      ...runningJob('searching', 1, 1_000),
      raw_error: privateValue,
    }),
    async () => {
      throw new Error(privateValue)
    },
  ]) {
    const scheduler = manualScheduler()
    const coordinator = requiredCoordinator({
      transport: transportFixture({ poll }),
      scheduler,
      onState: () => undefined,
    })
    coordinator.start(FIRST_CONTEXT, 30)
    await settlePromises()
    scheduler.runNext()
    await settlePromises()
    const serialized = JSON.stringify(coordinator.getState())
    assert.equal(coordinator.getState().job?.state, 'failed')
    assert.doesNotMatch(
      serialized,
      /alice|秘密の作品|12\.3|45\.6|internal_id/iu,
    )
  }

  const rejectedBegin = requiredCoordinator({
    transport: transportFixture({
      begin: async () => {
        throw new Error(privateValue)
      },
    }),
    scheduler: manualScheduler(),
    onState: () => undefined,
  })
  assert.equal(rejectedBegin.start(FIRST_CONTEXT, 30), true)
  await settlePromises()
  assert.equal(rejectedBegin.getState().job?.state, 'failed')
  assert.equal(
    rejectedBegin.getState().job?.state === 'failed'
      ? rejectedBegin.getState().job.error_category
      : null,
    'worker_unavailable',
  )
  assert.doesNotMatch(JSON.stringify(rejectedBegin.getState()), /alice|秘密/iu)
})

test('cancel stays visible and retries a rejected native request on the same generation', async () => {
  const scheduler = manualScheduler()
  const begin = deferred<Readonly<{
    jobId: string
    job: GlobalFlatFoldabilityJobDto
  }>>()
  const cancelCalls: string[] = []
  let cancelAttempt = 0
  const pollJobs = [
    runningJob('searching', 4, 500),
    cancelledJob(5, 600),
  ]
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: () => begin.promise,
      poll: async () => pollJobs.shift(),
      cancel: async (jobId) => {
        cancelCalls.push(jobId)
        cancelAttempt += 1
        if (cancelAttempt === 1) {
          throw new Error('private cancellation transport error')
        }
      },
    }),
    scheduler,
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  assert.equal(coordinator.cancel(), true)
  assert.equal(coordinator.cancel(), false)
  assert.equal(activeCancelRequested(coordinator.getState()), true)
  assert.deepEqual(cancelCalls, [])

  begin.resolve({ jobId: FIRST_JOB_ID, job: queuedJob() })
  await settlePromises()
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
  assert.equal(activeCancelRequested(coordinator.getState()), true)

  scheduler.runNext()
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'running')
  assert.equal(activeCancelRequested(coordinator.getState()), true)
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID, FIRST_JOB_ID])
  assert.equal(coordinator.cancel(), false)
  assert.equal(scheduler.pendingCount(), 1)

  scheduler.runNext()
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'cancelled')
  assert.equal(scheduler.pendingCount(), 0)
  assert.equal(coordinator.cancel(), false)
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID, FIRST_JOB_ID])
})

test('synchronous and delayed cancel failures remain retryable until one request succeeds', async () => {
  const scheduler = manualScheduler()
  const delayedFailure = deferred<void>()
  const cancelCalls: string[] = []
  let attempt = 0
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      cancel: (jobId) => {
        cancelCalls.push(jobId)
        attempt += 1
        if (attempt === 1) {
          throw new GlobalFlatFoldabilityNativeError('result_unavailable')
        }
        if (attempt === 2) return delayedFailure.promise
        return Promise.resolve()
      },
    }),
    scheduler,
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()

  assert.equal(coordinator.cancel(), true)
  assert.equal(activeCancelRequested(coordinator.getState()), true)
  assert.equal(coordinator.cancel(), true)
  assert.equal(coordinator.cancel(), false, 'an in-flight retry is not duplicated')
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID, FIRST_JOB_ID])

  delayedFailure.reject(new Error('private delayed rejection'))
  await settlePromises()
  assert.equal(coordinator.cancel(), true)
  assert.equal(coordinator.cancel(), false, 'a fulfilled request seals cancellation')
  await settlePromises()
  assert.equal(coordinator.cancel(), false)
  assert.deepEqual(cancelCalls, [
    FIRST_JOB_ID,
    FIRST_JOB_ID,
    FIRST_JOB_ID,
  ])
})

test('a late cancel rejection cannot mutate a replacement generation or disposed state', async () => {
  const scheduler = manualScheduler()
  const lateCancellation = deferred<void>()
  const secondBegin = deferred<Readonly<{
    jobId: string
    job: GlobalFlatFoldabilityJobDto
  }>>()
  let beginCount = 0
  let firstCancelPending = true
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: () => {
        beginCount += 1
        return beginCount === 1
          ? Promise.resolve({ jobId: FIRST_JOB_ID, job: queuedJob() })
          : secondBegin.promise
      },
      cancel: () => {
        if (firstCancelPending) {
          firstCancelPending = false
          return lateCancellation.promise
        }
        return Promise.resolve()
      },
    }),
    scheduler,
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  assert.equal(coordinator.cancel(), true)
  assert.equal(coordinator.start(SECOND_CONTEXT, 30), true)

  lateCancellation.reject(new Error('late private cancellation error'))
  secondBegin.resolve({ jobId: SECOND_JOB_ID, job: queuedJob() })
  await settlePromises()
  assert.equal(coordinator.getState().generation, 2)
  assert.equal(activeCancelRequested(coordinator.getState()), false)

  coordinator.dispose()
  await settlePromises()
  assert.equal(coordinator.getState().job, null)
})

test('observer re-entry sends one cancellation request and cannot duplicate the in-flight call', async () => {
  const scheduler = manualScheduler()
  const cancellation = deferred<void>()
  const cancelCalls: string[] = []
  let reentered: boolean | null = null
  let coordinator: GlobalFlatFoldabilityCoordinator
  coordinator = requiredCoordinator({
    transport: transportFixture({
      cancel: (jobId) => {
        cancelCalls.push(jobId)
        return cancellation.promise
      },
    }),
    scheduler,
    onState: (state) => {
      if (
        reentered === null
        && activeCancelRequested(state) === true
      ) {
        reentered = coordinator.cancel()
      }
    },
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  assert.equal(coordinator.cancel(), true)
  assert.equal(reentered, true)
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
  assert.equal(coordinator.cancel(), false)

  cancellation.resolve()
  await settlePromises()
  assert.equal(coordinator.cancel(), false)
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
})

test('closed native error categories survive coordinator failures without raw text', async () => {
  const beginFailure = requiredCoordinator({
    transport: transportFixture({
      begin: async () => {
        throw new GlobalFlatFoldabilityNativeError('snapshot_unavailable')
      },
    }),
    scheduler: manualScheduler(),
    onState: () => undefined,
  })
  beginFailure.start(FIRST_CONTEXT, 30)
  await settlePromises()
  const beginJob = beginFailure.getState().job
  assert.equal(beginJob?.state, 'failed')
  assert.equal(
    beginJob?.state === 'failed' ? beginJob.error_category : null,
    'snapshot_unavailable',
  )

  const scheduler = manualScheduler()
  const pollFailure = requiredCoordinator({
    transport: transportFixture({
      poll: async () => {
        throw new GlobalFlatFoldabilityNativeError('internal_failure')
      },
    }),
    scheduler,
    onState: () => undefined,
  })
  pollFailure.start(FIRST_CONTEXT, 30)
  await settlePromises()
  scheduler.runNext()
  await settlePromises()
  const pollJob = pollFailure.getState().job
  assert.equal(pollJob?.state, 'failed')
  assert.equal(
    pollJob?.state === 'failed' ? pollJob.error_category : null,
    'internal_failure',
  )
  assert.doesNotMatch(
    JSON.stringify([beginFailure.getState(), pollFailure.getState()]),
    /stack|path|private/iu,
  )
})

test('every closed terminal DTO stops polling without changing its outcome', async () => {
  const terminals = [
    possibleJob(10, 2_000),
    impossibleJob(2_000),
    unknownJob(2_000),
    cancelledJob(10, 2_000),
    failedJob(2_000),
    staleJob(2_000),
  ]
  for (const terminal of terminals) {
    const scheduler = manualScheduler()
    const coordinator = requiredCoordinator({
      transport: transportFixture({
        poll: async () => terminal,
      }),
      scheduler,
      onState: () => undefined,
    })
    coordinator.start(FIRST_CONTEXT, 30)
    await settlePromises()
    scheduler.runNext()
    await settlePromises()
    assert.deepEqual(coordinator.getState().job, terminal)
    assert.equal(scheduler.pendingCount(), 0)
  }
})

test('a terminal summary cannot move elapsed time or bounded counts backward', async () => {
  const scheduler = manualScheduler()
  const previous = runningJob('searching', 100, 1_500)
  const regressed = possibleJob(100, 1_499)
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({ jobId: FIRST_JOB_ID, job: previous }),
      poll: async () => regressed,
    }),
    scheduler,
    onState: () => undefined,
  })
  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  scheduler.runNext()
  await settlePromises()
  const job = coordinator.getState().job
  assert.equal(job?.state, 'failed')
  assert.equal(
    job?.state === 'failed' ? job.error_category : null,
    'result_unavailable',
  )
  assert.equal(
    job?.state === 'failed' ? job.summary.elapsed_ms : null,
    1_500,
  )
})

test('context invalidation publishes stale from known summary and revokes timers', async () => {
  const scheduler = manualScheduler()
  const cancelCalls: string[] = []
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({
        jobId: FIRST_JOB_ID,
        job: runningJob('building_constraints', 8, 1_200),
      }),
      cancel: async (jobId) => {
        cancelCalls.push(jobId)
      },
    }),
    scheduler,
    onState: () => undefined,
  })
  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  const oldHandle = scheduler.onlyPendingHandle()

  assert.equal(coordinator.invalidate(FIRST_CONTEXT), false)
  assert.equal(coordinator.invalidate(SECOND_CONTEXT), true)
  const stale = coordinator.getState()
  assert.equal(stale.generation, 2)
  assert.equal(stale.job?.state, 'stale')
  assert.equal(
    stale.job?.state === 'stale' ? stale.job.summary.elapsed_ms : null,
    1_200,
  )
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
  assert.equal(scheduler.pendingCount(), 0)
  scheduler.force(oldHandle)
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'stale')
})

test('a fingerprint change invalidates a completed result with the same ID and revision', async () => {
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({
        jobId: FIRST_JOB_ID,
        job: possibleJob(20, 2_000),
      }),
    }),
    scheduler: manualScheduler(),
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'completed')

  assert.equal(coordinator.invalidate(FIRST_CONTEXT), false)
  assert.equal(coordinator.invalidate({
    ...FIRST_CONTEXT,
    foldModelFingerprint: 'c'.repeat(64),
  }), true)
  const stale = coordinator.getState()
  assert.equal(stale.generation, 2)
  assert.equal(stale.job?.state, 'stale')
  assert.equal(
    stale.job?.state === 'stale' ? stale.job.summary.elapsed_ms : null,
    2_000,
  )
})

test('forced replacement invalidates even an identical completed snapshot tuple', async () => {
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({
        jobId: FIRST_JOB_ID,
        job: possibleJob(20, 2_000),
      }),
    }),
    scheduler: manualScheduler(),
    onState: () => undefined,
  })

  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'completed')
  assert.equal(coordinator.invalidate(FIRST_CONTEXT), false)
  assert.equal(coordinator.invalidate(FIRST_CONTEXT, true), true)
  assert.equal(coordinator.getState().job?.state, 'stale')
})

test('a reopened project instance invalidates an otherwise identical snapshot tuple', async () => {
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      begin: async () => ({
        jobId: FIRST_JOB_ID,
        job: possibleJob(10, 2_000),
      }),
    }),
    scheduler: manualScheduler(),
    onState: () => undefined,
  })
  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  assert.equal(coordinator.getState().job?.state, 'completed')

  assert.equal(coordinator.invalidate({
    ...FIRST_CONTEXT,
    projectInstanceId: SECOND_CONTEXT.projectInstanceId,
  }), true)
  assert.equal(coordinator.getState().job?.state, 'stale')
})

test('dispose is permanent, clears work, cancels once and ignores late callbacks', async () => {
  const scheduler = manualScheduler()
  const poll = deferred<unknown>()
  const cancelCalls: string[] = []
  const coordinator = requiredCoordinator({
    transport: transportFixture({
      poll: () => poll.promise,
      cancel: async (jobId) => {
        cancelCalls.push(jobId)
      },
    }),
    scheduler,
    onState: () => undefined,
  })
  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  const handle = scheduler.onlyPendingHandle()
  scheduler.runNext()
  await settlePromises()

  coordinator.dispose()
  coordinator.dispose()
  assert.equal(coordinator.getState().job, null)
  assert.equal(coordinator.start(SECOND_CONTEXT, 30), false)
  assert.equal(coordinator.cancel(), false)
  assert.deepEqual(cancelCalls, [FIRST_JOB_ID])
  poll.resolve(possibleJob(10, 2_000))
  scheduler.force(handle)
  await settlePromises()
  assert.equal(coordinator.getState().job, null)
})

test('observer re-entry cannot let an old publication schedule more work', async () => {
  const scheduler = manualScheduler()
  let coordinator: GlobalFlatFoldabilityCoordinator
  let replaced = false
  coordinator = requiredCoordinator({
    transport: transportFixture({
      poll: async () => runningJob('searching', 5, 500),
    }),
    scheduler,
    onState: (state) => {
      if (
        !replaced
        && state.job?.state === 'running'
        && state.job.progress.phase === 'searching'
      ) {
        replaced = true
        coordinator.start(SECOND_CONTEXT, 5)
      }
    },
  })
  coordinator.start(FIRST_CONTEXT, 30)
  await settlePromises()
  scheduler.runNext()
  await settlePromises()

  assert.equal(replaced, true)
  assert.equal(coordinator.getState().generation, 2)
  assert.equal(coordinator.getState().job?.state, 'queued')
  assert.equal(scheduler.pendingCount(), 1)
})

test('synchronous throws, scheduler failure and hostile thenables remain closed', async () => {
  const privateValue = 'C:\\Users\\alice\\private.ori'
  const throwingBegin = transportFixture()
  Object.defineProperty(throwingBegin, 'begin', {
    value: () => {
      throw new Error(privateValue)
    },
  })
  const first = requiredCoordinator({
    transport: throwingBegin,
    scheduler: manualScheduler(),
    onState: () => undefined,
  })
  assert.equal(first.start(FIRST_CONTEXT, 30), false)
  assert.equal(first.getState().job?.state, 'failed')

  const throwingScheduler = {
    setTimeout() {
      throw new Error(privateValue)
    },
    clearTimeout() {
      throw new Error(privateValue)
    },
  }
  const second = requiredCoordinator({
    transport: transportFixture(),
    scheduler: throwingScheduler,
    onState: () => undefined,
  })
  assert.equal(second.start(FIRST_CONTEXT, 30), true)
  await settlePromises()
  assert.equal(second.getState().job?.state, 'failed')
  assert.equal(
    second.getState().job?.state === 'failed'
      ? second.getState().job.error_category
      : null,
    'internal_failure',
  )

  const hostileThenable = Object.create(null) as Record<string, unknown>
  Object.defineProperty(hostileThenable, 'then', {
    get() {
      throw new Error(privateValue)
    },
  })
  const third = requiredCoordinator({
    transport: {
      begin: () => hostileThenable,
      poll: async () => queuedJob(),
      cancel: async () => undefined,
    } as unknown as GlobalFlatFoldabilityTransport,
    scheduler: manualScheduler(),
    onState: () => undefined,
  })
  assert.equal(third.start(FIRST_CONTEXT, 30), true)
  await settlePromises()
  assert.equal(third.getState().job?.state, 'failed')
  assert.doesNotMatch(JSON.stringify(third.getState()), /alice|private/iu)
})

function requiredCoordinator<Handle>(
  options: GlobalFlatFoldabilityCoordinatorOptions<Handle>,
) {
  const coordinator = createGlobalFlatFoldabilityCoordinator(options)
  assert.ok(coordinator)
  return coordinator
}

function transportFixture(
  overrides: Partial<GlobalFlatFoldabilityTransport> = {},
): GlobalFlatFoldabilityTransport {
  return {
    begin: overrides.begin ?? (async () => ({
      jobId: FIRST_JOB_ID,
      job: queuedJob(),
    })),
    poll: overrides.poll ?? (async () => runningJob('searching', 1, 100)),
    cancel: overrides.cancel ?? (async () => undefined),
  }
}

function manualScheduler(): GlobalFlatFoldabilityTimeoutScheduler<number> & {
  pendingCount(): number
  onlyPendingHandle(): number
  runNext(): void
  force(handle: number): void
  delays(): readonly number[]
} {
  let nextHandle = 1
  const callbacks = new Map<number, () => void>()
  const pending = new Set<number>()
  const recordedDelays: number[] = []
  return {
    setTimeout(callback, delayMs) {
      const handle = nextHandle
      nextHandle += 1
      callbacks.set(handle, callback)
      pending.add(handle)
      recordedDelays.push(delayMs)
      return handle
    },
    clearTimeout(handle) {
      pending.delete(handle)
    },
    pendingCount: () => pending.size,
    onlyPendingHandle() {
      assert.equal(pending.size, 1)
      return [...pending][0] as number
    },
    runNext() {
      const handle = [...pending][0]
      assert.ok(handle !== undefined)
      pending.delete(handle)
      callbacks.get(handle)?.()
    },
    force(handle) {
      callbacks.get(handle)?.()
    },
    delays: () => Object.freeze([...recordedDelays]),
  }
}

function queuedJob(): GlobalFlatFoldabilityJobDto {
  return {
    state: 'queued',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'capturing',
      completed_work: 0,
      total_work: null,
      elapsed_ms: 0,
      counts: {
        face_count: 0,
        overlap_cell_count: 0,
        constraint_count: 0,
        search_node_count: 0,
      },
    },
  }
}

function runningJob(
  phase: GlobalFlatFoldabilityPhase,
  completedWork: number,
  elapsedMs: number,
  counts = {
    face_count: 4,
    overlap_cell_count: 10,
    constraint_count: 20,
    search_node_count: 30,
  },
  totalWork: number | null = null,
): Extract<GlobalFlatFoldabilityJobDto, { state: 'running' }> {
  return {
    state: 'running',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase,
      completed_work: completedWork,
      total_work: totalWork,
      elapsed_ms: elapsedMs,
      counts,
    },
  }
}

function possibleJob(
  completedWork: number,
  elapsedMs: number,
): GlobalFlatFoldabilityJobDto {
  return {
    state: 'completed',
    result: {
      verdict: 'possible',
      summary: {
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        elapsed_ms: elapsedMs,
        counts: {
          face_count: 4,
          overlap_cell_count: 10,
          constraint_count: 20,
          search_node_count: Math.max(30, completedWork),
        },
      },
      layer_order: {
        model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
        layer_count: 4,
        max_ply: 3,
        reference_face_number: 1,
        layer_view_available: true,
      },
    },
  }
}

function cancelledJob(
  completedWork: number,
  elapsedMs: number,
): GlobalFlatFoldabilityJobDto {
  return {
    state: 'cancelled',
    summary: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      elapsed_ms: elapsedMs,
      counts: {
        face_count: 4,
        overlap_cell_count: 10,
        constraint_count: 20,
        search_node_count: Math.max(30, completedWork),
      },
    },
  }
}

function impossibleJob(elapsedMs: number): GlobalFlatFoldabilityJobDto {
  return {
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: terminalFixtureSummary(elapsedMs),
      proof: {
        category: 'layer_constraints_contradictory',
        face_numbers: [1, 2],
      },
    },
  }
}

function unknownJob(elapsedMs: number): GlobalFlatFoldabilityJobDto {
  return {
    state: 'completed',
    result: {
      verdict: 'unknown',
      summary: terminalFixtureSummary(elapsedMs),
      reason: 'proof_not_completed',
    },
  }
}

function failedJob(elapsedMs: number): GlobalFlatFoldabilityJobDto {
  return {
    state: 'failed',
    summary: terminalFixtureSummary(elapsedMs),
    error_category: 'internal_failure',
  }
}

function staleJob(elapsedMs: number): GlobalFlatFoldabilityJobDto {
  return {
    state: 'stale',
    summary: terminalFixtureSummary(elapsedMs),
  }
}

function terminalFixtureSummary(elapsedMs: number) {
  return {
    model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    elapsed_ms: elapsedMs,
    counts: {
      face_count: 4,
      overlap_cell_count: 10,
      constraint_count: 20,
      search_node_count: 30,
    },
  } as const
}

function activeWork(state: GlobalFlatFoldabilityCoordinatorState) {
  const job = state.job
  return job?.state === 'queued' || job?.state === 'running'
    ? job.progress.completed_work
    : null
}

function activeCancelRequested(
  state: GlobalFlatFoldabilityCoordinatorState,
) {
  const job = state.job
  return job?.state === 'queued' || job?.state === 'running'
    ? job.cancel_requested
    : null
}

function completedVerdict(state: GlobalFlatFoldabilityCoordinatorState) {
  return state.job?.state === 'completed'
    ? state.job.result.verdict
    : null
}

function deferred<Value>() {
  let resolvePromise: (value: Value) => void = () => undefined
  let rejectPromise: (reason?: unknown) => void = () => undefined
  const promise = new Promise<Value>((resolve, reject) => {
    resolvePromise = resolve
    rejectPromise = reject
  })
  return Object.freeze({
    promise,
    resolve: resolvePromise,
    reject: rejectPromise,
  })
}

async function settlePromises() {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}
