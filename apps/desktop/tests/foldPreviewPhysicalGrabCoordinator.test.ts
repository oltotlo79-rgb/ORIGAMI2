import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  type FoldPreviewPhysicalGrabTarget,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import {
  MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_EFFECTS,
  MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_ID_LENGTH,
  MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_KEY_LENGTH,
  currentFoldPreviewPhysicalGrabGuardKey,
  planFoldPreviewPhysicalGrabTransition,
  type FoldPreviewPhysicalGrabGuardSnapshot,
  type FoldPreviewPhysicalGrabTransitionPlanInput,
} from '../src/lib/foldPreviewPhysicalGrabCoordinator.ts'
import {
  createFoldPreviewPhysicalGrabGestureState,
  type FoldPreviewPhysicalGrabGestureEffect,
  type FoldPreviewPhysicalGrabGestureState,
  type FoldPreviewPhysicalGrabGestureTransition,
} from '../src/lib/foldPreviewPhysicalGrabGesture.ts'
import {
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
} from '../src/lib/foldPreviewTreeMotionContext.ts'

test('guard keys require runner identity plus exact view and context snapshots', () => {
  const runnerState = Object.freeze({ status: 'clear', applied: 30 })
  const baseline: FoldPreviewPhysicalGrabGuardSnapshot<typeof runnerState> = {
    guardKey: 'guard-1',
    startedRunnerState: runnerState,
    currentRunnerState: runnerState,
    startedViewKey: 'view-1',
    currentViewKey: 'view-1',
    activeContextKey: 'context-1',
    renderedContextKey: 'context-1',
    latestContextKey: 'context-1',
  }
  assert.equal(
    currentFoldPreviewPhysicalGrabGuardKey(baseline),
    'guard-1',
  )

  const staleSnapshots = [
    {
      ...baseline,
      currentRunnerState: Object.freeze({ status: 'clear', applied: 30 }),
    },
    { ...baseline, currentViewKey: 'view-2' },
    { ...baseline, renderedContextKey: 'context-2' },
    { ...baseline, latestContextKey: 'context-2' },
    { ...baseline, activeContextKey: null },
    { ...baseline, guardKey: null },
    { ...baseline, startedRunnerState: null },
    {
      ...baseline,
      startedRunnerState: undefined as never,
      currentRunnerState: undefined as never,
    },
    {
      ...baseline,
      startedRunnerState: 1 as never,
      currentRunnerState: 1 as never,
    },
    { ...baseline, startedViewKey: null },
    { ...baseline, currentViewKey: null },
  ]
  for (const snapshot of staleSnapshots) {
    assert.equal(
      currentFoldPreviewPhysicalGrabGuardKey(snapshot),
      null,
    )
  }
})

test('guard snapshots every identity once and contains throwing Proxies', () => {
  const runnerState = Object.freeze({ status: 'clear', applied: 30 })
  const reads = new Map<string, number>()
  const once = <Value>(key: string, value: Value, later: Value) => ({
    enumerable: true,
    get() {
      const count = (reads.get(key) ?? 0) + 1
      reads.set(key, count)
      return count === 1 ? value : later
    },
  })
  const snapshot = Object.defineProperties({}, {
    guardKey: once('guardKey', 'guard-1', null),
    startedRunnerState: once('startedRunnerState', runnerState, null),
    currentRunnerState: once('currentRunnerState', runnerState, null),
    startedViewKey: once('startedViewKey', 'view-1', null),
    currentViewKey: once('currentViewKey', 'view-1', null),
    activeContextKey: once('activeContextKey', 'context-1', null),
    renderedContextKey: once('renderedContextKey', 'context-1', null),
    latestContextKey: once('latestContextKey', 'context-1', null),
  }) as FoldPreviewPhysicalGrabGuardSnapshot<typeof runnerState>

  assert.equal(currentFoldPreviewPhysicalGrabGuardKey(snapshot), 'guard-1')
  assert.deepEqual(
    [...reads.entries()],
    [
      ['guardKey', 1],
      ['startedRunnerState', 1],
      ['currentRunnerState', 1],
      ['startedViewKey', 1],
      ['currentViewKey', 1],
      ['activeContextKey', 1],
      ['renderedContextKey', 1],
      ['latestContextKey', 1],
    ],
  )
  assert.equal(
    currentFoldPreviewPhysicalGrabGuardKey(
      throwingProxy<FoldPreviewPhysicalGrabGuardSnapshot<unknown>>(),
    ),
    null,
  )
  assert.equal(currentFoldPreviewPhysicalGrabGuardKey({
    ...snapshot,
    guardKey: 1 as never,
  }), null)
})

