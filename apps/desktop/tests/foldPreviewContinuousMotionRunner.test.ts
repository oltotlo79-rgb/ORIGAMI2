import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewContinuousMotionRunner,
  type FoldPreviewContinuousMotionRunner,
  type FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'
import type {
  FoldPreviewContinuousMotionJob,
  FoldPreviewContinuousMotionStep,
} from '../src/lib/foldPreviewContinuousMotion.ts'

const stats = Object.freeze({
  intervalTests: 0,
  pointTests: 0,
  pointCacheHits: 0,
  maximumDepthReached: 0,
})

test('one work unit is scheduled at a time and a clear result applies the target', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([
    pending(0.25),
    clear(),
  ])
  const factoryCalls: Array<readonly [number, number]> = []
  const applied: number[] = []
  const states: FoldPreviewContinuousMotionRunnerState[] = []
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: (start, target) => {
      factoryCalls.push([start, target])
      return job
    },
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: (state) => states.push(state),
  })
  assert.ok(runner)

  assert.equal(runner.request(80), true)
  assert.deepEqual(factoryCalls, [[0, 80]])
  assert.deepEqual(job.stepBudgets, [])
  assert.equal(states.at(-1)?.status, 'running')

  const firstHandle = scheduler.latestHandle()
  scheduler.run(firstHandle)
  assert.deepEqual(job.stepBudgets, [1])
  assert.deepEqual(applied, [20])
  assert.deepEqual(runner.getState(), {
    requested: 80,
    applied: 20,
    start: 0,
    status: 'running',
    reason: null,
    result: null,
  })

  // A scheduler invoking the already-consumed callback again cannot advance
  // the mutable job a second time.
  scheduler.run(firstHandle, true)
  assert.deepEqual(job.stepBudgets, [1])
  assert.deepEqual(applied, [20])

  scheduler.run(scheduler.latestHandle())
  assert.deepEqual(job.stepBudgets, [1, 1])
  assert.deepEqual(applied, [20, 80])
  assert.equal(runner.getState().status, 'clear')
  assert.equal(runner.getState().result?.kind, 'clear')
  assert.equal(states.at(-1)?.applied, 80)
})

test('the last successfully applied angle becomes the next request start', () => {
  const scheduler = new ManualScheduler()
  const first = scriptedJob([pending(0.4), clear()])
  const second = scriptedJob([clear()])
  const starts: Array<readonly [number, number]> = []
  let factoryIndex = 0
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 10,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: (start, target) => {
      starts.push([start, target])
      return factoryIndex++ === 0 ? first : second
    },
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(60)
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().applied, 30)
  runner.request(90)

  assert.deepEqual(starts, [
    [10, 60],
    [30, 90],
  ])
  assert.equal(first.cancelCalls, 1)
  assert.equal(runner.getState().start, 30)
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().applied, 90)
})

test('reverse paths interpolate certified lower bounds toward smaller angles', () => {
  const scheduler = new ManualScheduler()
  const applied: number[] = []
  const job = scriptedJob([blocked(0.25, 'paper-contact')])
  const runner = createFoldPreviewContinuousMotionRunner<string>({
    initialAngle: 90,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(10)
  scheduler.run(scheduler.latestHandle())

  assert.deepEqual(applied, [70])
  assert.deepEqual(runner.getState(), {
    requested: 10,
    applied: 70,
    start: 90,
    status: 'blocked',
    reason: 'motion_blocked',
    result: blocked(0.25, 'paper-contact'),
  })
})

test('indeterminate terminals apply only their certified safe lower bound', () => {
  const scheduler = new ManualScheduler()
  const applied: number[] = []
  const terminal = indeterminate(0.6, 'narrow_phase_unknown')
  const job = scriptedJob([terminal])
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 20,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(70)
  scheduler.run(scheduler.latestHandle())
  assert.deepEqual(applied, [50])
  assert.equal(runner.getState().status, 'indeterminate')
  assert.equal(runner.getState().reason, 'narrow_phase_unknown')
  assert.deepEqual(runner.getState().result, terminal)
})

test('a replacement cancels the old job and frame before stale work can run', () => {
  const scheduler = new ManualScheduler()
  const first = scriptedJob([clear()])
  const second = scriptedJob([clear()])
  const jobs = [first, second]
  const states: FoldPreviewContinuousMotionRunnerState[] = []
  const applied: number[] = []
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => jobs.shift() ?? null,
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: (state) => states.push(state),
  })
  assert.ok(runner)

  runner.request(40)
  const staleHandle = scheduler.latestHandle()
  runner.request(70)
  const currentHandle = scheduler.latestHandle()
  const stateCount = states.length

  assert.equal(first.cancelCalls, 1)
  assert.ok(scheduler.cancelledHandles.includes(staleHandle))
  scheduler.run(staleHandle, true)
  assert.deepEqual(first.stepBudgets, [])
  assert.deepEqual(applied, [])
  assert.equal(states.length, stateCount)

  scheduler.run(currentHandle)
  assert.deepEqual(second.stepBudgets, [1])
  assert.deepEqual(applied, [70])
  assert.equal(runner.getState().status, 'clear')
})

