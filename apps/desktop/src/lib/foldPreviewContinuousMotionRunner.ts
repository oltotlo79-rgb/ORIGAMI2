import type {
  FoldPreviewContinuousMotionJob,
  FoldPreviewContinuousMotionResult,
  FoldPreviewContinuousMotionStats,
  FoldPreviewContinuousMotionStep,
} from './foldPreviewContinuousMotion'

export type FoldPreviewContinuousMotionRunnerStatus =
  | 'idle'
  | 'running'
  | 'clear'
  | 'blocked'
  | 'indeterminate'
  | 'disposed'

export type FoldPreviewContinuousMotionRunnerState<Blocker = unknown> =
  Readonly<{
    /** The angle requested for the active or most recently completed run. */
    requested: number | null
    /** The last angle that `applyAngle` accepted. */
    applied: number
    /** The applied angle from which the current or last run was created. */
    start: number
    status: FoldPreviewContinuousMotionRunnerStatus
    /** Stable runner failure reason or the motion job's unknown-path reason. */
    reason: string | null
    /** A validated terminal motion result, otherwise `null`. */
    result: FoldPreviewContinuousMotionResult<Blocker> | null
  }>

export type FoldPreviewContinuousMotionRunnerOptions<
  Blocker = unknown,
  ScheduledHandle = unknown,
> = Readonly<{
  initialAngle: number
  schedule(callback: () => void): ScheduledHandle
  cancel(handle: ScheduledHandle): void
  jobFactory(
    startAngle: number,
    targetAngle: number,
  ): FoldPreviewContinuousMotionJob<Blocker> | null
  /**
   * Applies the job's latest non-advancing or certified boundary angle to the
   * view. At time zero this can be an unverified or blocking start pose; only
   * a positive certified time proves motion beyond it. An exact `true`
   * commits the displayed angle as the next run's starting point.
   */
  applyAngle(angle: number): boolean
  onState(state: FoldPreviewContinuousMotionRunnerState<Blocker>): void
}>

export type FoldPreviewContinuousMotionRunner<Blocker = unknown> =
  Readonly<{
    /**
     * Replaces any active request. Returns `false` when the request cannot be
     * started without losing a fail-closed guarantee.
     */
    request(targetAngle: number): boolean
    /** Cancels all work permanently. Repeated calls are harmless. */
    dispose(): void
    getState(): FoldPreviewContinuousMotionRunnerState<Blocker>
  }>

type ScheduledFrame<ScheduledHandle> = {
  handle: ScheduledHandle | undefined
  handleReady: boolean
  cancelRequested: boolean
  invoked: boolean
}

type NormalizedStep<Blocker> = Readonly<{
  value: FoldPreviewContinuousMotionStep<Blocker>
  certifiedSafeThrough: number
}>

const RUNNER_REASON = Object.freeze({
  invalidTarget: 'invalid_target_angle',
  factoryError: 'job_factory_error',
  factoryNull: 'job_factory_returned_null',
  factoryMalformed: 'job_factory_returned_malformed_job',
  scheduleError: 'scheduler_error',
  stepError: 'job_step_error',
  malformedStep: 'malformed_job_step',
  nonMonotonicStep: 'non_monotonic_certified_time',
  interpolationError: 'angle_interpolation_error',
  applyError: 'apply_angle_error',
  applyRejected: 'apply_angle_rejected',
})

/**
 * Drives one continuous single-angle motion job with exactly one unit of work
 * per scheduled callback.
 *
 * The runner never publishes a newly advanced angle until `applyAngle`
 * accepts it. Superseded generations remain inert even if a cancelled
 * scheduler callback is invoked later.
 */
export function createFoldPreviewContinuousMotionRunner<
  Blocker = unknown,
  ScheduledHandle = unknown,
