import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  resolveFoldPreviewPhysicalGrabTarget,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabRay,
  type FoldPreviewPhysicalGrabResolveReason,
  type FoldPreviewPhysicalGrabSession,
  type FoldPreviewPhysicalGrabTarget,
} from './foldPreviewPhysicalGrab.ts'

export type FoldPreviewPhysicalGrabGesturePointerType =
  | 'mouse'
  | 'pen'
  | 'touch'

export type FoldPreviewPhysicalGrabGestureState =
  | FoldPreviewPhysicalGrabGestureIdleState
  | FoldPreviewPhysicalGrabGestureArmedState
  | FoldPreviewPhysicalGrabGestureDraggingState

export type FoldPreviewPhysicalGrabGestureIdleState = Readonly<{
  kind: 'idle'
  /**
   * Pointer IDs from a failed multi-pointer or malformed sequence. A fresh
   * physical grab is blocked until every identifiable pointer has ended.
   */
  suppressedPointerIds: readonly number[]
  /** A sample without an identifiable pointer requires an explicit reset. */
  requiresReset: boolean
}>

type FoldPreviewPhysicalGrabGestureActiveState = Readonly<{
  pointerId: number
  pointerType: FoldPreviewPhysicalGrabGesturePointerType
  startClientX: number
  startClientY: number
  thresholdPixels: 6 | 10
  /** Opaque caller snapshot covering camera, runner, model, and view identity. */
  guardKey: string
  contextKey: string
  session: FoldPreviewPhysicalGrabSession
  /**
   * The most recently accepted raw solver angle. It is intentionally not the
   * quantized presentation angle, so side-on branch selection remains stable.
   */
  referenceAngleDegrees: number
}>

export type FoldPreviewPhysicalGrabGestureArmedState =
  FoldPreviewPhysicalGrabGestureActiveState & Readonly<{
    kind: 'armed'
  }>

export type FoldPreviewPhysicalGrabGestureDraggingState =
  FoldPreviewPhysicalGrabGestureActiveState & Readonly<{
    kind: 'dragging'
    /**
     * A rejected move clears this value. An older accepted target is never a
     * fallback for either presentation or completion.
     */
    presentationTarget: FoldPreviewPhysicalGrabTarget | null
  }>

type FoldPreviewPhysicalGrabGesturePointerSample = Readonly<{
  pointerId: number
  pointerType: FoldPreviewPhysicalGrabGesturePointerType
  clientX: number
  clientY: number
}>

type FoldPreviewPhysicalGrabGestureGuardedSample =
  FoldPreviewPhysicalGrabGesturePointerSample & Readonly<{
    guardKey: string
    contextKey: string
    ray: FoldPreviewPhysicalGrabRay
    isInside: boolean
  }>

export type FoldPreviewPhysicalGrabGestureEvent =
  | (
      FoldPreviewPhysicalGrabGesturePointerSample
      & Readonly<{
        kind: 'pointer_down'
        button: number
        buttons: number
        isPrimary: boolean
        altKey: boolean
        ctrlKey: boolean
        metaKey: boolean
        shiftKey: boolean
        hadActivePointer: boolean
        guardKey: string
        contextKey: string
        session: FoldPreviewPhysicalGrabSession
      }>
    )
  | (
      FoldPreviewPhysicalGrabGestureGuardedSample
      & Readonly<{
        kind: 'pointer_move'
        buttons: number
      }>
    )
  | (
      FoldPreviewPhysicalGrabGestureGuardedSample
      & Readonly<{
        kind: 'pointer_up'
        button: number
        buttons: number
      }>
    )
  | Readonly<{
      kind: 'pointer_cancel'
      pointerId: number
      pointerType: FoldPreviewPhysicalGrabGesturePointerType
      reason: 'pointer_cancel' | 'lost_pointer_capture'
    }>
  | Readonly<{
      kind: 'reset'
      reason: 'reset' | 'window_blur' | 'dispose'
    }>