test('a cancelled job result and stale terminal callback publish no state', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([cancelled(0.3)])
  const states: FoldPreviewContinuousMotionRunnerState[] = []
  let applyCalls = 0
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 5,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: () => {
      applyCalls += 1
      return true
    },
    onState: (state) => states.push(state),
  })
  assert.ok(runner)

  runner.request(30)
  const handle = scheduler.latestHandle()
  const before = states.length
  scheduler.run(handle)
  scheduler.run(handle, true)

  assert.equal(states.length, before)
  assert.equal(applyCalls, 0)
  assert.equal(job.stepBudgets.length, 1)
})

test('null, throwing, and malformed factories fail closed', () => {
  const cases: ReadonlyArray<{
    expected: string
    factory: () => unknown
  }> = [
    {
      expected: 'job_factory_returned_null',
      factory: () => null,
    },
    {
      expected: 'job_factory_error',
      factory: () => {
        throw new Error('factory failure')
      },
    },
    {
      expected: 'job_factory_returned_malformed_job',
      factory: () => ({ step: () => clear() }),
    },
  ]

  for (const { expected, factory } of cases) {
    const scheduler = new ManualScheduler()
    let applyCalls = 0
    const runner = createFoldPreviewContinuousMotionRunner({
      initialAngle: 0,
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      jobFactory: factory as never,
      applyAngle: () => {
        applyCalls += 1
        return true
      },
      onState: () => {},
    })
    assert.ok(runner)
    assert.equal(runner.request(45), false)
    assert.equal(runner.getState().status, 'indeterminate')
    assert.equal(runner.getState().reason, expected)
    assert.equal(runner.getState().applied, 0)
    assert.equal(applyCalls, 0)
  }
})

test('scheduler failure cancels the job and preserves the applied angle', () => {
  const job = scriptedJob([clear()])
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 12,
    schedule: () => {
      throw new Error('scheduler unavailable')
    },
    cancel: () => {},
    jobFactory: () => job,
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)

  assert.equal(runner.request(80), false)
  assert.equal(job.cancelCalls, 1)
  assert.deepEqual(job.stepBudgets, [])
  assert.equal(runner.getState().status, 'indeterminate')
  assert.equal(runner.getState().reason, 'scheduler_error')
  assert.equal(runner.getState().applied, 12)
})

test('throwing and malformed job steps never reach angle application', () => {
  const malformedSteps: unknown[] = [
    null,
    { kind: 'pending', certifiedSafeThrough: 0.5 },
    { ...pending(0.5), certifiedSafeThrough: 1.1 },
    pending(1),
    { ...clear(), certifiedSafeThrough: 0.9 },
    { ...blocked(0.2), unsafeBracket: [0.3, 0.2] },
    { ...blocked(0.2), unsafeBracket: [0.1, 0.3] },
    blocked(1),
    { ...indeterminate(0.2, 'unknown'), reason: '' },
    indeterminate(1, 'unknown'),
  ]

  for (const rawStep of malformedSteps) {
    const scheduler = new ManualScheduler()
    const job = scriptedJob([rawStep])
    let applyCalls = 0
    const runner = createFoldPreviewContinuousMotionRunner({
      initialAngle: 7,
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      jobFactory: () => job,
      applyAngle: () => {
        applyCalls += 1
        return true
      },
      onState: () => {},
    })
    assert.ok(runner)
    runner.request(20)
    scheduler.run(scheduler.latestHandle())
    assert.equal(runner.getState().reason, 'malformed_job_step')
    assert.equal(runner.getState().applied, 7)
    assert.equal(applyCalls, 0)
    assert.equal(job.cancelCalls, 1)
  }

  const scheduler = new ManualScheduler()
  const throwingJob = scriptedJob([])
  throwingJob.step = () => {
    throw new Error('step failure')
  }
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 9,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => throwingJob,
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)
  runner.request(20)
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().reason, 'job_step_error')
  assert.equal(runner.getState().applied, 9)
})