>(
  options: FoldPreviewContinuousMotionRunnerOptions<Blocker, ScheduledHandle>,
): FoldPreviewContinuousMotionRunner<Blocker> | null {
  const resolved = resolveOptions(options)
  if (!resolved) return null
  const {
    initialAngle,
    schedule,
    cancel,
    jobFactory,
    applyAngle,
    onState,
  } = resolved

  let disposed = false
  let applyingAngle = false
  let disposeAfterApply = false
  let generation = 0
  let activeJob: FoldPreviewContinuousMotionJob<Blocker> | null = null
  let activeFrame: ScheduledFrame<ScheduledHandle> | null = null
  let lastAppliedAngle = initialAngle
  let state = freezeState<Blocker>({
    requested: null,
    applied: lastAppliedAngle,
    start: lastAppliedAngle,
    status: 'idle',
    reason: null,
    result: null,
  })

  const notify = () => {
    try {
      onState(state)
    } catch {
      // State reporting is observational and must not affect motion safety.
    }
  }

  const replaceState = (
    next: FoldPreviewContinuousMotionRunnerState<Blocker>,
  ) => {
    state = freezeState(next)
    notify()
  }

  const cancelFrame = (frame: ScheduledFrame<ScheduledHandle> | null) => {
    if (!frame) return
    frame.cancelRequested = true
    if (!frame.handleReady) return
    try {
      cancel(frame.handle as ScheduledHandle)
    } catch {
      // The generation check remains authoritative if scheduler cancellation
      // itself fails.
    }
  }

  const cancelJob = (job: FoldPreviewContinuousMotionJob<Blocker> | null) => {
    if (!job) return
    try {
      job.cancel()
    } catch {
      // A replaced generation can no longer apply or publish a result.
    }
  }

  const invalidateActive = () => {
    const job = activeJob
    const frame = activeFrame
    activeJob = null
    activeFrame = null
    generation += 1
    const invalidationGeneration = generation
    cancelJob(job)
    cancelFrame(frame)
    return invalidationGeneration
  }

  const failCurrent = (
    runGeneration: number,
    job: FoldPreviewContinuousMotionJob<Blocker>,
    start: number,
    target: number,
    reason: string,
  ) => {
    if (
      disposed
      || generation !== runGeneration
      || activeJob !== job
    ) return
    const failureGeneration = invalidateActive()
    if (disposed || generation !== failureGeneration) return
    replaceState({
      requested: target,
      applied: lastAppliedAngle,
      start,
      status: 'indeterminate',
      reason,
      result: null,
    })
  }

  const publishFactoryFailure = (
    runGeneration: number,
    start: number,
    target: number,
    reason: string,
  ) => {
    if (disposed || generation !== runGeneration) return
    generation += 1
    replaceState({
      requested: target,
      applied: lastAppliedAngle,
      start,
      status: 'indeterminate',
      reason,
      result: null,
    })
  }

  const disposeNow = () => {
    if (disposed) return
    disposed = true
    invalidateActive()
    disposeAfterApply = false
    replaceState({
      requested: state.requested,
      applied: lastAppliedAngle,
      start: state.start,
      status: 'disposed',
      reason: null,
      result: null,
    })
  }

  const scheduleNext = (
    runGeneration: number,
    job: FoldPreviewContinuousMotionJob<Blocker>,
    start: number,
    target: number,
    previousCertifiedTime: number,
  ): boolean => {
    if (
      disposed
      || generation !== runGeneration
      || activeJob !== job
    ) return false

    const frame: ScheduledFrame<ScheduledHandle> = {
      handle: undefined,
      handleReady: false,
      cancelRequested: false,
      invoked: false,
    }
    activeFrame = frame

    const callback = () => {
      if (frame.invoked) return
      frame.invoked = true
      if (activeFrame === frame) activeFrame = null
      if (
        disposed
        || generation !== runGeneration
        || activeJob !== job
      ) return

      let rawStep: unknown
      try {
        rawStep = job.step(1)
      } catch {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          RUNNER_REASON.stepError,
        )
        return
      }

      if (
        disposed
        || generation !== runGeneration
        || activeJob !== job
      ) return

      let normalized: NormalizedStep<Blocker> | null
      try {
        normalized = normalizeStep<Blocker>(rawStep)
      } catch {
        normalized = null
      }
      if (!normalized) {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          RUNNER_REASON.malformedStep,
        )
        return
      }

      const { value, certifiedSafeThrough } = normalized
      if (value.kind === 'cancelled') {
        invalidateActive()
        return
      }
      if (certifiedSafeThrough < previousCertifiedTime) {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          RUNNER_REASON.nonMonotonicStep,
        )
        return
      }

      const certifiedAngle = value.kind === 'clear'
        ? target
        : interpolateAngle(start, target, certifiedSafeThrough)
      if (certifiedAngle === null) {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          RUNNER_REASON.interpolationError,
        )
        return
      }

      let applied = false
      let applyFailureReason: string | null = null
      applyingAngle = true
      try {
        applied = applyAngle(certifiedAngle) === true
      } catch {
        applyFailureReason = RUNNER_REASON.applyError
      } finally {
        applyingAngle = false
      }

      if (disposeAfterApply) {
        if (applied) lastAppliedAngle = certifiedAngle
        disposeNow()
        return
      }
      if (applyFailureReason) {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          applyFailureReason,
        )
        return
      }
      if (!applied) {
        failCurrent(
          runGeneration,
          job,
          start,
          target,
          RUNNER_REASON.applyRejected,
        )
        return
      }

      if (
        disposed
        || generation !== runGeneration
        || activeJob !== job
      ) return

      lastAppliedAngle = certifiedAngle
      if (value.kind === 'pending') {
        replaceState({
          requested: target,
          applied: lastAppliedAngle,
          start,
          status: 'running',
          reason: null,
          result: null,
        })
        if (
          disposed
          || generation !== runGeneration
          || activeJob !== job
        ) return
        scheduleNext(
          runGeneration,
          job,
          start,
          target,
          certifiedSafeThrough,
        )
        return
      }

      activeJob = null
      activeFrame = null
      generation += 1
      replaceState({
        requested: target,
        applied: lastAppliedAngle,
        start,
        status: value.kind,
        reason: value.kind === 'indeterminate'
          ? value.reason
          : value.kind === 'blocked'
            ? 'motion_blocked'
            : null,
        result: value,
      })
    }

    try {
      frame.handle = schedule(callback)
      frame.handleReady = true
      if (frame.cancelRequested) {
        try {
          cancel(frame.handle)
        } catch {
          // The callback is stale even if cancelling the returned handle fails.
        }
      }
      return true
    } catch {
      if (activeFrame === frame) activeFrame = null
      failCurrent(
        runGeneration,
        job,
        start,
        target,
        RUNNER_REASON.scheduleError,
      )
      return false
    }
  }

  const request = (targetAngle: number): boolean => {
    if (disposed || applyingAngle) return false

    // Cancellation intentionally precedes the generation update. A scheduler
    // that still invokes the old callback is stopped by the new generation.
    const runGeneration = invalidateActive()
    if (disposed || generation !== runGeneration) return false
    const start = lastAppliedAngle

    if (!validAngle(targetAngle)) {
      publishFactoryFailure(
        runGeneration,
        start,
        start,
        RUNNER_REASON.invalidTarget,
      )
      return false
    }

    replaceState({
      requested: targetAngle,
      applied: lastAppliedAngle,
      start,
      status: 'running',
      reason: null,
      result: null,
    })
    if (disposed || generation !== runGeneration) return false

    let job: FoldPreviewContinuousMotionJob<Blocker> | null
    try {
      job = jobFactory(start, targetAngle)
    } catch {
      publishFactoryFailure(
        runGeneration,
        start,
        targetAngle,
        RUNNER_REASON.factoryError,
      )
      return false
    }
    if (job === null) {
      publishFactoryFailure(
        runGeneration,
        start,
        targetAngle,
        RUNNER_REASON.factoryNull,
      )
      return false
    }
    if (!validJob(job)) {
      publishFactoryFailure(
        runGeneration,
        start,
        targetAngle,
        RUNNER_REASON.factoryMalformed,
      )
      return false
    }
    if (disposed || generation !== runGeneration) {
      cancelJob(job)
      return false
    }

    activeJob = job
    return scheduleNext(runGeneration, job, start, targetAngle, 0)
  }

  const dispose = () => {
    if (disposed || disposeAfterApply) return
    if (applyingAngle) {
      disposeAfterApply = true
      return
    }
    disposeNow()
  }

  return Object.freeze({
    request,
    dispose,
    getState: () => state,
  })
}

