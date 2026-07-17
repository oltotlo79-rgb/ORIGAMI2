export type FoldPreviewAngleDragPointerType = 'mouse' | 'pen' | 'touch'

export const FOLD_PREVIEW_ANGLE_DRAG_MAPPING = 'vertical_parameter_v1' as const

export type FoldPreviewAngleDragScreenHit = Readonly<{
  viewport: Readonly<{
    left: number
    top: number
    width: number
    height: number
  }>
  pointer: Readonly<{ clientX: number; clientY: number }>
  startNdc: Readonly<{ x: number; y: number; z: number }>
  endNdc: Readonly<{ x: number; y: number; z: number }>
  minimumLengthPixels: number
  maximumDistancePixels: number
}>

export type FoldPreviewAngleDragState =
  | FoldPreviewAngleDragIdleState
  | FoldPreviewAngleDragArmedState
  | FoldPreviewAngleDragDraggingState

export type FoldPreviewAngleDragIdleState = Readonly<{
  kind: 'idle'
  /**
   * Pointer IDs consumed after an ambiguous or multi-pointer sequence. A new
   * drag cannot start until every known pointer has ended.
   */
  suppressedPointerIds: readonly number[]
  /** An unidentifiable malformed sample requires an explicit reset. */
  requiresReset: boolean
}>

type FoldPreviewAngleDragActiveState = Readonly<{
  pointerId: number
  pointerType: FoldPreviewAngleDragPointerType
  startClientX: number
  startClientY: number
  startAppliedAngle: number
  degreesPerPixel: number
  thresholdPixels: number
}>

export type FoldPreviewAngleDragArmedState =
  FoldPreviewAngleDragActiveState & Readonly<{
    kind: 'armed'
  }>

export type FoldPreviewAngleDragDraggingState =
  FoldPreviewAngleDragActiveState & Readonly<{
    kind: 'dragging'
    targetAngle: number
  }>

type FoldPreviewAngleDragPointerSample = Readonly<{
  pointerId: number
  pointerType: FoldPreviewAngleDragPointerType
  clientX: number
  clientY: number
}>

export type FoldPreviewAngleDragEvent =
  | (
      FoldPreviewAngleDragPointerSample
      & Readonly<{
        kind: 'pointer_down'
        button: number
        isPrimary: boolean
        altKey: boolean
        ctrlKey: boolean
        metaKey: boolean
        shiftKey: boolean
        hadActivePointer: boolean
        appliedAngle: number
        viewportHeight: number
      }>
    )
  | (
      FoldPreviewAngleDragPointerSample
      & Readonly<{ kind: 'pointer_move' }>
    )
  | (
      FoldPreviewAngleDragPointerSample
      & Readonly<{ kind: 'pointer_up' }>
    )
  | Readonly<{
      kind: 'pointer_cancel'
      pointerId: number
      pointerType: FoldPreviewAngleDragPointerType
      reason: 'pointer_cancel' | 'lost_pointer_capture'
    }>
  | Readonly<{
      kind: 'reset'
      reason: 'reset' | 'window_blur' | 'dispose'
    }>

export type FoldPreviewAngleDragCancelReason =
  | 'invalid_sample'
  | 'multiple_pointers'
  | 'pointer_mismatch'
  | 'horizontal_gesture'
  | 'pointer_cancel'
  | 'lost_pointer_capture'
  | 'reset'
  | 'window_blur'
  | 'dispose'

export type FoldPreviewAngleDragEffect =
  | Readonly<{
      kind: 'handled'
      pointerId: number
    }>
  | Readonly<{
      kind: 'cancel'
      pointerId: number
      reason: FoldPreviewAngleDragCancelReason
    }>
  | Readonly<{
      kind: 'target'
      pointerId: number
      targetAngle: number
    }>
  | Readonly<{
      kind: 'end'
      pointerId: number
      outcome: 'tap'
      targetAngle: null
    }>
  | Readonly<{
      kind: 'end'
      pointerId: number
      outcome: 'drag'
      targetAngle: number
    }>

export type FoldPreviewAngleDragTransition = Readonly<{
  state: FoldPreviewAngleDragState
  effects: readonly FoldPreviewAngleDragEffect[]
}>

const MOUSE_AND_PEN_THRESHOLD_PIXELS = 6
const TOUCH_THRESHOLD_PIXELS = 10
const MIN_ANGLE = 0
const MAX_ANGLE = 180
const MIN_DRAG_HEIGHT = 180
const MAX_DRAG_HEIGHT = 480
const VIEWPORT_VERTICAL_RESERVE = 64

