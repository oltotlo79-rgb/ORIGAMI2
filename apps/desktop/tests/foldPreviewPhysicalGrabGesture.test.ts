import assert from 'node:assert/strict'
import test from 'node:test'

import {
  prepareFoldPreviewPhysicalGrab,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabRay,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES,
  collectFoldPreviewPhysicalGrabPointerSamples,
  createFoldPreviewPhysicalGrabGestureState,
  reduceFoldPreviewPhysicalGrabGesture,
  type FoldPreviewPhysicalGrabGestureEffect,
  type FoldPreviewPhysicalGrabGestureEvent,
  type FoldPreviewPhysicalGrabGestureState,
  type FoldPreviewPhysicalGrabGestureTransition,
} from '../src/lib/foldPreviewPhysicalGrabGesture.ts'

type DownEvent = Extract<
  FoldPreviewPhysicalGrabGestureEvent,
  { kind: 'pointer_down' }
>
type MoveEvent = Extract<
  FoldPreviewPhysicalGrabGestureEvent,
  { kind: 'pointer_move' }
>
type UpEvent = Extract<
  FoldPreviewPhysicalGrabGestureEvent,
  { kind: 'pointer_up' }
>

test('coalesced pointer batches are ordered, detached, frozen, and strictly bounded', () => {
  const current = { id: 'current' }
  const empty: Array<{ id: string }> = []
  const currentOnly =
    collectFoldPreviewPhysicalGrabPointerSamples(current, empty)
  assert.deepEqual(currentOnly, [current])
  assert.ok(Object.isFrozen(currentOnly))

  const maximumCoalesced = Array.from(
    { length: FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES - 1 },
    (_, index) => ({ id: `coalesced-${index}` }),
  )
  const maximum =
    collectFoldPreviewPhysicalGrabPointerSamples(
      current,
      maximumCoalesced,
    )
  assert.equal(
    maximum?.length,
    FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES,
  )
  assert.strictEqual(maximum?.[0], maximumCoalesced[0])
  assert.strictEqual(maximum?.at(-1), current)
  assert.ok(Object.isFrozen(maximum))
  maximumCoalesced.splice(0)
  assert.equal(
    maximum?.length,
    FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES,
  )

  const includesCurrent = [
    current,
    ...Array.from(
      { length: FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES - 2 },
      (_, index) => ({ id: `distinct-${index}` }),
    ),
  ]
  const deduplicated =
    collectFoldPreviewPhysicalGrabPointerSamples(
      current,
      includesCurrent,
    )
  assert.equal(
    deduplicated?.filter((sample) => sample === current).length,
    1,
  )
  assert.strictEqual(deduplicated?.at(-1), current)

  const excessive = Array.from(
    { length: FOLD_PREVIEW_PHYSICAL_GRAB_MAX_POINTER_SAMPLES },
    (_, index) => ({ id: `excessive-${index}` }),
  )
  assert.equal(
    collectFoldPreviewPhysicalGrabPointerSamples(current, excessive),
    null,
  )
  assert.equal(
    collectFoldPreviewPhysicalGrabPointerSamples(
      current,
      { length: 0 } as unknown as ReadonlyArray<typeof current>,
    ),
    null,
  )
})

test('accepted moves track forward and reverse branches by raw solver angle', () => {
  const armed = start(down({ session: readySession(30) }))
  const forward = reduceFoldPreviewPhysicalGrabGesture(
    armed,
    move({
      clientY: 93,
      ray: normalRayThrough(orbitPoint(50)),
    }),
  )
  assert.equal(forward.state.kind, 'dragging')
  if (forward.state.kind !== 'dragging') assert.fail('expected dragging')
  assert.ok(Math.abs(forward.state.referenceAngleDegrees - 50) < 0.001)
  assert.equal(forward.state.presentationTarget?.angleDegrees, 50)
  assert.deepEqual(presentationEffect(forward)?.target?.angleDegrees, 50)

  const reverse = reduceFoldPreviewPhysicalGrabGesture(
    forward.state,
    move({
      clientY: 92,
      ray: normalRayThrough(orbitPoint(25)),
    }),
  )
  assert.equal(reverse.state.kind, 'dragging')
  if (reverse.state.kind !== 'dragging') assert.fail('expected dragging')
  assert.ok(Math.abs(reverse.state.referenceAngleDegrees - 25) < 0.001)
  assert.equal(reverse.state.presentationTarget?.angleDegrees, 25)
  assert.equal(presentationEffect(reverse)?.target?.angleDegrees, 25)
})

