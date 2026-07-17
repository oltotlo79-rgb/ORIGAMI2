import assert from 'node:assert/strict'
import test from 'node:test'

import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'
import type { FoldPreviewHingeAngle } from '../src/lib/foldPreviewKinematics.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  type FoldPreviewTreeMotionContext,
} from '../src/lib/foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeMotionOwnerState,
  transitionFoldPreviewTreeMotionOwner,
} from '../src/lib/foldPreviewTreeMotionOwner.ts'
import {
  completeFoldPreviewTreeMotionRuntimePoseApplication,
  createFoldPreviewTreeMotionRuntime,
  FOLD_PREVIEW_TREE_MOTION_RUNTIME_VERSION,
  transitionFoldPreviewTreeMotionRuntime,
  type FoldPreviewTreeMotionRuntimeCommand,
  type FoldPreviewTreeMotionRuntimeEvent,
  type FoldPreviewTreeMotionRuntimePlan,
  type FoldPreviewTreeMotionRuntimeRunnerToken,
  type FoldPreviewTreeMotionRuntimeState,
} from '../src/lib/foldPreviewTreeMotionRuntime.ts'

const BASE_ANGLES: readonly FoldPreviewHingeAngle[] = [
  { edgeId: 'hinge-z', angleDegrees: 55 },
  { edgeId: 'hinge-x', angleDegrees: 35 },
]

test('creates a frozen complete-vector runtime only from authentic context and owner state', () => {
  const fixture = preparedFixture()
  assert.deepEqual(fixture.runtime, {
    version: FOLD_PREVIEW_TREE_MOTION_RUNTIME_VERSION,
    generation: 1,
    contextKey: fixture.context.contextKey,
    hingeEdgeId: 'hinge-z',
    appliedAngles: [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 55 },
    ],
    activeRunnerToken: null,
    activeRequestSequence: null,
    activeTargetSelectedAngleDegrees: null,
    pendingApplicationToken: null,
    latestRequestSequence: 0,
    committedRequestSequence: null,
    disposed: false,
  })
  assertDeeplyFrozen(fixture.runtime)
  assert.notEqual(
    fixture.runtime.appliedAngles,
    fixture.context.appliedAngles,
  )

  assert.equal(createFoldPreviewTreeMotionRuntime({
    context: { ...fixture.context },
    ownerState: fixture.ownerState,
  }), null)
  assert.equal(createFoldPreviewTreeMotionRuntime({
    context: fixture.context,
    ownerState: { ...fixture.ownerState },
  }), null)
  assert.equal(createFoldPreviewTreeMotionRuntime({
    context: fixture.context,
    ownerState: throwingProxy(),
  }), null)

  const active = activeFixture(100)
  const completed = runtimeStep(active.runtime, {
    kind: 'runner_state',
    runnerToken: active.runnerToken,
    runnerState: runnerState('blocked', 100, 55),
  })
  assert.equal(createFoldPreviewTreeMotionRuntime({
    context: active.context,
    ownerState: completed.ownerState,
  }), null)
})

test('requests through the retained owner and exposes no owner/request token', () => {
  const fixture = preparedFixture()
  const requested = runtimeStep(fixture.runtime, {
    kind: 'request',
    targetSelectedAngleDegrees: 120,
  })
  const command = onlyRuntimeCommand(requested, 'start_runner')

  assert.equal(requested.accepted, true)
  assert.equal(requested.reason, null)
  assert.equal(requested.state.generation, 2)
  assert.equal(requested.state.activeRequestSequence, 1)
  assert.equal(requested.state.activeTargetSelectedAngleDegrees, 120)
  assert.strictEqual(
    requested.state.activeRunnerToken,
    command.runnerToken,
  )
  assert.deepEqual(command, {
    kind: 'start_runner',
    generation: 2,
    contextKey: fixture.context.contextKey,
    hingeEdgeId: 'hinge-z',
    runnerToken: command.runnerToken,
    requestSequence: 1,
    targetSelectedAngleDegrees: 120,
  })
  assert.equal('ownerToken' in command, false)
  assert.equal('requestToken' in command, false)
  assert.equal(requested.ownerState.activeRequestSequence, 1)
  assert.ok(requested.ownerState.activeRequestToken)
  assertDeeplyFrozen(requested)
})