export type FoldPreviewPhysicalGrabGestureCancelReason =
  | 'invalid_sample'
  | 'multiple_pointers'
  | 'pointer_mismatch'
  | 'stale_guard'
  | 'stale_context'
  | 'invalid_ray'
  | 'buttons_changed'
  | 'pointer_outside'
  | 'pointer_cancel'
  | 'lost_pointer_capture'
  | 'reset'
  | 'window_blur'
  | 'dispose'

export type FoldPreviewPhysicalGrabGestureEffect =
  | Readonly<{
      kind: 'handled'
      pointerId: number
    }>
  | Readonly<{
      kind: 'cancel'
      pointerId: number
      reason: FoldPreviewPhysicalGrabGestureCancelReason
    }>
  | Readonly<{
      kind: 'presentation'
      pointerId: number
      target: FoldPreviewPhysicalGrabTarget | null
      rejectionReason: FoldPreviewPhysicalGrabResolveReason | null
    }>
  | Readonly<{
      kind: 'end'
      pointerId: number
      outcome: 'tap' | 'drag'
      /**
       * Present only when the pointer-up ray itself resolves. Consumers must
       * never substitute a target from an earlier presentation effect.
       */
      completionTarget: FoldPreviewPhysicalGrabTarget | null
      rejectionReason: FoldPreviewPhysicalGrabResolveReason | null
    }>

export type FoldPreviewPhysicalGrabGestureTransition = Readonly<{
  state: FoldPreviewPhysicalGrabGestureState
  effects: readonly FoldPreviewPhysicalGrabGestureEffect[]
}>

const MOUSE_AND_PEN_THRESHOLD_PIXELS = 6 as const
const TOUCH_THRESHOLD_PIXELS = 10 as const
const UNIT_TOLERANCE = 1e-10

const EMPTY_POINTER_IDS: readonly number[] = Object.freeze([])
const CLEAN_IDLE_STATE: FoldPreviewPhysicalGrabGestureIdleState = Object.freeze({
  kind: 'idle',
  suppressedPointerIds: EMPTY_POINTER_IDS,
  requiresReset: false,
})
const NO_EFFECTS: readonly FoldPreviewPhysicalGrabGestureEffect[] =
  Object.freeze([])

export function createFoldPreviewPhysicalGrabGestureState(): FoldPreviewPhysicalGrabGestureState {
  return CLEAN_IDLE_STATE
}

/**
 * Reduces physical-grab pointer input without applying a pose, mutating a
 * renderer, or scheduling the continuous-motion runner.
 *
 * `presentation` and `end.completionTarget` are unverified solver targets.
 * The caller remains responsible for sending a completion target through the
 * existing continuous-motion safety runner.
 */
export function reduceFoldPreviewPhysicalGrabGesture(
  state: FoldPreviewPhysicalGrabGestureState,
  event: FoldPreviewPhysicalGrabGestureEvent,
): FoldPreviewPhysicalGrabGestureTransition {
  if (!validState(state)) return transition(CLEAN_IDLE_STATE, NO_EFFECTS)
  if (!validEventKind(event)) {
    return state.kind === 'idle'
      ? transition(CLEAN_IDLE_STATE, NO_EFFECTS)
      : transition(
          suppressedIdle([state.pointerId], true),
          [cancel(state.pointerId, 'invalid_sample')],
        )
  }
  if (state.kind === 'idle') return reduceIdle(state, event)
  return reduceActive(state, event)
}

function reduceIdle(
  state: FoldPreviewPhysicalGrabGestureIdleState,
  event: FoldPreviewPhysicalGrabGestureEvent,
): FoldPreviewPhysicalGrabGestureTransition {
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
  const session = snapshotSession(event.session)
  if (!session || session.contextKey !== event.contextKey) {
    return transition(state, NO_EFFECTS)
  }
  const thresholdPixels = event.pointerType === 'touch'
    ? TOUCH_THRESHOLD_PIXELS
    : MOUSE_AND_PEN_THRESHOLD_PIXELS
  const next: FoldPreviewPhysicalGrabGestureArmedState = Object.freeze({
    kind: 'armed',
    pointerId: event.pointerId,
    pointerType: event.pointerType,
    startClientX: event.clientX,
    startClientY: event.clientY,
    thresholdPixels,
    guardKey: event.guardKey,
    contextKey: event.contextKey,
    session,
    referenceAngleDegrees: session.appliedAngleDegrees,
  })
  return transition(next, [handled(event.pointerId)])
}

