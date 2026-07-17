import type {
  FoldPreviewContinuousMotionRunnerState,
} from './foldPreviewContinuousMotionRunner.ts'
import type { FoldPreviewHingeAngle } from './foldPreviewKinematics.ts'
import {
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH,
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'
import {
  transitionFoldPreviewTreeMotionOwner,
  type FoldPreviewTreeMotionOwnerCommand,
  type FoldPreviewTreeMotionOwnerEvent,
  type FoldPreviewTreeMotionOwnerPlan,
  type FoldPreviewTreeMotionOwnerState,
  type FoldPreviewTreeMotionRunnerTerminalStatus,
} from './foldPreviewTreeMotionOwner.ts'

export const FOLD_PREVIEW_TREE_MOTION_RUNTIME_VERSION =
  'tree_motion_runtime_v1'

declare const APPLICATION_TOKEN_BRAND: unique symbol
declare const RUNNER_TOKEN_BRAND: unique symbol

export type FoldPreviewTreeMotionRuntimeApplicationToken = Readonly<{
  [APPLICATION_TOKEN_BRAND]: true
}>

export type FoldPreviewTreeMotionRuntimeRunnerToken = Readonly<{
  [RUNNER_TOKEN_BRAND]: true
}>

export type FoldPreviewTreeMotionRuntimeInput = Readonly<{
  context: FoldPreviewTreeMotionContext
  /**
   * The authentic owner state returned after its `prepare_runner` event.
   * Runtime code retains it privately and creates every later owner event.
   */
  ownerState: FoldPreviewTreeMotionOwnerState
}>

export type FoldPreviewTreeMotionRuntimeState = Readonly<{
  version: typeof FOLD_PREVIEW_TREE_MOTION_RUNTIME_VERSION
  generation: number
  contextKey: string
  hingeEdgeId: string
  /** The complete canonical vector confirmed as applied to the scene. */
  appliedAngles: readonly FoldPreviewHingeAngle[]
  activeRunnerToken: FoldPreviewTreeMotionRuntimeRunnerToken | null
  activeRequestSequence: number | null
  activeTargetSelectedAngleDegrees: number | null
  /**
   * Non-null while one emitted complete pose awaits an atomic scene result.
   * `appliedAngles` remains the prior confirmed scene vector until then.
   */
  pendingApplicationToken:
    FoldPreviewTreeMotionRuntimeApplicationToken | null
  latestRequestSequence: number
  committedRequestSequence: number | null
  disposed: boolean
}>

export type FoldPreviewTreeMotionRuntimeEvent =
  | Readonly<{
      kind: 'request'
      targetSelectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'runner_apply'
      runnerToken: FoldPreviewTreeMotionRuntimeRunnerToken
      selectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'runner_state'
      runnerToken: FoldPreviewTreeMotionRuntimeRunnerToken
      runnerState: FoldPreviewContinuousMotionRunnerState<unknown>
    }>
  | Readonly<{ kind: 'dispose' }>

export type FoldPreviewTreeMotionRuntimeCommand =
  | Readonly<{
      kind: 'start_runner'
      generation: number
      contextKey: string
      hingeEdgeId: string
      runnerToken: FoldPreviewTreeMotionRuntimeRunnerToken
      requestSequence: number
      targetSelectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'apply_complete_pose'
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      selectedAngleDegrees: number
      appliedAngles: readonly FoldPreviewHingeAngle[]
      applicationToken: FoldPreviewTreeMotionRuntimeApplicationToken
    }>
  | Readonly<{
      kind: 'commit_complete_applied'
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      status: FoldPreviewTreeMotionRunnerTerminalStatus
      selectedAngleDegrees: number
      appliedAngles: readonly FoldPreviewHingeAngle[]
    }>
  | Readonly<{ kind: 'dispose_runner' }>

export type FoldPreviewTreeMotionRuntimeRejectionReason =
  | 'invalid_event'
  | 'unsupported_event'
  | 'disposed'
  | 'counter_exhausted'
  | 'owner_rejected'
  | 'request_not_active'
  | 'runner_state_mismatch'
  | 'runner_token_mismatch'
  | 'application_pending'
  | 'application_not_pending'
  | 'application_token_mismatch'

export type FoldPreviewTreeMotionRuntimePlan = Readonly<{
  accepted: boolean
  reason: FoldPreviewTreeMotionRuntimeRejectionReason | null
  state: FoldPreviewTreeMotionRuntimeState
  /**
   * Authentic owner state to adopt with `state` before commands execute.
   * Opaque owner/request tokens never appear in runtime events or commands.
   */
  ownerState: FoldPreviewTreeMotionOwnerState
  commands: readonly FoldPreviewTreeMotionRuntimeCommand[]
}>

type RuntimeFields = Omit<
  FoldPreviewTreeMotionRuntimeState,
  'version'
>

type PendingApplication = Readonly<{
  applicationToken: FoldPreviewTreeMotionRuntimeApplicationToken
  appliedAngles: readonly FoldPreviewHingeAngle[]
}>

type RunnerStateSnapshot = Readonly<{
  status: FoldPreviewContinuousMotionRunnerState['status']
  requested: number | null
  applied: number
}>

const runtimeStates = new WeakSet<object>()
const runtimeContexts = new WeakMap<
  object,
  FoldPreviewTreeMotionContext
>()
const runtimeOwnerStates = new WeakMap<
  object,
  FoldPreviewTreeMotionOwnerState
>()
const runtimePendingApplications = new WeakMap<
  object,
  PendingApplication
>()
const applicationTokens = new WeakSet<object>()
const runnerTokens = new WeakSet<object>()

/**
 * Creates a selected-hinge runtime from an authentic prepared owner state.
 *
 * Owner-state provenance is checked by the owner's private WeakSet boundary,
 * not by structural token checks. This module then retains that state in a
 * WeakMap and creates every owner event itself, so forged owner commands and
 * arbitrary request-token objects never enter the public runtime API.
 */
export function createFoldPreviewTreeMotionRuntime(
  input: FoldPreviewTreeMotionRuntimeInput,
): FoldPreviewTreeMotionRuntimeState | null {
  try {
    if (!isRecord(input)) return null
    const context = input.context
    const ownerState = input.ownerState
    if (
      !isRecord(context)
      || !isAuthenticOwnerState(ownerState)
    ) return null

    const contextKey = context.contextKey
    const hingeEdgeId = context.selectedHingeEdgeId
    const selectedAngleDegrees = context.selectedAngleDegrees
    const generation = ownerState.generation
    const ownerKind = ownerState.owner
    const directPending = ownerState.directPending
    const directKey = ownerState.directKey
    const runnerContextKey = ownerState.runnerContextKey
    const runnerHingeEdgeId = ownerState.runnerHingeEdgeId
    const activeRequestSequence = ownerState.activeRequestSequence
    const activeRequestToken = ownerState.activeRequestToken
    const activeTargetSelectedAngleDegrees =
      ownerState.activeTargetSelectedAngleDegrees
    const committedRequestSequence =
      ownerState.committedRequestSequence
    const committedRequestToken = ownerState.committedRequestToken
    if (
      ownerKind !== 'runner'
      || directPending !== false
      || directKey !== null
      || runnerContextKey !== contextKey
      || runnerHingeEdgeId !== hingeEdgeId
      || activeRequestSequence !== null
      || activeRequestToken !== null
      || activeTargetSelectedAngleDegrees !== null
      || committedRequestSequence !== null
      || committedRequestToken !== null
      || !validCounter(generation)
      || !validKey(contextKey)
      || !validId(hingeEdgeId)
      || !validAngle(selectedAngleDegrees)
    ) return null

    // Reuses the context module's opaque provenance and vector bounds.
    const appliedAngles = replaceFoldPreviewTreeMotionSelectedAngle(
      context,
      selectedAngleDegrees,
    )
    if (!appliedAngles) return null
    return createState({
      generation,
      contextKey,
      hingeEdgeId,
      appliedAngles: copyAngles(appliedAngles),
      activeRunnerToken: null,
      activeRequestSequence: null,
      activeTargetSelectedAngleDegrees: null,
      pendingApplicationToken: null,
      latestRequestSequence: 0,
      committedRequestSequence: null,
      disposed: false,
    }, context, ownerState)
  } catch {
    return null
  }
}

/**
 * Transitions request and runner callbacks without accepting owner commands.
 *
 * Callers adopt both returned states before executing commands. That ordering
 * lets a synchronous runner callback see its exact active owner generation.
 */
export function transitionFoldPreviewTreeMotionRuntime(
  state: FoldPreviewTreeMotionRuntimeState,
  event: FoldPreviewTreeMotionRuntimeEvent,
): FoldPreviewTreeMotionRuntimePlan | null {
  if (!isRuntimeState(state)) return null
  try {
    if (!isRecord(event)) return reject(state, 'invalid_event')
    const kind = event.kind
    if (kind === 'dispose') return disposeRuntime(state)
    if (state.disposed) return reject(state, 'disposed')
    if (state.pendingApplicationToken !== null) {
      return reject(state, 'application_pending')
    }
    switch (kind) {
      case 'request': {
        const targetSelectedAngleDegrees =
          event.targetSelectedAngleDegrees
        return validAngle(targetSelectedAngleDegrees)
          ? requestRunner(state, normalizeZero(targetSelectedAngleDegrees))
          : reject(state, 'invalid_event')
      }
      case 'runner_apply': {
        const runnerToken = event.runnerToken
        const selectedAngleDegrees = event.selectedAngleDegrees
        return isRunnerToken(runnerToken)
          && runnerToken === state.activeRunnerToken
          && validAngle(selectedAngleDegrees)
          ? applyRunner(state, normalizeZero(selectedAngleDegrees))
          : reject(
              state,
              isRunnerToken(runnerToken)
                ? 'runner_token_mismatch'
                : 'invalid_event',
            )
      }
      case 'runner_state': {
        const runnerToken = event.runnerToken
        const runnerState = event.runnerState
        if (
          !isRunnerToken(runnerToken)
          || runnerToken !== state.activeRunnerToken
        ) {
          return reject(
            state,
            isRunnerToken(runnerToken)
              ? 'runner_token_mismatch'
              : 'invalid_event',
          )
        }
        const snapshot = snapshotRunnerState(runnerState)
        return snapshot
          ? publishRunnerState(state, snapshot)
          : reject(state, 'invalid_event')
      }
      default:
        return reject(state, 'unsupported_event')
    }
  } catch {
    return reject(state, 'invalid_event')
  }
}

/**
 * Confirms the exact atomic result of an `apply_complete_pose` command.
 *
 * Exact `true` promotes the staged complete vector. Exact `false` permanently
 * disposes this runtime and its runner, so an application failure cannot be
 * misreported later as a terminal safe commit. An old, forged, or consumed
 * token can never change the pose.
 */
export function completeFoldPreviewTreeMotionRuntimePoseApplication(
  state: FoldPreviewTreeMotionRuntimeState,
  applicationToken: FoldPreviewTreeMotionRuntimeApplicationToken,
  applied: boolean,
): FoldPreviewTreeMotionRuntimePlan | null {
  if (!isRuntimeState(state)) return null
  try {
    if (state.disposed) return reject(state, 'disposed')
    if (state.pendingApplicationToken === null) {
      return reject(state, 'application_not_pending')
    }
    if (
      !isApplicationToken(applicationToken)
      || applicationToken !== state.pendingApplicationToken
    ) return reject(state, 'application_token_mismatch')
    if (applied !== true && applied !== false) {
      return reject(state, 'invalid_event')
    }
    const pending = runtimePendingApplications.get(state)
    if (
      !pending
      || pending.applicationToken !== applicationToken
    ) return reject(state, 'application_token_mismatch')
    if (!applied) return disposeRuntime(state)
    return accept(nextStateFor(state, {
      appliedAngles: copyAngles(pending.appliedAngles),
      pendingApplicationToken: null,
    }), [])
  } catch {
    return reject(state, 'invalid_event')
  }
}

function requestRunner(
  state: FoldPreviewTreeMotionRuntimeState,
  targetSelectedAngleDegrees: number,
) {
  const ownerState = ownerStateFor(state)
  if (!ownerState) return reject(state, 'owner_rejected')
  const ownerPlan = transitionFoldPreviewTreeMotionOwner(ownerState, {
    kind: 'request_runner',
    ownerToken: ownerState.ownerToken,
    generation: ownerState.generation,
    contextKey: state.contextKey,
    hingeEdgeId: state.hingeEdgeId,
    targetSelectedAngleDegrees,
  })
  if (!ownerPlan) return reject(state, 'owner_rejected')
  if (!ownerPlan.accepted) {
    if (ownerPlan.reason === 'counter_exhausted') {
      return ownerExhausted(state, ownerPlan.state)
    }
    return reject(state, 'owner_rejected')
  }
  const command = soleOwnerCommand(ownerPlan, 'request_runner')
  if (
    !command
    || command.contextKey !== state.contextKey
    || command.hingeEdgeId !== state.hingeEdgeId
    || command.targetSelectedAngleDegrees
      !== targetSelectedAngleDegrees
    || command.requestSequence !== state.latestRequestSequence + 1
  ) return reject(state, 'owner_rejected')

  const runnerToken = createRunnerToken()
  const nextState = nextStateFor(state, {
    generation: command.generation,
    activeRunnerToken: runnerToken,
    activeRequestSequence: command.requestSequence,
    activeTargetSelectedAngleDegrees:
      command.targetSelectedAngleDegrees,
    latestRequestSequence: command.requestSequence,
  }, ownerPlan.state)
  return accept(nextState, [{
    kind: 'start_runner',
    generation: command.generation,
    contextKey: command.contextKey,
    hingeEdgeId: command.hingeEdgeId,
    runnerToken,
    requestSequence: command.requestSequence,
    targetSelectedAngleDegrees:
      command.targetSelectedAngleDegrees,
  }])
}

function applyRunner(
  state: FoldPreviewTreeMotionRuntimeState,
  selectedAngleDegrees: number,
) {
  const ownerState = ownerStateFor(state)
  const context = runtimeContexts.get(state)
  if (
    !ownerState
    || !context
    || state.activeRunnerToken === null
    || state.activeRequestSequence === null
    || ownerState.activeRequestToken === null
  ) return reject(state, 'request_not_active')
  const ownerPlan = transitionFoldPreviewTreeMotionOwner(ownerState, {
    kind: 'runner_apply',
    ownerToken: ownerState.ownerToken,
    requestToken: ownerState.activeRequestToken,
    generation: ownerState.generation,
    contextKey: state.contextKey,
    hingeEdgeId: state.hingeEdgeId,
    requestSequence: state.activeRequestSequence,
    selectedAngleDegrees,
  })
  if (!ownerPlan?.accepted) return reject(state, 'owner_rejected')
  const command = soleOwnerCommand(
    ownerPlan,
    'apply_runner_selected',
  )
  if (
    !command
    || command.selectedAngleDegrees !== selectedAngleDegrees
    || command.requestSequence !== state.activeRequestSequence
  ) return reject(state, 'owner_rejected')
  const appliedAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    command.selectedAngleDegrees,
  )
  if (!appliedAngles) return reject(state, 'owner_rejected')

  const applicationToken = createApplicationToken()
  const pending = deepFreeze({
    applicationToken,
    appliedAngles: copyAngles(appliedAngles),
  })
  const nextState = nextStateFor(state, {
    generation: command.generation,
    pendingApplicationToken: applicationToken,
  }, ownerPlan.state, pending)
  return accept(nextState, [{
    kind: 'apply_complete_pose',
    generation: command.generation,
    contextKey: command.contextKey,
    hingeEdgeId: command.hingeEdgeId,
    requestSequence: command.requestSequence,
    selectedAngleDegrees: command.selectedAngleDegrees,
    appliedAngles: copyAngles(appliedAngles),
    applicationToken,
  }])
}

