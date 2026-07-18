import {
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
  type FoldPreviewTreeSingleHingeStaticCandidatePathPresentation,
} from './foldPreviewTreeSingleHingeStaticCandidatePathPresentation.ts'
import {
  FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisJob as AnalysisJob,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase as AnalysisJobPhase,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep as AnalysisJobStep,
} from './foldPreviewTreeSingleHingeCorrectionAnalysisRequest.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION =
  'tree_single_hinge_correction_analysis_coordinator_v1'

export type FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase =
  AnalysisJobPhase

export type FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep =
  AnalysisJobStep

export type FoldPreviewTreeSingleHingeCorrectionAnalysisJob =
  AnalysisJob

export type FoldPreviewTreeSingleHingeCorrectionAnalysisRun =
  Readonly<{
    /**
     * Creates the composite analysis job. The coordinator invokes this only
     * from its first scheduled callback, never inline from `start`.
     */
    createJob():
      FoldPreviewTreeSingleHingeCorrectionAnalysisJob | null
    /**
     * Revalidates the exact request/owner/lease captured by the caller.
     * Only the primitive value `true` is accepted.
     */
    validateTerminalLease(): boolean
  }>

type StateBase = Readonly<{
  version:
    typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION
  generation: number
}>

export type FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState =
  StateBase & (
    | Readonly<{
      status: 'idle'
    }>
    | Readonly<{
      status: 'working'
      phase:
        | 'preparing'
        | FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase
    }>
    | Readonly<{
      status: 'stale'
    }>
    | Readonly<{
      status: 'no_candidate'
    }>
    | Readonly<{
      status: 'indeterminate'
      reason: string
    }>
    | Readonly<{
      status: 'certified'
      presentation:
        FoldPreviewTreeSingleHingeStaticCandidatePathPresentation
    }>
  )

export type FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorOptions<
  ScheduledHandle = unknown,
> = Readonly<{
  schedule(callback: () => void): ScheduledHandle
  cancel(handle: ScheduledHandle): void
  onState(
    state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
  ): void
}>

export type FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator =
  Readonly<{
    /**
     * Supersedes all prior work and immediately clears prior terminal output.
     * Returns `false` only when the run cannot be accepted safely.
     */
    start(
      run: FoldPreviewTreeSingleHingeCorrectionAnalysisRun,
    ): boolean
    /**
     * Revokes the current generation and publishes a detached stale state.
     */
    invalidate(): void
    /** Permanently cancels this coordinator. Repeated calls are harmless. */
    dispose(): void
    getState():
      FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState
  }>

type ScheduledFrame<ScheduledHandle> = {
  handle: ScheduledHandle | undefined
  handleReady: boolean
  cancelRequested: boolean
  invoked: boolean
}

type ResolvedJob = Readonly<{
  step(): unknown
  cancel(): void
}>

type ResolvedRun = Readonly<{
  createJob(): unknown
  validateTerminalLease(): boolean
}>

type NormalizedStep =
  | Readonly<{
    kind: 'pending'
    phase: FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase
  }>
  | Readonly<{ kind: 'no_candidate' }>
  | Readonly<{ kind: 'indeterminate'; reason: string }>
  | Readonly<{
    kind: 'certified'
    presentation:
      FoldPreviewTreeSingleHingeStaticCandidatePathPresentation
  }>
  | Readonly<{ kind: 'cancelled' }>

const COORDINATOR_REASON = Object.freeze({
  invalidRun: 'invalid_run',
  counterExhausted: 'generation_counter_exhausted',
  factoryError: 'job_factory_error',
  factoryNull: 'job_factory_returned_null',
  factoryMalformed: 'job_factory_returned_malformed_job',
  scheduleError: 'scheduler_error',
  stepError: 'job_step_error',
  malformedStep: 'malformed_job_step',
  cancelled: 'job_cancelled',
})

const VALID_PHASES =
  new Set<FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase>([
    'static_candidate_preparation',
    'static_candidate_analysis',
    'candidate_path_preparation',
    'candidate_path_analysis',
  ])
const VALID_INDETERMINATE_REASONS = new Set([
  'invalid_work_budget',
  'static_candidate_job_creation_failed',
  'static_candidate_job_failed',
  'candidate_path_job_creation_failed',
  'candidate_path_job_failed',
  'candidate_path_exhausted_indeterminate',
  'certified_presentation_failed',
])

