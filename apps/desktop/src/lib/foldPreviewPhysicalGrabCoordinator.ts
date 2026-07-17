import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  type FoldPreviewPhysicalGrabTarget,
} from './foldPreviewPhysicalGrab.ts'
import {
  createFoldPreviewPhysicalGrabGestureState,
  type FoldPreviewPhysicalGrabGestureEffect,
  type FoldPreviewPhysicalGrabGestureState,
  type FoldPreviewPhysicalGrabGestureTransition,
} from './foldPreviewPhysicalGrabGesture.ts'
import {
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
} from './foldPreviewTreeMotionContext.ts'

export const MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_ID_LENGTH = 1_024
export const MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_KEY_LENGTH =
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH
export const MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_EFFECTS = 64

export type FoldPreviewPhysicalGrabGuardSnapshot<RunnerState> = Readonly<{
  guardKey: string | null
  startedRunnerState: RunnerState | null
  currentRunnerState: RunnerState | null
  startedViewKey: string | null
  currentViewKey: string | null
  activeContextKey: string | null
  renderedContextKey: string | null
  latestContextKey: string | null
}>

export type FoldPreviewPhysicalGrabCommand =
  | Readonly<{
      kind: 'queue_presentation'
    }>
  | Readonly<{
      kind: 'select_hinge'
      hingeId: string
    }>
  | Readonly<{
      kind: 'discard_presentation'
    }>
  | Readonly<{
      kind: 'release_capture'
      pointerId: number
    }>
  | Readonly<{
      kind: 'clear_interaction'
      clearEventSession: boolean
    }>
  | Readonly<{
      kind: 'restore_camera'
    }>
  | Readonly<{
      kind: 'sync_presentation'
    }>
  | Readonly<{
      kind: 'request_fold_angle'
      hingeEdgeId: string
      contextKey: string
      angleDegrees: number
    }>

export type FoldPreviewPhysicalGrabTransitionPlanInput = Readonly<{
  transition: FoldPreviewPhysicalGrabGestureTransition
  eventPointerId: number | null
  selectedHingeId: string | null
  activeHingeId: string | null
  modelHingeId: string | null
  activeContextKey: string | null
  disposed: boolean
  guardIsCurrent: boolean
}>

export type FoldPreviewPhysicalGrabTransitionPlan = Readonly<{
  state: FoldPreviewPhysicalGrabGestureState
  handled: boolean
  endedAsTap: boolean
  commands: readonly FoldPreviewPhysicalGrabCommand[]
}>

/**
 * Resolves the opaque guard key only while every external identity captured
 * at pointer-down is still current. Runner identity is intentionally checked
 * by reference so any published motion state invalidates the gesture.
 */
export function currentFoldPreviewPhysicalGrabGuardKey<RunnerState>(
  snapshot: FoldPreviewPhysicalGrabGuardSnapshot<RunnerState>,
): string | null {
  try {
    if (!isRecord(snapshot)) return null
    const guardKey = snapshot.guardKey
    const startedRunnerState = snapshot.startedRunnerState
    const currentRunnerState = snapshot.currentRunnerState
    const startedViewKey = snapshot.startedViewKey
    const currentViewKey = snapshot.currentViewKey
    const activeContextKey = snapshot.activeContextKey
    const renderedContextKey = snapshot.renderedContextKey
    const latestContextKey = snapshot.latestContextKey
    if (
      !validCoordinatorKey(guardKey)
      || !isIdentityObject(startedRunnerState)
      || !validCoordinatorKey(startedViewKey)
      || !validCoordinatorKey(currentViewKey)
      || !validCoordinatorKey(activeContextKey)
      || !validCoordinatorKey(renderedContextKey)
      || !validCoordinatorKey(latestContextKey)
      || activeContextKey !== renderedContextKey
      || activeContextKey !== latestContextKey
      || currentRunnerState !== startedRunnerState
      || currentViewKey !== startedViewKey
    ) return null
    return guardKey
  } catch {
    return null
  }
}

/**
 * Converts one reducer transition into an ordered, DOM-independent command
 * plan. The caller executes these commands in order through its WebView
 * adapters. In particular, a verified completion request is always last.
 */