function publishRunnerState(
  state: FoldPreviewTreeMotionRuntimeState,
  runnerState: RunnerStateSnapshot,
) {
  const ownerState = ownerStateFor(state)
  const selectedAngleDegrees = selectedAngle(state)
  if (
    !ownerState
    || state.activeRunnerToken === null
    || state.activeRequestSequence === null
    || state.activeTargetSelectedAngleDegrees === null
    || ownerState.activeRequestToken === null
  ) return reject(state, 'request_not_active')
  if (
    runnerState.requested !== state.activeTargetSelectedAngleDegrees
    || !validAngle(runnerState.applied)
    || runnerState.applied !== selectedAngleDegrees
  ) return reject(state, 'runner_state_mismatch')

  const binding = {
    ownerToken: ownerState.ownerToken,
    requestToken: ownerState.activeRequestToken,
    generation: ownerState.generation,
    contextKey: state.contextKey,
    hingeEdgeId: state.hingeEdgeId,
    requestSequence: state.activeRequestSequence,
  }
  const ownerEvent: FoldPreviewTreeMotionOwnerEvent =
    runnerState.status === 'running'
      ? { kind: 'runner_callback', ...binding }
      : validTerminalStatus(runnerState.status)
        ? {
            kind: 'runner_terminal',
            ...binding,
            status: runnerState.status,
            appliedSelectedAngleDegrees: runnerState.applied,
          }
        : null as never
  if (!ownerEvent) return reject(state, 'runner_state_mismatch')
  const ownerPlan = transitionFoldPreviewTreeMotionOwner(
    ownerState,
    ownerEvent,
  )
  if (!ownerPlan?.accepted) return reject(state, 'owner_rejected')

  if (runnerState.status === 'running') {
    if (!soleOwnerCommand(ownerPlan, 'accept_runner_callback')) {
      return reject(state, 'owner_rejected')
    }
    return accept(nextStateFor(state, {
      generation: ownerPlan.state.generation,
    }, ownerPlan.state), [])
  }

  const accepted = ownerPlan.commands.filter(
    (command) => command.kind === 'accept_runner_callback',
  )
  const commits = ownerPlan.commands.filter(
    (command) => command.kind === 'commit_selected_applied',
  )
  const commit = commits[0]
  if (
    accepted.length !== 1
    || commits.length !== 1
    || !commit
    || commit.selectedAngleDegrees !== selectedAngleDegrees
    || commit.requestSequence !== state.activeRequestSequence
    || commit.status !== runnerState.status
  ) return reject(state, 'owner_rejected')

  const nextState = nextStateFor(state, {
    generation: ownerPlan.state.generation,
    activeRunnerToken: null,
    activeRequestSequence: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: commit.requestSequence,
  }, ownerPlan.state)
  return accept(nextState, [{
    kind: 'commit_complete_applied',
    generation: commit.generation,
    contextKey: commit.contextKey,
    hingeEdgeId: commit.hingeEdgeId,
    requestSequence: commit.requestSequence,
    status: commit.status,
    selectedAngleDegrees: commit.selectedAngleDegrees,
    appliedAngles: copyAngles(state.appliedAngles),
  }])
}

