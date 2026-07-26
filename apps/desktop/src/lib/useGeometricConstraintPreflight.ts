import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'

import {
  analyzeGeometricConstraints,
  cancelGeometricConstraintAnalysis,
  type GeometricConstraintPreflightResponse,
  type ProjectSnapshot,
} from './coreClient'
import { isCanonicalNonNilUuid as isCanonicalUuid } from './canonicalUuid.ts'

type SnapshotBinding = Readonly<
  Pick<ProjectSnapshot, 'project_instance_id' | 'project_id' | 'revision'>
>

type AnalyzeGeometricConstraints = (
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  requestGenerationId: string,
) => Promise<GeometricConstraintPreflightResponse>

type CancelGeometricConstraintAnalysis = (
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  expectedRevision: number,
  requestGenerationId: string,
) => Promise<boolean>

type IntentToken = Readonly<{
  enabled: boolean
  retrySequence: number
  snapshotPresent: boolean
}>

type CoordinatorState = Readonly<{
  intentToken: IntentToken | null
  status: 'idle' | 'analyzing' | 'result' | 'failed'
  response: GeometricConstraintPreflightResponse | null
}>

type Work = Readonly<{
  intentToken: IntentToken
  binding: SnapshotBinding
  requestGenerationId: string
}>

type Coordinator = Readonly<{
  activate(): void
  deactivate(): void
  request(intentToken: IntentToken, snapshot: SnapshotBinding): void
  clear(intentToken: IntentToken): void
  dispose(): void
}>

export type GeometricConstraintPreflightView = Readonly<{
  preflight: GeometricConstraintPreflightResponse | null
  analyzing: boolean
  failed: boolean
  retry(): void
}>

export type UseGeometricConstraintPreflightOptions = Readonly<{
  snapshot: SnapshotBinding | null
  enabled: boolean
  analyze?: AnalyzeGeometricConstraints
  cancel?: CancelGeometricConstraintAnalysis
  createRequestGenerationId?: () => string
  onFailure?: () => void
}>

const IDLE_STATE: CoordinatorState = Object.freeze({
  intentToken: null,
  status: 'idle',
  response: null,
})

/**
 * Owns the latest-only native preflight lane.
 *
 * Native exact work is cooperatively cancelled by an exact
 * project/revision/request-generation binding. One active request and at most
 * one latest pending intent are retained. Every superseded result, failure,
 * delayed cancellation, and queued intermediate snapshot is observationally
 * discarded.
 */
export function useGeometricConstraintPreflight({
  snapshot,
  enabled,
  analyze = analyzeGeometricConstraints,
  cancel = cancelGeometricConstraintAnalysis,
  createRequestGenerationId = () => crypto.randomUUID(),
  onFailure,
}: UseGeometricConstraintPreflightOptions): GeometricConstraintPreflightView {
  const analyzeRef = useRef(analyze)
  const cancelRef = useRef(cancel)
  const createRequestGenerationIdRef = useRef(createRequestGenerationId)
  const onFailureRef = useRef(onFailure)
  analyzeRef.current = analyze
  cancelRef.current = cancel
  createRequestGenerationIdRef.current = createRequestGenerationId
  onFailureRef.current = onFailure

  const [retrySequence, setRetrySequence] = useState(0)
  const [coordinatorState, setCoordinatorState] =
    useState<CoordinatorState>(IDLE_STATE)
  const currentIntentRef = useRef<IntentToken | null>(null)
  const lifecycleGenerationRef = useRef(0)

  const intentToken = useMemo<IntentToken>(
    () => Object.freeze({
      enabled,
      retrySequence,
      snapshotPresent: snapshot !== null,
    }),
    [enabled, retrySequence, snapshot],
  )

  const coordinatorRef = useRef<Coordinator | null>(null)
  if (coordinatorRef.current === null) {
    coordinatorRef.current = createCoordinator({
      analyze: (...arguments_) => analyzeRef.current(...arguments_),
      cancel: (...arguments_) => cancelRef.current(...arguments_),
      createRequestGenerationId: () => createRequestGenerationIdRef.current(),
      isCurrentIntent: (candidate) => currentIntentRef.current === candidate,
      onFailure: () => onFailureRef.current?.(),
      onState: setCoordinatorState,
    })
  }
  const coordinator = coordinatorRef.current

  const active = enabled && snapshot !== null
  useLayoutEffect(() => {
    currentIntentRef.current = active ? intentToken : null
    return () => {
      if (currentIntentRef.current === intentToken) {
        currentIntentRef.current = null
      }
    }
  }, [active, intentToken])

  useEffect(() => {
    const generation = lifecycleGenerationRef.current + 1
    lifecycleGenerationRef.current = generation
    coordinator.activate()
    return () => {
      coordinator.deactivate()
      queueMicrotask(() => {
        if (lifecycleGenerationRef.current === generation) {
          coordinator.dispose()
        }
      })
    }
  }, [coordinator])

  useEffect(() => {
    if (!active) {
      coordinator.clear(intentToken)
      return
    }
    coordinator.request(intentToken, snapshot)
  }, [active, coordinator, intentToken, snapshot])

  const retry = useCallback(() => {
    setRetrySequence((sequence) =>
      sequence === Number.MAX_SAFE_INTEGER ? 0 : sequence + 1)
  }, [])

  if (!active || coordinatorState.intentToken !== intentToken) {
    return Object.freeze({
      preflight: null,
      analyzing: active,
      failed: false,
      retry,
    })
  }
  return Object.freeze({
    preflight: coordinatorState.status === 'result'
      ? coordinatorState.response
      : null,
    analyzing: coordinatorState.status === 'analyzing',
    failed: coordinatorState.status === 'failed',
    retry,
  })
}