function resolveOptions<Blocker, ScheduledHandle>(
  options: FoldPreviewContinuousMotionRunnerOptions<Blocker, ScheduledHandle>,
): FoldPreviewContinuousMotionRunnerOptions<
  Blocker,
  ScheduledHandle
> | null {
  try {
    if (!options || typeof options !== 'object') return null
    const initialAngle = options.initialAngle
    const schedule = options.schedule
    const cancel = options.cancel
    const jobFactory = options.jobFactory
    const applyAngle = options.applyAngle
    const onState = options.onState
    if (
      !validAngle(initialAngle)
      || typeof schedule !== 'function'
      || typeof cancel !== 'function'
      || typeof jobFactory !== 'function'
      || typeof applyAngle !== 'function'
      || typeof onState !== 'function'
    ) return null
    return Object.freeze({
      initialAngle,
      schedule,
      cancel,
      jobFactory,
      applyAngle,
      onState,
    })
  } catch {
    return null
  }
}

function validJob<Blocker>(
  job: FoldPreviewContinuousMotionJob<Blocker>,
): boolean {
  try {
    return Boolean(
      job
      && typeof job === 'object'
      && typeof job.step === 'function'
      && typeof job.cancel === 'function',
    )
  } catch {
    return false
  }
}

function normalizeStep<Blocker>(
  value: unknown,
): NormalizedStep<Blocker> | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  const certifiedSafeThrough = record.certifiedSafeThrough
  if (!validUnitTime(certifiedSafeThrough)) return null
  const stats = normalizeStats(record.stats)
  if (!stats) return null

  if (record.kind === 'pending') {
    if (!validNonTerminalTime(certifiedSafeThrough)) return null
    return Object.freeze({
      value: Object.freeze({
        kind: 'pending',
        certifiedSafeThrough,
        stats,
      }),
      certifiedSafeThrough,
    })
  }
  if (record.kind === 'clear') {
    if (
      certifiedSafeThrough !== 1
      || record.stopTime !== 1
    ) return null
    return Object.freeze({
      value: Object.freeze({
        kind: 'clear',
        certifiedSafeThrough: 1,
        stopTime: 1,
        stats,
      }),
      certifiedSafeThrough: 1,
    })
  }
  if (record.kind === 'cancelled') {
    return Object.freeze({
      value: Object.freeze({
        kind: 'cancelled',
        certifiedSafeThrough,
        stats,
      }),
      certifiedSafeThrough,
    })
  }
  if (record.kind === 'blocked') {
    if (
      !validNonTerminalTime(certifiedSafeThrough)
      || record.stopTime !== certifiedSafeThrough
      || !validBracket(record.unsafeBracket)
      || record.unsafeBracket[0] !== certifiedSafeThrough
      || !validUnitTime(record.blockingSampleTime)
      || record.blockingSampleTime !== record.unsafeBracket[1]
    ) return null
    const result = Object.freeze(
      Object.hasOwn(record, 'blocker')
        ? {
            kind: 'blocked' as const,
            certifiedSafeThrough,
            stopTime: certifiedSafeThrough,
            unsafeBracket: freezeBracket(record.unsafeBracket),
            blockingSampleTime: record.blockingSampleTime,
            blocker: record.blocker as Blocker,
            stats,
          }
        : {
            kind: 'blocked' as const,
            certifiedSafeThrough,
            stopTime: certifiedSafeThrough,
            unsafeBracket: freezeBracket(record.unsafeBracket),
            blockingSampleTime: record.blockingSampleTime,
            stats,
          },
    )
    return Object.freeze({
      value: result,
      certifiedSafeThrough,
    })
  }
  if (record.kind === 'indeterminate') {
    if (
      !validNonTerminalTime(certifiedSafeThrough)
      || record.stopTime !== certifiedSafeThrough
      || !validBracket(record.unresolvedBracket)
      || record.unresolvedBracket[0] !== certifiedSafeThrough
      || !validReason(record.reason)
    ) return null
    return Object.freeze({
      value: Object.freeze({
        kind: 'indeterminate',
        certifiedSafeThrough,
        stopTime: certifiedSafeThrough,
        unresolvedBracket: freezeBracket(record.unresolvedBracket),
        reason: record.reason,
        stats,
      }),
      certifiedSafeThrough,
    })
  }
  return null
}

