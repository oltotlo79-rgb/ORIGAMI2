import assert from 'node:assert/strict'
import test from 'node:test'
import {
  ASSIGNED_LOCAL_SUMMARY_MAX_RETRIES,
  createAssignedLocalSufficiencySummaryCoordinator,
} from '../src/lib/assignedLocalSufficiencySummaryCoordinator.ts'
import { AssignedLocalSufficiencySummaryError } from '../src/lib/coreClient.ts'

const context = {
  expectedProjectInstanceId: '018f47a2-4b7a-7cc1-8abc-112233445566',
  expectedProjectId: '018f47a2-4b7a-7cc1-8abc-665544332211',
  expectedRevision: 7,
  expectedFoldModelFingerprint: 'a'.repeat(64),
}
const response = {
  version: 1 as const,
  projectInstanceId: context.expectedProjectInstanceId,
  projectId: context.expectedProjectId,
  revision: 7,
  foldModelFingerprint: context.expectedFoldModelFingerprint,
  vertices: [],
  totalReductionSteps: 0,
  authorizesProjectMutation: false as const,
}

test('busy replacement retries boundedly and publishes only the current generation', async () => {
  const timers: (() => void)[] = []
  let calls = 0
  const states: string[] = []
  const coordinator = createAssignedLocalSufficiencySummaryCoordinator({
    analyze: async () => {
      calls += 1
      if (calls === 1) throw new AssignedLocalSufficiencySummaryError('busy')
      return response
    },
    cancel: async () => undefined,
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    onState(state) { states.push(state.status) },
  })
  assert.equal(coordinator.start(context), true)
  await settle()
  timers.shift()?.()
  await settle()
  assert.equal(coordinator.getState().status, 'ready')
  assert.deepEqual(states, ['running', 'retrying', 'ready'])
})

test('retry count is capped and dispose rejects every late completion', async () => {
  const timers: (() => void)[] = []
  const coordinator = createAssignedLocalSufficiencySummaryCoordinator({
    analyze: async () => { throw new AssignedLocalSufficiencySummaryError('busy') },
    cancel: async () => undefined,
    setTimer(callback) {
      timers.push(callback)
      return timers.length as unknown as ReturnType<typeof setTimeout>
    },
    clearTimer() {},
    now: () => 0,
    onState() {},
  })
  coordinator.start(context)
  for (let index = 0; index <= ASSIGNED_LOCAL_SUMMARY_MAX_RETRIES; index += 1) {
    await settle()
    timers.shift()?.()
  }
  await settle()
  assert.deepEqual(coordinator.getState(), {
    status: 'failed',
    generation: 1,
    reason: 'busy',
  })
  coordinator.dispose()
  assert.equal(coordinator.getState().status, 'idle')
  assert.equal(coordinator.start(context), false)
})

test('a late old completion cannot publish across an instance generation replacement', async () => {
  let resolveFirst!: (value: typeof response) => void
  let resolveSecond!: (value: typeof response) => void
  let calls = 0
  const coordinator = createAssignedLocalSufficiencySummaryCoordinator({
    analyze: () => new Promise((resolve) => {
      calls += 1
      if (calls === 1) resolveFirst = resolve
      else resolveSecond = resolve
    }),
    cancel: async () => undefined,
    onState() {},
  })
  coordinator.start(context)
  const replacement = {
    ...context,
    expectedProjectInstanceId: '018f47a2-4b7a-7cc1-8abc-778899aabbcc',
  }
  coordinator.start(replacement)
  resolveFirst(response)
  await settle()
  assert.equal(coordinator.getState().status, 'running')
  resolveSecond({ ...response, projectInstanceId: replacement.expectedProjectInstanceId })
  await settle()
  assert.equal(coordinator.getState().status, 'ready')
  assert.equal(coordinator.getState().generation, 2)
})

function settle() {
  return new Promise<void>((resolve) => setImmediate(resolve))
}
