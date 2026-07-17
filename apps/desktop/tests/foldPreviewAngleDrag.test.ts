import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_PREVIEW_ANGLE_DRAG_MAPPING,
  createFoldPreviewAngleDragState,
  isFoldPreviewAngleDragScreenHit,
  reduceFoldPreviewAngleDrag,
  type FoldPreviewAngleDragEvent,
  type FoldPreviewAngleDragState,
} from '../src/lib/foldPreviewAngleDrag.ts'

test('the interaction mapping has a stable non-physical version', () => {
  assert.equal(FOLD_PREVIEW_ANGLE_DRAG_MAPPING, 'vertical_parameter_v1')
})

test('screen-space hinge validation accepts only a visible nearby segment', () => {
  const valid = screenHit()
  assert.equal(isFoldPreviewAngleDragScreenHit(valid), true)
  assert.equal(isFoldPreviewAngleDragScreenHit({
    ...valid,
    startNdc: { x: 0, y: 0, z: 0 },
    endNdc: { x: 0.01, y: 0, z: 0 },
  }), false)
  assert.equal(isFoldPreviewAngleDragScreenHit({
    ...valid,
    endNdc: { x: 0.5, y: 0, z: 1.01 },
  }), false)
  assert.equal(isFoldPreviewAngleDragScreenHit({
    ...valid,
    pointer: { clientX: 100, clientY: 130 },
  }), false)
  assert.equal(isFoldPreviewAngleDragScreenHit({
    ...valid,
    pointer: { clientX: 201, clientY: 100 },
  }), false)
})

test('screen-space hinge validation rejects malformed and overflowing projections', () => {
  const valid = screenHit()
  for (const malformed of [
    { ...valid, viewport: { ...valid.viewport, width: 0 } },
    { ...valid, viewport: { ...valid.viewport, left: Number.MAX_VALUE } },
    { ...valid, pointer: { clientX: Number.NaN, clientY: 100 } },
    { ...valid, startNdc: { x: Number.POSITIVE_INFINITY, y: 0, z: 0 } },
    { ...valid, minimumLengthPixels: 0 },
    { ...valid, maximumDistancePixels: -1 },
  ]) {
    assert.equal(isFoldPreviewAngleDragScreenHit(malformed), false)
  }
  assert.equal(isFoldPreviewAngleDragScreenHit(null as never), false)
})

test('starts one primary unmodified mouse drag from the supplied applied angle', () => {
  const initial = createFoldPreviewAngleDragState()
  const result = reduceFoldPreviewAngleDrag(initial, down())

  assert.deepEqual(result.state, {
    kind: 'armed',
    pointerId: 1,
    pointerType: 'mouse',
    startClientX: 40,
    startClientY: 100,
    startAppliedAngle: 52.04,
    degreesPerPixel: 1,
    thresholdPixels: 6,
  })
  assert.deepEqual(result.effects, [{ kind: 'handled', pointerId: 1 }])
})

test('mouse and pen use an exclusive six-pixel threshold', () => {
  for (const pointerType of ['mouse', 'pen'] as const) {
    const armed = start(down({ pointerType }))
    const atThreshold = reduceFoldPreviewAngleDrag(
      armed,
      move({ pointerType, clientX: 40, clientY: 106 }),
    )
    assert.equal(atThreshold.state.kind, 'armed')
    assert.deepEqual(atThreshold.effects, [{ kind: 'handled', pointerId: 1 }])

    const beyond = reduceFoldPreviewAngleDrag(
      atThreshold.state,
      move({ pointerType, clientX: 40, clientY: 106.01 }),
    )
    assert.equal(beyond.state.kind, 'dragging')
    assert.deepEqual(beyond.effects, [
      { kind: 'handled', pointerId: 1 },
      { kind: 'target', pointerId: 1, targetAngle: 46 },
    ])
  }
})

test('touch uses an exclusive ten-pixel threshold', () => {
  const armed = start(down({ pointerType: 'touch' }))
  const atThreshold = reduceFoldPreviewAngleDrag(
    armed,
    move({ pointerType: 'touch', clientX: 46, clientY: 108 }),
  )
  assert.equal(atThreshold.state.kind, 'armed')

  const beyond = reduceFoldPreviewAngleDrag(
    atThreshold.state,
    move({ pointerType: 'touch', clientX: 46.01, clientY: 108 }),
  )
  assert.equal(beyond.state.kind, 'dragging')
  assert.deepEqual(beyond.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 44,
  })
})