test('drag presentations use only the queued frame and select a new hinge', () => {
  const accepted = plan(transition(
    activeState('dragging'),
    [
      handled(1),
      {
        kind: 'presentation',
        pointerId: 1,
        target: target(),
        rejectionReason: null,
      },
    ],
  ))
  assert.equal(accepted.handled, true)
  assert.deepEqual(accepted.commands, [
    { kind: 'queue_presentation' },
    { kind: 'select_hinge', hingeId: 'hinge-1' },
    { kind: 'restore_camera' },
  ])
  assert.equal(
    accepted.commands.some(
      (command) => command.kind === 'sync_presentation',
    ),
    false,
  )

  const rejected = plan(transition(
    activeState('dragging'),
    [{
      kind: 'presentation',
      pointerId: 1,
      target: null,
      rejectionReason: 'target_too_far',
    }],
  ))
  assert.deepEqual(rejected.commands, [
    { kind: 'queue_presentation' },
    { kind: 'restore_camera' },
  ])

  const alreadySelected = plan(
    transition(
      activeState('dragging'),
      [{
        kind: 'presentation',
        pointerId: 1,
        target: target(),
        rejectionReason: null,
      }],
    ),
    { selectedHingeId: 'hinge-1' },
  )
  assert.equal(
    alreadySelected.commands.some(
      (command) => command.kind === 'select_hinge',
    ),
    false,
  )
})

test('a valid completion cleans up before exactly one final angle request', () => {
  const completed = plan(transition(
    createFoldPreviewPhysicalGrabGestureState(),
    [
      handled(1),
      {
        kind: 'end',
        pointerId: 1,
        outcome: 'drag',
        completionTarget: target({ angleDegrees: 52 }),
        rejectionReason: null,
      },
    ],
  ))
  assert.equal(completed.handled, true)
  assert.equal(completed.endedAsTap, false)
  assert.deepEqual(completed.commands, [
    { kind: 'discard_presentation' },
    { kind: 'release_capture', pointerId: 1 },
    { kind: 'clear_interaction', clearEventSession: true },
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
    {
      kind: 'request_fold_angle',
      hingeEdgeId: 'hinge-1',
      contextKey: 'context-1',
      angleDegrees: 52,
    },
  ])
  assert.equal(
    completed.commands.filter(
      (command) => command.kind === 'request_fold_angle',
    ).length,
    1,
  )
  assert.equal(completed.commands.at(-1)?.kind, 'request_fold_angle')
})

test('stale or mismatched completions clean up without requesting an angle', () => {
  const cases: ReadonlyArray<Readonly<{
    input?: Partial<FoldPreviewPhysicalGrabTransitionPlanInput>
    completionTarget?: FoldPreviewPhysicalGrabTarget
  }>> = [
    { input: { guardIsCurrent: false } },
    { input: { disposed: true } },
    { input: { activeContextKey: 'stale-context' } },
    { input: { activeHingeId: 'other-hinge' } },
    { input: { modelHingeId: 'other-hinge' } },
    { input: { modelHingeId: null } },
    { input: { guardIsCurrent: 'yes' as never } },
    { input: { disposed: 0 as never } },
    { completionTarget: target({ kind: 'forged_target' }) },
    { completionTarget: target({ mapping: 'physical_grab_v1' }) },
    { completionTarget: target({ contextKey: 'other-context' }) },
    { completionTarget: target({ angleDegrees: Number.NaN }) },
    { completionTarget: target({ angleDegrees: -0.001 }) },
    { completionTarget: target({ angleDegrees: 180.001 }) },
  ]

  for (const current of cases) {
    const completed = plan(
      transition(
        createFoldPreviewPhysicalGrabGestureState(),
        [{
          kind: 'end',
          pointerId: 1,
          outcome: 'drag',
          completionTarget: current.completionTarget ?? target(),
          rejectionReason: null,
        }],
      ),
      current.input,
    )
    assert.equal(
      completed.commands.some(
        (command) => command.kind === 'request_fold_angle',
      ),
      false,
    )
    assert.deepEqual(completed.commands.slice(0, 3), [
      { kind: 'discard_presentation' },
      { kind: 'release_capture', pointerId: 1 },
      { kind: 'clear_interaction', clearEventSession: true },
    ])
  }
})