const EMPTY_POINTER_IDS: readonly number[] = Object.freeze([])
const CLEAN_IDLE_STATE: FoldPreviewAngleDragIdleState = Object.freeze({
  kind: 'idle',
  suppressedPointerIds: EMPTY_POINTER_IDS,
  requiresReset: false,
})
const NO_EFFECTS: readonly FoldPreviewAngleDragEffect[] = Object.freeze([])

export function createFoldPreviewAngleDragState(): FoldPreviewAngleDragState {
  return CLEAN_IDLE_STATE
}

/**
 * Rechecks a projected hinge in CSS pixels. World-space ray thresholds alone
 * are camera-distance dependent and cannot safely identify a usable drag line.
 */
export function isFoldPreviewAngleDragScreenHit(
  input: FoldPreviewAngleDragScreenHit,
) {
  if (!input || typeof input !== 'object') return false
  const {
    viewport,
    pointer,
    startNdc,
    endNdc,
    minimumLengthPixels,
    maximumDistancePixels,
  } = input
  if (
    !finiteRecord(viewport, ['left', 'top', 'width', 'height'])
    || viewport.width <= 0
    || viewport.height <= 0
    || !finiteRecord(pointer, ['clientX', 'clientY'])
    || !finiteRecord(startNdc, ['x', 'y', 'z'])
    || !finiteRecord(endNdc, ['x', 'y', 'z'])
    || startNdc.z < -1
    || startNdc.z > 1
    || endNdc.z < -1
    || endNdc.z > 1
    || !Number.isFinite(minimumLengthPixels)
    || minimumLengthPixels <= 0
    || !Number.isFinite(maximumDistancePixels)
    || maximumDistancePixels < 0
  ) return false
  const right = viewport.left + viewport.width
  const bottom = viewport.top + viewport.height
  if (
    !Number.isFinite(right)
    || !Number.isFinite(bottom)
    || pointer.clientX < viewport.left
    || pointer.clientX > right
    || pointer.clientY < viewport.top
    || pointer.clientY > bottom
  ) return false
  const startX = viewport.left + (startNdc.x + 1) * viewport.width / 2
  const startY = viewport.top + (1 - startNdc.y) * viewport.height / 2
  const endX = viewport.left + (endNdc.x + 1) * viewport.width / 2
  const endY = viewport.top + (1 - endNdc.y) * viewport.height / 2
  const deltaX = endX - startX
  const deltaY = endY - startY
  const lengthSquared = deltaX * deltaX + deltaY * deltaY
  if (
    !Number.isFinite(lengthSquared)
    || lengthSquared < minimumLengthPixels * minimumLengthPixels
  ) return false
  const projection = (
    (pointer.clientX - startX) * deltaX
    + (pointer.clientY - startY) * deltaY
  ) / lengthSquared
  if (!Number.isFinite(projection)) return false
  const fraction = Math.min(1, Math.max(0, projection))
  const distance = Math.hypot(
    pointer.clientX - (startX + deltaX * fraction),
    pointer.clientY - (startY + deltaY * fraction),
  )
  return Number.isFinite(distance) && distance <= maximumDistancePixels
}

/**
 * Reduces parameter-drag input without applying or rendering a fold pose.
 *
 * Consumers may translate `target` effects into requested angles, but those
 * requests must still pass through the continuous-motion runner before any
 * displayed angle changes.
 */
export function reduceFoldPreviewAngleDrag(
  state: FoldPreviewAngleDragState,
  event: FoldPreviewAngleDragEvent,
): FoldPreviewAngleDragTransition {
  if (!validState(state)) return transition(CLEAN_IDLE_STATE, NO_EFFECTS)
  if (!validEventKind(event)) {
    return state.kind === 'idle'
      ? transition(CLEAN_IDLE_STATE, NO_EFFECTS)
      : transition(
          CLEAN_IDLE_STATE,
          [cancel(state.pointerId, 'invalid_sample')],
        )
  }
  if (state.kind === 'idle') return reduceIdle(state, event)
  return reduceActive(state, event)
}