test('mouse and pen cross only beyond six CSS pixels', () => {
  for (const pointerType of ['mouse', 'pen'] as const) {
    const armed = start(down({ pointerType }))
    const threshold = reduceFoldPreviewPhysicalGrabGesture(
      armed,
      move({
        pointerType,
        clientY: 106,
        ray: normalRayThrough(orbitPoint(35)),
      }),
    )
    assert.equal(threshold.state.kind, 'armed')
    assert.deepEqual(threshold.effects, [
      { kind: 'handled', pointerId: 1 },
    ])

    const crossed = reduceFoldPreviewPhysicalGrabGesture(
      threshold.state,
      move({
        pointerType,
        clientY: 106.01,
        ray: normalRayThrough(orbitPoint(35)),
      }),
    )
    assert.equal(crossed.state.kind, 'dragging')
    assert.equal(presentationEffect(crossed)?.target?.angleDegrees, 35)
  }
})

test('touch crosses only beyond ten CSS pixels', () => {
  const armed = start(down({ pointerType: 'touch' }))
  const threshold = reduceFoldPreviewPhysicalGrabGesture(
    armed,
    move({
      pointerType: 'touch',
      clientX: 46,
      clientY: 108,
      ray: normalRayThrough(orbitPoint(35)),
    }),
  )
  assert.equal(threshold.state.kind, 'armed')

  const crossed = reduceFoldPreviewPhysicalGrabGesture(
    threshold.state,
    move({
      pointerType: 'touch',
      clientX: 46.01,
      clientY: 108,
      ray: normalRayThrough(orbitPoint(35)),
    }),
  )
  assert.equal(crossed.state.kind, 'dragging')
  assert.equal(presentationEffect(crossed)?.target?.angleDegrees, 35)
})

test('an armed release at the inclusive threshold remains a tap', () => {
  const result = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    up({
      clientY: 106,
      ray: normalRayThrough(orbitPoint(40)),
    }),
  )
  assert.deepEqual(result.state, createFoldPreviewPhysicalGrabGestureState())
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 1 },
    {
      kind: 'end',
      pointerId: 1,
      outcome: 'tap',
      completionTarget: null,
      rejectionReason: null,
    },
  ])
})

test('a release that crosses the threshold resolves only its final ray', () => {
  const result = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    up({
      clientY: 106.01,
      ray: normalRayThrough(orbitPoint(44)),
    }),
  )
  const end = endEffect(result)
  assert.equal(end?.outcome, 'drag')
  assert.equal(end?.completionTarget?.angleDegrees, 44)
  assert.equal(end?.rejectionReason, null)
  assert.equal(presentationEffect(result), undefined)
})

test('pointer-up re-solves its final ray instead of reusing a move target', () => {
  const moved = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    move({
      clientY: 93,
      ray: normalRayThrough(orbitPoint(40)),
    }),
  )
  assert.equal(moved.state.kind, 'dragging')
  if (moved.state.kind !== 'dragging') assert.fail('expected dragging')
  const moveTarget = moved.state.presentationTarget
  assert.equal(moveTarget?.angleDegrees, 40)

  const finished = reduceFoldPreviewPhysicalGrabGesture(
    moved.state,
    up({
      clientY: 93,
      ray: normalRayThrough(orbitPoint(52)),
    }),
  )
  const end = endEffect(finished)
  assert.equal(end?.outcome, 'drag')
  assert.equal(end?.completionTarget?.angleDegrees, 52)
  assert.notStrictEqual(end?.completionTarget, moveTarget)
  assert.equal(
    finished.effects.filter(
      (effect) =>
        effect.kind === 'end'
        && effect.completionTarget !== null,
    ).length,
    1,
  )
})

test('a rejected final ray submits nothing and never falls back to a move', () => {
  const moved = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    move({
      clientY: 93,
      ray: normalRayThrough(orbitPoint(40)),
    }),
  )
  assert.equal(presentationEffect(moved)?.target?.angleDegrees, 40)

  const finished = reduceFoldPreviewPhysicalGrabGesture(
    moved.state,
    up({ clientY: 93, ray: farRay() }),
  )
  const end = endEffect(finished)
  assert.equal(end?.outcome, 'drag')
  assert.equal(end?.completionTarget, null)
  assert.equal(end?.rejectionReason, 'target_too_far')
  assert.equal(
    finished.effects.some(
      (effect) =>
        effect.kind === 'end'
        && effect.completionTarget?.angleDegrees === 40,
    ),
    false,
  )
})

