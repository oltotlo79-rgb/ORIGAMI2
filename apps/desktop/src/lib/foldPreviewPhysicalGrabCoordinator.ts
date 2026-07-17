import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  type FoldPreviewPhysicalGrabTarget,
} from './foldPreviewPhysicalGrab.ts'
import type {
  FoldPreviewPhysicalGrabGestureState,
  FoldPreviewPhysicalGrabGestureTransition,
} from './foldPreviewPhysicalGrabGesture.ts'

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
  if (
    snapshot.guardKey === null
    || snapshot.startedRunnerState === null
    || snapshot.startedViewKey === null
    || snapshot.activeContextKey === null
    || snapshot.activeContextKey !== snapshot.renderedContextKey
    || snapshot.activeContextKey !== snapshot.latestContextKey
    || snapshot.currentRunnerState !== snapshot.startedRunnerState
    || snapshot.currentViewKey !== snapshot.startedViewKey
  ) return null
  return snapshot.guardKey
}

/**
 * Converts one reducer transition into an ordered, DOM-independent command
 * plan. The caller executes these commands in order through its WebView
 * adapters. In particular, a verified completion request is always last.
 */
export function planFoldPreviewPhysicalGrabTransition(
  input: FoldPreviewPhysicalGrabTransitionPlanInput,
): FoldPreviewPhysicalGrabTransitionPlan {
  const state = input.transition.state
  const cleanState = isCleanPhysicalGrabState(state)
  const commands: FoldPreviewPhysicalGrabCommand[] = []
  let handled = false
  let endedAsTap = false
  let queuedPresentation = false
  let clearedInteraction = false
  let completionAngle: number | null = null
  const terminalEffectCount = input.transition.effects.reduce(
    (count, effect) =>
      count + Number(effect.kind === 'cancel' || effect.kind === 'end'),
    0,
  )

  for (const effect of input.transition.effects) {
    if (effect.kind === 'handled') {
      if (effect.pointerId === input.eventPointerId) handled = true
      continue
    }
    if (effect.kind === 'presentation') {
      queuedPresentation = true
      commands.push(command({ kind: 'queue_presentation' }))
      if (
        effect.target !== null
        && input.activeHingeId
        && input.selectedHingeId !== input.activeHingeId
      ) {
        commands.push(command({
          kind: 'select_hinge',
          hingeId: input.activeHingeId,
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
      && cleanState
      && terminalEffectCount === 1
    ) {
      completionAngle = resolveCompletionAngle(
        effect.completionTarget,
        input,
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
  if (!queuedPresentation || state.kind !== 'dragging') {
    commands.push(command({ kind: 'sync_presentation' }))
  }
  if (completionAngle !== null) {
    commands.push(command({
      kind: 'request_fold_angle',
      angleDegrees: completionAngle,
    }))
  }

  return Object.freeze({
    state,
    handled,
    endedAsTap,
    commands: Object.freeze(commands),
  })
}

function resolveCompletionAngle(
  target: FoldPreviewPhysicalGrabTarget,
  input: FoldPreviewPhysicalGrabTransitionPlanInput,
): number | null {
  if (
    input.disposed
    || input.modelHingeId === null
    || target.mapping !== FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
    || target.contextKey !== input.activeContextKey
    || input.activeHingeId !== input.modelHingeId
    || !input.guardIsCurrent
    || !isFoldPreviewAngle(target.angleDegrees)
  ) return null
  return target.angleDegrees
}

function isCleanPhysicalGrabState(
  state: FoldPreviewPhysicalGrabGestureState,
) {
  return state.kind === 'idle'
    && state.suppressedPointerIds.length === 0
    && !state.requiresReset
}

function isFoldPreviewAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function command<Command extends FoldPreviewPhysicalGrabCommand>(
  value: Command,
): Command {
  return Object.freeze(value)
}