function reduceIdle(
  state: FoldPreviewAngleDragIdleState,
  event: FoldPreviewAngleDragEvent,
): FoldPreviewAngleDragTransition {
  if (event.kind === 'reset') {
    return validResetReason(event.reason)
      ? transition(CLEAN_IDLE_STATE, NO_EFFECTS)
      : transition(state, NO_EFFECTS)
  }
  if (state.requiresReset || state.suppressedPointerIds.length > 0) {
    return reduceSuppressedIdle(state, event)
  }
  if (event.kind !== 'pointer_down' || !validStart(event)) {
    return transition(state, NO_EFFECTS)
  }
  const degreesPerPixel = degreesPerPixelFor(event.viewportHeight)
  if (degreesPerPixel === null) return transition(state, NO_EFFECTS)
  const thresholdPixels = event.pointerType === 'touch'
    ? TOUCH_THRESHOLD_PIXELS
    : MOUSE_AND_PEN_THRESHOLD_PIXELS
  const next: FoldPreviewAngleDragArmedState = Object.freeze({
    kind: 'armed',
    pointerId: event.pointerId,
    pointerType: event.pointerType,
    startClientX: event.clientX,
    startClientY: event.clientY,
    startAppliedAngle: event.appliedAngle,
    degreesPerPixel,
    thresholdPixels,
  })
  return transition(next, [handled(event.pointerId)])
}

function reduceSuppressedIdle(
  state: FoldPreviewAngleDragIdleState,
  event: FoldPreviewAngleDragEvent,
): FoldPreviewAngleDragTransition {
  if (event.kind === 'reset') {
    return validResetReason(event.reason)
      ? transition(CLEAN_IDLE_STATE, NO_EFFECTS)
      : transition(state, NO_EFFECTS)
  }
  if (!validPointerId(event.pointerId)) {
    return transition(suppressedIdle(state.suppressedPointerIds, true), NO_EFFECTS)
  }
  if (event.kind === 'pointer_down') {
    return transition(
      suppressedIdle(addPointerId(state.suppressedPointerIds, event.pointerId), state.requiresReset),
      [handled(event.pointerId)],
    )
  }
  if (event.kind === 'pointer_move') {
    const pointerIds = addPointerId(state.suppressedPointerIds, event.pointerId)
    return transition(
      suppressedIdle(pointerIds, state.requiresReset),
      [handled(event.pointerId)],
    )
  }
  const remaining = removePointerId(state.suppressedPointerIds, event.pointerId)
  const next = remaining.length === 0 && !state.requiresReset
    ? CLEAN_IDLE_STATE
    : suppressedIdle(remaining, state.requiresReset)
  return transition(next, [handled(event.pointerId)])
}

function reduceActive(
  state: FoldPreviewAngleDragArmedState | FoldPreviewAngleDragDraggingState,
  event: FoldPreviewAngleDragEvent,
): FoldPreviewAngleDragTransition {
  if (event.kind === 'reset') {
    if (!validResetReason(event.reason)) {
      return transition(
        CLEAN_IDLE_STATE,
        [cancel(state.pointerId, 'invalid_sample')],
      )
    }
    return transition(
      CLEAN_IDLE_STATE,
      [cancel(state.pointerId, event.reason)],
    )
  }
  if (event.kind === 'pointer_down') {
    const pointerIds = validPointerId(event.pointerId)
      ? addPointerId([state.pointerId], event.pointerId)
      : [state.pointerId]
    return transition(
      suppressedIdle(pointerIds, !validPointerId(event.pointerId)),
      validPointerId(event.pointerId)
        ? [handled(event.pointerId), cancel(state.pointerId, 'multiple_pointers')]
        : [cancel(state.pointerId, 'invalid_sample')],
    )
  }
  if (
    !validPointerId(event.pointerId)
    || !validPointerType(event.pointerType)
  ) {
    return transition(
      suppressedIdle([state.pointerId], true),
      [cancel(state.pointerId, 'invalid_sample')],
    )
  }
  if (
    event.pointerId !== state.pointerId
    || event.pointerType !== state.pointerType
  ) {
    const foreignPointerStillActive = event.kind === 'pointer_move'
    const pointerIds = foreignPointerStillActive
      ? addPointerId([state.pointerId], event.pointerId)
      : [state.pointerId]
    return transition(
      suppressedIdle(pointerIds, false),
      [handled(event.pointerId), cancel(state.pointerId, 'pointer_mismatch')],
    )
  }
  if (event.kind === 'pointer_cancel') {
    if (!validPointerCancelReason(event.reason)) {
      return transition(
        suppressedIdle([state.pointerId], true),
        [handled(event.pointerId), cancel(event.pointerId, 'invalid_sample')],
      )
    }
    return transition(
      CLEAN_IDLE_STATE,
      [handled(event.pointerId), cancel(event.pointerId, event.reason)],
    )
  }
  if (!validPointerSample(event)) {
    return transition(
      suppressedIdle([state.pointerId], false),
      [handled(state.pointerId), cancel(state.pointerId, 'invalid_sample')],
    )
  }
  if (
    state.kind === 'armed'
    && exceedsThreshold(state, event)
    && horizontalMotionDominates(state, event)
  ) {
    return transition(
      event.kind === 'pointer_up'
        ? CLEAN_IDLE_STATE
        : suppressedIdle([state.pointerId], false),
      [
        handled(event.pointerId),
        cancel(event.pointerId, 'horizontal_gesture'),
      ],
    )
  }
  if (event.kind === 'pointer_move') {
    const advanced = advance(state, event)
    return transition(
      advanced.state,
      [handled(event.pointerId), ...advanced.effects],
    )
  }

  const advanced = advance(state, event)
  const endEffect: FoldPreviewAngleDragEffect = advanced.state.kind === 'dragging'
    ? Object.freeze({
        kind: 'end',
        pointerId: event.pointerId,
        outcome: 'drag',
        targetAngle: advanced.state.targetAngle,
      })
    : Object.freeze({
        kind: 'end',
        pointerId: event.pointerId,
        outcome: 'tap',
        targetAngle: null,
      })
  return transition(
    CLEAN_IDLE_STATE,
    [handled(event.pointerId), ...advanced.effects, endEffect],
  )
}