/**
 * Owns one generation of correction analysis without exposing its request,
 * runtime lease, model identities, certificates, or scene authority.
 *
 * Each scheduled work callback invokes exactly `step(1)`. The job factory is
 * itself deferred to a separate first callback so `start` remains lightweight.
 */
export function createFoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator<
  ScheduledHandle = unknown,
>(
  options:
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorOptions<
      ScheduledHandle
    >,
): FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator | null {
  const resolvedOptions = resolveOptions(options)
  if (!resolvedOptions) return null
  const { schedule, cancel, onState } = resolvedOptions

  let disposed = false
  let generation = 0
  let activeRun: ResolvedRun | null = null
  let activeJob: ResolvedJob | null = null
  let activeFrame: ScheduledFrame<ScheduledHandle> | null = null
  const synchronousFrameQueue: Array<() => void> = []
  let drainingSynchronousFrames = false
  let state = freezeState({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
    generation,
    status: 'idle',
  })

  const notify = () => {
    try {
      onState(state)
    } catch {
      // Reporting is observational and cannot acquire analysis authority.
    }
  }

  const replaceState = (
    next: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
  ) => {
    state = freezeState(next)
    notify()
  }

  const cancelFrame = (
    frame: ScheduledFrame<ScheduledHandle> | null,
  ) => {
    if (!frame) return
    frame.cancelRequested = true
    if (!frame.handleReady) return
    try {
      cancel(frame.handle as ScheduledHandle)
    } catch {
      // Generation identity remains authoritative if cancellation fails.
    }
  }

  const cancelJob = (job: ResolvedJob | null) => {
    if (!job) return
    try {
      job.cancel()
    } catch {
      // Detached jobs cannot publish even when their cancellation throws.
    }
  }

  const cancelUnknownJob = (value: unknown) => {
    const job = resolveJob(value)
    if (job) cancelJob(job)
  }

  const isCurrent = (
    runGeneration: number,
    run: ResolvedRun,
  ) => !disposed
    && generation === runGeneration
    && activeRun === run

  const enqueueSynchronousFrame = (callback: () => void) => {
    synchronousFrameQueue.push(callback)
    if (drainingSynchronousFrames) return
    drainingSynchronousFrames = true
    let cursor = 0
    try {
      while (cursor < synchronousFrameQueue.length) {
        const next = synchronousFrameQueue[cursor]
        cursor += 1
        next?.()
      }
    } finally {
      synchronousFrameQueue.length = 0
      drainingSynchronousFrames = false
    }
  }

  const detachCurrent = (
    runGeneration: number,
    run: ResolvedRun,
  ) => {
    if (!isCurrent(runGeneration, run)) return null
    const detached = Object.freeze({
      job: activeJob,
      frame: activeFrame,
    })
    activeRun = null
    activeJob = null
    activeFrame = null
    return detached
  }

  const leaseIsValid = (run: ResolvedRun) => {
    try {
      return run.validateTerminalLease() === true
    } catch {
      return false
    }
  }

  const publishStale = (
    runGeneration: number,
    run: ResolvedRun,
  ) => {
    const detached = detachCurrent(runGeneration, run)
    if (!detached) return
    replaceState({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
      generation: runGeneration,
      status: 'stale',
    })
    cancelFrame(detached.frame)
    cancelJob(detached.job)
  }

  const failCurrent = (
    runGeneration: number,
    run: ResolvedRun,
    reason: string,
  ) => {
    const detached = detachCurrent(runGeneration, run)
    if (!detached) return
    replaceState({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
      generation: runGeneration,
      status: 'indeterminate',
      reason,
    })
    cancelFrame(detached.frame)
    cancelJob(detached.job)
  }

  const scheduleFrame = (
    runGeneration: number,
    run: ResolvedRun,
    callback: () => void,
  ): boolean => {
    if (!isCurrent(runGeneration, run)) return false
    const frame: ScheduledFrame<ScheduledHandle> = {
      handle: undefined,
      handleReady: false,
      cancelRequested: false,
      invoked: false,
    }
    activeFrame = frame

    let scheduling = true
    let synchronousInvocationRequested = false
    const invokeFrame = () => {
      if (frame.invoked) return
      frame.invoked = true
      if (activeFrame === frame) activeFrame = null
      if (!isCurrent(runGeneration, run)) return
      callback()
    }
    const scheduledCallback = () => {
      if (scheduling) {
        synchronousInvocationRequested = true
        return
      }
      invokeFrame()
    }

    try {
      frame.handle = schedule(scheduledCallback)
      frame.handleReady = true
      scheduling = false
      if (frame.cancelRequested) {
        try {
          cancel(frame.handle)
        } catch {
          // The callback is stale regardless of scheduler cancellation.
        }
      }
      if (
        synchronousInvocationRequested
        && !frame.cancelRequested
      ) {
        enqueueSynchronousFrame(invokeFrame)
      }
      return !frame.cancelRequested
    } catch {
      scheduling = false
      if (activeFrame === frame) activeFrame = null
      if (!frame.invoked) {
        failCurrent(
          runGeneration,
          run,
          COORDINATOR_REASON.scheduleError,
        )
      }
      return false
    }
  }

  const finish = (
    runGeneration: number,
    run: ResolvedRun,
    next:
      | Readonly<{ status: 'no_candidate' }>
      | Readonly<{ status: 'indeterminate'; reason: string }>
      | Readonly<{
        status: 'certified'
        presentation:
          FoldPreviewTreeSingleHingeStaticCandidatePathPresentation
      }>,
  ) => {
    const detached = detachCurrent(runGeneration, run)
    if (!detached) return
    replaceState({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
      generation: runGeneration,
      ...next,
    })
    cancelFrame(detached.frame)
  }

  const runWorkFrame = (
    runGeneration: number,
    run: ResolvedRun,
    job: ResolvedJob,
  ) => {
    if (!isCurrent(runGeneration, run) || activeJob !== job) return
    if (!leaseIsValid(run)) {
      publishStale(runGeneration, run)
      return
    }
    if (!isCurrent(runGeneration, run) || activeJob !== job) return

    let rawStep: unknown
    try {
      rawStep = job.step()
    } catch {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.stepError,
      )
      return
    }
    if (!isCurrent(runGeneration, run) || activeJob !== job) return
    if (!leaseIsValid(run)) {
      publishStale(runGeneration, run)
      return
    }
    if (!isCurrent(runGeneration, run) || activeJob !== job) return

    const normalized = normalizeStep(rawStep)
    if (!normalized) {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.malformedStep,
      )
      return
    }
    if (!isCurrent(runGeneration, run) || activeJob !== job) return

    if (normalized.kind === 'pending') {
      if (
        state.status !== 'working'
        || state.phase !== normalized.phase
      ) {
        replaceState({
          version:
            FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
          generation: runGeneration,
          status: 'working',
          phase: normalized.phase,
        })
      }
      if (
        !isCurrent(runGeneration, run)
        || activeJob !== job
      ) return
      scheduleFrame(
        runGeneration,
        run,
        () => runWorkFrame(runGeneration, run, job),
      )
      return
    }
    if (normalized.kind === 'cancelled') {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.cancelled,
      )
      return
    }

    // The exact lease is checked once more at the final publication boundary,
    // after all result validation and detached snapshot construction.
    if (!leaseIsValid(run)) {
      publishStale(runGeneration, run)
      return
    }
    if (!isCurrent(runGeneration, run) || activeJob !== job) return
    if (normalized.kind === 'certified') {
      finish(runGeneration, run, {
        status: 'certified',
        presentation: normalized.presentation,
      })
      return
    }
    if (normalized.kind === 'indeterminate') {
      finish(runGeneration, run, {
        status: 'indeterminate',
        reason: normalized.reason,
      })
      return
    }
    finish(runGeneration, run, { status: 'no_candidate' })
  }

  const runFactoryFrame = (
    runGeneration: number,
    run: ResolvedRun,
  ) => {
    if (!isCurrent(runGeneration, run)) return
    if (!leaseIsValid(run)) {
      publishStale(runGeneration, run)
      return
    }
    if (!isCurrent(runGeneration, run)) return

    let rawJob: unknown
    try {
      rawJob = run.createJob()
    } catch {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.factoryError,
      )
      return
    }
    if (!isCurrent(runGeneration, run)) {
      cancelUnknownJob(rawJob)
      return
    }
    if (!leaseIsValid(run)) {
      publishStale(runGeneration, run)
      cancelUnknownJob(rawJob)
      return
    }
    if (!isCurrent(runGeneration, run)) {
      cancelUnknownJob(rawJob)
      return
    }
    if (rawJob === null) {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.factoryNull,
      )
      return
    }
    const job = resolveJob(rawJob)
    if (!job) {
      failCurrent(
        runGeneration,
        run,
        COORDINATOR_REASON.factoryMalformed,
      )
      cancelUnknownJob(rawJob)
      return
    }
    if (!isCurrent(runGeneration, run)) {
      cancelJob(job)
      return
    }

    activeJob = job
    scheduleFrame(
      runGeneration,
      run,
      () => runWorkFrame(runGeneration, run, job),
    )
  }

  const publishRejectedStart = (reason: string) => {
    const oldJob = activeJob
    const oldFrame = activeFrame
    activeRun = null
    activeJob = null
    activeFrame = null

    if (!incrementGeneration()) {
      disposed = true
      replaceState({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
        generation,
        status: 'indeterminate',
        reason: COORDINATOR_REASON.counterExhausted,
      })
    } else {
      replaceState({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
        generation,
        status: 'indeterminate',
        reason,
      })
    }
    cancelFrame(oldFrame)
    cancelJob(oldJob)
  }

  const start = (
    suppliedRun: FoldPreviewTreeSingleHingeCorrectionAnalysisRun,
  ): boolean => {
    if (disposed) return false
    const run = resolveRun(suppliedRun)
    if (!run) {
      publishRejectedStart(COORDINATOR_REASON.invalidRun)
      return false
    }

    const oldJob = activeJob
    const oldFrame = activeFrame
    activeRun = null
    activeJob = null
    activeFrame = null
    if (!incrementGeneration()) {
      disposed = true
      replaceState({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
        generation,
        status: 'indeterminate',
        reason: COORDINATOR_REASON.counterExhausted,
      })
      cancelFrame(oldFrame)
      cancelJob(oldJob)
      return false
    }
    const runGeneration = generation
    activeRun = run
    replaceState({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
      generation: runGeneration,
      status: 'working',
      phase: 'preparing',
    })

    // New public state is authoritative before cancellation can re-enter.
    cancelFrame(oldFrame)
    cancelJob(oldJob)
    if (!isCurrent(runGeneration, run)) return false
    return scheduleFrame(
      runGeneration,
      run,
      () => runFactoryFrame(runGeneration, run),
    )
  }

  const invalidate = () => {
    if (disposed) return
    const oldJob = activeJob
    const oldFrame = activeFrame
    activeRun = null
    activeJob = null
    activeFrame = null
    if (!incrementGeneration()) {
      disposed = true
      replaceState({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
        generation,
        status: 'indeterminate',
        reason: COORDINATOR_REASON.counterExhausted,
      })
    } else {
      replaceState({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
        generation,
        status: 'stale',
      })
    }
    cancelFrame(oldFrame)
    cancelJob(oldJob)
  }

  const dispose = () => {
    if (disposed) return
    disposed = true
    const oldJob = activeJob
    const oldFrame = activeFrame
    activeRun = null
    activeJob = null
    activeFrame = null
    incrementGeneration()
    replaceState({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_COORDINATOR_VERSION,
      generation,
      status: 'idle',
    })
    cancelFrame(oldFrame)
    cancelJob(oldJob)
  }

  const incrementGeneration = () => {
    if (generation >= Number.MAX_SAFE_INTEGER) return false
    generation += 1
    return true
  }

  return Object.freeze({
    start,
    invalidate,
    dispose,
    getState: () => state,
  })
}

