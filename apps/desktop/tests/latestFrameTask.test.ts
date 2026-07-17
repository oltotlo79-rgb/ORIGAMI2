import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createLatestFrameTask,
  type FrameTaskScheduler,
} from '../src/lib/latestFrameTask.ts'

test('many inputs before one frame execute only the latest value', () => {
  const scheduler = fakeScheduler()
  const values: number[] = []
  const task = createLatestFrameTask(scheduler, (value: number) => values.push(value))

  assert.equal(task.schedule(1), true)
  assert.equal(task.schedule(2), true)
  assert.equal(task.schedule(3), true)
  assert.equal(scheduler.pendingCount(), 1)
  assert.equal(task.hasPending(), true)

  scheduler.runNext()
  assert.deepEqual(values, [3])
  assert.equal(task.hasPending(), false)
})

test('input scheduled during execution is deferred to a second frame', () => {
  const scheduler = fakeScheduler()
  const values: number[] = []
  let task: ReturnType<typeof createLatestFrameTask<number>>
  task = createLatestFrameTask(scheduler, (value) => {
    values.push(value)
    if (value === 1) assert.equal(task.schedule(2), true)
  })

  task.schedule(1)
  scheduler.runNext()
  assert.deepEqual(values, [1])
  assert.equal(scheduler.pendingCount(), 1)
  scheduler.runNext()
  assert.deepEqual(values, [1, 2])
})

test('dispose cancels the pending callback and permanently rejects work', () => {
  const scheduler = fakeScheduler()
  const values: number[] = []
  const task = createLatestFrameTask(scheduler, (value: number) => values.push(value))

  task.schedule(1)
  task.dispose()
  task.dispose()
  assert.equal(scheduler.pendingCount(), 0)
  assert.equal(task.hasPending(), false)
  assert.equal(task.schedule(2), false)
  assert.deepEqual(values, [])
})

test('scheduler failures and task errors stay inside the explicit boundary', () => {
  const reported: unknown[] = []
  const throwingScheduler: FrameTaskScheduler = {
    request: () => { throw new Error('request failed') },
    cancel: () => undefined,
  }
  const rejected = createLatestFrameTask(throwingScheduler, () => undefined, (error) => {
    reported.push(error)
  })
  assert.equal(rejected.schedule('value'), false)
  assert.equal(rejected.hasPending(), false)
  assert.deepEqual(reported, [])

  const scheduler = fakeScheduler()
  const task = createLatestFrameTask(scheduler, () => {
    throw new Error('run failed')
  }, (error) => {
    reported.push(error)
  })
  task.schedule('value')
  assert.doesNotThrow(() => scheduler.runNext())
  assert.equal(reported.length, 1)
  assert.match(String(reported[0]), /run failed/u)
})

function fakeScheduler() {
  let nextHandle = 1
  const callbacks = new Map<number, () => void>()
  return {
    request: (callback: () => void) => {
      const handle = nextHandle
      nextHandle += 1
      callbacks.set(handle, callback)
      return handle
    },
    cancel: (handle: number) => {
      callbacks.delete(handle)
    },
    pendingCount: () => callbacks.size,
    runNext: () => {
      const next = callbacks.entries().next().value as [number, () => void] | undefined
      assert.ok(next)
      callbacks.delete(next[0])
      next[1]()
    },
  }
}