function horizontalMotionDominates(
  state: FoldPreviewAngleDragActiveState,
  sample: FoldPreviewAngleDragPointerSample,
) {
  return Math.abs(sample.clientX - state.startClientX)
    > Math.abs(sample.clientY - state.startClientY)
}

function advance(
  state: FoldPreviewAngleDragArmedState | FoldPreviewAngleDragDraggingState,
  sample: FoldPreviewAngleDragPointerSample,
): Readonly<{
  state: FoldPreviewAngleDragArmedState | FoldPreviewAngleDragDraggingState
  effects: readonly FoldPreviewAngleDragEffect[]
}> {
  if (state.kind === 'armed' && !exceedsThreshold(state, sample)) {
    return Object.freeze({ state, effects: NO_EFFECTS })
  }
  const targetAngle = targetFor(state, sample.clientY)
  if (state.kind === 'dragging' && targetAngle === state.targetAngle) {
    return Object.freeze({ state, effects: NO_EFFECTS })
  }
  const next: FoldPreviewAngleDragDraggingState = Object.freeze({
    kind: 'dragging',
    pointerId: state.pointerId,
    pointerType: state.pointerType,
    startClientX: state.startClientX,
    startClientY: state.startClientY,
    startAppliedAngle: state.startAppliedAngle,
    degreesPerPixel: state.degreesPerPixel,
    thresholdPixels: state.thresholdPixels,
    targetAngle,
  })
  const effects = Object.freeze([
    target(state.pointerId, targetAngle),
  ])
  return Object.freeze({ state: next, effects })
}

function targetFor(
  state: FoldPreviewAngleDragActiveState,
  clientY: number,
) {
  const raw = state.startAppliedAngle
    + (state.startClientY - clientY) * state.degreesPerPixel
  const bounded = Math.min(MAX_ANGLE, Math.max(MIN_ANGLE, raw))
  const rounded = Math.round(bounded * 10) / 10
  return Object.is(rounded, -0) ? 0 : rounded
}

function exceedsThreshold(
  state: FoldPreviewAngleDragActiveState,
  sample: FoldPreviewAngleDragPointerSample,
) {
  const deltaX = sample.clientX - state.startClientX
  const deltaY = sample.clientY - state.startClientY
  const distanceSquared = deltaX * deltaX + deltaY * deltaY
  return Number.isFinite(distanceSquared)
    && distanceSquared > state.thresholdPixels * state.thresholdPixels
}

function degreesPerPixelFor(viewportHeight: number) {
  if (!Number.isFinite(viewportHeight) || viewportHeight <= 0) return null
  const activeHeight = Math.min(
    MAX_DRAG_HEIGHT,
    Math.max(MIN_DRAG_HEIGHT, viewportHeight - VIEWPORT_VERTICAL_RESERVE),
  )
  const value = MAX_ANGLE / activeHeight
  return Number.isFinite(value) && value > 0 ? value : null
}

function validStart(
  event: Extract<FoldPreviewAngleDragEvent, { kind: 'pointer_down' }>,
) {
  return validPointerSample(event)
    && event.button === 0
    && event.isPrimary === true
    && event.altKey === false
    && event.ctrlKey === false
    && event.metaKey === false
    && event.shiftKey === false
    && event.hadActivePointer === false
    && validAngle(event.appliedAngle)
}