test('completion identity strings must be nonblank and bounded', () => {
  assert.equal(
    MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_KEY_LENGTH,
    MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
  )
  const overlongId = 'h'.repeat(
    MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_ID_LENGTH + 1,
  )
  const overlongKey = 'c'.repeat(
    MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_KEY_LENGTH + 1,
  )
  const cases: ReadonlyArray<Readonly<{
    input?: Partial<FoldPreviewPhysicalGrabTransitionPlanInput>
    completionTarget?: FoldPreviewPhysicalGrabTarget
  }>> = [
    { input: { activeHingeId: '   ', modelHingeId: '   ' } },
    { input: { activeHingeId: overlongId, modelHingeId: overlongId } },
    { input: { activeContextKey: '   ' }, completionTarget: target({ contextKey: '   ' }) },
    {
      input: { activeContextKey: overlongKey },
      completionTarget: target({ contextKey: overlongKey }),
    },
  ]

  for (const current of cases) {
    const completed = plan(
      transition(
        createFoldPreviewPhysicalGrabGestureState(),
        [{
          kind: 'end',
          pointerId: 1,
          outcome: 'drag',
          completionTarget: current.completionTarget ?? target(),
          rejectionReason: null,
        }],
      ),
      current.input,
    )
    assert.equal(
      completed.commands.some(
        (command) => command.kind === 'request_fold_angle',
      ),
      false,
    )
  }

  const aboveLegacyLimit = 'c'.repeat(262_145)
  const accepted = plan(
    transition(
      createFoldPreviewPhysicalGrabGestureState(),
      [{
        kind: 'end',
        pointerId: 1,
        outcome: 'drag',
        completionTarget: target({ contextKey: aboveLegacyLimit }),
        rejectionReason: null,
      }],
    ),
    { activeContextKey: aboveLegacyLimit },
  )
  assert.equal(accepted.commands.at(-1)?.kind, 'request_fold_angle')
})

test('completion fields are captured once into the immutable request', () => {
  const reads = {
    transition: 0,
    eventPointerId: 0,
    selectedHingeId: 0,
    activeHingeId: 0,
    modelHingeId: 0,
    activeContextKey: 0,
    disposed: 0,
    guardIsCurrent: 0,
    targetKind: 0,
    mapping: 0,
    targetContextKey: 0,
    angleDegrees: 0,
  }
  const completionTarget = {
    ...target(),
    get kind() {
      reads.targetKind += 1
      return reads.targetKind === 1
        ? 'unverified_target' as const
        : 'changed-target' as never
    },
    get mapping() {
      reads.mapping += 1
      return reads.mapping === 1
        ? FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
        : 'physical_grab_v1'
    },
    get contextKey() {
      reads.targetContextKey += 1
      return reads.targetContextKey === 1 ? 'context-1' : 'changed-context'
    },
    get angleDegrees() {
      reads.angleDegrees += 1
      return reads.angleDegrees === 1 ? 73 : Number.NaN
    },
  } as FoldPreviewPhysicalGrabTarget
  const trustedTransition = transition(
    createFoldPreviewPhysicalGrabGestureState(),
    [{
      kind: 'end',
      pointerId: 1,
      outcome: 'drag',
      completionTarget,
      rejectionReason: null,
    }],
  )
  const input = {
    get transition() {
      reads.transition += 1
      return reads.transition === 1
        ? trustedTransition
        : throwingProxy<FoldPreviewPhysicalGrabGestureTransition>()
    },
    get eventPointerId() {
      reads.eventPointerId += 1
      return reads.eventPointerId === 1 ? 1 : Number.NaN
    },
    get selectedHingeId() {
      reads.selectedHingeId += 1
      return reads.selectedHingeId === 1 ? null : 'changed-hinge'
    },
    get activeHingeId() {
      reads.activeHingeId += 1
      return reads.activeHingeId === 1 ? 'hinge-1' : 'changed-hinge'
    },
    get modelHingeId() {
      reads.modelHingeId += 1
      return reads.modelHingeId === 1 ? 'hinge-1' : 'changed-hinge'
    },
    get activeContextKey() {
      reads.activeContextKey += 1
      return reads.activeContextKey === 1 ? 'context-1' : 'changed-context'
    },
    get disposed() {
      reads.disposed += 1
      return reads.disposed === 1 ? false : true
    },
    get guardIsCurrent() {
      reads.guardIsCurrent += 1
      return reads.guardIsCurrent === 1 ? true : false
    },
  } as FoldPreviewPhysicalGrabTransitionPlanInput

  const completed = planFoldPreviewPhysicalGrabTransition(input)
  assert.deepEqual(completed.commands.at(-1), {
    kind: 'request_fold_angle',
    hingeEdgeId: 'hinge-1',
    contextKey: 'context-1',
    angleDegrees: 73,
  })
  assert.deepEqual(reads, {
    transition: 1,
    eventPointerId: 1,
    selectedHingeId: 1,
    activeHingeId: 1,
    modelHingeId: 1,
    activeContextKey: 1,
    disposed: 1,
    guardIsCurrent: 1,
    targetKind: 1,
    mapping: 1,
    targetContextKey: 1,
    angleDegrees: 1,
  })
  assertDeepFrozen(completed)
})