test('ambiguous moves enter irreversible dragging and clear old targets', () => {
  const ambiguous = reduceFoldPreviewPhysicalGrabGesture(
    start(down({ session: readySession(90) })),
    move({
      clientY: 93,
      ray: sideRayThroughHeight(Math.sin(degreesToRadians(60))),
    }),
  )
  assert.equal(ambiguous.state.kind, 'dragging')
  if (ambiguous.state.kind !== 'dragging') assert.fail('expected dragging')
  assert.equal(ambiguous.state.presentationTarget, null)
  assert.deepEqual(presentationEffect(ambiguous), {
    kind: 'presentation',
    pointerId: 1,
    target: null,
    rejectionReason: 'ambiguous_projection',
  })

  const recoveredInsideThreshold = reduceFoldPreviewPhysicalGrabGesture(
    ambiguous.state,
    move({
      clientY: 100,
      ray: normalRayThrough(orbitPoint(80)),
    }),
  )
  assert.equal(recoveredInsideThreshold.state.kind, 'dragging')
  assert.equal(presentationEffect(recoveredInsideThreshold)?.target?.angleDegrees, 80)

  const rejectedAgain = reduceFoldPreviewPhysicalGrabGesture(
    recoveredInsideThreshold.state,
    move({ clientY: 100, ray: farRay() }),
  )
  assert.equal(rejectedAgain.state.kind, 'dragging')
  if (rejectedAgain.state.kind !== 'dragging') assert.fail('expected dragging')
  assert.equal(rejectedAgain.state.presentationTarget, null)
  assert.ok(Math.abs(rejectedAgain.state.referenceAngleDegrees - 80) < 0.001)
  assert.deepEqual(presentationEffect(rejectedAgain), {
    kind: 'presentation',
    pointerId: 1,
    target: null,
    rejectionReason: 'branch_jump',
  })
})

test('stale guards and contexts cancel without emitting a target', () => {
  for (const event of [
    move({ guardKey: 'other-guard', clientY: 93 }),
    move({ contextKey: 'other-context', clientY: 93 }),
  ]) {
    const result = reduceFoldPreviewPhysicalGrabGesture(start(down()), event)
    assert.equal(result.state.kind, 'idle')
    if (result.state.kind !== 'idle') assert.fail('expected idle')
    assert.deepEqual(result.state.suppressedPointerIds, [1])
    assert.equal(
      result.effects.some(
        (effect) =>
          effect.kind === 'presentation'
          || effect.kind === 'end',
      ),
      false,
    )
  }

  const invalidUp = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    up({
      guardKey: 'other-guard',
      clientY: 93,
    }),
  )
  assert.deepEqual(invalidUp.state, createFoldPreviewPhysicalGrabGestureState())
  assert.equal(endEffect(invalidUp), undefined)
})

test('pointer type, unit ray, buttons, and bounds are guarded per sample', () => {
  const cases: ReadonlyArray<{
    event: MoveEvent
    reason: string
  }> = [
    {
      event: move({ pointerType: 'pen', clientY: 93 }),
      reason: 'pointer_mismatch',
    },
    {
      event: move({
        clientY: 93,
        ray: {
          ...normalRayThrough(orbitPoint(40)),
          direction: { x: 0, y: 0, z: -2 },
        },
      }),
      reason: 'invalid_ray',
    },
    {
      event: move({ clientY: 93, buttons: 0 }),
      reason: 'buttons_changed',
    },
    {
      event: move({ clientY: 93, isInside: false }),
      reason: 'pointer_outside',
    },
  ]
  for (const { event, reason } of cases) {
    const result = reduceFoldPreviewPhysicalGrabGesture(start(down()), event)
    assert.equal(
      result.effects.some(
        (effect) => effect.kind === 'cancel' && effect.reason === reason,
      ),
      true,
    )
    assert.equal(presentationEffect(result), undefined)
  }
})