test('stages a complete selected-angle pose and promotes it only after exact scene success', () => {
  const requested = activeFixture(120)
  const staged = runtimeStep(requested.runtime, {
    kind: 'runner_apply',
    runnerToken: requested.runnerToken,
    selectedAngleDegrees: 90,
  })
  const command = onlyRuntimeCommand(staged, 'apply_complete_pose')

  assert.deepEqual(staged.state.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 55 },
  ])
  assert.strictEqual(
    staged.state.pendingApplicationToken,
    command.applicationToken,
  )
  assert.deepEqual(command.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 90 },
  ])

  const confirmed = completeApplication(staged.state, command, true)
  assert.deepEqual(confirmed.state.appliedAngles, command.appliedAngles)
  assert.equal(confirmed.state.pendingApplicationToken, null)
  assert.notEqual(confirmed.state.appliedAngles, command.appliedAngles)
  assert.deepEqual(confirmed.commands, [])

  const duplicate = completeFoldPreviewTreeMotionRuntimePoseApplication(
    confirmed.state,
    command.applicationToken,
    true,
  )
  assert.equal(duplicate?.accepted, false)
  assert.equal(duplicate?.reason, 'application_not_pending')
})

test('a rejected scene application disposes and can never produce a terminal commit', () => {
  const requested = activeFixture(170)
  const staged = runtimeStep(requested.runtime, {
    kind: 'runner_apply',
    runnerToken: requested.runnerToken,
    selectedAngleDegrees: 82,
  })
  const command = onlyRuntimeCommand(staged, 'apply_complete_pose')
  const rejected = completeApplication(staged.state, command, false)
  assert.deepEqual(rejected.state.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 55 },
  ])
  assert.equal(rejected.state.disposed, true)
  assert.equal(rejected.state.activeRunnerToken, null)
  assert.equal(rejected.ownerState.owner, 'disposed')
  assert.deepEqual(rejected.commands, [{ kind: 'dispose_runner' }])

  const terminal = runtimeStep(rejected.state, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: runnerState('indeterminate', 170, 55),
  })
  assert.equal(terminal.accepted, false)
  assert.equal(terminal.reason, 'disposed')
  assert.deepEqual(terminal.commands, [])
})

test('clear commits the confirmed target vector exactly once', () => {
  const requested = activeFixture(120)
  const applied = applyAndConfirm(
    requested.runtime,
    requested.runnerToken,
    120,
  )
  const running = runtimeStep(applied, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: runnerState('running', 120, 120),
  })
  assert.deepEqual(running.commands, [])

  const terminal = runtimeStep(running.state, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: runnerState('clear', 120, 120),
  })
  const commit = onlyRuntimeCommand(
    terminal,
    'commit_complete_applied',
  )
  assert.equal(commit.status, 'clear')
  assert.equal(commit.selectedAngleDegrees, 120)
  assert.equal(terminal.state.activeRunnerToken, null)
  assert.equal(terminal.state.activeRequestSequence, null)
  assert.equal(terminal.state.committedRequestSequence, 1)

  const duplicate = runtimeStep(terminal.state, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: runnerState('clear', 120, 120),
  })
  assert.equal(duplicate.accepted, false)
  assert.equal(duplicate.reason, 'runner_token_mismatch')
  assert.deepEqual(duplicate.commands, [])
})

for (const status of ['blocked', 'indeterminate'] as const) {
  test(`${status} commits only the last confirmed safe angle`, () => {
    const requested = activeFixture(170)
    const applied = applyAndConfirm(
      requested.runtime,
      requested.runnerToken,
      82,
    )
    const terminal = runtimeStep(applied, {
      kind: 'runner_state',
      runnerToken: requested.runnerToken,
      runnerState: runnerState(status, 170, 82),
    })
    const commit = onlyRuntimeCommand(
      terminal,
      'commit_complete_applied',
    )
    assert.equal(commit.status, status)
    assert.equal(commit.selectedAngleDegrees, 82)
    assert.deepEqual(commit.appliedAngles, [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 82 },
    ])
  })
}

