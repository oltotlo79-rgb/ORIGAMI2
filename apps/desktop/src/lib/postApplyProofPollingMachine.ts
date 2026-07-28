import {
  normalizePostApplyProofProgressV1,
  samePostApplyProofJobBindingV1,
  type PostApplyProofJobRequestV1,
  type PostApplyProofProgressV1,
} from './postApplyProofSchedulerClient.ts'

export const POST_APPLY_PROOF_POLL_INTERVAL_MS_V1 = 250

export type PostApplyProofPollingFailureReasonV1 =
  | 'invalid_job'
  | 'invalid_response'
  | 'transport_failure'
  | 'scheduler_failure'
  | 'generation_exhausted'

export type PostApplyProofPollingStateV1 =
  | Readonly<{ status: 'idle'; generation: number }>
  | Readonly<{
      status: 'polling'
      generation: number
      progress: PostApplyProofProgressV1
    }>
  | Readonly<{
      status: 'terminal'
      generation: number
      progress: PostApplyProofProgressV1
    }>
  | Readonly<{
      status: 'failed'
      generation: number
      reason: PostApplyProofPollingFailureReasonV1
    }>
  | Readonly<{ status: 'cancelled'; generation: number }>

type TimerHandle = ReturnType<typeof setTimeout>

type PollingRun = {
  generation: number
  request: PostApplyProofJobRequestV1
  progress: PostApplyProofProgressV1
  inFlight: boolean
}