test('a decreasing certified time fails closed without moving backward', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([
    pending(0.6),
    pending(0.5),
  ])
  const applied: number[] = []
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(100)
  scheduler.run(scheduler.latestHandle())
  scheduler.run(scheduler.latestHandle())

  assert.deepEqual(applied, [60])
  assert.equal(runner.getState().applied, 60)
  assert.equal(runner.getState().reason, 'non_monotonic_certified_time')
  assert.equal(job.cancelCalls, 1)
})

test('rejected or throwing angle application publishes no new applied angle', () => {
  const cases: ReadonlyArray<{
    expected: string
    apply: (angle: number) => boolean
  }> = [
    {
      expected: 'apply_angle_rejected',
      apply: () => false,
    },
    {
      expected: 'apply_angle_error',
      apply: () => {
        throw new Error('renderer failure')
      },
    },
  ]

  for (const { expected, apply } of cases) {
    const scheduler = new ManualScheduler()
    const job = scriptedJob([pending(0.75)])
    const publishedApplied: number[] = []
    const runner = createFoldPreviewContinuousMotionRunner({
      initialAngle: 10,
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      jobFactory: () => job,
      applyAngle: apply,
      onState: (state) => publishedApplied.push(state.applied),
    })
    assert.ok(runner)
    runner.request(50)
    scheduler.run(scheduler.latestHandle())

    assert.equal(runner.getState().status, 'indeterminate')
    assert.equal(runner.getState().reason, expected)
    assert.equal(runner.getState().applied, 10)
    assert.ok(publishedApplied.every((angle) => angle === 10))
    assert.equal(job.cancelCalls, 1)
  }
})

test('a request made from applyAngle is rejected before it can use a stale start', () => {
  const scheduler = new ManualScheduler()
  const first = scriptedJob([clear()])
  const second = scriptedJob([clear()])
  const jobs = [first, second]
  const factoryCalls: Array<readonly [number, number]> = []
  let nestedRequest: boolean | null = null
  let runner: FoldPreviewContinuousMotionRunner | null = null
  runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: (start, target) => {
      factoryCalls.push([start, target])
      return jobs.shift() ?? null
    },
    applyAngle: () => {
      nestedRequest = runner?.request(90) ?? null
      return true
    },
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(40)
  scheduler.run(scheduler.latestHandle())
  assert.equal(nestedRequest, false)
  assert.deepEqual(factoryCalls, [[0, 40]])
  assert.equal(runner.getState().applied, 40)

  runner.request(80)
  assert.deepEqual(factoryCalls, [
    [0, 40],
    [40, 80],
  ])
})

test('dispose inside applyAngle records a successful view angle before disposal', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([pending(0.5)])
  const states: FoldPreviewContinuousMotionRunnerState[] = []
  let runner: FoldPreviewContinuousMotionRunner | null = null
  runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: () => {
      runner?.dispose()
      return true
    },
    onState: (state) => states.push(state),
  })
  assert.ok(runner)

  runner.request(100)
  const handle = scheduler.latestHandle()
  scheduler.run(handle)
  const afterDispose = states.length

  assert.equal(runner.getState().status, 'disposed')
  assert.equal(runner.getState().applied, 50)
  assert.equal(job.cancelCalls, 1)
  assert.equal(runner.request(120), false)
  scheduler.run(handle, true)
  assert.equal(states.length, afterDispose)
})