test('malformed public records and oversized effect arrays return cleanup only', () => {
  const invalidInputs: FoldPreviewPhysicalGrabTransitionPlanInput[] = [
    throwingProxy<FoldPreviewPhysicalGrabTransitionPlanInput>(),
    {
      ...planInput(),
      transition: throwingProxy<FoldPreviewPhysicalGrabGestureTransition>(),
    },
    {
      ...planInput(),
      transition: {
        state: createFoldPreviewPhysicalGrabGestureState(),
        effects: [throwingProxy<FoldPreviewPhysicalGrabGestureEffect>()],
      },
    },
    {
      ...planInput(),
      transition: {
        state: createFoldPreviewPhysicalGrabGestureState(),
        effects: oversizedEffectsProxy(
          MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_EFFECTS + 1,
        ),
      },
    },
  ]

  for (const input of invalidInputs) {
    assert.doesNotThrow(() =>
      planFoldPreviewPhysicalGrabTransition(input))
    const result = planFoldPreviewPhysicalGrabTransition(input)
    assert.deepEqual(result, {
      state: createFoldPreviewPhysicalGrabGestureState(),
      handled: false,
      endedAsTap: false,
      commands: [
        { kind: 'discard_presentation' },
        { kind: 'clear_interaction', clearEventSession: true },
        { kind: 'restore_camera' },
        { kind: 'sync_presentation' },
      ],
    })
    assertDeepFrozen(result)
  }
})

test('throwing completion target Proxies fail closed after cleanup', () => {
  const completionTarget = new Proxy(target(), {
    get() {
      throw new Error('unexpected target access')
    },
  })
  let completed: ReturnType<typeof planFoldPreviewPhysicalGrabTransition> | null = null
  assert.doesNotThrow(() => {
    completed = plan(transition(
      createFoldPreviewPhysicalGrabGestureState(),
      [{
        kind: 'end',
        pointerId: 1,
        outcome: 'drag',
        completionTarget,
        rejectionReason: null,
      }],
    ))
  })
  assert.ok(completed)
  assert.deepEqual(completed.commands, [
    { kind: 'discard_presentation' },
    { kind: 'release_capture', pointerId: 1 },
    { kind: 'clear_interaction', clearEventSession: true },
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
  ])
})

test('malformed terminal combinations never submit a completion', () => {
  const mixedTerminal = plan(transition(
    createFoldPreviewPhysicalGrabGestureState(),
    [
      { kind: 'cancel', pointerId: 1, reason: 'reset' },
      {
        kind: 'end',
        pointerId: 1,
        outcome: 'drag',
        completionTarget: target(),
        rejectionReason: null,
      },
    ],
  ))
  assert.equal(
    mixedTerminal.commands.some(
      (command) => command.kind === 'request_fold_angle',
    ),
    false,
  )

  const suppressedTerminal = plan(transition(
    idleState([2]),
    [{
      kind: 'end',
      pointerId: 1,
      outcome: 'drag',
      completionTarget: target(),
      rejectionReason: null,
    }],
  ))
  assert.equal(
    suppressedTerminal.commands.some(
      (command) => command.kind === 'request_fold_angle',
    ),
    false,
  )
})

test('tap completion reports selection intent but never requests an angle', () => {
  const tapped = plan(transition(
    createFoldPreviewPhysicalGrabGestureState(),
    [
      handled(1),
      {
        kind: 'end',
        pointerId: 1,
        outcome: 'tap',
        completionTarget: null,
        rejectionReason: null,
      },
    ],
  ))
  assert.equal(tapped.handled, true)
  assert.equal(tapped.endedAsTap, true)
  assert.deepEqual(tapped.commands, [
    { kind: 'discard_presentation' },
    { kind: 'release_capture', pointerId: 1 },
    { kind: 'clear_interaction', clearEventSession: true },
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
  ])
})