function normalizeStats(value: unknown): FoldPreviewContinuousMotionStats | null {
  if (!value || typeof value !== 'object') return null
  const stats = value as Record<string, unknown>
  if (
    !validCount(stats.intervalTests)
    || !validCount(stats.pointTests)
    || !validCount(stats.pointCacheHits)
    || !validCount(stats.maximumDepthReached)
  ) return null
  return Object.freeze({
    intervalTests: stats.intervalTests,
    pointTests: stats.pointTests,
    pointCacheHits: stats.pointCacheHits,
    maximumDepthReached: stats.maximumDepthReached,
  })
}

function validCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validUnitTime(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
}

function validNonTerminalTime(value: unknown): value is number {
  return validUnitTime(value) && value < 1
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validBracket(value: unknown): value is readonly [number, number] {
  return Array.isArray(value)
    && value.length === 2
    && validUnitTime(value[0])
    && validUnitTime(value[1])
    && value[0] <= value[1]
    && (value[0] < value[1] || value[0] === 0)
}

function freezeBracket(
  value: readonly [number, number],
): readonly [number, number] {
  return Object.freeze([value[0], value[1]])
}

function validReason(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function interpolateAngle(
  start: number,
  target: number,
  time: number,
): number | null {
  const angle = start + (target - start) * time
  return Number.isFinite(angle) ? angle : null
}

function freezeState<Blocker>(
  state: FoldPreviewContinuousMotionRunnerState<Blocker>,
): FoldPreviewContinuousMotionRunnerState<Blocker> {
  return Object.freeze({ ...state })
}