export function planFoldPreviewPhysicalGrabTransition(
  input: FoldPreviewPhysicalGrabTransitionPlanInput,
): FoldPreviewPhysicalGrabTransitionPlan {
  try {
    return planTransition(input)
  } catch {
    return invalidTransitionPlan()
  }
}

function planTransition(
  input: FoldPreviewPhysicalGrabTransitionPlanInput,
): FoldPreviewPhysicalGrabTransitionPlan {
  if (!isRecord(input)) return invalidTransitionPlan()
  const transition = input.transition
  const eventPointerId = input.eventPointerId
  const selectedHingeId = input.selectedHingeId
  const activeHingeId = input.activeHingeId
  const modelHingeId = input.modelHingeId
  const activeContextKey = input.activeContextKey
  const rawDisposed = input.disposed
  const rawGuardIsCurrent = input.guardIsCurrent
  const disposed = typeof rawDisposed === 'boolean'
    ? rawDisposed
    : true
  const guardIsCurrent = typeof rawGuardIsCurrent === 'boolean'
    ? rawGuardIsCurrent
    : false
  if (!isRecord(transition)) return invalidTransitionPlan()
  const state = transition.state
  const rawEffects = transition.effects
  const stateKind = snapshotStateKind(state)
  const cleanState = isCleanPhysicalGrabState(state, stateKind)
  if (!Array.isArray(rawEffects)) return invalidTransitionPlan()
  const effectCount = rawEffects.length
  if (
    !Number.isSafeInteger(effectCount)
    || effectCount < 0
    || effectCount > MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_EFFECTS
  ) return invalidTransitionPlan()
  const effects: FoldPreviewPhysicalGrabGestureEffectSnapshot[] = []
  for (let effectIndex = 0; effectIndex < effectCount; effectIndex += 1) {
    const effect = snapshotEffect(rawEffects[effectIndex])
    if (!effect) return invalidTransitionPlan()
    effects.push(effect)
  }
  const commands: FoldPreviewPhysicalGrabCommand[] = []
  let handled = false
  let endedAsTap = false
  let queuedPresentation = false
  let clearedInteraction = false
  let completionRequest: Extract<
    FoldPreviewPhysicalGrabCommand,
    { kind: 'request_fold_angle' }
  > | null = null
  const terminalEffectCount = effects.reduce(
    (count, effect) =>
      count + Number(effect.kind === 'cancel' || effect.kind === 'end'),
    0,
  )

  for (const effect of effects) {
    if (effect.kind === 'handled') {
      if (effect.pointerId === eventPointerId) handled = true
      continue
    }
    if (effect.kind === 'presentation') {
      queuedPresentation = true
      commands.push(command({ kind: 'queue_presentation' }))
      if (
        effect.target !== null
        && validCoordinatorId(activeHingeId)
        && selectedHingeId !== activeHingeId
      ) {
        commands.push(command({
          kind: 'select_hinge',
          hingeId: activeHingeId,
        }))
      }
      continue
    }

    commands.push(command({ kind: 'discard_presentation' }))
    commands.push(command({
      kind: 'release_capture',
      pointerId: effect.pointerId,
    }))
    commands.push(command({
      kind: 'clear_interaction',
      clearEventSession: cleanState,
    }))
    clearedInteraction = true

    if (effect.kind === 'cancel') continue
    endedAsTap = effect.outcome === 'tap'
    if (
      effect.outcome === 'drag'
      && effect.completionTarget !== null
      && effect.rejectionReason === null
      && cleanState
      && terminalEffectCount === 1
      && authenticCompletionSequence(
        effects,
        effect,
        eventPointerId,
      )
    ) {
      completionRequest = resolveCompletionRequest(
        effect.completionTarget,
        {
          activeHingeId,
          modelHingeId,
          activeContextKey,
          disposed,
          guardIsCurrent,
        },
      )
    }
  }

  if (cleanState && !clearedInteraction) {
    commands.push(command({
      kind: 'clear_interaction',
      clearEventSession: true,
    }))
  }

  commands.push(command({ kind: 'restore_camera' }))
  if (!queuedPresentation || stateKind !== 'dragging') {
    commands.push(command({ kind: 'sync_presentation' }))
  }
  if (completionRequest !== null) {
    commands.push(completionRequest)
  }

  return Object.freeze({
    state,
    handled,
    endedAsTap,
    commands: Object.freeze(commands),
  })
}