function disposeRuntime(state: FoldPreviewTreeMotionRuntimeState) {
  if (state.disposed) return accept(state, [])
  const ownerState = ownerStateFor(state)
  if (!ownerState) return reject(state, 'owner_rejected')
  const ownerPlan = transitionFoldPreviewTreeMotionOwner(
    ownerState,
    { kind: 'dispose' },
  )
  if (!ownerPlan?.accepted) return reject(state, 'owner_rejected')
  return accept(nextStateFor(state, {
    generation: ownerPlan.state.generation,
    activeRequestSequence: null,
    activeRunnerToken: null,
    activeTargetSelectedAngleDegrees: null,
    pendingApplicationToken: null,
    disposed: true,
  }, ownerPlan.state), [{ kind: 'dispose_runner' }])
}

function ownerExhausted(
  state: FoldPreviewTreeMotionRuntimeState,
  ownerState: FoldPreviewTreeMotionOwnerState,
) {
  const nextState = nextStateFor(state, {
    generation: ownerState.generation,
    activeRequestSequence: null,
    activeRunnerToken: null,
    activeTargetSelectedAngleDegrees: null,
    pendingApplicationToken: null,
    disposed: true,
  }, ownerState)
  return plan(
    false,
    'counter_exhausted',
    nextState,
    [{ kind: 'dispose_runner' }],
  )
}

