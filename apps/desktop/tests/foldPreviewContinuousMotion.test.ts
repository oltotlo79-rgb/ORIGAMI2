import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewContinuousMotionJob,
  type FoldPreviewContinuousMotionResult,
  type FoldPreviewContinuousMotionStep,
} from '../src/lib/foldPreviewContinuousMotion.ts'

test('one certified root interval clears the complete path', () => {
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => ({ kind: 'clear' }),
  })
  assert.ok(job)
  assert.deepEqual(job.step(1), {
    kind: 'clear',
    certifiedSafeThrough: 1,
    stopTime: 1,
    stats: {
      intervalTests: 1,
      pointTests: 2,
      pointCacheHits: 0,
      maximumDepthReached: 0,
    },
  })
})

test('unsafe or indeterminate start poses allow no positive motion', () => {
  const blocked = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'blocked', blocker: 'start-contact' }),
    certifyInterval: () => ({ kind: 'clear' }),
  })
  assert.ok(blocked)
  const blockedResult = blocked.step(1)
  assert.deepEqual(blockedResult, {
    kind: 'blocked',
    certifiedSafeThrough: 0,
    stopTime: 0,
    unsafeBracket: [0, 0],
    blockingSampleTime: 0,
    blocker: 'start-contact',
    stats: {
      intervalTests: 0,
      pointTests: 1,
      pointCacheHits: 0,
      maximumDepthReached: 0,
    },
  })
  assert.ok(Object.isFrozen(blockedResult))
  assert.ok(blockedResult.kind === 'blocked')
  assert.ok(Object.isFrozen(blockedResult.unsafeBracket))
  assert.ok(Object.isFrozen(blockedResult.stats))

  const indeterminate = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'indeterminate', reason: 'start-unknown' }),
    certifyInterval: () => ({ kind: 'clear' }),
  })
  assert.ok(indeterminate)
  assert.deepEqual(indeterminate.step(1), {
    kind: 'indeterminate',
    certifiedSafeThrough: 0,
    stopTime: 0,
    unresolvedBracket: [0, 0],
    reason: 'start-unknown',
    stats: {
      intervalTests: 0,
      pointTests: 1,
      pointCacheHits: 0,
      maximumDepthReached: 0,
    },
  })
})

test('safe endpoints cannot hide a blocking midpoint', () => {
  const evaluatedTimes = new Set<number>()
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: (time) => {
      evaluatedTimes.add(time)
      return time >= 0.49 && time <= 0.51
        ? { kind: 'blocked' }
        : { kind: 'safe' }
    },
    certifyInterval: (start, end) =>
      end < 0.49 || start > 0.51
        ? { kind: 'clear' }
        : { kind: 'unresolved' },
  }, {
    maxDepth: 12,
    minTimeSpan: 2 ** -12,
    maxIntervalTests: 100,
  })
  assert.ok(job)
  const result = runToTerminal(job, 4)
  assert.equal(result.kind, 'blocked')
  assert.ok(result.kind === 'blocked')
  assert.ok(result.certifiedSafeThrough < 0.49)
  assert.ok(result.unsafeBracket[0] <= 0.5)
  assert.ok(result.unsafeBracket[1] >= 0.49)
  assert.equal(result.blockingSampleTime, result.unsafeBracket[1])
  assert.ok(evaluatedTimes.has(result.blockingSampleTime))
  assert.equal(result.stopTime, result.certifiedSafeThrough)
})

test('chronological search stops at the first collision before later separation', () => {
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: (time) => time >= 0.3 && time <= 0.4
      ? { kind: 'blocked', blocker: 'first-window' }
      : { kind: 'safe' },
    certifyInterval: (start, end) =>
      end < 0.3 || start > 0.4
        ? { kind: 'clear' }
        : { kind: 'unresolved' },
  }, {
    maxDepth: 12,
    minTimeSpan: 2 ** -12,
    maxIntervalTests: 100,
  })
  assert.ok(job)
  const result = runToTerminal(job, 3)
  assert.equal(result.kind, 'blocked')
  assert.ok(result.kind === 'blocked')
  assert.equal(result.blocker, 'first-window')
  assert.ok(result.certifiedSafeThrough < 0.4)
  assert.ok(result.unsafeBracket[1] <= 0.4)
  assert.equal(result.blockingSampleTime, result.unsafeBracket[1])
})