test('dispose cancels active resources and forced callbacks stay inert', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([clear()])
  const states: FoldPreviewContinuousMotionRunnerState[] = []
  let applyCalls = 0
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 15,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: () => {
      applyCalls += 1
      return true
    },
    onState: (state) => states.push(state),
  })
  assert.ok(runner)
  runner.request(45)
  const handle = scheduler.latestHandle()

  runner.dispose()
  const afterDispose = states.length
  assert.equal(job.cancelCalls, 1)
  assert.ok(scheduler.cancelledHandles.includes(handle))
  assert.equal(runner.getState().status, 'disposed')
  assert.equal(runner.request(90), false)
  scheduler.run(handle, true)

  assert.equal(states.length, afterDispose)
  assert.equal(applyCalls, 0)
  assert.deepEqual(job.stepBudgets, [])
  runner.dispose()
  assert.equal(states.length, afterDispose)
})

test('scheduler cancellation failures cannot reactivate an old generation', () => {
  const scheduler = new ManualScheduler()
  scheduler.throwOnCancel = true
  const first = scriptedJob([clear()])
  const second = scriptedJob([clear()])
  const jobs = [first, second]
  const applied: number[] = []
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => jobs.shift() ?? null,
    applyAngle: (angle) => {
      applied.push(angle)
      return true
    },
    onState: () => {},
  })
  assert.ok(runner)
  runner.request(30)
  const stale = scheduler.latestHandle()
  runner.request(60)

  scheduler.run(stale, true)
  assert.deepEqual(first.stepBudgets, [])
  scheduler.run(scheduler.latestHandle(), true)
  assert.deepEqual(applied, [60])
})

test('a reentrant job cancellation keeps the nested replacement authoritative', () => {
  const scheduler = new ManualScheduler()
  const second = scriptedJob([clear()])
  const factoryTargets: number[] = []
  let nestedAccepted: boolean | null = null
  let runner: FoldPreviewContinuousMotionRunner | null = null
  const first = scriptedJob([clear()], () => {
    nestedAccepted = runner?.request(70) ?? null
  })
  runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: (_start, target) => {
      factoryTargets.push(target)
      return target === 30 ? first : second
    },
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)

  assert.equal(runner.request(30), true)
  const staleHandle = scheduler.latestHandle()
  assert.equal(runner.request(50), false)
  assert.equal(nestedAccepted, true)
  assert.deepEqual(factoryTargets, [30, 70])
  assert.equal(runner.getState().requested, 70)

  scheduler.run(staleHandle, true)
  assert.deepEqual(first.stepBudgets, [])
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().status, 'clear')
  assert.equal(runner.getState().applied, 70)
})

test('a reentrant frame cancellation can dispose without an outer overwrite', () => {
  const scheduler = new ManualScheduler()
  const first = scriptedJob([clear()])
  let runner: FoldPreviewContinuousMotionRunner | null = null
  scheduler.onCancel = () => runner?.dispose()
  runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => first,
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)

  assert.equal(runner.request(30), true)
  assert.equal(runner.request(60), false)
  assert.equal(runner.getState().status, 'disposed')
  assert.equal(runner.getState().applied, 0)
})

test('failure cleanup cannot overwrite a request made from job cancellation', () => {
  const scheduler = new ManualScheduler()
  const replacement = scriptedJob([clear()])
  let runner: FoldPreviewContinuousMotionRunner | null = null
  const malformed = scriptedJob([{ kind: 'not-a-step' }], () => {
    runner?.request(80)
  })
  runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: (_start, target) => target === 20 ? malformed : replacement,
    applyAngle: () => true,
    onState: () => {},
  })
  assert.ok(runner)

  runner.request(20)
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().status, 'running')
  assert.equal(runner.getState().requested, 80)
  assert.notEqual(runner.getState().reason, 'malformed_job_step')

  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().status, 'clear')
  assert.equal(runner.getState().applied, 80)
})

test('state callback failures do not interrupt a safe clear result', () => {
  const scheduler = new ManualScheduler()
  const job = scriptedJob([clear()])
  const runner = createFoldPreviewContinuousMotionRunner({
    initialAngle: 0,
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    jobFactory: () => job,
    applyAngle: () => true,
    onState: () => {
      throw new Error('observer failure')
    },
  })
  assert.ok(runner)

  assert.equal(runner.request(25), true)
  scheduler.run(scheduler.latestHandle())
  assert.equal(runner.getState().status, 'clear')
  assert.equal(runner.getState().applied, 25)
})