test('upward motion increases and downward motion decreases the target in tenths', () => {
  const armed = start(down({ appliedAngle: 50 }))
  const upward = reduceFoldPreviewAngleDrag(
    armed,
    move({ clientY: 92.74 }),
  )
  assert.deepEqual(upward.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 57.3,
  })

  const downward = reduceFoldPreviewAngleDrag(
    start(down({ appliedAngle: 50 })),
    move({ clientY: 107.26 }),
  )
  assert.deepEqual(downward.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 42.7,
  })
})

test('targets derive from the displayed applied angle in both path directions', () => {
  const upward = reduceFoldPreviewAngleDrag(
    start(down({
      appliedAngle: 40,
      viewportHeight: 424,
    })),
    move({ clientY: 80 }),
  )
  assert.deepEqual(upward.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 50,
  })

  const reverse = reduceFoldPreviewAngleDrag(
    start(down({
      appliedAngle: 90,
      viewportHeight: 424,
    })),
    move({ clientY: 140 }),
  )
  assert.deepEqual(reverse.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 70,
  })
})

test('angle targets clamp to zero and 180 degrees', () => {
  const upper = reduceFoldPreviewAngleDrag(
    start(down({ appliedAngle: 179 })),
    move({ clientY: -100 }),
  )
  assert.deepEqual(upper.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 180,
  })

  const lower = reduceFoldPreviewAngleDrag(
    start(down({ appliedAngle: 1 })),
    move({ clientY: 300 }),
  )
  assert.deepEqual(lower.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 0,
  })
  assert.equal(Object.is((lower.state as { targetAngle: number }).targetAngle, -0), false)
})

test('viewport height determines a bounded deterministic angle scale', () => {
  assert.equal(
    (start(down({ viewportHeight: 20 })) as { degreesPerPixel: number }).degreesPerPixel,
    1,
  )
  assert.equal(
    (start(down({ viewportHeight: 244 })) as { degreesPerPixel: number }).degreesPerPixel,
    1,
  )
  assert.equal(
    (start(down({ viewportHeight: 544 })) as { degreesPerPixel: number }).degreesPerPixel,
    0.375,
  )
  assert.equal(
    (start(down({ viewportHeight: 400 })) as { degreesPerPixel: number }).degreesPerPixel,
    180 / 336,
  )
  assert.equal(
    (start(down({ viewportHeight: 2_000 })) as { degreesPerPixel: number }).degreesPerPixel,
    0.375,
  )
})

test('a drag never turns back into a tap after returning to its origin', () => {
  const armed = start(down({ appliedAngle: 50 }))
  const dragged = reduceFoldPreviewAngleDrag(armed, move({ clientY: 93 }))
  assert.equal(dragged.state.kind, 'dragging')

  const returned = reduceFoldPreviewAngleDrag(
    dragged.state,
    move({ clientY: 100 }),
  )
  assert.equal(returned.state.kind, 'dragging')
  assert.deepEqual(returned.effects.at(-1), {
    kind: 'target',
    pointerId: 1,
    targetAngle: 50,
  })

  const ended = reduceFoldPreviewAngleDrag(returned.state, up())
  assert.equal(ended.state.kind, 'idle')
  assert.deepEqual(ended.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'end', pointerId: 1, outcome: 'drag', targetAngle: 50 },
  ])
})

test('horizontal-dominant motion cancels the vertical parameter gesture', () => {
  let transition = reduceFoldPreviewAngleDrag(createFoldPreviewAngleDragState(), down())
  transition = reduceFoldPreviewAngleDrag(transition.state, move({
    clientX: 47,
    clientY: 101,
  }))

  assert.equal(transition.state.kind, 'idle')
  assert.deepEqual(transition.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'cancel', pointerId: 1, reason: 'horizontal_gesture' },
  ])
  assert.equal(
    transition.effects.some((effect) => effect.kind === 'target'),
    false,
  )

  transition = reduceFoldPreviewAngleDrag(transition.state, up({
    clientX: 47,
    clientY: 101,
  }))
  assert.deepEqual(transition.state, createFoldPreviewAngleDragState())
})