test('suppressed pointers preserve the event session until the last pointer drains', () => {
  const suppressed = idleState([1, 2])
  const cancelled = plan(
    transition(
      suppressed,
      [
        handled(2),
        { kind: 'cancel', pointerId: 1, reason: 'multiple_pointers' },
      ],
    ),
    { eventPointerId: 2 },
  )
  assert.equal(cancelled.handled, true)
  assert.deepEqual(cancelled.commands, [
    { kind: 'discard_presentation' },
    { kind: 'release_capture', pointerId: 1 },
    { kind: 'clear_interaction', clearEventSession: false },
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
  ])

  const stillSuppressed = plan(
    transition(idleState([1]), [handled(2)]),
    { eventPointerId: 2 },
  )
  assert.equal(
    stillSuppressed.commands.some(
      (command) => command.kind === 'clear_interaction',
    ),
    false,
  )

  const drained = plan(
    transition(
      createFoldPreviewPhysicalGrabGestureState(),
      [handled(1)],
    ),
  )
  assert.deepEqual(drained.commands, [
    { kind: 'clear_interaction', clearEventSession: true },
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
  ])
})

test('handled status is scoped to the DOM event pointer and plans are frozen', () => {
  const result = plan(
    transition(activeState('armed'), [handled(2)]),
    { eventPointerId: 1 },
  )
  assert.equal(result.handled, false)
  assert.deepEqual(result.commands, [
    { kind: 'restore_camera' },
    { kind: 'sync_presentation' },
  ])
  assertDeepFrozen(result)
})

function plan(
  currentTransition: FoldPreviewPhysicalGrabGestureTransition,
  overrides: Partial<FoldPreviewPhysicalGrabTransitionPlanInput> = {},
) {
  return planFoldPreviewPhysicalGrabTransition({
    transition: currentTransition,
    eventPointerId: 1,
    selectedHingeId: null,
    activeHingeId: 'hinge-1',
    modelHingeId: 'hinge-1',
    activeContextKey: 'context-1',
    disposed: false,
    guardIsCurrent: true,
    ...overrides,
  })
}

function planInput(): FoldPreviewPhysicalGrabTransitionPlanInput {
  return {
    transition: transition(
      createFoldPreviewPhysicalGrabGestureState(),
      [],
    ),
    eventPointerId: 1,
    selectedHingeId: null,
    activeHingeId: 'hinge-1',
    modelHingeId: 'hinge-1',
    activeContextKey: 'context-1',
    disposed: false,
    guardIsCurrent: true,
  }
}

function transition(
  state: FoldPreviewPhysicalGrabGestureState,
  effects: readonly FoldPreviewPhysicalGrabGestureEffect[],
): FoldPreviewPhysicalGrabGestureTransition {
  return Object.freeze({
    state,
    effects: Object.freeze(
      effects.map((effect) => Object.freeze({ ...effect })),
    ),
  })
}

function activeState(
  kind: 'armed' | 'dragging',
): FoldPreviewPhysicalGrabGestureState {
  return Object.freeze({ kind }) as unknown as FoldPreviewPhysicalGrabGestureState
}

function idleState(
  suppressedPointerIds: readonly number[],
): FoldPreviewPhysicalGrabGestureState {
  return Object.freeze({
    kind: 'idle' as const,
    suppressedPointerIds: Object.freeze([...suppressedPointerIds]),
    requiresReset: false,
  })
}

function handled(pointerId: number): FoldPreviewPhysicalGrabGestureEffect {
  return { kind: 'handled', pointerId }
}

function target(
  overrides: Readonly<Record<string, unknown>> = {},
): FoldPreviewPhysicalGrabTarget {
  return Object.freeze({
    kind: 'unverified_target',
    mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
    contextKey: 'context-1',
    angleDegrees: 40,
    rawAngleDegrees: 40,
    endpoint: null,
    missDistance: 0,
    orbitWorldPoint: Object.freeze({ x: 1, y: 0, z: 0 }),
    evaluationCount: 1,
    rootEvaluationCount: 1,
    stationaryCandidateCount: 0,
    boundaryCandidateCount: 0,
    equivalentCandidateCount: 1,
    ...overrides,
  }) as FoldPreviewPhysicalGrabTarget
}

function assertDeepFrozen(value: unknown, seen = new Set<unknown>()) {
  if (value === null || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const child of Object.values(value)) {
    assertDeepFrozen(child, seen)
  }
}

function throwingProxy<Value>(): Value {
  return new Proxy({}, {
    get() {
      throw new Error('unexpected access')
    },
  }) as Value
}

function oversizedEffectsProxy(
  reportedLength: number,
): readonly FoldPreviewPhysicalGrabGestureEffect[] {
  return new Proxy<FoldPreviewPhysicalGrabGestureEffect[]>([], {
    get(target, property, receiver) {
      if (property === 'length') return reportedLength
      if (
        typeof property === 'string'
        && /^(?:0|[1-9]\d*)$/u.test(property)
      ) throw new Error('oversized effects must not be indexed')
      return Reflect.get(target, property, receiver)
    },
  })
}