test('replacement gives the runner a new opaque token and rejects all stale callbacks', () => {
  const first = activeFixture(100)
  const second = runtimeStep(first.runtime, {
    kind: 'request',
    targetSelectedAngleDegrees: 130,
  })
  const secondStart = onlyRuntimeCommand(second, 'start_runner')
  assert.notStrictEqual(secondStart.runnerToken, first.runnerToken)
  assert.equal(second.state.latestRequestSequence, 2)

  for (const event of [
    {
      kind: 'runner_apply',
      runnerToken: first.runnerToken,
      selectedAngleDegrees: 70,
    },
    {
      kind: 'runner_state',
      runnerToken: first.runnerToken,
      runnerState: runnerState('blocked', 100, 55),
    },
  ] satisfies FoldPreviewTreeMotionRuntimeEvent[]) {
    const stale = runtimeStep(second.state, event)
    assert.equal(stale.accepted, false)
    assert.equal(stale.reason, 'runner_token_mismatch')
    assert.deepEqual(stale.commands, [])
  }
  assert.deepEqual(second.state.appliedAngles, [
    { edgeId: 'hinge-x', angleDegrees: 35 },
    { edgeId: 'hinge-z', angleDegrees: 55 },
  ])
})

test('runner notifications reject forged tokens, other requests, and unapplied angles', () => {
  const requested = activeFixture(100)
  const forged = Object.freeze(
    {},
  ) as FoldPreviewTreeMotionRuntimeRunnerToken
  const cases: readonly [
    FoldPreviewTreeMotionRuntimeEvent,
    FoldPreviewTreeMotionRuntimePlan['reason'],
  ][] = [
    [{
      kind: 'runner_apply',
      runnerToken: forged,
      selectedAngleDegrees: 80,
    }, 'invalid_event'],
    [{
      kind: 'runner_state',
      runnerToken: requested.runnerToken,
      runnerState: runnerState('running', 101, 55),
    }, 'runner_state_mismatch'],
    [{
      kind: 'runner_state',
      runnerToken: requested.runnerToken,
      runnerState: runnerState('running', 100, 56),
    }, 'runner_state_mismatch'],
    [{
      kind: 'runner_state',
      runnerToken: requested.runnerToken,
      runnerState: runnerState('idle', 100, 55),
    }, 'runner_state_mismatch'],
  ]
  for (const [event, reason] of cases) {
    const result = runtimeStep(requested.runtime, event)
    assert.equal(result.accepted, false)
    assert.equal(result.reason, reason)
  }
})

test('a pending scene application blocks every runner event and stale confirmation', () => {
  const requested = activeFixture(100)
  const staged = runtimeStep(requested.runtime, {
    kind: 'runner_apply',
    runnerToken: requested.runnerToken,
    selectedAngleDegrees: 75,
  })
  const command = onlyRuntimeCommand(staged, 'apply_complete_pose')
  const blocked = runtimeStep(staged.state, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: runnerState('blocked', 100, 55),
  })
  assert.equal(blocked.accepted, false)
  assert.equal(blocked.reason, 'application_pending')

  const forged = completeFoldPreviewTreeMotionRuntimePoseApplication(
    staged.state,
    Object.freeze({}) as typeof command.applicationToken,
    true,
  )
  assert.equal(forged?.accepted, false)
  assert.equal(forged?.reason, 'application_token_mismatch')

  const disposed = runtimeStep(staged.state, { kind: 'dispose' })
  assert.equal(disposed.state.disposed, true)
  assert.equal(disposed.state.pendingApplicationToken, null)
  assert.equal(
    completeFoldPreviewTreeMotionRuntimePoseApplication(
      disposed.state,
      command.applicationToken,
      true,
    )?.reason,
    'disposed',
  )
})

test('event and nested runner-state getters are each captured exactly once', () => {
  const requested = activeFixture(100)
  const runnerReads = { status: 0, requested: 0, applied: 0 }
  const eventReads = { kind: 0, runnerToken: 0, runnerState: 0 }
  const nested = {
    get status() {
      runnerReads.status += 1
      return 'running' as const
    },
    get requested() {
      runnerReads.requested += 1
      return 100
    },
    get applied() {
      runnerReads.applied += 1
      return 55
    },
  } as FoldPreviewContinuousMotionRunnerState
  const event = {
    get kind() {
      eventReads.kind += 1
      return 'runner_state' as const
    },
    get runnerToken() {
      eventReads.runnerToken += 1
      return requested.runnerToken
    },
    get runnerState() {
      eventReads.runnerState += 1
      return nested
    },
  }
  const result = runtimeStep(requested.runtime, event)
  assert.equal(result.accepted, true)
  assert.deepEqual(eventReads, {
    kind: 1,
    runnerToken: 1,
    runnerState: 1,
  })
  assert.deepEqual(runnerReads, {
    status: 1,
    requested: 1,
    applied: 1,
  })
})