test('a release can cross the threshold and emits the final target before drag end', () => {
  const result = reduceFoldPreviewAngleDrag(
    start(down({ appliedAngle: 50 })),
    up({ clientY: 92 }),
  )
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'target', pointerId: 1, targetAngle: 58 },
    { kind: 'end', pointerId: 1, outcome: 'drag', targetAngle: 58 },
  ])
})

test('a release at the inclusive threshold remains a tap without a target', () => {
  const result = reduceFoldPreviewAngleDrag(
    start(down()),
    up({ clientX: 46 }),
  )
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'end', pointerId: 1, outcome: 'tap', targetAngle: null },
  ])
})

test('repeated samples in the same tenth do not emit duplicate targets', () => {
  const first = reduceFoldPreviewAngleDrag(
    start(down({ appliedAngle: 50 })),
    move({ clientY: 92.99 }),
  )
  const repeated = reduceFoldPreviewAngleDrag(
    first.state,
    move({ clientY: 92.96 }),
  )
  assert.deepEqual(repeated.effects, [{ kind: 'handled', pointerId: 1 }])
  assert.equal(
    (repeated.state as { targetAngle: number }).targetAngle,
    57,
  )
})

test('secondary buttons, competing pointers, modifiers, and unknown types pass through', () => {
  const rejected: FoldPreviewAngleDragEvent[] = [
    down({ button: 1 }),
    down({ button: 2 }),
    down({ isPrimary: false }),
    down({ hadActivePointer: true }),
    down({ altKey: true }),
    down({ ctrlKey: true }),
    down({ metaKey: true }),
    down({ shiftKey: true }),
    down({ pointerType: 'trackpad' as never }),
  ]
  for (const event of rejected) {
    const result = reduceFoldPreviewAngleDrag(createFoldPreviewAngleDragState(), event)
    assert.equal(result.state.kind, 'idle')
    assert.deepEqual(result.effects, [])
  }
})

test('invalid start angles, dimensions, coordinates, and pointer IDs pass through', () => {
  const rejected: FoldPreviewAngleDragEvent[] = [
    down({ appliedAngle: -0.1 }),
    down({ appliedAngle: 180.1 }),
    down({ appliedAngle: Number.NaN }),
    down({ viewportHeight: 0 }),
    down({ viewportHeight: Number.POSITIVE_INFINITY }),
    down({ clientX: Number.NaN }),
    down({ clientY: Number.NEGATIVE_INFINITY }),
    down({ pointerId: -1 }),
    down({ pointerId: 1.5 }),
  ]
  for (const event of rejected) {
    const result = reduceFoldPreviewAngleDrag(createFoldPreviewAngleDragState(), event)
    assert.equal(result.state.kind, 'idle')
    assert.deepEqual(result.effects, [])
  }
})

test('a second pointer cancels the drag and suppresses all known pointers until release', () => {
  const armed = start(down())
  const interrupted = reduceFoldPreviewAngleDrag(
    armed,
    down({
      pointerId: 2,
      pointerType: 'touch',
      isPrimary: false,
    }),
  )
  assert.deepEqual(interrupted.state, {
    kind: 'idle',
    suppressedPointerIds: [1, 2],
    requiresReset: false,
  })
  assert.deepEqual(interrupted.effects, [
    { kind: 'handled', pointerId: 2 },
    { kind: 'cancel', pointerId: 1, reason: 'multiple_pointers' },
  ])

  const ignoredMove = reduceFoldPreviewAngleDrag(
    interrupted.state,
    move({ pointerId: 1, clientY: 20 }),
  )
  assert.deepEqual(ignoredMove.effects, [{ kind: 'handled', pointerId: 1 }])
  assert.equal(ignoredMove.effects.some((effect) => effect.kind === 'target'), false)

  const secondEnded = reduceFoldPreviewAngleDrag(
    ignoredMove.state,
    up({ pointerId: 2, pointerType: 'touch' }),
  )
  assert.deepEqual(secondEnded.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: false,
  })
  const firstEnded = reduceFoldPreviewAngleDrag(secondEnded.state, up())
  assert.deepEqual(firstEnded.state, createFoldPreviewAngleDragState())
})