function resolveOptions<ScheduledHandle>(
  options:
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorOptions<
      ScheduledHandle
    >,
) {
  try {
    if (!options || typeof options !== 'object') return null
    const schedule = options.schedule
    const cancel = options.cancel
    const onState = options.onState
    if (
      typeof schedule !== 'function'
      || typeof cancel !== 'function'
      || typeof onState !== 'function'
    ) return null
    return Object.freeze({
      schedule: (callback: () => void) =>
        Reflect.apply(schedule, options, [callback]) as ScheduledHandle,
      cancel: (handle: ScheduledHandle) =>
        Reflect.apply(cancel, options, [handle]) as void,
      onState: (
        state:
          FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
      ) => Reflect.apply(onState, options, [state]) as void,
    })
  } catch {
    return null
  }
}

function resolveRun(value: unknown): ResolvedRun | null {
  try {
    if (!value || typeof value !== 'object') return null
    const owner = value as Record<string, unknown>
    const createJob = owner.createJob
    const validateTerminalLease = owner.validateTerminalLease
    if (
      typeof createJob !== 'function'
      || typeof validateTerminalLease !== 'function'
    ) return null
    return Object.freeze({
      createJob: () => Reflect.apply(createJob, value, []),
      validateTerminalLease: () =>
        Reflect.apply(validateTerminalLease, value, []) as boolean,
    })
  } catch {
    return null
  }
}