function reduceSuppressedIdle(
  state: FoldPreviewPhysicalGrabGestureIdleState,
  event: FoldPreviewPhysicalGrabGestureEvent,
): FoldPreviewPhysicalGrabGestureTransition {
  if (event.kind === 'reset') {
    return validResetReason(event.reason)
      ? transition(CLEAN_IDLE_STATE, NO_EFFECTS)
      : transition(state, NO_EFFECTS)
  }
  if (!validPointerId(event.pointerId)) {
    return transition(
      suppressedIdle(state.suppressedPointerIds, true),
      NO_EFFECTS,
    )
  }
  if (event.kind === 'pointer_down' || event.kind === 'pointer_move') {
    return transition(
      suppressedIdle(
        addPointerId(state.suppressedPointerIds, event.pointerId),
        state.requiresReset,
      ),
      [handled(event.pointerId)],
    )
  }
  const remaining = removePointerId(
    state.suppressedPointerIds,
    event.pointerId,
  )
  const next = remaining.length === 0 && !state.requiresReset
    ? CLEAN_IDLE_STATE
    : suppressedIdle(remaining, state.requiresReset)
  return transition(next, [handled(event.pointerId)])
}

function reduceActive(
  state:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  event: FoldPreviewPhysicalGrabGestureEvent,
): FoldPreviewPhysicalGrabGestureTransition {
  if (event.kind === 'reset') {
    const reason = validResetReason(event.reason)
      ? event.reason
      : 'invalid_sample'
    return transition(
      CLEAN_IDLE_STATE,
      [cancel(state.pointerId, reason)],
    )
  }
  if (event.kind === 'pointer_down') {
    const validId = validPointerId(event.pointerId)
    const pointerIds = validId
      ? addPointerId([state.pointerId], event.pointerId)
      : [state.pointerId]
    return transition(
      suppressedIdle(pointerIds, !validId),
      validId
        ? [handled(event.pointerId), cancel(state.pointerId, 'multiple_pointers')]
        : [cancel(state.pointerId, 'invalid_sample')],
    )
  }
  if (event.kind === 'pointer_cancel') {
    return reducePointerCancel(state, event)
  }
  if (
    !validPointerId(event.pointerId)
    || !validPointerType(event.pointerType)
  ) {
    return failActive(
      state,
      event.kind,
      state.pointerId,
      'invalid_sample',
      true,
    )
  }
  if (
    event.pointerId !== state.pointerId
    || event.pointerType !== state.pointerType
  ) {
    const foreignStillActive = event.kind === 'pointer_move'
    const pointerIds = foreignStillActive
      ? addPointerId([state.pointerId], event.pointerId)
      : [state.pointerId]
    return transition(
      suppressedIdle(pointerIds, false),
      [
        handled(event.pointerId),
        cancel(state.pointerId, 'pointer_mismatch'),
      ],
    )
  }
  if (!validPointerCoordinates(event)) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'invalid_sample',
      false,
    )
  }
  if (!validOpaqueKey(event.guardKey) || event.guardKey !== state.guardKey) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'stale_guard',
      false,
    )
  }
  if (
    !validOpaqueKey(event.contextKey)
    || event.contextKey !== state.contextKey
  ) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'stale_context',
      false,
    )
  }
  if (!validRay(event.ray)) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'invalid_ray',
      false,
    )
  }
  if (event.isInside !== true) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'pointer_outside',
      false,
    )
  }
  const validButtons = event.kind === 'pointer_move'
    ? event.buttons === 1
    : event.button === 0 && event.buttons === 0
  if (!validButtons) {
    return failActive(
      state,
      event.kind,
      event.pointerId,
      'buttons_changed',
      false,
    )
  }

  if (event.kind === 'pointer_move') {
    if (state.kind === 'armed' && !exceedsThreshold(state, event)) {
      return transition(state, [handled(event.pointerId)])
    }
    const advanced = advanceMove(state, event)
    return transition(
      advanced.state,
      [handled(event.pointerId), advanced.effect],
    )
  }
  return finishPointerUp(state, event)
}