function createCoordinator({
  analyze,
  cancel,
  createRequestGenerationId,
  isCurrentIntent,
  onFailure,
  onState,
}: Readonly<{
  analyze: AnalyzeGeometricConstraints
  cancel: CancelGeometricConstraintAnalysis
  createRequestGenerationId(): string
  isCurrentIntent(intentToken: IntentToken): boolean
  onFailure(): void
  onState(state: CoordinatorState): void
}>): Coordinator {
  let active: Work | null = null
  let pending: Work | null = null
  let latestIntentToken: IntentToken | null = null
  let state = IDLE_STATE
  let consumerActive = false
  let disposed = false

  const cancelWork = (work: Work) => {
    try {
      void Promise.resolve(cancel(
        work.binding.project_instance_id,
        work.binding.project_id,
        work.binding.revision,
        work.requestGenerationId,
      )).catch(() => undefined)
    } catch {
      // Cancellation is best-effort; the native absolute deadline remains authoritative.
    }
  }

  const publish = (next: CoordinatorState) => {
    state = next
    if (
      consumerActive
      && (
        next.intentToken === null
        || isCurrentIntent(next.intentToken)
      )
    ) {
      onState(next)
    }
  }

  const reportCurrentFailure = (work: Work) => {
    if (!workIsCurrent(work)) return
    publish(failedState(work.intentToken))
    try {
      onFailure()
    } catch {
      // Diagnostics must never prevent the blocking state from being stored.
    }
  }

  const finish = (
    work: Work,
    outcome:
      | Readonly<{ ok: true; response: GeometricConstraintPreflightResponse }>
      | Readonly<{ ok: false }>,
  ) => {
    if (active !== work) return
    active = null

    if (workIsCurrent(work)) {
      if (
        outcome.ok
        && responseMatchesBinding(outcome.response, work.binding)
      ) {
        publish(resultState(work.intentToken, outcome.response))
      } else {
        reportCurrentFailure(work)
      }
    }

    const next = pending
    pending = null
    if (
      next !== null
      && consumerActive
      && !disposed
      && latestIntentToken === next.intentToken
      && isCurrentIntent(next.intentToken)
    ) {
      start(next)
    }
  }

  const start = (work: Work) => {
    if (disposed || !consumerActive) return
    active = work
    let task: Promise<GeometricConstraintPreflightResponse>
    try {
      task = analyze(
        work.binding.project_instance_id,
        work.binding.project_id,
        work.binding.revision,
        work.requestGenerationId,
      )
    } catch {
      finish(work, { ok: false })
      return
    }
    void Promise.resolve(task).then(
      (response) => finish(work, { ok: true, response }),
      () => finish(work, { ok: false }),
    )
  }

  const workIsCurrent = (work: Work) => (
    !disposed
    && consumerActive
    && latestIntentToken === work.intentToken
    && isCurrentIntent(work.intentToken)
  )

  return Object.freeze({
    activate() {
      if (disposed) return
      consumerActive = true
      if (
        state.intentToken === null
        || isCurrentIntent(state.intentToken)
      ) {
        onState(state)
      }
    },
    deactivate() {
      consumerActive = false
    },
    request(intentToken, snapshot) {
      if (disposed || !consumerActive || latestIntentToken === intentToken) return
      latestIntentToken = intentToken

      const binding = detachBinding(snapshot)
      if (binding === null) {
        if (active !== null) cancelWork(active)
        pending = null
        const invalidWork = Object.freeze({
          intentToken,
          binding: snapshot,
          requestGenerationId: '',
        })
        reportCurrentFailure(invalidWork)
        return
      }

      let requestGenerationId: string
      try {
        requestGenerationId = createRequestGenerationId()
      } catch {
        if (active !== null) cancelWork(active)
        pending = null
        const invalidWork = Object.freeze({
          intentToken,
          binding,
          requestGenerationId: '',
        })
        reportCurrentFailure(invalidWork)
        return
      }
      if (!isCanonicalUuid(requestGenerationId)) {
        if (active !== null) cancelWork(active)
        pending = null
        const invalidWork = Object.freeze({
          intentToken,
          binding,
          requestGenerationId,
        })
        reportCurrentFailure(invalidWork)
        return
      }

      const work = Object.freeze({ intentToken, binding, requestGenerationId })
      publish(analyzingState(intentToken))
      if (active === null) {
        start(work)
      } else {
        cancelWork(active)
        pending = work
      }
    },
    clear(intentToken) {
      if (disposed || latestIntentToken === intentToken) return
      latestIntentToken = intentToken
      if (active !== null) cancelWork(active)
      pending = null
      publish(idleState(intentToken))
    },
    dispose() {
      if (disposed) return
      disposed = true
      consumerActive = false
      latestIntentToken = null
      if (active !== null) cancelWork(active)
      active = null
      pending = null
      state = IDLE_STATE
    },
  })
}

