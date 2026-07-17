import {
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH,
} from './foldPreviewTreeMotionContext.ts'

export const FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION =
  'tree_motion_owner_v1'
export const MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_KEY_LENGTH =
  MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH
export const MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_ID_LENGTH = 1_024

declare const OWNER_TOKEN_BRAND: unique symbol
declare const REQUEST_TOKEN_BRAND: unique symbol

export type FoldPreviewTreeMotionOwnerToken = Readonly<{
  [OWNER_TOKEN_BRAND]: true
}>

export type FoldPreviewTreeMotionRequestToken = Readonly<{
  [REQUEST_TOKEN_BRAND]: true
}>

export type FoldPreviewTreeMotionOwnerKind =
  | 'none'
  | 'direct'
  | 'runner'
  | 'disposed'

export type FoldPreviewTreeMotionOwnerState = Readonly<{
  version: typeof FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION
  ownerToken: FoldPreviewTreeMotionOwnerToken
  generation: number
  owner: FoldPreviewTreeMotionOwnerKind
  directPending: boolean
  directKey: string | null
  runnerContextKey: string | null
  runnerHingeEdgeId: string | null
  activeRequestSequence: number | null
  activeRequestToken: FoldPreviewTreeMotionRequestToken | null
  activeTargetSelectedAngleDegrees: number | null
  committedRequestSequence: number | null
  committedRequestToken: FoldPreviewTreeMotionRequestToken | null
}>

export type FoldPreviewTreeMotionRunnerTerminalStatus =
  | 'clear'
  | 'blocked'
  | 'indeterminate'

export type FoldPreviewTreeMotionOwnerEvent =
  | Readonly<{
      kind: 'schedule_direct'
      key: string
    }>
  | Readonly<{
      kind: 'external_direct_change'
      key: string
    }>
  | Readonly<{
      kind: 'direct_callback'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      key: string
    }>
  | Readonly<{
      kind: 'prepare_runner'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      contextKey: string
      hingeEdgeId: string
    }>
  | Readonly<{
      kind: 'request_runner'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      targetSelectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'runner_apply'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      selectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'runner_callback'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
    }>
  | Readonly<{
      kind: 'runner_terminal'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      status: FoldPreviewTreeMotionRunnerTerminalStatus
      appliedSelectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'dispose'
    }>

export type FoldPreviewTreeMotionOwnerCommand =
  | Readonly<{ kind: 'reset_gesture' }>
  | Readonly<{ kind: 'dispose_runner' }>
  | Readonly<{ kind: 'dispose_direct' }>
  | Readonly<{
      kind: 'schedule_direct'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      key: string
    }>
  | Readonly<{
      kind: 'apply_direct'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      key: string
    }>
  | Readonly<{
      kind: 'prepare_runner'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      generation: number
      contextKey: string
      hingeEdgeId: string
    }>
  | Readonly<{
      kind: 'request_runner'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      targetSelectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'apply_runner_selected'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      selectedAngleDegrees: number
    }>
  | Readonly<{
      kind: 'accept_runner_callback'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
    }>
  | Readonly<{
      kind: 'commit_selected_applied'
      ownerToken: FoldPreviewTreeMotionOwnerToken
      requestToken: FoldPreviewTreeMotionRequestToken
      generation: number
      contextKey: string
      hingeEdgeId: string
      requestSequence: number
      status: FoldPreviewTreeMotionRunnerTerminalStatus
      selectedAngleDegrees: number
    }>

export type FoldPreviewTreeMotionOwnerRejectionReason =
  | 'invalid_event'
  | 'disposed'
  | 'counter_exhausted'
  | 'direct_pending'
  | 'direct_not_pending'
  | 'owner_mismatch'
  | 'owner_token_mismatch'
  | 'stale_generation'
  | 'direct_key_mismatch'
  | 'context_mismatch'
  | 'hinge_mismatch'
  | 'request_mismatch'
  | 'request_token_mismatch'

export type FoldPreviewTreeMotionOwnerPlan = Readonly<{
  accepted: boolean
  reason: FoldPreviewTreeMotionOwnerRejectionReason | null
  state: FoldPreviewTreeMotionOwnerState
  commands: readonly FoldPreviewTreeMotionOwnerCommand[]
}>

