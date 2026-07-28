import {
  cancelPostApplyProofJobV1,
  normalizePostApplyProofProgressV1,
  normalizeStartPostApplyProofJobRequestV1,
  pollPostApplyProofJobV1,
  samePostApplyProofJobBindingV1,
  startPostApplyProofJobV1,
  type PostApplyProofJobRequestV1,
  type PostApplyProofProgressV1,
  type PostApplyProofSchedulerClientV1,
  type StartPostApplyProofJobRequestV1,
} from './postApplyProofSchedulerClient.ts'
import {
  createPostApplyProofPollingMachineV1,
  type PostApplyProofPollingStateV1,
} from './postApplyProofPollingMachine.ts'

export type PostApplyProofSchedulerViewStateV1 =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'starting' }>
  | Readonly<{
      kind: 'progress'
      progress: PostApplyProofProgressV1
    }>
  | Readonly<{ kind: 'unavailable' }>

type SchedulerClient = Readonly<{
  start(request: StartPostApplyProofJobRequestV1): Promise<unknown>
  poll(request: PostApplyProofJobRequestV1): Promise<unknown>
  cancel(request: PostApplyProofJobRequestV1): Promise<unknown>
}>

const DEFAULT_CLIENT: SchedulerClient = Object.freeze({
  start: startPostApplyProofJobV1,
  poll: pollPostApplyProofJobV1,
  cancel: cancelPostApplyProofJobV1,
})

/**
 * Owns the asynchronous start boundary in front of the single-flight polling
 * machine. The native command vocabulary stays confined to the client adapter;
 * this coordinator deals only in the validated v1 domain contract.
 */
