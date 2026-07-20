import {
  AssignedLocalSufficiencySummaryError,
  cancelCurrentAssignedLocalSufficiencySummaryV1,
  summarizeCurrentAssignedLocalSufficiencyV1,
  type AssignedLocalSufficiencySummaryResponseV1,
} from './coreClient.ts'

export const ASSIGNED_LOCAL_SUMMARY_RETRY_DELAY_MS = 25
export const ASSIGNED_LOCAL_SUMMARY_MAX_RETRIES = 20
export const ASSIGNED_LOCAL_SUMMARY_MAX_RETRY_MS = 1_000

export type AssignedLocalSummaryContext = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  expectedFoldModelFingerprint: string
}>
export type AssignedLocalSummaryState =
  | Readonly<{ status: 'idle'; generation: number }>
  | Readonly<{ status: 'running' | 'retrying'; generation: number; retryCount: number }>
  | Readonly<{ status: 'ready'; generation: number; response: AssignedLocalSufficiencySummaryResponseV1 }>
  | Readonly<{ status: 'failed'; generation: number; reason: 'busy' | 'native_failure' }>

export function createAssignedLocalSufficiencySummaryCoordinator(options: Readonly<{
  analyze?: typeof summarizeCurrentAssignedLocalSufficiencyV1
  cancel?: typeof cancelCurrentAssignedLocalSufficiencySummaryV1
  setTimer?: (callback: () => void, delay: number) => ReturnType<typeof setTimeout>
  clearTimer?: (handle: ReturnType<typeof setTimeout>) => void
  now?: () => number
  onState(state: AssignedLocalSummaryState): void
}>) {
  const analyze = options.analyze ?? summarizeCurrentAssignedLocalSufficiencyV1
  const cancelNative = options.cancel ?? cancelCurrentAssignedLocalSufficiencySummaryV1
  const setTimer = options.setTimer ?? setTimeout
  const clearTimer = options.clearTimer ?? clearTimeout
  const now = options.now ?? Date.now
  let generation = 0
  let disposed = false
  let timer: ReturnType<typeof setTimeout> | null = null
  let state: AssignedLocalSummaryState = { status: 'idle', generation }

  const publish = (next: AssignedLocalSummaryState) => {
    state = next
    try { options.onState(next) } catch { /* observational */ }
  }
  const stopTimer = () => {
    if (timer !== null) {
      try { clearTimer(timer) } catch { /* best-effort cleanup */ }
    }
    timer = null
  }
  const isCurrent = (value: number) => !disposed && generation === value

  const start = (context: AssignedLocalSummaryContext) => {
    if (disposed || generation >= Number.MAX_SAFE_INTEGER) return false
    stopTimer()
    void cancelNative().catch(() => undefined)
    generation += 1
    const ownGeneration = generation
    let startedAt: number
    try {
      startedAt = now()
    } catch {
      publish({ status: 'failed', generation, reason: 'native_failure' })
      return true
    }
    const attempt = (retryCount: number) => {
      if (!isCurrent(ownGeneration)) return
      publish({ status: retryCount === 0 ? 'running' : 'retrying', generation, retryCount })
      void analyze(context).then((response) => {
        if (!isCurrent(ownGeneration)
          || response.projectInstanceId !== context.expectedProjectInstanceId
          || response.projectId !== context.expectedProjectId
          || response.revision !== context.expectedRevision
          || response.foldModelFingerprint !== context.expectedFoldModelFingerprint) return
        publish({ status: 'ready', generation, response })
      }).catch((error) => {
        if (!isCurrent(ownGeneration)) return
        const busy = error instanceof AssignedLocalSufficiencySummaryError
          && error.reason === 'busy'
        let withinRetryWindow = false
        if (busy && retryCount < ASSIGNED_LOCAL_SUMMARY_MAX_RETRIES) {
          try {
            withinRetryWindow = now() - startedAt < ASSIGNED_LOCAL_SUMMARY_MAX_RETRY_MS
          } catch {
            publish({ status: 'failed', generation, reason: 'native_failure' })
            return
          }
        }
        if (withinRetryWindow) {
          try {
            timer = setTimer(() => {
              timer = null
              attempt(retryCount + 1)
            }, ASSIGNED_LOCAL_SUMMARY_RETRY_DELAY_MS)
            return
          } catch {
            publish({ status: 'failed', generation, reason: 'native_failure' })
            return
          }
        }
        publish({
          status: 'failed',
          generation,
          reason: busy ? 'busy' : 'native_failure',
        })
      })
    }
    attempt(0)
    return true
  }
  const dispose = () => {
    if (disposed) return
    disposed = true
    stopTimer()
    void cancelNative().catch(() => undefined)
    if (generation < Number.MAX_SAFE_INTEGER) generation += 1
    publish({ status: 'idle', generation })
  }
  return Object.freeze({ start, dispose, getState: () => state })
}