function snapshotRunnerState(
  runnerState: unknown,
): RunnerStateSnapshot | null {
  try {
    if (!isRecord(runnerState)) return null
    const status = runnerState.status
    const requested = runnerState.requested
    const applied = runnerState.applied
    if (
      !validRunnerStatus(status)
      || (
        requested !== null
        && !validAngle(requested)
      )
      || !validAngle(applied)
    ) return null
    return {
      status,
      requested: requested as number | null,
      applied,
    }
  } catch {
    return null
  }
}

function soleOwnerCommand<
  Kind extends FoldPreviewTreeMotionOwnerCommand['kind'],
>(
  ownerPlan: FoldPreviewTreeMotionOwnerPlan,
  kind: Kind,
): Extract<FoldPreviewTreeMotionOwnerCommand, { kind: Kind }> | null {
  const matches = ownerPlan.commands.filter(
    (command) => command.kind === kind,
  )
  return matches.length === 1
    ? matches[0] as Extract<
        FoldPreviewTreeMotionOwnerCommand,
        { kind: Kind }
      >
    : null
}

function isAuthenticOwnerState(
  value: unknown,
): value is FoldPreviewTreeMotionOwnerState {
  if (!isRecord(value)) return false
  try {
    // A structurally identical object fails the owner's private WeakSet check.
    // `dispose` is a pure transition here; its unexecuted plan has no effect.
    return transitionFoldPreviewTreeMotionOwner(
      value as FoldPreviewTreeMotionOwnerState,
      { kind: 'dispose' },
    ) !== null
  } catch {
    return false
  }
}

