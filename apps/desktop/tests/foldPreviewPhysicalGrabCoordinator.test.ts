import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  type FoldPreviewPhysicalGrabTarget,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import {
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
    { kind: 'request_fold_angle', angleDegrees: 52 },
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
    { input: { modelHingeId: null } },
    { completionTarget: target({ mapping: 'physical_grab_v1' }) },
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