function resolveJob(value: unknown): ResolvedJob | null {
  try {
    if (!value || typeof value !== 'object') return null
    const owner = value as Record<string, unknown>
    const step = owner.step
    const cancel = owner.cancel
    if (typeof step !== 'function' || typeof cancel !== 'function') {
      return null
    }
    return Object.freeze({
      step: () => Reflect.apply(step, value, [1]),
      cancel: () => Reflect.apply(cancel, value, []) as void,
    })
  } catch {
    return null
  }
}

function normalizeStep(value: unknown): NormalizedStep | null {
  try {
    if (!value || typeof value !== 'object') return null
    const record = value as Record<string, unknown>
    if (
      record.version
        !== FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION
      || !validAnalysisOnlyJobSafety(record.safety)
    ) return null
    if (record.kind === 'pending') {
      if (
        record.status !== 'working'
        || !validPhase(record.phase)
      ) return null
      return Object.freeze({
        kind: 'pending',
        phase: record.phase,
      })
    }
    if (record.kind === 'no_candidate') {
      if (
        record.exhaustedPhase !== 'static_candidate_analysis'
        && record.exhaustedPhase !== 'candidate_path_analysis'
      ) return null
      return Object.freeze({ kind: 'no_candidate' })
    }
    if (record.kind === 'indeterminate') {
      if (
        !validPhase(record.phase)
        || typeof record.reason !== 'string'
        || !VALID_INDETERMINATE_REASONS.has(record.reason)
      ) return null
      return Object.freeze({
        kind: 'indeterminate',
        reason: record.reason,
      })
    }
    if (record.kind === 'certified') {
      const presentation = snapshotPresentation(record.presentation)
      if (!presentation) return null
      return Object.freeze({
        kind: 'certified',
        presentation,
      })
    }
    if (record.kind === 'cancelled') {
      return Object.freeze({ kind: 'cancelled' })
    }
    return null
  } catch {
    return null
  }
}