function selectedAngle(
  state: FoldPreviewTreeMotionRuntimeState,
): number | null {
  let selected: number | null = null
  let matches = 0
  for (const angle of state.appliedAngles) {
    if (angle.edgeId !== state.hingeEdgeId) continue
    selected = angle.angleDegrees
    matches += 1
  }
  return matches === 1 && validAngle(selected) ? selected : null
}

function nextStateFor(
  state: FoldPreviewTreeMotionRuntimeState,
  updates: Partial<RuntimeFields>,
  ownerState = ownerStateFor(state),
  pendingApplication: PendingApplication | null = null,
) {
  const context = runtimeContexts.get(state)
  if (!context || !ownerState) throw new Error('missing runtime context')
  return createState({
    generation: state.generation,
    contextKey: state.contextKey,
    hingeEdgeId: state.hingeEdgeId,
    appliedAngles: copyAngles(state.appliedAngles),
    activeRunnerToken: state.activeRunnerToken,
    activeRequestSequence: state.activeRequestSequence,
    activeTargetSelectedAngleDegrees:
      state.activeTargetSelectedAngleDegrees,
    pendingApplicationToken: state.pendingApplicationToken,
    latestRequestSequence: state.latestRequestSequence,
    committedRequestSequence: state.committedRequestSequence,
    disposed: state.disposed,
    ...updates,
  }, context, ownerState, pendingApplication)
}