export type FoldPreviewTreeMotionOwnerInitialOptions = Readonly<{
  initialGeneration?: number
}>

type RunnerOwnerToken = Readonly<{
  ownerToken: FoldPreviewTreeMotionOwnerToken
  generation: number
  contextKey: string
  hingeEdgeId: string
}>

type RunnerCallbackFields = RunnerOwnerToken & Readonly<{
  requestToken: FoldPreviewTreeMotionRequestToken
  requestSequence: number
}>

const ownerStates = new WeakSet<object>()
const ownerTokens = new WeakSet<object>()
const requestTokens = new WeakSet<object>()

/**
 * Creates the first immutable ownership snapshot.
 *
 * An injectable generation is useful when restoring an outer lifecycle and
 * lets callers prove that exhaustion fails closed without ever wrapping.
 */
export function createFoldPreviewTreeMotionOwnerState(
  options: FoldPreviewTreeMotionOwnerInitialOptions = {},
): FoldPreviewTreeMotionOwnerState | null {
  try {
    if (!isRecord(options)) return null
    const initialGeneration = options.initialGeneration
    const generation = initialGeneration ?? 0
    if (!validCounter(generation)) return null
    const ownerToken = createOwnerToken()
    return createState({
      ownerToken,
      generation: normalizeZero(generation),
      owner: 'none',
      directPending: false,
      directKey: null,
      runnerContextKey: null,
      runnerHingeEdgeId: null,
      activeRequestSequence: null,
      activeRequestToken: null,
      activeTargetSelectedAngleDegrees: null,
      committedRequestSequence: null,
      committedRequestToken: null,
    })
  } catch {
    return null
  }
}

/**
 * Pure ownership transition for direct frame work and a selected-hinge runner.
 *
 * The returned command order is authoritative. Callers must adopt `plan.state`
 * before executing its commands, and commands carry the generation token that
 * asynchronous callbacks must return in a later event.
 */
export function transitionFoldPreviewTreeMotionOwner(
  state: FoldPreviewTreeMotionOwnerState,
  event: FoldPreviewTreeMotionOwnerEvent,
): FoldPreviewTreeMotionOwnerPlan | null {
  if (!isOwnerState(state)) return null
  try {
    if (!isRecord(event)) return reject(state, 'invalid_event')
    if (state.owner === 'disposed') return reject(state, 'disposed')

    const kind = event.kind
    switch (kind) {
      case 'schedule_direct':
      case 'external_direct_change':
        return scheduleDirect(state, event)
      case 'direct_callback':
        return directCallback(state, event)
      case 'prepare_runner':
        return prepareRunner(state, event)
      case 'request_runner':
        return requestRunner(state, event)
      case 'runner_apply':
        return runnerApply(state, event)
      case 'runner_callback':
        return runnerCallback(state, event)
      case 'runner_terminal':
        return runnerTerminal(state, event)
      case 'dispose':
        return dispose(state)
      default:
        return reject(state, 'invalid_event')
    }
  } catch {
    return reject(state, 'invalid_event')
  }
}

function scheduleDirect(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const key = event.key
  if (!validKey(key)) return reject(state, 'invalid_event')
  const generation = nextCounter(state.generation)
  if (generation === null) return exhaust(state)
  const nextState = createState({
    ownerToken: state.ownerToken,
    generation,
    owner: 'direct',
    directPending: true,
    directKey: key,
    runnerContextKey: null,
    runnerHingeEdgeId: null,
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: null,
    committedRequestToken: null,
  })
  return accept(nextState, [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    {
      kind: 'schedule_direct',
      ownerToken: state.ownerToken,
      generation,
      key,
    },
  ])
}

function directCallback(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const generation = event.generation
  const key = event.key
  if (
    !isOwnerToken(ownerToken)
    || !validCounter(generation)
    || !validKey(key)
  ) return reject(state, 'invalid_event')
  if (ownerToken !== state.ownerToken) {
    return reject(state, 'owner_token_mismatch')
  }
  if (generation !== state.generation) {
    return reject(state, 'stale_generation')
  }
  if (state.owner !== 'direct') return reject(state, 'owner_mismatch')
  if (key !== state.directKey) {
    return reject(state, 'direct_key_mismatch')
  }
  if (!state.directPending) return reject(state, 'direct_not_pending')

  return accept(createState({
    ...stateFields(state),
    directPending: false,
  }), [
    {
      kind: 'apply_direct',
      ownerToken,
      generation,
      key,
    },
  ])
}