function reducePointerCancel(
  state:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  event: Extract<
    FoldPreviewPhysicalGrabGestureEvent,
    { kind: 'pointer_cancel' }
  >,
) {
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
    return transition(
      suppressedIdle([state.pointerId], false),
      [
        handled(event.pointerId),
        cancel(state.pointerId, 'pointer_mismatch'),
      ],
    )
  }
  if (!validPointerCancelReason(event.reason)) {
    return transition(
      CLEAN_IDLE_STATE,
      [
        handled(event.pointerId),
        cancel(event.pointerId, 'invalid_sample'),
      ],
    )
  }
  return transition(
    CLEAN_IDLE_STATE,
    [
      handled(event.pointerId),
      cancel(event.pointerId, event.reason),
    ],
  )
}

function advanceMove(
  state:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  event: Extract<
    FoldPreviewPhysicalGrabGestureEvent,
    { kind: 'pointer_move' }
  >,
): Readonly<{
  state: FoldPreviewPhysicalGrabGestureDraggingState
  effect: FoldPreviewPhysicalGrabGestureEffect
}> {
  const result = resolveFoldPreviewPhysicalGrabTarget(state.session, {
    contextKey: event.contextKey,
    referenceAngleDegrees: state.referenceAngleDegrees,
    ray: event.ray,
  })
  if (result.kind === 'rejected') {
    const next = draggingState(state, state.referenceAngleDegrees, null)
    return Object.freeze({
      state: next,
      effect: presentation(event.pointerId, null, result.reason),
    })
  }
  const stateTarget = snapshotTarget(result)
  const effectTarget = snapshotTarget(result)
  const next = draggingState(
    state,
    stateTarget.rawAngleDegrees,
    stateTarget,
  )
  return Object.freeze({
    state: next,
    effect: presentation(event.pointerId, effectTarget, null),
  })
}

function finishPointerUp(
  state:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  event: Extract<
    FoldPreviewPhysicalGrabGestureEvent,
    { kind: 'pointer_up' }
  >,
) {
  if (state.kind === 'armed' && !exceedsThreshold(state, event)) {
    return transition(
      CLEAN_IDLE_STATE,
      [
        handled(event.pointerId),
        end(event.pointerId, 'tap', null, null),
      ],
    )
  }

  // Completion is intentionally solved from pointer-up itself. No target held
  // by the dragging state participates in this decision.
  const result = resolveFoldPreviewPhysicalGrabTarget(state.session, {
    contextKey: event.contextKey,
    referenceAngleDegrees: state.referenceAngleDegrees,
    ray: event.ray,
  })
  const endEffect = result.kind === 'unverified_target'
    ? end(event.pointerId, 'drag', snapshotTarget(result), null)
    : end(event.pointerId, 'drag', null, result.reason)
  return transition(
    CLEAN_IDLE_STATE,
    [handled(event.pointerId), endEffect],
  )
}

function failActive(
  state:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  eventKind: 'pointer_move' | 'pointer_up',
  handledPointerId: number,
  reason: FoldPreviewPhysicalGrabGestureCancelReason,
  requiresReset: boolean,
) {
  const next = eventKind === 'pointer_up' && !requiresReset
    ? CLEAN_IDLE_STATE
    : suppressedIdle([state.pointerId], requiresReset)
  return transition(
    next,
    [
      handled(handledPointerId),
      cancel(state.pointerId, reason),
    ],
  )
}

function draggingState(
  previous:
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState,
  referenceAngleDegrees: number,
  presentationTarget: FoldPreviewPhysicalGrabTarget | null,
): FoldPreviewPhysicalGrabGestureDraggingState {
  return Object.freeze({
    kind: 'dragging',
    pointerId: previous.pointerId,
    pointerType: previous.pointerType,
    startClientX: previous.startClientX,
    startClientY: previous.startClientY,
    thresholdPixels: previous.thresholdPixels,
    guardKey: previous.guardKey,
    contextKey: previous.contextKey,
    session: previous.session,
    referenceAngleDegrees,
    presentationTarget,
  })
}