test('Proxies and owner-command-shaped events fail closed without effects', () => {
  const requested = activeFixture(100)
  const malformed = transitionFoldPreviewTreeMotionRuntime(
    requested.runtime,
    throwingProxy(),
  )
  assert.equal(malformed?.accepted, false)
  assert.equal(malformed?.reason, 'invalid_event')

  const nested = runtimeStep(requested.runtime, {
    kind: 'runner_state',
    runnerToken: requested.runnerToken,
    runnerState: throwingProxy(),
  })
  assert.equal(nested.accepted, false)
  assert.equal(nested.reason, 'invalid_event')

  const ownerCommandShape = runtimeStep(requested.runtime, {
    kind: 'apply_runner_selected',
    ownerToken: requested.ownerState.ownerToken,
    requestToken: requested.ownerState.activeRequestToken,
    generation: requested.ownerState.generation,
    contextKey: requested.runtime.contextKey,
    hingeEdgeId: requested.runtime.hingeEdgeId,
    requestSequence: 1,
    selectedAngleDegrees: 90,
  } as never)
  assert.equal(ownerCommandShape.accepted, false)
  assert.equal(ownerCommandShape.reason, 'unsupported_event')
})

test('dispose is idempotent and permanently rejects new requests', () => {
  const requested = activeFixture(100)
  const disposed = runtimeStep(requested.runtime, { kind: 'dispose' })
  assert.equal(disposed.state.disposed, true)
  assert.equal(disposed.state.activeRunnerToken, null)
  assert.equal(disposed.ownerState.owner, 'disposed')
  assert.deepEqual(disposed.commands, [{ kind: 'dispose_runner' }])

  const repeated = runtimeStep(disposed.state, { kind: 'dispose' })
  assert.equal(repeated.accepted, true)
  assert.deepEqual(repeated.commands, [])
  const rejected = runtimeStep(disposed.state, {
    kind: 'request',
    targetSelectedAngleDegrees: 90,
  })
  assert.equal(rejected.accepted, false)
  assert.equal(rejected.reason, 'disposed')
})

test('owner generation exhaustion is adopted and disposes instead of wrapping', () => {
  const fixture = preparedFixture(Number.MAX_SAFE_INTEGER - 1)
  assert.equal(fixture.runtime.generation, Number.MAX_SAFE_INTEGER)
  const exhausted = runtimeStep(fixture.runtime, {
    kind: 'request',
    targetSelectedAngleDegrees: 90,
  })
  assert.equal(exhausted.accepted, false)
  assert.equal(exhausted.reason, 'counter_exhausted')
  assert.equal(exhausted.state.disposed, true)
  assert.equal(exhausted.ownerState.owner, 'disposed')
  assert.deepEqual(exhausted.commands, [{ kind: 'dispose_runner' }])
})

function activeFixture(targetSelectedAngleDegrees: number) {
  const fixture = preparedFixture()
  const requested = runtimeStep(fixture.runtime, {
    kind: 'request',
    targetSelectedAngleDegrees,
  })
  const start = onlyRuntimeCommand(requested, 'start_runner')
  return {
    ...fixture,
    ownerState: requested.ownerState,
    runtime: requested.state,
    runnerToken: start.runnerToken,
  }
}

function applyAndConfirm(
  state: FoldPreviewTreeMotionRuntimeState,
  runnerToken: FoldPreviewTreeMotionRuntimeRunnerToken,
  selectedAngleDegrees: number,
) {
  const staged = runtimeStep(state, {
    kind: 'runner_apply',
    runnerToken,
    selectedAngleDegrees,
  })
  return completeApplication(
    staged.state,
    onlyRuntimeCommand(staged, 'apply_complete_pose'),
    true,
  ).state
}