function prepareRunner(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const expectedGeneration = event.generation
  const contextKey = event.contextKey
  const hingeEdgeId = event.hingeEdgeId
  if (
    !isOwnerToken(ownerToken)
    || !validCounter(expectedGeneration)
    || !validKey(contextKey)
    || !validId(hingeEdgeId)
  ) return reject(state, 'invalid_event')
  if (ownerToken !== state.ownerToken) {
    return reject(state, 'owner_token_mismatch')
  }
  if (expectedGeneration !== state.generation) {
    return reject(state, 'stale_generation')
  }
  if (state.directPending) return reject(state, 'direct_pending')

  const generation = nextCounter(state.generation)
  if (generation === null) return exhaust(state)
  const nextState = createState({
    ownerToken,
    generation,
    owner: 'runner',
    directPending: false,
    directKey: null,
    runnerContextKey: contextKey,
    runnerHingeEdgeId: hingeEdgeId,
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: null,
    committedRequestToken: null,
  })
  return accept(nextState, [
    { kind: 'dispose_runner' },
    {
      kind: 'prepare_runner',
      ownerToken,
      generation,
      contextKey,
      hingeEdgeId,
    },
  ])
}

function requestRunner(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const expectedGeneration = event.generation
  const contextKey = event.contextKey
  const hingeEdgeId = event.hingeEdgeId
  const rawTargetSelectedAngleDegrees =
    event.targetSelectedAngleDegrees
  if (
    !isOwnerToken(ownerToken)
    || !validCounter(expectedGeneration)
    || !validKey(contextKey)
    || !validId(hingeEdgeId)
    || !validAngle(rawTargetSelectedAngleDegrees)
  ) return reject(state, 'invalid_event')
  const token: RunnerOwnerToken = {
    ownerToken,
    generation: expectedGeneration,
    contextKey,
    hingeEdgeId,
  }
  const mismatch = runnerOwnerMismatch(state, token)
  if (mismatch) return reject(state, mismatch)

  const generation = nextCounter(state.generation)
  const previousSequence = Math.max(
    state.activeRequestSequence ?? 0,
    state.committedRequestSequence ?? 0,
  )
  const requestSequence = nextCounter(previousSequence)
  if (generation === null || requestSequence === null) {
    return exhaust(state)
  }
  const requestToken = createRequestToken()
  const targetSelectedAngleDegrees = normalizeZero(
    rawTargetSelectedAngleDegrees,
  )
  const nextState = createState({
    ...stateFields(state),
    generation,
    activeRequestSequence: requestSequence,
    activeRequestToken: requestToken,
    activeTargetSelectedAngleDegrees: targetSelectedAngleDegrees,
  })
  return accept(nextState, [{
    kind: 'request_runner',
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
    targetSelectedAngleDegrees,
  }])
}

function runnerApply(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const requestToken = event.requestToken
  const generation = event.generation
  const contextKey = event.contextKey
  const hingeEdgeId = event.hingeEdgeId
  const requestSequence = event.requestSequence
  const rawSelectedAngleDegrees = event.selectedAngleDegrees
  if (
    !isOwnerToken(ownerToken)
    || !isRequestToken(requestToken)
    || !validCounter(generation)
    || !validKey(contextKey)
    || !validId(hingeEdgeId)
    || !validPositiveCounter(requestSequence)
    || !validAngle(rawSelectedAngleDegrees)
  ) return reject(state, 'invalid_event')
  const token: RunnerCallbackFields = {
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
  }
  const mismatch = activeRunnerMismatch(state, token)
  if (mismatch) return reject(state, mismatch)

  return accept(state, [{
    kind: 'apply_runner_selected',
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
    selectedAngleDegrees: normalizeZero(rawSelectedAngleDegrees),
  }])
}