function exceedsThreshold(
  state: FoldPreviewPhysicalGrabGestureActiveState,
  sample: FoldPreviewPhysicalGrabGesturePointerSample,
) {
  const deltaX = sample.clientX - state.startClientX
  const deltaY = sample.clientY - state.startClientY
  const distanceSquared = deltaX * deltaX + deltaY * deltaY
  return Number.isFinite(distanceSquared)
    && distanceSquared > state.thresholdPixels * state.thresholdPixels
}

function validStart(
  event: Extract<
    FoldPreviewPhysicalGrabGestureEvent,
    { kind: 'pointer_down' }
  >,
) {
  return validPointerCoordinates(event)
    && event.button === 0
    && event.buttons === 1
    && event.isPrimary === true
    && event.altKey === false
    && event.ctrlKey === false
    && event.metaKey === false
    && event.shiftKey === false
    && event.hadActivePointer === false
    && validOpaqueKey(event.guardKey)
    && validOpaqueKey(event.contextKey)
    && validSessionInput(event.session)
}

function validPointerCoordinates(
  value: unknown,
): boolean {
  if (!value || typeof value !== 'object') return false
  const sample = value as Partial<FoldPreviewPhysicalGrabGesturePointerSample>
  return validPointerId(sample.pointerId)
    && validPointerType(sample.pointerType)
    && Number.isFinite(sample.clientX)
    && Number.isFinite(sample.clientY)
}

function validPointerType(
  value: unknown,
): value is FoldPreviewPhysicalGrabGesturePointerType {
  return value === 'mouse' || value === 'pen' || value === 'touch'
}