type FoldPreviewPhysicalGrabCompletionIdentity = Readonly<{
  activeHingeId: string | null
  modelHingeId: string | null
  activeContextKey: string | null
  disposed: boolean
  guardIsCurrent: boolean
}>

function resolveCompletionRequest(
  target: FoldPreviewPhysicalGrabTarget,
  identity: FoldPreviewPhysicalGrabCompletionIdentity,
): Extract<
  FoldPreviewPhysicalGrabCommand,
  { kind: 'request_fold_angle' }
  > | null {
  try {
    if (!Object.isFrozen(target)) return null
    const targetKind = target.kind
    const mapping = target.mapping
    const targetContextKey = target.contextKey
    const angleDegrees = target.angleDegrees
    const rawAngleDegrees = target.rawAngleDegrees
    const endpoint = target.endpoint
    const missDistance = target.missDistance
    const orbitWorldPoint = target.orbitWorldPoint
    const evaluationCount = target.evaluationCount
    const rootEvaluationCount = target.rootEvaluationCount
    const stationaryCandidateCount = target.stationaryCandidateCount
    const boundaryCandidateCount = target.boundaryCandidateCount
    const equivalentCandidateCount = target.equivalentCandidateCount
    if (
      !isRecord(orbitWorldPoint)
      || !Object.isFrozen(orbitWorldPoint)
    ) return null
    const orbitWorldX = orbitWorldPoint.x
    const orbitWorldY = orbitWorldPoint.y
    const orbitWorldZ = orbitWorldPoint.z
    const {
      activeHingeId,
      modelHingeId,
      activeContextKey,
      disposed,
      guardIsCurrent,
    } = identity
    if (
      disposed
      || !validCoordinatorId(activeHingeId)
      || !validCoordinatorId(modelHingeId)
      || activeHingeId !== modelHingeId
      || targetKind !== 'unverified_target'
      || mapping !== FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
      || !validCoordinatorKey(targetContextKey)
      || !validCoordinatorKey(activeContextKey)
      || targetContextKey !== activeContextKey
      || !guardIsCurrent
      || !isFoldPreviewAngle(angleDegrees)
      || !isFoldPreviewAngle(rawAngleDegrees)
      || (
        endpoint !== null
        && endpoint !== 'zero'
        && endpoint !== 'one_eighty'
      )
      || !isNonnegativeFiniteNumber(missDistance)
      || !isFiniteNumber(orbitWorldX)
      || !isFiniteNumber(orbitWorldY)
      || !isFiniteNumber(orbitWorldZ)
      || !isNonnegativeSafeInteger(evaluationCount)
      || !isNonnegativeSafeInteger(rootEvaluationCount)
      || !isNonnegativeSafeInteger(stationaryCandidateCount)
      || !isNonnegativeSafeInteger(boundaryCandidateCount)
      || !isPositiveSafeInteger(equivalentCandidateCount)
    ) return null
    return command({
      kind: 'request_fold_angle',
      hingeEdgeId: activeHingeId,
      contextKey: activeContextKey,
      angleDegrees,
    })
  } catch {
    return null
  }
}

type FoldPreviewPhysicalGrabGestureEffectSnapshot =
  | Readonly<{ kind: 'handled'; pointerId: number }>
  | Readonly<{
      kind: 'presentation'
      target: FoldPreviewPhysicalGrabTarget | null
    }>
  | Readonly<{ kind: 'cancel'; pointerId: number }>
  | Readonly<{
      kind: 'end'
      pointerId: number
      outcome: 'tap' | 'drag'
      completionTarget: FoldPreviewPhysicalGrabTarget | null
      rejectionReason: unknown
    }>