function validPointerSample(value: unknown): value is FoldPreviewAngleDragPointerSample {
  if (!value || typeof value !== 'object') return false
  const sample = value as Partial<FoldPreviewAngleDragPointerSample>
  return validPointerId(sample.pointerId)
    && validPointerType(sample.pointerType)
    && Number.isFinite(sample.clientX)
    && Number.isFinite(sample.clientY)
}

function validPointerType(value: unknown): value is FoldPreviewAngleDragPointerType {
  return value === 'mouse' || value === 'pen' || value === 'touch'
}

function validPointerId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validAngle(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= MIN_ANGLE
    && (value as number) <= MAX_ANGLE
}

function finiteRecord<Key extends string>(
  value: unknown,
  keys: readonly Key[],
): value is Record<Key, number> {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return keys.every((key) => Number.isFinite(record[key]))
}

function validEventKind(value: unknown): value is FoldPreviewAngleDragEvent {
  if (!value || typeof value !== 'object') return false
  const kind = (value as { kind?: unknown }).kind
  return kind === 'pointer_down'
    || kind === 'pointer_move'
    || kind === 'pointer_up'
    || kind === 'pointer_cancel'
    || kind === 'reset'
}

function validPointerCancelReason(
  value: unknown,
): value is Extract<
  FoldPreviewAngleDragCancelReason,
  'pointer_cancel' | 'lost_pointer_capture'
> {
  return value === 'pointer_cancel' || value === 'lost_pointer_capture'
}

function validResetReason(
  value: unknown,
): value is Extract<
  FoldPreviewAngleDragCancelReason,
  'reset' | 'window_blur' | 'dispose'
> {
  return value === 'reset' || value === 'window_blur' || value === 'dispose'
}

function validState(value: unknown): value is FoldPreviewAngleDragState {
  if (!value || typeof value !== 'object' || !Object.isFrozen(value)) return false
  const state = value as Partial<FoldPreviewAngleDragState>
  if (state.kind === 'idle') {
    return Array.isArray(state.suppressedPointerIds)
      && Object.isFrozen(state.suppressedPointerIds)
      && state.suppressedPointerIds.every(validPointerId)
      && new Set(state.suppressedPointerIds).size === state.suppressedPointerIds.length
      && typeof state.requiresReset === 'boolean'
  }
  if (state.kind !== 'armed' && state.kind !== 'dragging') return false
  if (
    !validPointerId(state.pointerId)
    || !validPointerType(state.pointerType)
    || !Number.isFinite(state.startClientX)
    || !Number.isFinite(state.startClientY)
    || !validAngle(state.startAppliedAngle)
    || !Number.isFinite(state.degreesPerPixel)
    || (state.degreesPerPixel as number) <= 0
    || (
      state.thresholdPixels !== MOUSE_AND_PEN_THRESHOLD_PIXELS
      && state.thresholdPixels !== TOUCH_THRESHOLD_PIXELS
    )
  ) return false
  return state.kind !== 'dragging' || validAngle(state.targetAngle)
}

function suppressedIdle(
  pointerIds: readonly number[],
  requiresReset: boolean,
): FoldPreviewAngleDragIdleState {
  const normalizedIds = Object.freeze(
    [...new Set(pointerIds.filter(validPointerId))].sort((a, b) => a - b),
  )
  if (normalizedIds.length === 0 && !requiresReset) return CLEAN_IDLE_STATE
  return Object.freeze({
    kind: 'idle',
    suppressedPointerIds: normalizedIds,
    requiresReset,
  })
}

function addPointerId(pointerIds: readonly number[], pointerId: number) {
  return pointerIds.includes(pointerId)
    ? pointerIds
    : [...pointerIds, pointerId]
}

function removePointerId(pointerIds: readonly number[], pointerId: number) {
  return pointerIds.filter((current) => current !== pointerId)
}

function handled(pointerId: number): FoldPreviewAngleDragEffect {
  return Object.freeze({ kind: 'handled', pointerId })
}

function cancel(
  pointerId: number,
  reason: FoldPreviewAngleDragCancelReason,
): FoldPreviewAngleDragEffect {
  return Object.freeze({ kind: 'cancel', pointerId, reason })
}

function target(
  pointerId: number,
  targetAngle: number,
): FoldPreviewAngleDragEffect {
  return Object.freeze({ kind: 'target', pointerId, targetAngle })
}

function transition(
  state: FoldPreviewAngleDragState,
  effects: readonly FoldPreviewAngleDragEffect[],
): FoldPreviewAngleDragTransition {
  const frozenEffects = effects === NO_EFFECTS
    ? NO_EFFECTS
    : Object.freeze([...effects])
  return Object.freeze({ state, effects: frozenEffects })
}