function completeApplication(
  state: FoldPreviewTreeMotionRuntimeState,
  command: Extract<
    FoldPreviewTreeMotionRuntimeCommand,
    { kind: 'apply_complete_pose' }
  >,
  applied: boolean,
) {
  const result = completeFoldPreviewTreeMotionRuntimePoseApplication(
    state,
    command.applicationToken,
    applied,
  )
  assert.ok(result)
  return result
}

function preparedFixture(initialGeneration = 0) {
  const context = preparedContext()
  const initial = createFoldPreviewTreeMotionOwnerState({
    initialGeneration,
  })
  assert.ok(initial)
  const prepared = transitionFoldPreviewTreeMotionOwner(initial, {
    kind: 'prepare_runner',
    ownerToken: initial.ownerToken,
    generation: initial.generation,
    contextKey: context.contextKey,
    hingeEdgeId: context.selectedHingeEdgeId,
  })
  assert.ok(prepared?.accepted)
  const runtime = createFoldPreviewTreeMotionRuntime({
    context,
    ownerState: prepared.state,
  })
  assert.ok(runtime)
  return {
    context,
    ownerState: prepared.state,
    runtime,
  }
}

function preparedContext(): FoldPreviewTreeMotionContext {
  const context = prepareFoldPreviewTreeMotionContext({
    model: treeModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    appliedAngles: BASE_ANGLES,
    collisionThickness: 0.1,
    visualThickness: 0.12,
  })
  assert.ok(context)
  return context
}

function runtimeStep(
  state: FoldPreviewTreeMotionRuntimeState,
  event: FoldPreviewTreeMotionRuntimeEvent,
): FoldPreviewTreeMotionRuntimePlan {
  const result = transitionFoldPreviewTreeMotionRuntime(state, event)
  assert.ok(result)
  return result
}

function onlyRuntimeCommand<
  Kind extends FoldPreviewTreeMotionRuntimeCommand['kind'],
>(
  plan: FoldPreviewTreeMotionRuntimePlan,
  kind: Kind,
): Extract<FoldPreviewTreeMotionRuntimeCommand, { kind: Kind }> {
  const commands = plan.commands.filter((command) => command.kind === kind)
  assert.equal(commands.length, 1, `runtime command: ${kind}`)
  return commands[0] as Extract<
    FoldPreviewTreeMotionRuntimeCommand,
    { kind: Kind }
  >
}

function runnerState(
  status: FoldPreviewContinuousMotionRunnerState['status'],
  requested: number,
  applied: number,
): FoldPreviewContinuousMotionRunnerState {
  return Object.freeze({
    requested,
    applied,
    start: 55,
    status,
    reason: null,
    result: null,
  })
}

function treeModel(): FoldGraphPreviewModel {
  const root = face('root', -1, 0)
  const middle = face('middle', 0, 1)
  const leaf = face('leaf', 1, 2)
  const hingeZ: FoldPreviewHingeModel = {
    edgeId: 'hinge-z',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: { vertexId: 'z-start', x: 0, z: -1 },
    end: { vertexId: 'z-end', x: 0, z: 1 },
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const hingeX: FoldPreviewHingeModel = {
    edgeId: 'hinge-x',
    leftFaceId: 'middle',
    rightFaceId: 'leaf',
    start: { vertexId: 'x-start', x: 0, z: 0 },
    end: { vertexId: 'x-end', x: 1, z: 0 },
    axis: { x: 1, z: 0 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.5, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 2, maxZ: 1 },
    faces: [root, middle, leaf],
    hinges: [hingeZ, hingeX],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'middle',
          hinge: hingeZ,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'middle',
          childFaceId: 'leaf',
          hinge: hingeX,
          childRotationSign: -1,
        },
      ],
    },
  }
}

function face(
  id: string,
  minimumX: number,
  maximumX: number,
): FoldPreviewFaceModel {
  return {
    id,
    polygon: [
      { vertexId: `${id}-a`, x: minimumX, z: -1 },
      { vertexId: `${id}-b`, x: maximumX, z: -1 },
      { vertexId: `${id}-c`, x: maximumX, z: 1 },
      { vertexId: `${id}-d`, x: minimumX, z: 1 },
    ],
  }
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