function createState(
  fields: RuntimeFields,
  context: FoldPreviewTreeMotionContext,
  ownerState: FoldPreviewTreeMotionOwnerState,
  pendingApplication: PendingApplication | null = null,
): FoldPreviewTreeMotionRuntimeState {
  const state = deepFreeze({
    version: FOLD_PREVIEW_TREE_MOTION_RUNTIME_VERSION,
    ...fields,
    appliedAngles: copyAngles(fields.appliedAngles),
  }) as FoldPreviewTreeMotionRuntimeState
  runtimeStates.add(state)
  runtimeContexts.set(state, context)
  runtimeOwnerStates.set(state, ownerState)
  if (pendingApplication) {
    runtimePendingApplications.set(state, pendingApplication)
  }
  return state
}

function ownerStateFor(
  state: FoldPreviewTreeMotionRuntimeState,
) {
  return runtimeOwnerStates.get(state) ?? null
}

function isRuntimeState(
  value: unknown,
): value is FoldPreviewTreeMotionRuntimeState {
  const candidate = value as FoldPreviewTreeMotionRuntimeState
  return typeof value === 'object'
    && value !== null
    && runtimeStates.has(value)
    && runtimeContexts.has(value)
    && runtimeOwnerStates.has(value)
    && (
      candidate.pendingApplicationToken === null
        ? !runtimePendingApplications.has(value)
        : runtimePendingApplications.get(value)?.applicationToken
          === candidate.pendingApplicationToken
    )
}