test('a second pointer cancels and suppresses the whole sequence', () => {
  const second = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    down({
      pointerId: 2,
      pointerType: 'touch',
      isPrimary: false,
      hadActivePointer: true,
    }),
  )
  assert.equal(second.state.kind, 'idle')
  if (second.state.kind !== 'idle') assert.fail('expected idle')
  assert.deepEqual(second.state.suppressedPointerIds, [1, 2])
  assert.deepEqual(second.effects, [
    { kind: 'handled', pointerId: 2 },
    { kind: 'cancel', pointerId: 1, reason: 'multiple_pointers' },
  ])

  const suppressedMove = reduceFoldPreviewPhysicalGrabGesture(
    second.state,
    move({
      pointerId: 2,
      pointerType: null,
      clientX: -1_000,
      clientY: -1_000,
      ray: null,
      isInside: false,
    }),
  )
  assert.equal(suppressedMove.state.kind, 'idle')
  if (suppressedMove.state.kind !== 'idle') assert.fail('expected idle')
  assert.deepEqual(suppressedMove.state.suppressedPointerIds, [1, 2])

  const pointerTwoEnded = reduceFoldPreviewPhysicalGrabGesture(
    suppressedMove.state,
    up({
      pointerId: 2,
      pointerType: null,
      clientX: -1_000,
      clientY: -1_000,
      ray: null,
      isInside: false,
    }),
  )
  assert.equal(pointerTwoEnded.state.kind, 'idle')
  if (pointerTwoEnded.state.kind !== 'idle') assert.fail('expected idle')
  assert.deepEqual(pointerTwoEnded.state.suppressedPointerIds, [1])

  const allEnded = reduceFoldPreviewPhysicalGrabGesture(
    pointerTwoEnded.state,
    up(),
  )
  assert.deepEqual(allEnded.state, createFoldPreviewPhysicalGrabGestureState())
})

test('cancel, lost capture, blur, and reset fail closed', () => {
  for (const reason of ['pointer_cancel', 'lost_pointer_capture'] as const) {
    const result = reduceFoldPreviewPhysicalGrabGesture(
      start(down()),
      {
        kind: 'pointer_cancel',
        pointerId: 1,
        pointerType: 'mouse',
        reason,
      },
    )
    assert.deepEqual(result.state, createFoldPreviewPhysicalGrabGestureState())
    assert.equal(
      result.effects.some(
        (effect) => effect.kind === 'cancel' && effect.reason === reason,
      ),
      true,
    )
  }

  for (const reason of ['window_blur', 'reset'] as const) {
    const result = reduceFoldPreviewPhysicalGrabGesture(
      start(down()),
      { kind: 'reset', reason },
    )
    assert.deepEqual(result.state, createFoldPreviewPhysicalGrabGestureState())
    assert.equal(
      result.effects.some(
        (effect) => effect.kind === 'cancel' && effect.reason === reason,
      ),
      true,
    )
  }
})

test('invalid starts and malformed samples fail closed', () => {
  for (const invalidDown of [
    down({ isPrimary: false }),
    down({ altKey: true }),
    down({ hadActivePointer: true }),
    down({ buttons: 0 }),
    down({ contextKey: 'stale-context' }),
  ]) {
    const result = reduceFoldPreviewPhysicalGrabGesture(
      createFoldPreviewPhysicalGrabGestureState(),
      invalidDown,
    )
    assert.deepEqual(result.state, createFoldPreviewPhysicalGrabGestureState())
    assert.deepEqual(result.effects, [])
  }

  const malformed = reduceFoldPreviewPhysicalGrabGesture(
    start(down()),
    {
      ...move(),
      clientX: Number.NaN,
    },
  )
  assert.equal(
    malformed.effects.some(
      (effect) =>
        effect.kind === 'cancel'
        && effect.reason === 'invalid_sample',
    ),
    true,
  )

  const forgedState = {
    ...start(down()),
    referenceAngleDegrees: Number.NaN,
  } as FoldPreviewPhysicalGrabGestureState
  const rejectedState = reduceFoldPreviewPhysicalGrabGesture(
    forgedState,
    move({ clientY: 93 }),
  )
  assert.deepEqual(
    rejectedState.state,
    createFoldPreviewPhysicalGrabGestureState(),
  )
  assert.deepEqual(rejectedState.effects, [])
})