function snapshotEffect(
  effect: FoldPreviewPhysicalGrabGestureEffect,
): FoldPreviewPhysicalGrabGestureEffectSnapshot | null {
  if (!isRecord(effect)) return null
  const kind = effect.kind
  if (kind === 'handled') {
    const pointerId = effect.pointerId
    if (!validPointerId(pointerId)) return null
    return { kind, pointerId }
  }
  if (kind === 'presentation') {
    const target = effect.target
    if (target !== null && !isRecord(target)) return null
    return { kind, target }
  }
  if (kind === 'cancel') {
    const pointerId = effect.pointerId
    if (!validPointerId(pointerId)) return null
    return { kind, pointerId }
  }
  if (kind !== 'end') return null
  const pointerId = effect.pointerId
  const outcome = effect.outcome
  const completionTarget = effect.completionTarget
  const rejectionReason = effect.rejectionReason
  if (
    !validPointerId(pointerId)
    || (outcome !== 'tap' && outcome !== 'drag')
    || (completionTarget !== null && !isRecord(completionTarget))
  ) return null
  return {
    kind,
    pointerId,
    outcome,
    completionTarget,
    rejectionReason,
  }
}

function authenticCompletionSequence(
  effects: readonly FoldPreviewPhysicalGrabGestureEffectSnapshot[],
  endEffect: Extract<
    FoldPreviewPhysicalGrabGestureEffectSnapshot,
    { kind: 'end' }
  >,
  eventPointerId: number | null,
) {
  if (
    effects.length !== 2
    || effects[1] !== endEffect
    || effects[0].kind !== 'handled'
  ) return false
  const pointerId = endEffect.pointerId
  return pointerId === eventPointerId
    && effects[0].pointerId === pointerId
}

function isCleanPhysicalGrabState(
  state: FoldPreviewPhysicalGrabGestureState,
  kind: FoldPreviewPhysicalGrabGestureState['kind'],
) {
  if (!Object.isFrozen(state)) {
    throw new TypeError('invalid physical grab state')
  }
  if (kind !== 'idle') return false
  const idleState = state as Extract<
    FoldPreviewPhysicalGrabGestureState,
    { kind: 'idle' }
  >
  const suppressedPointerIds = idleState.suppressedPointerIds
  const requiresReset = idleState.requiresReset
  if (
    !Array.isArray(suppressedPointerIds)
    || !Object.isFrozen(suppressedPointerIds)
    || typeof requiresReset !== 'boolean'
  ) throw new TypeError('invalid physical grab state')
  const suppressedPointerCount = suppressedPointerIds.length
  if (
    !Number.isSafeInteger(suppressedPointerCount)
    || suppressedPointerCount < 0
  ) throw new TypeError('invalid physical grab state')
  return suppressedPointerCount === 0 && !requiresReset
}

function snapshotStateKind(
  state: FoldPreviewPhysicalGrabGestureState,
): FoldPreviewPhysicalGrabGestureState['kind'] {
  if (!isRecord(state)) throw new TypeError('invalid physical grab state')
  const kind = state.kind
  if (kind !== 'idle' && kind !== 'armed' && kind !== 'dragging') {
    throw new TypeError('invalid physical grab state')
  }
  return kind
}

function invalidTransitionPlan(): FoldPreviewPhysicalGrabTransitionPlan {
  return Object.freeze({
    state: createFoldPreviewPhysicalGrabGestureState(),
    handled: false,
    endedAsTap: false,
    commands: Object.freeze([
      command({ kind: 'discard_presentation' }),
      command({
        kind: 'clear_interaction',
        clearEventSession: true,
      }),
      command({ kind: 'restore_camera' }),
      command({ kind: 'sync_presentation' }),
    ]),
  })
}

function isFoldPreviewAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function isNonnegativeFiniteNumber(value: unknown): value is number {
  return isFiniteNumber(value) && value >= 0
}

function isNonnegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 1
}

function validCoordinatorId(value: unknown): value is string {
  return validCoordinatorString(
    value,
    MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_ID_LENGTH,
  )
}

function validCoordinatorKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length
      <= MAX_FOLD_PREVIEW_PHYSICAL_GRAB_COORDINATOR_KEY_LENGTH
    // Runtime-generated context, view, and guard keys never start blank.
    // Checking one code unit keeps pointer guards O(1) for large tree keys.
    && value[0].trim().length > 0
}

function validCoordinatorString(
  value: unknown,
  maximumLength: number,
): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= maximumLength
    && value.trim().length > 0
}

function validPointerId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

function isIdentityObject(value: unknown): value is object {
  return typeof value === 'object' && value !== null
}

function command<Command extends FoldPreviewPhysicalGrabCommand>(
  value: Command,
): Command {
  return Object.freeze(value)
}
