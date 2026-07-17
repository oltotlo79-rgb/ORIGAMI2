export type FrameTaskScheduler = Readonly<{
  request: (callback: () => void) => number
  cancel: (handle: number) => void
}>

export type LatestFrameTask<T> = Readonly<{
  /** Replaces any pending value and requests at most one callback per frame. */
  schedule: (value: T) => boolean
  hasPending: () => boolean
  dispose: () => void
}>

/**
 * Coalesces high-frequency input into the latest value for the next frame.
 *
 * The injected scheduler keeps this deterministic in tests and avoids coupling
 * fold/collision logic to browser globals.
 */
export function createLatestFrameTask<T>(
  scheduler: FrameTaskScheduler,
  run: (value: T) => void,
  onError: (error: unknown) => void = () => undefined,
): LatestFrameTask<T> {
  let disposed = false
  let handle: number | null = null
  let pendingValue: T | undefined
  let pending = false

  const reportError = (error: unknown) => {
    try {
      onError(error)
    } catch {
      // Error reporting must never break scheduler cleanup.
    }
  }

  const requestFrame = () => {
    try {
      const nextHandle = scheduler.request(() => {
        handle = null
        if (disposed || !pending) return
        const value = pendingValue as T
        pending = false
        pendingValue = undefined
        try {
          run(value)
        } catch (error) {
          reportError(error)
        }
      })
      if (!Number.isFinite(nextHandle)) {
        pending = false
        pendingValue = undefined
        return false
      }
      handle = nextHandle
      return true
    } catch {
      pending = false
      pendingValue = undefined
      return false
    }
  }

  return {
    schedule: (value) => {
      if (disposed) return false
      pendingValue = value
      pending = true
      return handle !== null || requestFrame()
    },
    hasPending: () => !disposed && pending,
    dispose: () => {
      if (disposed) return
      disposed = true
      pending = false
      pendingValue = undefined
      if (handle !== null) {
        try {
          scheduler.cancel(handle)
        } catch {
          // The task is already inert; cancellation failure cannot revive it.
        }
        handle = null
      }
    },
  }
}