test('a foreign move fails closed and cannot advance the active target', () => {
  const result = reduceFoldPreviewAngleDrag(
    start(down()),
    move({ pointerId: 9, clientY: 20 }),
  )
  assert.deepEqual(result.state, {
    kind: 'idle',
    suppressedPointerIds: [1, 9],
    requiresReset: false,
  })
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 9 },
    { kind: 'cancel', pointerId: 1, reason: 'pointer_mismatch' },
  ])
})

test('a foreign up cancels and suppresses only the still-active original pointer', () => {
  const result = reduceFoldPreviewAngleDrag(
    start(down()),
    up({ pointerId: 9 }),
  )
  assert.deepEqual(result.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: false,
  })
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 9 },
    { kind: 'cancel', pointerId: 1, reason: 'pointer_mismatch' },
  ])
})

test('a pointer type change for the active ID fails closed', () => {
  const result = reduceFoldPreviewAngleDrag(
    start(down({ pointerType: 'pen' })),
    move({ pointerType: 'mouse', clientY: 20 }),
  )
  assert.deepEqual(result.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: false,
  })
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'cancel', pointerId: 1, reason: 'pointer_mismatch' },
  ])
})

test('pointer cancellation and lost capture end without producing a target', () => {
  for (const reason of ['pointer_cancel', 'lost_pointer_capture'] as const) {
    const result = reduceFoldPreviewAngleDrag(start(down()), {
      kind: 'pointer_cancel',
      pointerId: 1,
      pointerType: 'mouse',
      reason,
    })
    assert.deepEqual(result.state, createFoldPreviewAngleDragState())
    assert.deepEqual(result.effects, [
      { kind: 'handled', pointerId: 1 },
      { kind: 'cancel', pointerId: 1, reason },
    ])
  }
})

test('unknown cancellation reasons fail closed and require reset', () => {
  const result = reduceFoldPreviewAngleDrag(start(down()), {
    kind: 'pointer_cancel',
    pointerId: 1,
    pointerType: 'mouse',
    reason: 'unknown',
  } as never)
  assert.deepEqual(result.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: true,
  })
  assert.deepEqual(result.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'cancel', pointerId: 1, reason: 'invalid_sample' },
  ])
})

test('a non-finite active sample cancels and remains suppressed until pointer up', () => {
  const invalid = reduceFoldPreviewAngleDrag(
    start(down()),
    move({ clientY: Number.NaN }),
  )
  assert.deepEqual(invalid.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: false,
  })
  assert.deepEqual(invalid.effects, [
    { kind: 'handled', pointerId: 1 },
    { kind: 'cancel', pointerId: 1, reason: 'invalid_sample' },
  ])
  const recovered = reduceFoldPreviewAngleDrag(invalid.state, up())
  assert.deepEqual(recovered.state, createFoldPreviewAngleDragState())
})

test('an unidentifiable active sample requires explicit reset', () => {
  const invalid = reduceFoldPreviewAngleDrag(
    start(down()),
    move({ pointerId: Number.NaN }),
  )
  assert.deepEqual(invalid.state, {
    kind: 'idle',
    suppressedPointerIds: [1],
    requiresReset: true,
  })
  assert.deepEqual(invalid.effects, [
    { kind: 'cancel', pointerId: 1, reason: 'invalid_sample' },
  ])

  const stillSuppressed = reduceFoldPreviewAngleDrag(invalid.state, down({ pointerId: 2 }))
  assert.equal(stillSuppressed.state.kind, 'idle')
  assert.equal(
    (stillSuppressed.state as { requiresReset: boolean }).requiresReset,
    true,
  )
  const reset = reduceFoldPreviewAngleDrag(stillSuppressed.state, {
    kind: 'reset',
    reason: 'reset',
  })
  assert.deepEqual(reset.state, createFoldPreviewAngleDragState())
})

test('reset, blur, and dispose explicitly cancel active input', () => {
  for (const reason of ['reset', 'window_blur', 'dispose'] as const) {
    const result = reduceFoldPreviewAngleDrag(start(down()), {
      kind: 'reset',
      reason,
    })
    assert.deepEqual(result.state, createFoldPreviewAngleDragState())
    assert.deepEqual(result.effects, [
      { kind: 'cancel', pointerId: 1, reason },
    ])
  }
})