function runnerCallback(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const requestToken = event.requestToken
  const generation = event.generation
  const contextKey = event.contextKey
  const hingeEdgeId = event.hingeEdgeId
  const requestSequence = event.requestSequence
  if (
    !isOwnerToken(ownerToken)
    || !isRequestToken(requestToken)
    || !validCounter(generation)
    || !validKey(contextKey)
    || !validId(hingeEdgeId)
    || !validPositiveCounter(requestSequence)
  ) return reject(state, 'invalid_event')
  const token: RunnerCallbackFields = {
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
  }
  const mismatch = activeRunnerMismatch(state, token)
  if (mismatch) return reject(state, mismatch)

  return accept(state, [{
    kind: 'accept_runner_callback',
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
  }])
}

function runnerTerminal(
  state: FoldPreviewTreeMotionOwnerState,
  event: Record<string, unknown>,
) {
  const ownerToken = event.ownerToken
  const requestToken = event.requestToken
  const generation = event.generation
  const contextKey = event.contextKey
  const hingeEdgeId = event.hingeEdgeId
  const requestSequence = event.requestSequence
  const status = event.status
  const rawAppliedSelectedAngleDegrees =
    event.appliedSelectedAngleDegrees
  if (
    !isOwnerToken(ownerToken)
    || !isRequestToken(requestToken)
    || !validCounter(generation)
    || !validKey(contextKey)
    || !validId(hingeEdgeId)
    || !validPositiveCounter(requestSequence)
    || !validTerminalStatus(status)
    || !validAngle(rawAppliedSelectedAngleDegrees)
  ) return reject(state, 'invalid_event')
  const token: RunnerCallbackFields = {
    ownerToken,
    requestToken,
    generation,
    contextKey,
    hingeEdgeId,
    requestSequence,
  }
  const mismatch = activeRunnerMismatch(state, token)
  if (mismatch) return reject(state, mismatch)
  const selectedAngleDegrees = normalizeZero(
    rawAppliedSelectedAngleDegrees,
  )
  if (
    status === 'clear'
    && selectedAngleDegrees !== state.activeTargetSelectedAngleDegrees
  ) return reject(state, 'invalid_event')

  const nextState = createState({
    ...stateFields(state),
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: requestSequence,
    committedRequestToken: requestToken,
  })
  return accept(nextState, [
    {
      kind: 'accept_runner_callback',
      ownerToken,
      requestToken,
      generation,
      contextKey,
      hingeEdgeId,
      requestSequence,
    },
    {
      kind: 'commit_selected_applied',
      ownerToken,
      requestToken,
      generation,
      contextKey,
      hingeEdgeId,
      requestSequence,
      status,
      selectedAngleDegrees,
    },
  ])
}

function dispose(state: FoldPreviewTreeMotionOwnerState) {
  const generation = nextCounter(state.generation) ?? state.generation
  return accept(
    disposedState(generation, state.ownerToken),
    cleanupCommands(),
  )
}

function exhaust(state: FoldPreviewTreeMotionOwnerState) {
  return plan(
    false,
    'counter_exhausted',
    disposedState(state.generation, state.ownerToken),
    cleanupCommands(),
  )
}

function disposedState(
  generation: number,
  ownerToken: FoldPreviewTreeMotionOwnerToken,
) {
  return createState({
    ownerToken,
    generation,
    owner: 'disposed',
    directPending: false,
    directKey: null,
    runnerContextKey: null,
    runnerHingeEdgeId: null,
    activeRequestSequence: null,
    activeRequestToken: null,
    activeTargetSelectedAngleDegrees: null,
    committedRequestSequence: null,
    committedRequestToken: null,
  })
}

function cleanupCommands(): FoldPreviewTreeMotionOwnerCommand[] {
  return [
    { kind: 'reset_gesture' },
    { kind: 'dispose_runner' },
    { kind: 'dispose_direct' },
  ]
}