function snapshotPresentation(
  value: unknown,
): FoldPreviewTreeSingleHingeStaticCandidatePathPresentation | null {
  try {
    if (!value || typeof value !== 'object') return null
    const presentation = value as Record<string, unknown>
    const safety = asRecord(presentation.safety)
    if (
      presentation.version
        !== FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION
      || presentation.kind
        !== 'certified_static_candidate_path_presentation'
      || !Object.isFrozen(value)
      || !safety
      || safety.analysisOnly !== true
      || safety.runtimeRequestBound !== false
      || safety.activeRequestLeaseBound !== false
      || safety.startScenePoseMatched !== false
      || safety.sceneApplied !== false
      || safety.autoApplicable !== false
    ) return null
    return value as
      FoldPreviewTreeSingleHingeStaticCandidatePathPresentation
  } catch {
    return null
  }
}

function validAnalysisOnlyJobSafety(value: unknown) {
  const safety = asRecord(value)
  return safety !== null
    && safety.analysisOnly === true
    && safety.sceneApplied === false
    && safety.autoApplicable === false
}

function validPhase(
  value: unknown,
): value is FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase {
  return typeof value === 'string'
    && VALID_PHASES.has(
      value as FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase,
    )
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object'
    ? value as Record<string, unknown>
    : null
}

function freezeState(
  value: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
) {
  return Object.freeze({ ...value })
}