test('states and effects are deeply frozen detached snapshots', () => {
  const session = readySession(30)
  const armedTransition = reduceFoldPreviewPhysicalGrabGesture(
    createFoldPreviewPhysicalGrabGestureState(),
    down({ session }),
  )
  assertDeepFrozen(armedTransition)
  assert.equal(armedTransition.state.kind, 'armed')
  if (armedTransition.state.kind !== 'armed') assert.fail('expected armed')
  assert.notStrictEqual(armedTransition.state.session, session)
  assert.notStrictEqual(
    armedTransition.state.session.axisOrigin,
    session.axisOrigin,
  )

  const ray = normalRayThrough(orbitPoint(40))
  const moved = reduceFoldPreviewPhysicalGrabGesture(
    armedTransition.state,
    move({ clientY: 93, ray }),
  )
  assertDeepFrozen(moved)
  assert.equal(moved.state.kind, 'dragging')
  if (moved.state.kind !== 'dragging') assert.fail('expected dragging')
  const effectTarget = presentationEffect(moved)?.target
  assert.ok(effectTarget)
  assert.notStrictEqual(effectTarget, moved.state.presentationTarget)
  assert.notStrictEqual(
    effectTarget?.orbitWorldPoint,
    moved.state.presentationTarget?.orbitWorldPoint,
  )

  ;(ray.origin as { x: number }).x = 1_000
  assert.equal(moved.state.presentationTarget?.angleDegrees, 40)
  assert.ok(
    Math.abs((moved.state.presentationTarget?.orbitWorldPoint.x ?? 0)
      - Math.cos(degreesToRadians(40))) < 0.002,
  )
})

function start(event: DownEvent): FoldPreviewPhysicalGrabGestureState {
  const result = reduceFoldPreviewPhysicalGrabGesture(
    createFoldPreviewPhysicalGrabGestureState(),
    event,
  )
  assert.equal(result.state.kind, 'armed')
  return result.state
}

function down(overrides: Partial<DownEvent> = {}): DownEvent {
  const session = overrides.session ?? readySession(30)
  return {
    kind: 'pointer_down',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 100,
    button: 0,
    buttons: 1,
    isPrimary: true,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    hadActivePointer: false,
    guardKey: 'guard',
    contextKey: session.contextKey,
    session,
    ...overrides,
  }
}

function move(overrides: Partial<MoveEvent> = {}): MoveEvent {
  return {
    kind: 'pointer_move',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 93,
    buttons: 1,
    guardKey: 'guard',
    contextKey: 'context',
    ray: normalRayThrough(orbitPoint(40)),
    isInside: true,
    ...overrides,
  }
}

function up(overrides: Partial<UpEvent> = {}): UpEvent {
  return {
    kind: 'pointer_up',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 100,
    button: 0,
    buttons: 0,
    guardKey: 'guard',
    contextKey: 'context',
    ray: normalRayThrough(orbitPoint(40)),
    isInside: true,
    ...overrides,
  }
}

function readySession(appliedAngleDegrees: number) {
  const grabWorldPoint = orbitPoint(appliedAngleDegrees)
  const result = prepareFoldPreviewPhysicalGrab({
    contextKey: 'context',
    axisStart: { x: 0, y: 0, z: 0 },
    axisEnd: { x: 0, y: 0, z: 2 },
    movingRotationSign: 1,
    appliedAngleDegrees,
    grabRestWorldPoint: orbitPoint(0),
    grabWorldPoint,
    startRay: normalRayThrough(grabWorldPoint),
    minimumOrbitRadius: 0.01,
  })
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)
  return result.session
}

function orbitPoint(angleDegrees: number): FoldPreviewPhysicalGrabPoint {
  const angle = degreesToRadians(angleDegrees)
  return {
    x: Math.cos(angle),
    y: Math.sin(angle),
    z: 0,
  }
}

function normalRayThrough(
  point: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: point.x, y: point.y, z: point.z + 5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function sideRayThroughHeight(height: number): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: 5, y: height, z: 0 },
    direction: { x: -1, y: 0, z: 0 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function farRay(): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: 10, y: 0, z: 5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0.1,
    maximumDistance: 10,
  }
}

function presentationEffect(
  transition: FoldPreviewPhysicalGrabGestureTransition,
) {
  return transition.effects.find(
    (
      effect,
    ): effect is Extract<
      FoldPreviewPhysicalGrabGestureEffect,
      { kind: 'presentation' }
    > => effect.kind === 'presentation',
  )
}

function endEffect(
  transition: FoldPreviewPhysicalGrabGestureTransition,
) {
  return transition.effects.find(
    (
      effect,
    ): effect is Extract<
      FoldPreviewPhysicalGrabGestureEffect,
      { kind: 'end' }
    > => effect.kind === 'end',
  )
}

function assertDeepFrozen(value: unknown, seen = new Set<unknown>()) {
  if (value === null || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const child of Object.values(value)) {
    assertDeepFrozen(child, seen)
  }
}

function degreesToRadians(value: number) {
  return value * Math.PI / 180
}