test('malformed state and events fail closed to immutable idle', () => {
  const mutableState = {
    kind: 'armed',
    pointerId: 1,
    pointerType: 'mouse',
    startClientX: 0,
    startClientY: 0,
    startAppliedAngle: 50,
    degreesPerPixel: 1,
    thresholdPixels: 6,
  } as FoldPreviewAngleDragState
  const malformedState = reduceFoldPreviewAngleDrag(mutableState, move())
  assert.deepEqual(malformedState.state, createFoldPreviewAngleDragState())
  assert.deepEqual(malformedState.effects, [])

  const malformedEvent = reduceFoldPreviewAngleDrag(
    start(down()),
    { kind: 'unknown' } as never,
  )
  assert.deepEqual(malformedEvent.state, createFoldPreviewAngleDragState())
  assert.deepEqual(malformedEvent.effects, [
    { kind: 'cancel', pointerId: 1, reason: 'invalid_sample' },
  ])
})

test('states, transitions, nested arrays, and every effect are frozen', () => {
  const initial = createFoldPreviewAngleDragState()
  assert.ok(Object.isFrozen(initial))
  assert.equal(initial.kind, 'idle')
  if (initial.kind !== 'idle') assert.fail('expected an idle initial state')
  assert.ok(Object.isFrozen(initial.suppressedPointerIds))

  const armed = reduceFoldPreviewAngleDrag(initial, down())
  assert.ok(Object.isFrozen(armed))
  assert.ok(Object.isFrozen(armed.state))
  assert.ok(Object.isFrozen(armed.effects))
  assert.ok(armed.effects.every(Object.isFrozen))

  const dragging = reduceFoldPreviewAngleDrag(armed.state, move({ clientY: 90 }))
  assert.ok(Object.isFrozen(dragging))
  assert.ok(Object.isFrozen(dragging.state))
  assert.ok(Object.isFrozen(dragging.effects))
  assert.ok(dragging.effects.every(Object.isFrozen))

  const suppressed = reduceFoldPreviewAngleDrag(
    dragging.state,
    down({ pointerId: 2, pointerType: 'touch', isPrimary: false }),
  )
  assert.ok(Object.isFrozen(suppressed.state))
  assert.equal(suppressed.state.kind, 'idle')
  if (suppressed.state.kind !== 'idle') assert.fail('expected a suppressed idle state')
  assert.ok(Object.isFrozen(suppressed.state.suppressedPointerIds))
})

function start(event: Extract<FoldPreviewAngleDragEvent, { kind: 'pointer_down' }>) {
  return reduceFoldPreviewAngleDrag(createFoldPreviewAngleDragState(), event).state
}

function screenHit() {
  return {
    viewport: { left: 0, top: 0, width: 200, height: 200 },
    pointer: { clientX: 100, clientY: 100 },
    startNdc: { x: -0.5, y: 0, z: 0 },
    endNdc: { x: 0.5, y: 0, z: 0 },
    minimumLengthPixels: 12,
    maximumDistancePixels: 12,
  }
}

function down(
  overrides: Partial<Extract<
    FoldPreviewAngleDragEvent,
    { kind: 'pointer_down' }
  >> = {},
): Extract<FoldPreviewAngleDragEvent, { kind: 'pointer_down' }> {
  return {
    kind: 'pointer_down',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 100,
    button: 0,
    isPrimary: true,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    hadActivePointer: false,
    appliedAngle: 52.04,
    viewportHeight: 244,
    ...overrides,
  }
}

function move(
  overrides: Partial<Extract<
    FoldPreviewAngleDragEvent,
    { kind: 'pointer_move' }
  >> = {},
): Extract<FoldPreviewAngleDragEvent, { kind: 'pointer_move' }> {
  return {
    kind: 'pointer_move',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 100,
    ...overrides,
  }
}

function up(
  overrides: Partial<Extract<
    FoldPreviewAngleDragEvent,
    { kind: 'pointer_up' }
  >> = {},
): Extract<FoldPreviewAngleDragEvent, { kind: 'pointer_up' }> {
  return {
    kind: 'pointer_up',
    pointerId: 1,
    pointerType: 'mouse',
    clientX: 40,
    clientY: 100,
    ...overrides,
  }
}