test('safe samples without an interval proof remain indeterminate', () => {
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => ({ kind: 'unresolved' }),
  }, {
    maxDepth: 2,
    minTimeSpan: 2 ** -20,
    maxIntervalTests: 100,
  })
  assert.ok(job)
  const result = runToTerminal(job, 100)
  assert.deepEqual(result, {
    kind: 'indeterminate',
    certifiedSafeThrough: 0,
    stopTime: 0,
    unresolvedBracket: [0, 0.25],
    reason: 'uncertified_interval',
    stats: {
      intervalTests: 3,
      pointTests: 3,
      pointCacheHits: 1,
      maximumDepthReached: 2,
    },
  })
})

test('global interval work limits stop at the earliest pending interval', () => {
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => ({ kind: 'unresolved' }),
  }, {
    maxDepth: 10,
    minTimeSpan: 2 ** -20,
    maxIntervalTests: 1,
  })
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')
  assert.deepEqual(job.step(1), {
    kind: 'indeterminate',
    certifiedSafeThrough: 0,
    stopTime: 0,
    unresolvedBracket: [0, 0.5],
    reason: 'work_limit',
    stats: {
      intervalTests: 1,
      pointTests: 2,
      pointCacheHits: 1,
      maximumDepthReached: 1,
    },
  })
})

test('a blocking work-limit endpoint returns an unsafe bracket', () => {
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: (time) => time === 0.5
      ? { kind: 'blocked', blocker: 'limit-endpoint' }
      : { kind: 'safe' },
    certifyInterval: () => ({ kind: 'unresolved' }),
  }, {
    maxDepth: 10,
    minTimeSpan: 2 ** -20,
    maxIntervalTests: 1,
  })
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')
  const result = job.step(1)
  assert.equal(result.kind, 'blocked')
  assert.ok(result.kind === 'blocked')
  assert.deepEqual(result.unsafeBracket, [0, 0.5])
  assert.equal(result.blockingSampleTime, 0.5)
  assert.equal(result.blocker, 'limit-endpoint')
})

test('small step budgets resume one deterministic left-first search', () => {
  const calls: string[] = []
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: (time) => {
      calls.push(`p:${time}`)
      return { kind: 'safe' }
    },
    certifyInterval: (start, end) => {
      calls.push(`i:${start}-${end}`)
      return end - start <= 0.25
        ? { kind: 'clear' }
        : { kind: 'unresolved' }
    },
  })
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')
  assert.equal(job.step(1).kind, 'pending')
  assert.equal(job.step(1).kind, 'pending')
  const result = runToTerminal(job, 1)
  assert.equal(result.kind, 'clear')
  assert.deepEqual(calls.slice(0, 6), [
    'p:0',
    'i:0-1',
    'p:0.5',
    'i:0-0.5',
    'p:0.25',
    'i:0-0.25',
  ])
  assert.equal(result.stats.pointTests, 5)
})

test('cancellation is permanent and executes no later callbacks', () => {
  let callbackCalls = 0
  const job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => {
      callbackCalls += 1
      return { kind: 'safe' }
    },
    certifyInterval: () => {
      callbackCalls += 1
      return { kind: 'clear' }
    },
  })
  assert.ok(job)
  job.cancel()
  const first = job.step(1)
  const second = job.step(1)
  assert.deepEqual(first, second)
  assert.equal(first.kind, 'cancelled')
  assert.equal(callbackCalls, 0)
})

test('reentrant cancellation stops immediately after the active callback', () => {
  let job: ReturnType<typeof createFoldPreviewContinuousMotionJob>
  let pointCalls = 0
  job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => {
      pointCalls += 1
      return { kind: 'safe' }
    },
    certifyInterval: () => {
      job?.cancel()
      return { kind: 'clear' }
    },
  })
  assert.ok(job)
  const result = job.step(1)
  assert.equal(result.kind, 'cancelled')
  assert.equal(pointCalls, 1)
  assert.equal(result.stats.intervalTests, 1)
})

test('a reentrant step cancels the outer mutable search', () => {
  let job: ReturnType<typeof createFoldPreviewContinuousMotionJob>
  let nestedKind: string | null = null
  job = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => {
      nestedKind = job?.step(1).kind ?? null
      return { kind: 'clear' }
    },
  })
  assert.ok(job)
  const result = job.step(1)
  assert.equal(nestedKind, 'cancelled')
  assert.equal(result.kind, 'cancelled')
  assert.equal(job.step(1).kind, 'cancelled')
})