function accept(
  state: FoldPreviewTreeMotionRuntimeState,
  commands: readonly FoldPreviewTreeMotionRuntimeCommand[],
) {
  return plan(true, null, state, commands)
}

function reject(
  state: FoldPreviewTreeMotionRuntimeState,
  reason: FoldPreviewTreeMotionRuntimeRejectionReason,
) {
  return plan(false, reason, state, [])
}

function plan(
  accepted: boolean,
  reason: FoldPreviewTreeMotionRuntimeRejectionReason | null,
  state: FoldPreviewTreeMotionRuntimeState,
  commands: readonly FoldPreviewTreeMotionRuntimeCommand[],
): FoldPreviewTreeMotionRuntimePlan {
  const ownerState = ownerStateFor(state)
  if (!ownerState) throw new Error('missing runtime owner')
  return deepFreeze({
    accepted,
    reason,
    state,
    ownerState,
    commands: commands.map(copyRuntimeCommand),
  })
}

function copyRuntimeCommand(
  command: FoldPreviewTreeMotionRuntimeCommand,
): FoldPreviewTreeMotionRuntimeCommand {
  return 'appliedAngles' in command
    ? {
        ...command,
        appliedAngles: copyAngles(command.appliedAngles),
      }
    : { ...command }
}

function copyAngles(
  angles: readonly FoldPreviewHingeAngle[],
): FoldPreviewHingeAngle[] {
  return angles.map((angle) => ({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  }))
}

function createApplicationToken():
FoldPreviewTreeMotionRuntimeApplicationToken {
  const token = Object.freeze(
    {},
  ) as FoldPreviewTreeMotionRuntimeApplicationToken
  applicationTokens.add(token)
  return token
}

function createRunnerToken():
FoldPreviewTreeMotionRuntimeRunnerToken {
  const token = Object.freeze(
    {},
  ) as FoldPreviewTreeMotionRuntimeRunnerToken
  runnerTokens.add(token)
  return token
}

function isApplicationToken(
  value: unknown,
): value is FoldPreviewTreeMotionRuntimeApplicationToken {
  return typeof value === 'object'
    && value !== null
    && applicationTokens.has(value)
}

function isRunnerToken(
  value: unknown,
): value is FoldPreviewTreeMotionRuntimeRunnerToken {
  return typeof value === 'object'
    && value !== null
    && runnerTokens.has(value)
}

function validRunnerStatus(
  value: unknown,
): value is FoldPreviewContinuousMotionRunnerState['status'] {
  return value === 'idle'
    || value === 'running'
    || value === 'clear'
    || value === 'blocked'
    || value === 'indeterminate'
    || value === 'disposed'
}

function validTerminalStatus(
  value: unknown,
): value is FoldPreviewTreeMotionRunnerTerminalStatus {
  return value === 'clear'
    || value === 'blocked'
    || value === 'indeterminate'
}

function validCounter(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH
    && value[0]?.trim().length !== 0
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH
    && value.trim().length > 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function deepFreeze<T>(value: T, seen = new WeakSet<object>()): T {
  if (typeof value !== 'object' || value === null) return value
  const object = value as object
  if (seen.has(object)) return value
  seen.add(object)
  for (const key of Reflect.ownKeys(object)) {
    deepFreeze(
      (object as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
  return Object.freeze(value)
}