function validPointerId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validOpaqueKey(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function validRay(value: unknown): value is FoldPreviewPhysicalGrabRay {
  if (!value || typeof value !== 'object') return false
  const ray = value as Partial<FoldPreviewPhysicalGrabRay>
  if (
    !finitePoint(ray.origin)
    || !finitePoint(ray.direction)
    || !Number.isFinite(ray.minimumDistance)
    || (ray.minimumDistance as number) < 0
    || !validMaximumDistance(ray.maximumDistance)
    || (ray.maximumDistance as number) <= (ray.minimumDistance as number)
  ) return false
  const directionLength = Math.hypot(
    ray.direction.x,
    ray.direction.y,
    ray.direction.z,
  )
  return Number.isFinite(directionLength)
    && Math.abs(directionLength - 1) <= UNIT_TOLERANCE
}

function validSessionInput(
  value: unknown,
): value is FoldPreviewPhysicalGrabSession {
  return validSessionSnapshot(value)
}

function validSessionSnapshot(
  value: unknown,
): value is FoldPreviewPhysicalGrabSession {
  if (!value || typeof value !== 'object' || !Object.isFrozen(value)) {
    return false
  }
  const session = value as Partial<FoldPreviewPhysicalGrabSession>
  return session.mapping === FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
    && validOpaqueKey(session.contextKey)
    && (session.movingRotationSign === 1 || session.movingRotationSign === -1)
    && validAngle(session.appliedAngleDegrees)
    && finiteFrozenPoint(session.axisOrigin)
    && unitFrozenPoint(session.axisUnit)
    && finiteFrozenPoint(session.orbitCenter)
    && unitFrozenPoint(session.restRadialUnit)
    && unitFrozenPoint(session.positiveTangentUnit)
    && Number.isFinite(session.orbitRadius)
    && Number.isFinite(
      (session.orbitRadius as number) * (session.orbitRadius as number),
    )
    && Number.isFinite(session.minimumOrbitRadius)
    && (session.minimumOrbitRadius as number) > 0
    && (session.orbitRadius as number) >= (session.minimumOrbitRadius as number)
    && Number.isFinite(session.rayMinimumDistance)
    && (session.rayMinimumDistance as number) >= 0
    && validMaximumDistance(session.rayMaximumDistance)
    && (session.rayMaximumDistance as number)
      > (session.rayMinimumDistance as number)
}

function snapshotSession(
  session: FoldPreviewPhysicalGrabSession,
): FoldPreviewPhysicalGrabSession | null {
  if (!validSessionInput(session)) return null
  const snapshot: FoldPreviewPhysicalGrabSession = Object.freeze({
    mapping: session.mapping,
    contextKey: session.contextKey,
    movingRotationSign: session.movingRotationSign,
    appliedAngleDegrees: session.appliedAngleDegrees,
    axisOrigin: freezePoint(session.axisOrigin),
    axisUnit: freezePoint(session.axisUnit),
    orbitCenter: freezePoint(session.orbitCenter),
    restRadialUnit: freezePoint(session.restRadialUnit),
    positiveTangentUnit: freezePoint(session.positiveTangentUnit),
    orbitRadius: session.orbitRadius,
    rayMinimumDistance: session.rayMinimumDistance,
    rayMaximumDistance: session.rayMaximumDistance,
    minimumOrbitRadius: session.minimumOrbitRadius,
  })
  return validSessionSnapshot(snapshot) ? snapshot : null
}

function snapshotTarget(
  target: FoldPreviewPhysicalGrabTarget,
): FoldPreviewPhysicalGrabTarget {
  return Object.freeze({
    ...target,
    orbitWorldPoint: freezePoint(target.orbitWorldPoint),
  })
}

function validState(
  value: unknown,
): value is FoldPreviewPhysicalGrabGestureState {
  if (!value || typeof value !== 'object' || !Object.isFrozen(value)) return false
  const state = value as Partial<FoldPreviewPhysicalGrabGestureState>
  if (state.kind === 'idle') {
    return Array.isArray(state.suppressedPointerIds)
      && Object.isFrozen(state.suppressedPointerIds)
      && state.suppressedPointerIds.every(validPointerId)
      && state.suppressedPointerIds.every(
        (pointerId, index) =>
          index === 0
          || pointerId > (state.suppressedPointerIds as readonly number[])[index - 1],
      )
      && typeof state.requiresReset === 'boolean'
  }
  if (state.kind !== 'armed' && state.kind !== 'dragging') return false
  const active = state as Partial<
    | FoldPreviewPhysicalGrabGestureArmedState
    | FoldPreviewPhysicalGrabGestureDraggingState
  >
  if (
    !validPointerId(active.pointerId)
    || !validPointerType(active.pointerType)
    || !Number.isFinite(active.startClientX)
    || !Number.isFinite(active.startClientY)
    || active.thresholdPixels !== (
      active.pointerType === 'touch'
        ? TOUCH_THRESHOLD_PIXELS
        : MOUSE_AND_PEN_THRESHOLD_PIXELS
    )
    || !validOpaqueKey(active.guardKey)
    || !validOpaqueKey(active.contextKey)
    || !validSessionSnapshot(active.session)
    || active.session.contextKey !== active.contextKey
    || !validAngle(active.referenceAngleDegrees)
  ) return false
  if (state.kind === 'armed') {
    return active.referenceAngleDegrees === active.session.appliedAngleDegrees
  }
  const dragging =
    state as Partial<FoldPreviewPhysicalGrabGestureDraggingState>
  return dragging.presentationTarget === null
    || (
      validTargetSnapshot(dragging.presentationTarget)
      && dragging.presentationTarget.contextKey === active.contextKey
      && dragging.presentationTarget.rawAngleDegrees
        === active.referenceAngleDegrees
    )
}

function validTargetSnapshot(
  value: unknown,
): value is FoldPreviewPhysicalGrabTarget {
  if (!value || typeof value !== 'object' || !Object.isFrozen(value)) {
    return false
  }
  const target = value as Partial<FoldPreviewPhysicalGrabTarget>
  return target.kind === 'unverified_target'
    && target.mapping === FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
    && validOpaqueKey(target.contextKey)
    && validAngle(target.angleDegrees)
    && validAngle(target.rawAngleDegrees)
    && (
      target.endpoint === null
      || target.endpoint === 'zero'
      || target.endpoint === 'one_eighty'
    )
    && Number.isFinite(target.missDistance)
    && (target.missDistance as number) >= 0
    && finiteFrozenPoint(target.orbitWorldPoint)
    && Number.isSafeInteger(target.evaluationCount)
    && (target.evaluationCount as number) >= 0
    && Number.isSafeInteger(target.rootEvaluationCount)
    && (target.rootEvaluationCount as number) >= 0
    && Number.isSafeInteger(target.stationaryCandidateCount)
    && (target.stationaryCandidateCount as number) >= 0
    && Number.isSafeInteger(target.boundaryCandidateCount)
    && (target.boundaryCandidateCount as number) >= 0
    && Number.isSafeInteger(target.equivalentCandidateCount)
    && (target.equivalentCandidateCount as number) >= 1
}

function finitePoint(value: unknown): value is FoldPreviewPhysicalGrabPoint {
  if (!value || typeof value !== 'object') return false
  const point = value as Partial<FoldPreviewPhysicalGrabPoint>
  return Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && Number.isFinite(point.z)
}

function finiteFrozenPoint(
  value: unknown,
): value is FoldPreviewPhysicalGrabPoint {
  return finitePoint(value) && Object.isFrozen(value)
}

function unitFrozenPoint(
  value: unknown,
): value is FoldPreviewPhysicalGrabPoint {
  if (!finiteFrozenPoint(value)) return false
  const pointLength = Math.hypot(value.x, value.y, value.z)
  return Number.isFinite(pointLength)
    && Math.abs(pointLength - 1) <= UNIT_TOLERANCE
}

function validAngle(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= 0
    && (value as number) <= 180
}

function validMaximumDistance(value: unknown): value is number {
  return value === Number.POSITIVE_INFINITY || Number.isFinite(value)
}

function freezePoint(
  point: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabPoint {
  return Object.freeze({ x: point.x, y: point.y, z: point.z })
}

function validEventKind(
  value: unknown,
): value is FoldPreviewPhysicalGrabGestureEvent {
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
): value is 'pointer_cancel' | 'lost_pointer_capture' {
  return value === 'pointer_cancel' || value === 'lost_pointer_capture'
}

function validResetReason(
  value: unknown,
): value is 'reset' | 'window_blur' | 'dispose' {
  return value === 'reset' || value === 'window_blur' || value === 'dispose'
}

function suppressedIdle(
  pointerIds: readonly number[],
  requiresReset: boolean,
): FoldPreviewPhysicalGrabGestureIdleState {
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

function handled(
  pointerId: number,
): FoldPreviewPhysicalGrabGestureEffect {
  return Object.freeze({ kind: 'handled', pointerId })
}

function cancel(
  pointerId: number,
  reason: FoldPreviewPhysicalGrabGestureCancelReason,
): FoldPreviewPhysicalGrabGestureEffect {
  return Object.freeze({ kind: 'cancel', pointerId, reason })
}

function presentation(
  pointerId: number,
  target: FoldPreviewPhysicalGrabTarget | null,
  rejectionReason: FoldPreviewPhysicalGrabResolveReason | null,
): FoldPreviewPhysicalGrabGestureEffect {
  return Object.freeze({
    kind: 'presentation',
    pointerId,
    target,
    rejectionReason,
  })
}

function end(
  pointerId: number,
  outcome: 'tap' | 'drag',
  completionTarget: FoldPreviewPhysicalGrabTarget | null,
  rejectionReason: FoldPreviewPhysicalGrabResolveReason | null,
): FoldPreviewPhysicalGrabGestureEffect {
  return Object.freeze({
    kind: 'end',
    pointerId,
    outcome,
    completionTarget,
    rejectionReason,
  })
}

function transition(
  state: FoldPreviewPhysicalGrabGestureState,
  effects: readonly FoldPreviewPhysicalGrabGestureEffect[],
): FoldPreviewPhysicalGrabGestureTransition {
  const frozenEffects = effects === NO_EFFECTS
    ? NO_EFFECTS
    : Object.freeze([...effects])
  return Object.freeze({ state, effects: frozenEffects })
}
