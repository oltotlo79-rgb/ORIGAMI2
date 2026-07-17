import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewTreeMotionOwnerState,
  FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION,
  MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_ID_LENGTH,
  MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_KEY_LENGTH,
  transitionFoldPreviewTreeMotionOwner,
  type FoldPreviewTreeMotionOwnerEvent,
  type FoldPreviewTreeMotionOwnerPlan,
  type FoldPreviewTreeMotionOwnerState,
} from '../src/lib/foldPreviewTreeMotionOwner.ts'
import {
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
} from '../src/lib/foldPreviewTreeMotionContext.ts'

const CONTEXT_KEY = 'context-a'
const HINGE_EDGE_ID = 'hinge-a'

test('creates one immutable none-owner snapshot', () => {
  const state = initialState()
  assert.deepEqual(state, {
    version: FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION,
    ownerToken: state.ownerToken,
    generation: 0,
    owner: 'none',
    directPending: false,
    directKey: null,
    runnerContextKey: null,
    runnerHingeEdgeId: null,
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: null,
    committedRequestToken: null,
  })
  assertDeeplyFrozen(state)

  for (const invalidGeneration of [
    -1,
    0.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    assert.equal(createFoldPreviewTreeMotionOwnerState({
      initialGeneration: invalidGeneration,
    }), null)
  }
})

test('direct scheduling orders cancellation before work and makes old callbacks inert', () => {
  const first = step(initialState(), {
    kind: 'schedule_direct',
    key: 'direct-a',
  })
  assert.equal(first.accepted, true)
  assert.equal(first.reason, null)
  assert.deepEqual(first.commands, [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    {
      kind: 'schedule_direct',
      ownerToken: first.state.ownerToken,
      generation: 1,
      key: 'direct-a',
    },
  ])
  assert.deepEqual(ownerSummary(first.state), {
    generation: 1,
    owner: 'direct',
    directPending: true,
    directKey: 'direct-a',
  })
  assertDeeplyFrozen(first)

  const second = step(first.state, {
    kind: 'schedule_direct',
    key: 'direct-b',
  })
  assert.equal(second.state.generation, 2)
  assert.deepEqual(second.commands.map((command) => command.kind), [
    'reset_gesture',
    'dispose_runner',
    'schedule_direct',
  ])

  const stale = step(second.state, {
    kind: 'direct_callback',
    ownerToken: second.state.ownerToken,
    generation: 1,
    key: 'direct-a',
  })
  assertRejected(stale, second.state, 'stale_generation')

  const wrongKey = step(second.state, {
    kind: 'direct_callback',
    ownerToken: second.state.ownerToken,
    generation: 2,
    key: 'direct-a',
  })
  assertRejected(wrongKey, second.state, 'direct_key_mismatch')

  const current = step(second.state, {
    kind: 'direct_callback',
    ownerToken: second.state.ownerToken,
    generation: 2,
    key: 'direct-b',
  })
  assert.equal(current.accepted, true)
  assert.equal(current.state.directPending, false)
  assert.deepEqual(current.commands, [{
    kind: 'apply_direct',
    ownerToken: second.state.ownerToken,
    generation: 2,
    key: 'direct-b',
  }])

  const duplicate = step(current.state, {
    kind: 'direct_callback',
    ownerToken: current.state.ownerToken,
    generation: 2,
    key: 'direct-b',
  })
  assertRejected(duplicate, current.state, 'direct_not_pending')
})

test('runner preparation rejects pending direct work and snapshots a new generation', () => {
  const pending = step(initialState(), {
    kind: 'schedule_direct',
    key: 'direct-a',
  })
  const rejected = step(pending.state, {
    kind: 'prepare_runner',
    ownerToken: pending.state.ownerToken,
    generation: 1,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
  })
  assertRejected(rejected, pending.state, 'direct_pending')

  const applied = step(pending.state, {
    kind: 'direct_callback',
    ownerToken: pending.state.ownerToken,
    generation: 1,
    key: 'direct-a',
  })
  const prepared = step(applied.state, {
    kind: 'prepare_runner',
    ownerToken: applied.state.ownerToken,
    generation: 1,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
  })
  assert.equal(prepared.accepted, true)
  assert.deepEqual(prepared.state, {
    version: FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION,
    ownerToken: applied.state.ownerToken,
    generation: 2,
    owner: 'runner',
    directPending: false,
    directKey: null,
    runnerContextKey: CONTEXT_KEY,
    runnerHingeEdgeId: HINGE_EDGE_ID,
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: null,
    committedRequestToken: null,
  })
  assert.deepEqual(prepared.commands, [
    { kind: 'dispose_runner' },
    {
      kind: 'prepare_runner',
      ownerToken: prepared.state.ownerToken,
      generation: 2,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
    },
  ])

  const staleDirect = step(prepared.state, {
    kind: 'direct_callback',
    ownerToken: prepared.state.ownerToken,
    generation: 1,
    key: 'direct-a',
  })
  assertRejected(staleDirect, prepared.state, 'stale_generation')

  const stalePrepare = step(prepared.state, {
    kind: 'prepare_runner',
    ownerToken: prepared.state.ownerToken,
    generation: 1,
    contextKey: 'context-b',
    hingeEdgeId: 'hinge-b',
  })
  assertRejected(stalePrepare, prepared.state, 'stale_generation')
})

test('runner requests, applies, and callbacks require the exact owner token', () => {
  const prepared = preparedRunner()

  assertRejected(step(prepared, {
    kind: 'request_runner',
    ownerToken: prepared.ownerToken,
    generation: 0,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    targetSelectedAngleDegrees: 90,
  }), prepared, 'stale_generation')
  assertRejected(step(prepared, {
    kind: 'request_runner',
    ownerToken: prepared.ownerToken,
    generation: prepared.generation,
    contextKey: 'other-context',
    hingeEdgeId: HINGE_EDGE_ID,
    targetSelectedAngleDegrees: 90,
  }), prepared, 'context_mismatch')
  assertRejected(step(prepared, {
    kind: 'request_runner',
    ownerToken: prepared.ownerToken,
    generation: prepared.generation,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: 'other-hinge',
    targetSelectedAngleDegrees: 90,
  }), prepared, 'hinge_mismatch')

  const first = requestRunner(prepared, 90)
  assert.equal(first.state.generation, 2)
  assert.equal(first.state.activeRequestSequence, 1)
  assert.ok(first.state.activeRequestToken)
  assert.equal(first.state.activeTargetSelectedAngleDegrees, 90)
  assert.deepEqual(first.commands, [{
    kind: 'request_runner',
    ownerToken: first.state.ownerToken,
    requestToken: first.state.activeRequestToken,
    generation: 2,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    requestSequence: 1,
    targetSelectedAngleDegrees: 90,
  }])

  const applied = step(first.state, {
    kind: 'runner_apply',
    ...runnerToken(first.state, 1),
    selectedAngleDegrees: 45,
  })
  assert.deepEqual(applied.commands, [{
    kind: 'apply_runner_selected',
    ownerToken: first.state.ownerToken,
    requestToken: first.state.activeRequestToken,
    generation: 2,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    requestSequence: 1,
    selectedAngleDegrees: 45,
  }])
  assert.strictEqual(applied.state, first.state)

  const callback = step(first.state, {
    kind: 'runner_callback',
    ...runnerToken(first.state, 1),
  })
  assert.deepEqual(callback.commands, [{
    kind: 'accept_runner_callback',
    ownerToken: first.state.ownerToken,
    requestToken: first.state.activeRequestToken,
    generation: 2,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    requestSequence: 1,
  }])

  const replacement = requestRunner(first.state, 120)
  assert.equal(replacement.state.generation, 3)
  assert.equal(replacement.state.activeRequestSequence, 2)
  assert.notStrictEqual(
    replacement.state.activeRequestToken,
    first.state.activeRequestToken,
  )
  assertRejected(step(replacement.state, {
    kind: 'runner_apply',
    ...runnerToken(first.state, 1),
    selectedAngleDegrees: 90,
  }), replacement.state, 'stale_generation')
  assertRejected(step(replacement.state, {
    kind: 'runner_callback',
    ...runnerToken(replacement.state, 1),
  }), replacement.state, 'request_mismatch')
})

test('external direct changes cancel gesture and runner before scheduling', () => {
  const request = requestRunner(preparedRunner(), 90)
  const external = step(request.state, {
    kind: 'external_direct_change',
    key: 'external-pose',
  })
  assert.deepEqual(external.commands, [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    {
      kind: 'schedule_direct',
      ownerToken: external.state.ownerToken,
      generation: 3,
      key: 'external-pose',
    },
  ])
  assert.deepEqual(ownerSummary(external.state), {
    generation: 3,
    owner: 'direct',
    directPending: true,
    directKey: 'external-pose',
  })
  assert.equal(external.state.runnerContextKey, null)
  assert.equal(external.state.runnerHingeEdgeId, null)
  assert.equal(external.state.activeRequestSequence, null)

  const staleTerminal = step(external.state, {
    kind: 'runner_terminal',
    ...runnerToken(request.state, 1),
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assertRejected(staleTerminal, external.state, 'stale_generation')
  assert.equal(commitCommands(staleTerminal).length, 0)
})

test('one current terminal commits exactly once while stale, duplicate, and malformed terminals commit nothing', () => {
  const request = requestRunner(preparedRunner(), 90)
  const terminal = step(request.state, {
    kind: 'runner_terminal',
    ...runnerToken(request.state, 1),
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assert.equal(terminal.accepted, true)
  assert.equal(terminal.state.activeRequestSequence, null)
  assert.equal(terminal.state.activeRequestToken, null)
  assert.equal(terminal.state.committedRequestSequence, 1)
  assert.strictEqual(
    terminal.state.committedRequestToken,
    request.state.activeRequestToken,
  )
  assert.deepEqual(terminal.commands, [
    {
      kind: 'accept_runner_callback',
      ownerToken: request.state.ownerToken,
      requestToken: request.state.activeRequestToken,
      generation: 2,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
      requestSequence: 1,
    },
    {
      kind: 'commit_selected_applied',
      ownerToken: request.state.ownerToken,
      requestToken: request.state.activeRequestToken,
      generation: 2,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
      requestSequence: 1,
      status: 'clear',
      selectedAngleDegrees: 90,
    },
  ])
  assert.equal(commitCommands(terminal).length, 1)
  assertDeeplyFrozen(terminal)

  const duplicate = step(terminal.state, {
    kind: 'runner_terminal',
    ...runnerToken(request.state, 1),
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assertRejected(duplicate, terminal.state, 'request_token_mismatch')
  assert.equal(commitCommands(duplicate).length, 0)

  const nextRequest = requestRunner(terminal.state, 130)
  const stale = step(nextRequest.state, {
    kind: 'runner_terminal',
    ...runnerToken(request.state, 1),
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assertRejected(stale, nextRequest.state, 'stale_generation')
  assert.equal(commitCommands(stale).length, 0)

  const malformedClear = step(nextRequest.state, {
    kind: 'runner_terminal',
    ...runnerToken(nextRequest.state, 2),
    status: 'clear',
    appliedSelectedAngleDegrees: 129,
  })
  assertRejected(malformedClear, nextRequest.state, 'invalid_event')
  assert.equal(commitCommands(malformedClear).length, 0)

  for (const event of [
    {
      kind: 'runner_terminal',
      ...runnerToken(nextRequest.state, 2),
      status: 'running',
      appliedSelectedAngleDegrees: 120,
    },
    {
      kind: 'runner_terminal',
      ...runnerToken(nextRequest.state, 2),
      status: 'blocked',
      appliedSelectedAngleDegrees: Number.NaN,
    },
    {
      kind: 'runner_terminal',
      ...runnerToken(nextRequest.state, 2),
      status: 'blocked',
      appliedSelectedAngleDegrees: 181,
    },
  ]) {
    const malformed = step(
      nextRequest.state,
      event as FoldPreviewTreeMotionOwnerEvent,
    )
    assert.equal(malformed.accepted, false)
    assert.equal(commitCommands(malformed).length, 0)
  }

  const blocked = step(nextRequest.state, {
    kind: 'runner_terminal',
    ...runnerToken(nextRequest.state, 2),
    status: 'blocked',
    appliedSelectedAngleDegrees: 110,
  })
  assert.equal(commitCommands(blocked).length, 1)
  assert.equal(blocked.state.committedRequestSequence, 2)
})

test('dispose invalidates active work and is permanent', () => {
  const request = requestRunner(preparedRunner(), 90)
  const disposed = step(request.state, { kind: 'dispose' })
  assert.equal(disposed.accepted, true)
  assert.deepEqual(disposed.commands, [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    { kind: 'dispose_direct' },
  ])
  assert.equal(disposed.state.owner, 'disposed')
  assert.equal(disposed.state.generation, 3)
  assert.equal(disposed.state.directPending, false)
  assert.equal(disposed.state.activeRequestSequence, null)

  const events: FoldPreviewTreeMotionOwnerEvent[] = [
    { kind: 'dispose' },
    { kind: 'schedule_direct', key: 'after-dispose' },
    {
      kind: 'prepare_runner',
      ownerToken: disposed.state.ownerToken,
      generation: disposed.state.generation,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
    },
    {
      kind: 'runner_apply',
      ...runnerToken(request.state, 1),
      selectedAngleDegrees: 50,
    },
  ]
  for (const event of events) {
    const rejected = step(disposed.state, event)
    assertRejected(rejected, disposed.state, 'disposed')
  }
})

test('generation exhaustion never wraps and permanently disposes ownership', () => {
  const maximum = createFoldPreviewTreeMotionOwnerState({
    initialGeneration: Number.MAX_SAFE_INTEGER,
  })
  assert.ok(maximum)
  const exhausted = step(maximum, {
    kind: 'schedule_direct',
    key: 'cannot-schedule',
  })
  assert.equal(exhausted.accepted, false)
  assert.equal(exhausted.reason, 'counter_exhausted')
  assert.equal(exhausted.state.owner, 'disposed')
  assert.equal(exhausted.state.generation, Number.MAX_SAFE_INTEGER)
  assert.deepEqual(exhausted.commands, [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    { kind: 'dispose_direct' },
  ])
  assert.equal(
    exhausted.commands.some((command) => command.kind === 'schedule_direct'),
    false,
  )

  const penultimate = createFoldPreviewTreeMotionOwnerState({
    initialGeneration: Number.MAX_SAFE_INTEGER - 1,
  })
  assert.ok(penultimate)
  const prepared = step(penultimate, {
    kind: 'prepare_runner',
    ownerToken: penultimate.ownerToken,
    generation: Number.MAX_SAFE_INTEGER - 1,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
  })
  assert.equal(prepared.state.generation, Number.MAX_SAFE_INTEGER)
  const requestExhausted = step(prepared.state, {
    kind: 'request_runner',
    ownerToken: prepared.state.ownerToken,
    generation: Number.MAX_SAFE_INTEGER,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    targetSelectedAngleDegrees: 90,
  })
  assert.equal(requestExhausted.reason, 'counter_exhausted')
  assert.equal(requestExhausted.state.owner, 'disposed')
})

test('opaque identities reject callbacks from a recreated owner with identical scalar tokens', () => {
  const first = requestRunner(preparedRunner(), 90)
  const second = requestRunner(preparedRunner(), 90)
  assert.equal(first.state.generation, second.state.generation)
  assert.equal(
    first.state.activeRequestSequence,
    second.state.activeRequestSequence,
  )
  assert.notStrictEqual(first.state.ownerToken, second.state.ownerToken)
  assert.notStrictEqual(
    first.state.activeRequestToken,
    second.state.activeRequestToken,
  )

  const oldOwnerTerminal = step(second.state, {
    kind: 'runner_terminal',
    ...runnerToken(first.state, 1),
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assertRejected(
    oldOwnerTerminal,
    second.state,
    'owner_token_mismatch',
  )
  assert.equal(commitCommands(oldOwnerTerminal).length, 0)

  assert.ok(first.state.activeRequestToken)
  const oldRequestTerminal = step(second.state, {
    kind: 'runner_terminal',
    ownerToken: second.state.ownerToken,
    requestToken: first.state.activeRequestToken,
    generation: second.state.generation,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    requestSequence: 1,
    status: 'clear',
    appliedSelectedAngleDegrees: 90,
  })
  assertRejected(
    oldRequestTerminal,
    second.state,
    'request_token_mismatch',
  )
  assert.equal(commitCommands(oldRequestTerminal).length, 0)

  const firstDirect = step(initialState(), {
    kind: 'schedule_direct',
    key: 'same-direct',
  })
  const secondDirect = step(initialState(), {
    kind: 'schedule_direct',
    key: 'same-direct',
  })
  assert.equal(
    firstDirect.state.generation,
    secondDirect.state.generation,
  )
  assert.notStrictEqual(
    firstDirect.state.ownerToken,
    secondDirect.state.ownerToken,
  )
  const oldDirectCallback = step(secondDirect.state, {
    kind: 'direct_callback',
    ownerToken: firstDirect.state.ownerToken,
    generation: secondDirect.state.generation,
    key: 'same-direct',
  })
  assertRejected(
    oldDirectCallback,
    secondDirect.state,
    'owner_token_mismatch',
  )
})

test('malformed and throwing Proxy inputs fail closed without effects', () => {
  const state = initialState()
  assert.doesNotThrow(() => createFoldPreviewTreeMotionOwnerState(
    throwingProxy(),
  ))
  assert.equal(createFoldPreviewTreeMotionOwnerState(throwingProxy()), null)

  const throwingEvent = step(
    state,
    throwingProxy<FoldPreviewTreeMotionOwnerEvent>(),
  )
  assertRejected(throwingEvent, state, 'invalid_event')
  assert.equal(
    transitionFoldPreviewTreeMotionOwner(
      throwingProxy<FoldPreviewTreeMotionOwnerState>(),
      { kind: 'dispose' },
    ),
    null,
  )

  const invalidEvents = [
    { kind: 'schedule_direct', key: ' ' },
    {
      kind: 'prepare_runner',
      generation: Number.NaN,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
    },
    {
      kind: 'prepare_runner',
      generation: 0,
      contextKey: '',
      hingeEdgeId: HINGE_EDGE_ID,
    },
    {
      kind: 'request_runner',
      generation: 0,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: HINGE_EDGE_ID,
      targetSelectedAngleDegrees: Number.POSITIVE_INFINITY,
    },
    { kind: 'unknown' },
  ]
  for (const event of invalidEvents) {
    const rejected = step(
      state,
      event as FoldPreviewTreeMotionOwnerEvent,
    )
    assertRejected(rejected, state, 'invalid_event')
    assertDeeplyFrozen(rejected)
  }
})

test('every event snapshots each property exactly once before trusting it', () => {
  let optionReads = 0
  const options = Object.defineProperty({}, 'initialGeneration', {
    get() {
      optionReads += 1
      return optionReads === 1 ? 0 : Number.NaN
    },
  })
  const created = createFoldPreviewTreeMotionOwnerState(options)
  assert.ok(created)
  assert.equal(created.generation, 0)
  assert.equal(optionReads, 1)
  const hostileOwnerToken = initialState().ownerToken

  const scheduledEvent = statefulEvent({
    kind: ['schedule_direct', 'dispose'],
    key: ['trusted-direct', ''],
  })
  const scheduled = step(created, scheduledEvent.event)
  scheduledEvent.assertReadOnce()
  assert.equal(scheduled.state.directKey, 'trusted-direct')
  assert.deepEqual(scheduled.commands.at(-1), {
    kind: 'schedule_direct',
    ownerToken: created.ownerToken,
    generation: 1,
    key: 'trusted-direct',
  })

  const directEvent = statefulEvent({
    kind: ['direct_callback', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    generation: [1, Number.NaN],
    key: ['trusted-direct', ''],
  })
  const direct = step(scheduled.state, directEvent.event)
  directEvent.assertReadOnce()
  assert.deepEqual(direct.commands, [{
    kind: 'apply_direct',
    ownerToken: created.ownerToken,
    generation: 1,
    key: 'trusted-direct',
  }])

  const prepareEvent = statefulEvent({
    kind: ['prepare_runner', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    generation: [1, Number.NaN],
    contextKey: ['trusted-context', ''],
    hingeEdgeId: ['trusted-hinge', ''],
  })
  const prepared = step(direct.state, prepareEvent.event)
  prepareEvent.assertReadOnce()
  assert.equal(prepared.state.runnerContextKey, 'trusted-context')
  assert.equal(prepared.state.runnerHingeEdgeId, 'trusted-hinge')
  assert.deepEqual(prepared.commands.at(-1), {
    kind: 'prepare_runner',
    ownerToken: created.ownerToken,
    generation: 2,
    contextKey: 'trusted-context',
    hingeEdgeId: 'trusted-hinge',
  })

  const requestEvent = statefulEvent({
    kind: ['request_runner', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    generation: [2, Number.NaN],
    contextKey: ['trusted-context', ''],
    hingeEdgeId: ['trusted-hinge', ''],
    targetSelectedAngleDegrees: [30, Number.NaN],
  })
  const requested = step(prepared.state, requestEvent.event)
  requestEvent.assertReadOnce()
  assert.equal(requested.state.activeTargetSelectedAngleDegrees, 30)
  assert.ok(requested.state.activeRequestToken)
  assert.deepEqual(requested.commands, [{
    kind: 'request_runner',
    ownerToken: created.ownerToken,
    requestToken: requested.state.activeRequestToken,
    generation: 3,
    contextKey: 'trusted-context',
    hingeEdgeId: 'trusted-hinge',
    requestSequence: 1,
    targetSelectedAngleDegrees: 30,
  }])
  const hostileRequest = requestRunner(preparedRunner(), 30)
  assert.ok(hostileRequest.state.activeRequestToken)
  const hostileRequestToken = hostileRequest.state.activeRequestToken

  const applyEvent = statefulEvent({
    kind: ['runner_apply', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    requestToken: [
      requested.state.activeRequestToken,
      hostileRequestToken,
    ],
    generation: [3, Number.NaN],
    contextKey: ['trusted-context', ''],
    hingeEdgeId: ['trusted-hinge', ''],
    requestSequence: [1, Number.NaN],
    selectedAngleDegrees: [20, Number.NaN],
  })
  const applied = step(requested.state, applyEvent.event)
  applyEvent.assertReadOnce()
  assert.deepEqual(applied.commands, [{
    kind: 'apply_runner_selected',
    ownerToken: created.ownerToken,
    requestToken: requested.state.activeRequestToken,
    generation: 3,
    contextKey: 'trusted-context',
    hingeEdgeId: 'trusted-hinge',
    requestSequence: 1,
    selectedAngleDegrees: 20,
  }])

  const callbackEvent = statefulEvent({
    kind: ['runner_callback', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    requestToken: [
      requested.state.activeRequestToken,
      hostileRequestToken,
    ],
    generation: [3, Number.NaN],
    contextKey: ['trusted-context', ''],
    hingeEdgeId: ['trusted-hinge', ''],
    requestSequence: [1, Number.NaN],
  })
  const callback = step(requested.state, callbackEvent.event)
  callbackEvent.assertReadOnce()
  assert.deepEqual(callback.commands, [{
    kind: 'accept_runner_callback',
    ownerToken: created.ownerToken,
    requestToken: requested.state.activeRequestToken,
    generation: 3,
    contextKey: 'trusted-context',
    hingeEdgeId: 'trusted-hinge',
    requestSequence: 1,
  }])

  const terminalEvent = statefulEvent({
    kind: ['runner_terminal', 'dispose'],
    ownerToken: [created.ownerToken, hostileOwnerToken],
    requestToken: [
      requested.state.activeRequestToken,
      hostileRequestToken,
    ],
    generation: [3, Number.NaN],
    contextKey: ['trusted-context', ''],
    hingeEdgeId: ['trusted-hinge', ''],
    requestSequence: [1, Number.NaN],
    status: ['blocked', 'running'],
    appliedSelectedAngleDegrees: [25, Number.NaN],
  })
  const terminal = step(requested.state, terminalEvent.event)
  terminalEvent.assertReadOnce()
  assert.deepEqual(commitCommands(terminal), [{
    kind: 'commit_selected_applied',
    ownerToken: created.ownerToken,
    requestToken: requested.state.activeRequestToken,
    generation: 3,
    contextKey: 'trusted-context',
    hingeEdgeId: 'trusted-hinge',
    requestSequence: 1,
    status: 'blocked',
    selectedAngleDegrees: 25,
  }])

  const externalEvent = statefulEvent({
    kind: ['external_direct_change', 'dispose'],
    key: ['trusted-external', ''],
  })
  const external = step(terminal.state, externalEvent.event)
  externalEvent.assertReadOnce()
  assert.equal(external.state.directKey, 'trusted-external')
  assert.deepEqual(external.commands.at(-1), {
    kind: 'schedule_direct',
    ownerToken: created.ownerToken,
    generation: 4,
    key: 'trusted-external',
  })

  const disposeEvent = statefulEvent({
    kind: ['dispose', 'schedule_direct'],
  })
  const disposed = step(external.state, disposeEvent.event)
  disposeEvent.assertReadOnce()
  assert.equal(disposed.state.owner, 'disposed')
  assert.equal(disposed.commands.some(
    (command) => command.kind === 'schedule_direct',
  ), false)
  assertDeeplyFrozen(disposed)
})

test('direct/context keys and hinge IDs enforce bounded lengths', () => {
  assert.equal(
    MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_KEY_LENGTH,
    MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
  )
  const maximumKey = 'k'.repeat(
    MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_KEY_LENGTH,
  )
  const maximumId = 'h'.repeat(
    MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_ID_LENGTH,
  )
  const maximumDirect = step(initialState(), {
    kind: 'schedule_direct',
    key: maximumKey,
  })
  assert.equal(maximumDirect.accepted, true)
  assert.equal(maximumDirect.state.directKey?.length, maximumKey.length)

  const overlongInitial = initialState()
  const overlongDirect = step(overlongInitial, {
    kind: 'schedule_direct',
    key: `${maximumKey}x`,
  })
  assertRejected(overlongDirect, overlongInitial, 'invalid_event')

  const aboveLegacyOwnerLimit = step(initialState(), {
    kind: 'schedule_direct',
    key: 'k'.repeat(262_145),
  })
  assert.equal(aboveLegacyOwnerLimit.accepted, true)

  const maximumRunnerInitial = initialState()
  const maximumRunner = step(maximumRunnerInitial, {
    kind: 'prepare_runner',
    ownerToken: maximumRunnerInitial.ownerToken,
    generation: 0,
    contextKey: maximumKey,
    hingeEdgeId: maximumId,
  })
  assert.equal(maximumRunner.accepted, true)
  assert.equal(
    maximumRunner.state.runnerContextKey?.length,
    maximumKey.length,
  )
  assert.equal(
    maximumRunner.state.runnerHingeEdgeId?.length,
    maximumId.length,
  )

  for (const createEvent of [
    (ownerToken: FoldPreviewTreeMotionOwnerState['ownerToken']) => ({
      kind: 'prepare_runner',
      ownerToken,
      generation: 0,
      contextKey: `${maximumKey}x`,
      hingeEdgeId: HINGE_EDGE_ID,
    }),
    (ownerToken: FoldPreviewTreeMotionOwnerState['ownerToken']) => ({
      kind: 'prepare_runner',
      ownerToken,
      generation: 0,
      contextKey: CONTEXT_KEY,
      hingeEdgeId: `${maximumId}x`,
    }),
  ]) {
    const initial = initialState()
    const rejected = step(
      initial,
      createEvent(initial.ownerToken) as FoldPreviewTreeMotionOwnerEvent,
    )
    assertRejected(rejected, initial, 'invalid_event')
  }
})

function initialState() {
  const state = createFoldPreviewTreeMotionOwnerState()
  assert.ok(state)
  return state
}

function preparedRunner() {
  const initial = initialState()
  const prepared = step(initial, {
    kind: 'prepare_runner',
    ownerToken: initial.ownerToken,
    generation: initial.generation,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
  })
  assert.equal(prepared.accepted, true)
  return prepared.state
}

function requestRunner(
  state: FoldPreviewTreeMotionOwnerState,
  targetSelectedAngleDegrees: number,
) {
  const requested = step(state, {
    kind: 'request_runner',
    ownerToken: state.ownerToken,
    generation: state.generation,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    targetSelectedAngleDegrees,
  })
  assert.equal(requested.accepted, true)
  return requested
}

function runnerToken(
  state: FoldPreviewTreeMotionOwnerState,
  requestSequence: number,
) {
  assert.ok(state.activeRequestToken)
  return {
    ownerToken: state.ownerToken,
    requestToken: state.activeRequestToken,
    generation: state.generation,
    contextKey: CONTEXT_KEY,
    hingeEdgeId: HINGE_EDGE_ID,
    requestSequence,
  }
}

function step(
  state: FoldPreviewTreeMotionOwnerState,
  event: FoldPreviewTreeMotionOwnerEvent,
) {
  const result = transitionFoldPreviewTreeMotionOwner(state, event)
  assert.ok(result)
  return result
}

function assertRejected(
  plan: FoldPreviewTreeMotionOwnerPlan,
  state: FoldPreviewTreeMotionOwnerState,
  reason: FoldPreviewTreeMotionOwnerPlan['reason'],
) {
  assert.equal(plan.accepted, false)
  assert.equal(plan.reason, reason)
  assert.strictEqual(plan.state, state)
  assert.deepEqual(plan.commands, [])
}

function ownerSummary(state: FoldPreviewTreeMotionOwnerState) {
  return {
    generation: state.generation,
    owner: state.owner,
    directPending: state.directPending,
    directKey: state.directKey,
  }
}

function commitCommands(plan: FoldPreviewTreeMotionOwnerPlan) {
  return plan.commands.filter(
    (command) => command.kind === 'commit_selected_applied',
  )
}

function assertDeeplyFrozen(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const key of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[key],
    )
  }
}

function throwingProxy<T>(): T {
  return new Proxy({}, {
    get() {
      throw new Error('unexpected access')
    },
  }) as T
}

function statefulEvent(
  fields: Readonly<Record<string, readonly [unknown, unknown]>>,
) {
  const reads = new Map<string, number>()
  const event: Record<string, unknown> = {}
  for (const [key, values] of Object.entries(fields)) {
    Object.defineProperty(event, key, {
      enumerable: true,
      get() {
        const count = (reads.get(key) ?? 0) + 1
        reads.set(key, count)
        return count === 1 ? values[0] : values[1]
      },
    })
  }
  return {
    event: event as FoldPreviewTreeMotionOwnerEvent,
    assertReadOnce() {
      assert.deepEqual(
        Object.keys(fields).map((key) => [key, reads.get(key)]),
        Object.keys(fields).map((key) => [key, 1]),
      )
    },
  }
}