export function createPostApplyProofSchedulerCoordinatorV1(
  options: Readonly<{
    client?: SchedulerClient | PostApplyProofSchedulerClientV1
    onState(state: PostApplyProofSchedulerViewStateV1): void
  }>,
) {
  const client = options.client ?? DEFAULT_CLIENT
  let disposed = false
  let generation = 0
  let terminalRefreshGeneration = 0
  let activeBinding: StartPostApplyProofJobRequestV1 | null = null
  let observedRevision: number | null = null
  let state: PostApplyProofSchedulerViewStateV1 = Object.freeze({
    kind: 'idle',
  })

  const publish = (next: PostApplyProofSchedulerViewStateV1) => {
    state = Object.freeze(next)
    try {
      options.onState(state)
    } catch {
      // Rendering is non-authoritative and must not break cancellation.
    }
  }

  const polling = createPostApplyProofPollingMachineV1({
    poll: (request) => client.poll(request),
    cancel: (request) => client.cancel(request),
    onState(next) {
      acceptPollingState(next)
    },
  })

  function acceptPollingState(next: PostApplyProofPollingStateV1) {
    if (disposed) return
    if (next.status === 'polling' || next.status === 'terminal') {
      publish(Object.freeze({
        kind: 'progress',
        progress: next.progress,
      }))
    } else if (next.status === 'failed') {
      // Never expose native or transport failure detail through the view.
      publish(Object.freeze({ kind: 'unavailable' }))
    }
  }

  const cancelLateInitialJob = (progress: PostApplyProofProgressV1) => {
    const request = jobRequestFromProgress(progress)
    try {
      void Promise.resolve(client.cancel(request)).catch(() => undefined)
    } catch {
      // Best-effort cleanup; no remote failure detail is retained.
    }
  }

  const cancel = (): boolean => {
    if (disposed) return false
    generation = nextGeneration(generation)
    terminalRefreshGeneration = nextGeneration(terminalRefreshGeneration)
    const hadWork = activeBinding !== null || state.kind !== 'idle'
    activeBinding = null
    observedRevision = null
    polling.cancel()
    if (hadWork) publish(Object.freeze({ kind: 'idle' }))
    return hadWork
  }

  const start = (request: unknown): boolean => {
    if (disposed) return false
    const normalized = normalizeStartPostApplyProofJobRequestV1(request)
    cancel()
    if (!normalized) {
      publish(Object.freeze({ kind: 'unavailable' }))
      return false
    }
    if (generation === Number.MAX_SAFE_INTEGER) {
      publish(Object.freeze({ kind: 'unavailable' }))
      return false
    }
    generation += 1
    const runGeneration = generation
    activeBinding = normalized
    observedRevision = normalized.revision
    publish(Object.freeze({ kind: 'starting' }))

    let pending: Promise<unknown>
    try {
      pending = Promise.resolve(client.start(normalized))
    } catch {
      if (isCurrent(runGeneration, normalized)) {
        publish(Object.freeze({ kind: 'unavailable' }))
      }
      return true
    }
    void pending.then(
      (raw) => {
        const progress = normalizePostApplyProofProgressV1(raw)
        if (!progress) {
          if (isCurrent(runGeneration, normalized)) {
            publish(Object.freeze({ kind: 'unavailable' }))
          }
          return
        }
        if (!isCurrent(runGeneration, normalized)) {
          cancelLateInitialJob(progress)
          return
        }
        if (!sameStartBinding(progress, normalized)) {
          cancelLateInitialJob(progress)
          publish(Object.freeze({ kind: 'unavailable' }))
          return
        }
        polling.start(progress)
        if (observedRevision !== normalized.revision) {
          refreshTerminalReport()
        }
      },
      () => {
        if (!isCurrent(runGeneration, normalized)) return
        publish(Object.freeze({ kind: 'unavailable' }))
      },
    )
    return true
  }

  const observeAuthority = (request: unknown): boolean => {
    if (disposed) return false
    if (activeBinding === null) {
      if (state.kind === 'idle') return false
      cancel()
      return true
    }
    const normalized = normalizeStartPostApplyProofJobRequestV1(request)
    if (normalized && sameProjectIdentity(normalized, activeBinding)) {
      const revisionChanged = normalized.revision !== observedRevision
      observedRevision = normalized.revision
      if (revisionChanged) refreshTerminalReport()
      return false
    }
    cancel()
    return true
  }

  const markUnavailable = (): boolean => {
    if (disposed) return false
    cancel()
    publish(Object.freeze({ kind: 'unavailable' }))
    return true
  }

  const dispose = () => {
    if (disposed) return
    generation = nextGeneration(generation)
    terminalRefreshGeneration = nextGeneration(terminalRefreshGeneration)
    activeBinding = null
    observedRevision = null
    disposed = true
    polling.dispose()
    state = Object.freeze({ kind: 'idle' })
  }

  function isCurrent(
    runGeneration: number,
    binding: StartPostApplyProofJobRequestV1,
  ): boolean {
    return !disposed
      && generation === runGeneration
      && activeBinding !== null
      && sameStartBinding(activeBinding, binding)
  }

  function refreshTerminalReport(): boolean {
    if (
      disposed
      || activeBinding === null
      || state.kind !== 'progress'
      || state.progress.proofFailure === null
      || terminalRefreshGeneration >= Number.MAX_SAFE_INTEGER
    ) return false
    terminalRefreshGeneration += 1
    const refreshGeneration = terminalRefreshGeneration
    const runGeneration = generation
    const previous = state.progress
    let pending: Promise<unknown>
    try {
      pending = Promise.resolve(client.poll(jobRequestFromProgress(previous)))
    } catch {
      if (terminalRefreshIsCurrent(
        runGeneration,
        refreshGeneration,
        previous,
      )) {
        publish(Object.freeze({ kind: 'unavailable' }))
      }
      return true
    }
    void pending.then(
      (raw) => {
        if (!terminalRefreshIsCurrent(
          runGeneration,
          refreshGeneration,
          previous,
        )) return
        const progress = normalizePostApplyProofProgressV1(raw)
        if (
          !progress
          || !samePostApplyProofJobBindingV1(progress, previous)
          || progress.status !== previous.status
          || progress.totalPairCount !== previous.totalPairCount
          || progress.proofFailure === null
        ) {
          publish(Object.freeze({ kind: 'unavailable' }))
          return
        }
        publish(Object.freeze({ kind: 'progress', progress }))
      },
      () => {
        if (terminalRefreshIsCurrent(
          runGeneration,
          refreshGeneration,
          previous,
        )) {
          publish(Object.freeze({ kind: 'unavailable' }))
        }
      },
    )
    return true
  }

  function terminalRefreshIsCurrent(
    runGeneration: number,
    refreshGeneration: number,
    previous: PostApplyProofProgressV1,
  ): boolean {
    return !disposed
      && generation === runGeneration
      && terminalRefreshGeneration === refreshGeneration
      && activeBinding !== null
      && sameProjectIdentity(activeBinding, previous)
      && state.kind === 'progress'
      && samePostApplyProofJobBindingV1(state.progress, previous)
  }

  return Object.freeze({
    start,
    cancel,
    observeAuthority,
    refreshTerminalReport,
    markUnavailable,
    dispose,
    getState: () => state,
  })
}

function sameStartBinding(
  left: StartPostApplyProofJobRequestV1,
  right: StartPostApplyProofJobRequestV1,
): boolean {
  return left.version === right.version
    && left.projectInstanceId === right.projectInstanceId
    && left.projectId === right.projectId
    && left.revision === right.revision
}

function sameProjectIdentity(
  left: StartPostApplyProofJobRequestV1,
  right: StartPostApplyProofJobRequestV1,
): boolean {
  return left.version === right.version
    && left.projectInstanceId === right.projectInstanceId
    && left.projectId === right.projectId
}

function jobRequestFromProgress(
  progress: PostApplyProofProgressV1,
): PostApplyProofJobRequestV1 {
  return Object.freeze({
    version: progress.version,
    projectInstanceId: progress.projectInstanceId,
    projectId: progress.projectId,
    revision: progress.revision,
    jobToken: progress.jobToken,
  })
}

function nextGeneration(value: number): number {
  return value < Number.MAX_SAFE_INTEGER ? value + 1 : value
}