export function createPostApplyProofPollingMachineV1(
  options: Readonly<{
    poll(request: PostApplyProofJobRequestV1): Promise<unknown>
    cancel?(request: PostApplyProofJobRequestV1): Promise<unknown>
    setTimer?(
      callback: () => void,
      delay: number,
    ): TimerHandle
    clearTimer?(handle: TimerHandle): void
    pollIntervalMs?: number
    initialGeneration?: number
    onState(state: PostApplyProofPollingStateV1): void
  }>,
) {
  const setTimer = options.setTimer ?? setTimeout
  const clearTimer = options.clearTimer ?? clearTimeout
  const pollIntervalMs = isSafeDelay(options.pollIntervalMs)
    ? options.pollIntervalMs
    : POST_APPLY_PROOF_POLL_INTERVAL_MS_V1

  let generation = isSafeCount(options.initialGeneration)
    ? options.initialGeneration
    : 0
  let disposed = false
  let timer: TimerHandle | null = null
  let timerToken: object | null = null
  let active: PollingRun | null = null
  let state: PostApplyProofPollingStateV1 = Object.freeze({
    status: 'idle',
    generation,
  })

  const publish = (next: PostApplyProofPollingStateV1) => {
    state = Object.freeze(next)
    try {
      options.onState(state)
    } catch {
      // State observation is non-authoritative.
    }
  }

  const stopTimer = () => {
    timerToken = null
    const handle = timer
    timer = null
    if (handle !== null) {
      try {
        clearTimer(handle)
      } catch {
        // The run identity still rejects a late timer callback.
      }
    }
  }

  const cancelRemote = (request: PostApplyProofJobRequestV1) => {
    if (!options.cancel) return
    try {
      void Promise.resolve(options.cancel(request)).catch(() => undefined)
    } catch {
      // Cancellation is best-effort and exposes no native error detail.
    }
  }

  const isCurrent = (run: PollingRun) =>
    !disposed && active === run && generation === run.generation

  const fail = (
    run: PollingRun,
    reason: Exclude<PostApplyProofPollingFailureReasonV1, 'invalid_job'>,
  ) => {
    if (!isCurrent(run)) return
    stopTimer()
    active = null
    cancelRemote(run.request)
    publish(Object.freeze({
      status: 'failed',
      generation: run.generation,
      reason,
    }))
  }

  const schedule = (run: PollingRun) => {
    if (!isCurrent(run) || run.inFlight) return
    const token = Object.freeze({})
    timerToken = token
    let firedSynchronously = false
    try {
      const handle = setTimer(() => {
        firedSynchronously = true
        if (timerToken !== token || !isCurrent(run)) return
        timerToken = null
        timer = null
        beginPoll(run)
      }, pollIntervalMs)
      if (timerToken === token && !firedSynchronously) {
        timer = handle
      } else {
        try {
          clearTimer(handle)
        } catch {
          // A synchronously fired callback is already identity-guarded.
        }
      }
    } catch {
      if (timerToken === token) timerToken = null
      fail(run, 'scheduler_failure')
    }
  }

  const acceptResponse = (run: PollingRun, raw: unknown) => {
    if (!isCurrent(run)) return
    const progress = normalizePostApplyProofProgressV1(raw)
    // V1 pins one immutable work set per job and monotone proof completion.
    if (
      !progress
      || !samePostApplyProofJobBindingV1(progress, run.request)
      || progress.totalPairCount !== run.progress.totalPairCount
      || progress.provenPairCount < run.progress.provenPairCount
    ) {
      fail(run, 'invalid_response')
      return
    }
    run.inFlight = false
    run.progress = progress
    if (progress.status !== 'proving') {
      stopTimer()
      active = null
      publish(Object.freeze({
        status: 'terminal',
        generation: run.generation,
        progress,
      }))
      return
    }
    run.request = jobRequestFromProgress(progress)
    publish(Object.freeze({
      status: 'polling',
      generation: run.generation,
      progress,
    }))
    schedule(run)
  }

  function beginPoll(run: PollingRun): boolean {
    if (!isCurrent(run) || run.inFlight) return false
    run.inFlight = true
    let pending: Promise<unknown>
    try {
      pending = Promise.resolve(options.poll(run.request))
    } catch {
      run.inFlight = false
      fail(run, 'transport_failure')
      return true
    }
    void pending.then(
      (raw) => acceptResponse(run, raw),
      () => {
        if (!isCurrent(run)) return
        run.inFlight = false
        fail(run, 'transport_failure')
      },
    )
    return true
  }

  function pollNow(): boolean {
    const run = active
    if (!run || !isCurrent(run) || run.inFlight) return false
    stopTimer()
    return beginPoll(run)
  }

  const start = (initial: unknown): boolean => {
    if (disposed) return false
    if (generation >= Number.MAX_SAFE_INTEGER) {
      stopTimer()
      const previous = active
      active = null
      if (previous) cancelRemote(previous.request)
      publish(Object.freeze({
        status: 'failed',
        generation,
        reason: 'generation_exhausted',
      }))
      return false
    }
    stopTimer()
    const previous = active
    active = null
    if (previous) cancelRemote(previous.request)
    generation += 1

    const progress = normalizePostApplyProofProgressV1(initial)
    if (!progress) {
      publish(Object.freeze({
        status: 'failed',
        generation,
        reason: 'invalid_job',
      }))
      return false
    }
    if (progress.status !== 'proving') {
      publish(Object.freeze({
        status: 'terminal',
        generation,
        progress,
      }))
      return true
    }

    const run: PollingRun = {
      generation,
      request: jobRequestFromProgress(progress),
      progress,
      inFlight: false,
    }
    active = run
    publish(Object.freeze({
      status: 'polling',
      generation,
      progress,
    }))
    schedule(run)
    return true
  }

  const cancel = (): boolean => {
    const previous = active
    if (disposed || !previous) return false
    stopTimer()
    active = null
    if (generation < Number.MAX_SAFE_INTEGER) generation += 1
    cancelRemote(previous.request)
    publish(Object.freeze({ status: 'cancelled', generation }))
    return true
  }

  const dispose = () => {
    if (disposed) return
    disposed = true
    stopTimer()
    const previous = active
    active = null
    if (generation < Number.MAX_SAFE_INTEGER) generation += 1
    if (previous) cancelRemote(previous.request)
    publish(Object.freeze({ status: 'idle', generation }))
  }

  return Object.freeze({
    start,
    pollNow,
    cancel,
    dispose,
    getState: () => state,
  })
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

function isSafeDelay(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && Number(value) >= 0
    && !Object.is(value, -0)
}

function isSafeCount(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && Number(value) >= 0
    && !Object.is(value, -0)
}