test('throwing, malformed, and contradictory callbacks fail closed', () => {
  const thrown = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => {
      throw new Error('boom')
    },
  })
  assert.ok(thrown)
  assert.equal(thrown.step(1).kind, 'indeterminate')

  const malformed = createFoldPreviewContinuousMotionJob({
    evaluatePoint: () => ({ kind: 'safe' }),
    certifyInterval: () => null as never,
  })
  assert.ok(malformed)
  const malformedResult = malformed.step(1)
  assert.equal(malformedResult.kind, 'indeterminate')
  assert.equal(
    malformedResult.kind === 'indeterminate' && malformedResult.reason,
    'malformed_interval_decision',
  )

  let pointCalls = 0
  const contradiction = createFoldPreviewContinuousMotionJob({
    evaluatePoint: (time) => {
      pointCalls += 1
      return time === 0.5 ? { kind: 'blocked' } : { kind: 'safe' }
    },
    certifyInterval: (start, end) =>
      start === 0 && end === 1
        ? { kind: 'unresolved' }
        : { kind: 'clear' },
  })
  assert.ok(contradiction)
  assert.equal(contradiction.step(1).kind, 'pending')
  const contradictionResult = contradiction.step(1)
  assert.equal(contradictionResult.kind, 'indeterminate')
  assert.equal(
    contradictionResult.kind === 'indeterminate'
      && contradictionResult.reason,
    'contradictory_interval_certificate',
  )
  assert.equal(pointCalls, 2)
})

test('invalid construction and step budgets fail closed', () => {
  const callbacks = {
    evaluatePoint: () => ({ kind: 'safe' } as const),
    certifyInterval: () => ({ kind: 'clear' } as const),
  }
  assert.equal(createFoldPreviewContinuousMotionJob(null as never), null)
  assert.equal(createFoldPreviewContinuousMotionJob(callbacks, {
    maxDepth: -1,
  }), null)
  assert.equal(createFoldPreviewContinuousMotionJob(callbacks, {
    maxIntervalTests: 0,
  }), null)
  assert.equal(createFoldPreviewContinuousMotionJob(callbacks, {
    minTimeSpan: Number.NaN,
  }), null)

  const job = createFoldPreviewContinuousMotionJob(callbacks)
  assert.ok(job)
  const result = job.step(0)
  assert.equal(result.kind, 'indeterminate')
  assert.equal(
    result.kind === 'indeterminate' && result.reason,
    'invalid_work_budget',
  )
})

test('callback and option records are snapshotted before execution', () => {
  let pointCalls = 0
  const callbacks = {
    evaluatePoint: () => {
      pointCalls += 1
      return { kind: 'safe' } as const
    },
    certifyInterval: () => ({ kind: 'clear' } as const),
  }
  const options = {
    maxDepth: 4,
    maxIntervalTests: 8,
    minTimeSpan: 0.01,
  }
  const job = createFoldPreviewContinuousMotionJob(callbacks, options)
  assert.ok(job)
  callbacks.evaluatePoint = () => ({ kind: 'blocked' } as const)
  callbacks.certifyInterval = () => ({ kind: 'unresolved' } as const)
  options.maxIntervalTests = 0
  assert.equal(job.step(1).kind, 'clear')
  assert.equal(pointCalls, 2)
})

test('a clear interval cannot bypass an unsafe or unknown target pose', () => {
  for (const terminal of ['blocked', 'indeterminate'] as const) {
    const job = createFoldPreviewContinuousMotionJob({
      evaluatePoint: (time) => time === 1
        ? terminal === 'blocked'
          ? { kind: 'blocked', blocker: 'target' }
          : { kind: 'indeterminate', reason: 'target-unknown' }
        : { kind: 'safe' },
      certifyInterval: () => ({ kind: 'clear' }),
    })
    assert.ok(job)
    const result = job.step(1)
    assert.equal(result.kind, terminal)
    assert.equal(result.certifiedSafeThrough, 0)
    if (result.kind === 'blocked') {
      assert.equal(result.blockingSampleTime, 1)
    }
    assert.deepEqual(
      result.kind === 'blocked'
        ? result.unsafeBracket
        : result.kind === 'indeterminate'
          ? result.unresolvedBracket
          : null,
      [0, 1],
    )
  }
})

function runToTerminal<Blocker>(
  job: Readonly<{
    step(workBudget: number): FoldPreviewContinuousMotionStep<Blocker>
  }>,
  workBudget: number,
): FoldPreviewContinuousMotionResult<Blocker> {
  for (let index = 0; index < 10_000; index += 1) {
    const step = job.step(workBudget)
    if (step.kind !== 'pending') return step
  }
  throw new Error('continuous motion job did not terminate')
}