function runnerOwnerMismatch(
  state: FoldPreviewTreeMotionOwnerState,
  token: RunnerOwnerToken,
): FoldPreviewTreeMotionOwnerRejectionReason | null {
  if (token.ownerToken !== state.ownerToken) {
    return 'owner_token_mismatch'
  }
  if (token.generation !== state.generation) return 'stale_generation'
  if (state.owner !== 'runner') return 'owner_mismatch'
  if (token.contextKey !== state.runnerContextKey) return 'context_mismatch'
  if (token.hingeEdgeId !== state.runnerHingeEdgeId) return 'hinge_mismatch'
  return null
}

function activeRunnerMismatch(
  state: FoldPreviewTreeMotionOwnerState,
  token: RunnerCallbackFields,
): FoldPreviewTreeMotionOwnerRejectionReason | null {
  const ownerMismatch = runnerOwnerMismatch(state, token)
  if (ownerMismatch) return ownerMismatch
  if (token.requestToken !== state.activeRequestToken) {
    return 'request_token_mismatch'
  }
  return token.requestSequence !== state.activeRequestSequence
    ? 'request_mismatch'
    : null
}

function stateFields(
  state: FoldPreviewTreeMotionOwnerState,
): Omit<FoldPreviewTreeMotionOwnerState, 'version'> {
  return {
    ownerToken: state.ownerToken,
    generation: state.generation,
    owner: state.owner,
    directPending: state.directPending,
    directKey: state.directKey,
    runnerContextKey: state.runnerContextKey,
    runnerHingeEdgeId: state.runnerHingeEdgeId,
    activeRequestSequence: state.activeRequestSequence,
    activeRequestToken: state.activeRequestToken,
    activeTargetSelectedAngleDegrees:
      state.activeTargetSelectedAngleDegrees,
    committedRequestSequence: state.committedRequestSequence,
    committedRequestToken: state.committedRequestToken,
  }
}

function createState(
  fields: Omit<FoldPreviewTreeMotionOwnerState, 'version'>,
) {
  const state = deepFreeze({
    version: FOLD_PREVIEW_TREE_MOTION_OWNER_VERSION,
    ...fields,
  }) as FoldPreviewTreeMotionOwnerState
  ownerStates.add(state)
  return state
}

function createOwnerToken(): FoldPreviewTreeMotionOwnerToken {
  const token = Object.freeze({}) as FoldPreviewTreeMotionOwnerToken
  ownerTokens.add(token)
  return token
}

function createRequestToken(): FoldPreviewTreeMotionRequestToken {
  const token = Object.freeze({}) as FoldPreviewTreeMotionRequestToken
  requestTokens.add(token)
  return token
}

function accept(
  state: FoldPreviewTreeMotionOwnerState,
  commands: readonly FoldPreviewTreeMotionOwnerCommand[],
) {
  return plan(true, null, state, commands)
}

function reject(
  state: FoldPreviewTreeMotionOwnerState,
  reason: FoldPreviewTreeMotionOwnerRejectionReason,
) {
  return plan(false, reason, state, [])
}

function plan(
  accepted: boolean,
  reason: FoldPreviewTreeMotionOwnerRejectionReason | null,
  state: FoldPreviewTreeMotionOwnerState,
  commands: readonly FoldPreviewTreeMotionOwnerCommand[],
): FoldPreviewTreeMotionOwnerPlan {
  return deepFreeze({
    accepted,
    reason,
    state,
    commands: [...commands],
  })
}

function isOwnerState(
  value: unknown,
): value is FoldPreviewTreeMotionOwnerState {
  return typeof value === 'object'
    && value !== null
    && ownerStates.has(value)
}

function isOwnerToken(
  value: unknown,
): value is FoldPreviewTreeMotionOwnerToken {
  return typeof value === 'object'
    && value !== null
    && ownerTokens.has(value)
}

function isRequestToken(
  value: unknown,
): value is FoldPreviewTreeMotionRequestToken {
  return typeof value === 'object'
    && value !== null
    && requestTokens.has(value)
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

function validPositiveCounter(value: unknown): value is number {
  return validCounter(value) && value > 0
}

function nextCounter(value: number) {
  return value < Number.MAX_SAFE_INTEGER ? value + 1 : null
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
    && value.length <= MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_KEY_LENGTH
    && value.trim().length > 0
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_FOLD_PREVIEW_TREE_MOTION_OWNER_ID_LENGTH
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