function detachBinding(snapshot: SnapshotBinding): SnapshotBinding | null {
  try {
    if (
      snapshot === null
      || typeof snapshot !== 'object'
      || Array.isArray(snapshot)
    ) return null
    const prototype = Object.getPrototypeOf(snapshot)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(snapshot)
    const projectInstanceId = dataValue(descriptors, 'project_instance_id')
    const projectId = dataValue(descriptors, 'project_id')
    const revision = dataValue(descriptors, 'revision')
    if (
      !isCanonicalUuid(projectInstanceId)
      || !isCanonicalUuid(projectId)
      || !isRevision(revision)
    ) return null
    return Object.freeze({
      project_instance_id: projectInstanceId,
      project_id: projectId,
      revision,
    })
  } catch {
    return null
  }
}

function responseMatchesBinding(
  response: GeometricConstraintPreflightResponse,
  binding: SnapshotBinding,
) {
  try {
    if (
      response === null
      || typeof response !== 'object'
      || Array.isArray(response)
    ) return false
    const descriptors = Object.getOwnPropertyDescriptors(response)
    return dataValue(descriptors, 'project_instance_id')
        === binding.project_instance_id
      && dataValue(descriptors, 'project_id') === binding.project_id
      && dataValue(descriptors, 'revision') === binding.revision
  } catch {
    return false
  }
}

function dataValue(
  descriptors: Readonly<Record<PropertyKey, PropertyDescriptor>>,
  key: string,
) {
  const descriptor = descriptors[key]
  return descriptor && 'value' in descriptor && descriptor.enumerable
    ? descriptor.value
    : undefined
}


function isRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
}

function idleState(intentToken: IntentToken): CoordinatorState {
  return Object.freeze({
    intentToken,
    status: 'idle',
    response: null,
  })
}

function analyzingState(intentToken: IntentToken): CoordinatorState {
  return Object.freeze({
    intentToken,
    status: 'analyzing',
    response: null,
  })
}

function resultState(
  intentToken: IntentToken,
  response: GeometricConstraintPreflightResponse,
): CoordinatorState {
  return Object.freeze({
    intentToken,
    status: 'result',
    response,
  })
}

function failedState(intentToken: IntentToken): CoordinatorState {
  return Object.freeze({
    intentToken,
    status: 'failed',
    response: null,
  })
}