test('invalid construction and angles outside the fold range are rejected', () => {
  const validOptions = {
    initialAngle: 0,
    schedule: (_callback: () => void) => 1,
    cancel: (_handle: number) => {},
    jobFactory: () => scriptedJob([clear()]),
    applyAngle: () => true,
    onState: () => {},
  }
  assert.equal(createFoldPreviewContinuousMotionRunner({
    ...validOptions,
    initialAngle: Number.NaN,
  }), null)
  assert.equal(createFoldPreviewContinuousMotionRunner({
    ...validOptions,
    initialAngle: -0.001,
  }), null)
  assert.equal(createFoldPreviewContinuousMotionRunner({
    ...validOptions,
    initialAngle: 180.001,
  }), null)
  assert.equal(createFoldPreviewContinuousMotionRunner({
    ...validOptions,
    schedule: null as never,
  }), null)

  const runner = createFoldPreviewContinuousMotionRunner(validOptions)
  assert.ok(runner)
  assert.equal(runner.request(Number.POSITIVE_INFINITY), false)
  assert.equal(runner.getState().reason, 'invalid_target_angle')
  assert.equal(runner.getState().requested, 0)
  assert.equal(runner.getState().applied, 0)
  assert.equal(runner.request(-0.001), false)
  assert.equal(runner.getState().reason, 'invalid_target_angle')
  assert.equal(runner.request(180.001), false)
  assert.equal(runner.getState().reason, 'invalid_target_angle')
})

class ManualScheduler {
  private nextHandle = 1
  private readonly callbacks = new Map<number, () => void>()
  readonly cancelledHandles: number[] = []
  throwOnCancel = false
  onCancel: ((handle: number) => void) | null = null

  readonly schedule = (callback: () => void): number => {
    const handle = this.nextHandle
    this.nextHandle += 1
    this.callbacks.set(handle, callback)
    return handle
  }

  readonly cancel = (handle: number): void => {
    this.cancelledHandles.push(handle)
    this.onCancel?.(handle)
    if (this.throwOnCancel) throw new Error('cancel failure')
  }

  latestHandle(): number {
    const handle = this.nextHandle - 1
    assert.ok(this.callbacks.has(handle), `missing scheduled handle ${handle}`)
    return handle
  }

  run(handle: number, force = false): void {
    if (!force && this.cancelledHandles.includes(handle)) return
    const callback = this.callbacks.get(handle)
    assert.ok(callback, `missing callback for handle ${handle}`)
    callback()
  }
}

type ScriptedJob<Blocker = unknown> = FoldPreviewContinuousMotionJob<Blocker> & {
  step: (workBudget: number) => FoldPreviewContinuousMotionStep<Blocker>
  readonly stepBudgets: number[]
  cancelCalls: number
}

function scriptedJob<Blocker = unknown>(
  steps: readonly unknown[],
  onCancel?: () => void,
): ScriptedJob<Blocker> {
  const remaining = [...steps]
  return {
    stepBudgets: [],
    cancelCalls: 0,
    step(workBudget: number) {
      this.stepBudgets.push(workBudget)
      assert.ok(remaining.length > 0, 'scripted job exhausted')
      return remaining.shift() as FoldPreviewContinuousMotionStep<Blocker>
    },
    cancel() {
      this.cancelCalls += 1
      onCancel?.()
    },
  }
}

function pending(certifiedSafeThrough: number) {
  return {
    kind: 'pending' as const,
    certifiedSafeThrough,
    stats,
  }
}

function clear() {
  return {
    kind: 'clear' as const,
    certifiedSafeThrough: 1 as const,
    stopTime: 1 as const,
    stats,
  }
}

function blocked<Blocker = unknown>(
  certifiedSafeThrough: number,
  blocker?: Blocker,
) {
  return {
    kind: 'blocked' as const,
    certifiedSafeThrough,
    stopTime: certifiedSafeThrough,
    unsafeBracket: [certifiedSafeThrough, 1] as const,
    ...(arguments.length >= 2 ? { blocker } : {}),
    stats,
  }
}

function indeterminate(certifiedSafeThrough: number, reason: string) {
  return {
    kind: 'indeterminate' as const,
    certifiedSafeThrough,
    stopTime: certifiedSafeThrough,
    unresolvedBracket: [certifiedSafeThrough, 1] as const,
    reason,
    stats,
  }
}

function cancelled(certifiedSafeThrough: number) {
  return {
    kind: 'cancelled' as const,
    certifiedSafeThrough,
    stats,
  }
}
