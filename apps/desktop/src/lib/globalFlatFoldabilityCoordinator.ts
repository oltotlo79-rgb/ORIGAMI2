import {
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  isGlobalFlatFoldabilityTimePreset,
  parseGlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityCounts,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityPhase,
  type GlobalFlatFoldabilitySummary,
  type GlobalFlatFoldabilityTimePreset,
} from './globalFlatFoldability.ts'
import {
  GlobalFlatFoldabilityNativeError,
  type GlobalFlatFoldabilityNativeBegin,
  type GlobalFlatFoldabilityNativeContext,
  type GlobalFlatFoldabilityNativeTransport,
} from './globalFlatFoldabilityNative.ts'

export const GLOBAL_FLAT_FOLDABILITY_POLL_INTERVAL_MS = 250

export type GlobalFlatFoldabilityContext =
  GlobalFlatFoldabilityNativeContext
export type GlobalFlatFoldabilityBeginResponse =
  GlobalFlatFoldabilityNativeBegin
export type GlobalFlatFoldabilityTransport =
  GlobalFlatFoldabilityNativeTransport

export type GlobalFlatFoldabilityTimeoutScheduler<Handle = unknown> = Readonly<{
  setTimeout(callback: () => void, delayMs: number): Handle
  clearTimeout(handle: Handle): void
}>

export type GlobalFlatFoldabilityCoordinatorState = Readonly<{
  generation: number
  job: GlobalFlatFoldabilityJobDto | null
}>

export type GlobalFlatFoldabilityCoordinatorOptions<Handle = unknown> =
  Readonly<{
    transport: GlobalFlatFoldabilityTransport
    scheduler: GlobalFlatFoldabilityTimeoutScheduler<Handle>
    onState(state: GlobalFlatFoldabilityCoordinatorState): void
  }>

export type GlobalFlatFoldabilityCoordinator = Readonly<{
  start(
    context: GlobalFlatFoldabilityContext,
    timeLimitSeconds: GlobalFlatFoldabilityTimePreset,
  ): boolean
  cancel(): boolean
  invalidate(
    currentContext: GlobalFlatFoldabilityContext,
    forceSnapshotReplacement?: boolean,
  ): boolean
  dispose(): void
  getState(): GlobalFlatFoldabilityCoordinatorState
}>

type ResolvedTransport = Readonly<{
  begin(context: GlobalFlatFoldabilityContext, timeLimitMs: number): unknown
  poll(jobId: string): unknown
  cancel(jobId: string): unknown
}>

type ScheduledTimeout<Handle> = {
  handle: Handle | undefined
  handleReady: boolean
  clearRequested: boolean
  invoked: boolean
}

type ActiveRun<Handle> = {
  generation: number
  context: GlobalFlatFoldabilityContext
  jobId: string | null
  timer: ScheduledTimeout<Handle> | null
  pollInFlight: boolean
  cancelRequested: boolean
  cancelInFlight: boolean
  cancelSent: boolean
}

type ParsedBeginResponse = Readonly<{
  jobId: string
  job: unknown
}>

const PHASE_RANK: Readonly<Record<GlobalFlatFoldabilityPhase, number>> =
  Object.freeze({
    capturing: 0,
    validating_local_conditions: 1,
    building_flat_embedding: 2,
    building_overlap_arrangement: 3,
    building_constraints: 4,
    propagating: 5,
    searching: 6,
    verifying_certificate: 7,
    completed: 8,
  })

/**
 * Owns one revision-bound native analysis generation.
 *
 * Opaque job IDs and project binding stay private. Observers receive only the
 * existing closed UI DTO, so transport failures cannot introduce raw text.
 */
export function createGlobalFlatFoldabilityCoordinator<Handle = unknown>(
  options: GlobalFlatFoldabilityCoordinatorOptions<Handle>,
): GlobalFlatFoldabilityCoordinator | null {
  const resolved = resolveOptions(options)
  if (!resolved) return null
  const { transport, scheduler, onState } = resolved

  let disposed = false
  let generation = 0
  let binding: GlobalFlatFoldabilityContext | null = null
  let activeRun: ActiveRun<Handle> | null = null
  let state = freezeState({ generation, job: null })
  const synchronousTimerQueue: Array<() => void> = []
  let drainingSynchronousTimers = false

  const notify = () => {
    try {
      onState(state)
    } catch {
      // State reporting is observational and cannot acquire job authority.
    }
  }

  const publish = (job: GlobalFlatFoldabilityJobDto | null) => {
    state = freezeState({ generation, job })
    notify()
  }

  const isCurrent = (run: ActiveRun<Handle>) =>
    !disposed
    && activeRun === run
    && generation === run.generation

  const drainSynchronousTimer = (callback: () => void) => {
    synchronousTimerQueue.push(callback)
    if (drainingSynchronousTimers) return
    drainingSynchronousTimers = true
    let cursor = 0
    try {
      while (cursor < synchronousTimerQueue.length) {
        const next = synchronousTimerQueue[cursor]
        cursor += 1
        next?.()
      }
    } finally {
      synchronousTimerQueue.length = 0
      drainingSynchronousTimers = false
    }
  }

  const clearScheduledTimeout = (
    timeout: ScheduledTimeout<Handle> | null,
  ) => {
    if (!timeout) return
    timeout.clearRequested = true
    if (!timeout.handleReady) return
    try {
      scheduler.clearTimeout(timeout.handle as Handle)
    } catch {
      // Generation checks keep a callback inert if clearing fails.
    }
  }

  const clearRunTimer = (run: ActiveRun<Handle>) => {
    const timeout = run.timer
    run.timer = null
    clearScheduledTimeout(timeout)
  }

  const cancelDetachedJobId = (jobId: string | null) => {
    if (!jobId || activeRun?.jobId === jobId) return
    let result: unknown
    try {
      result = transport.cancel(jobId)
    } catch {
      return
    }
    observeAsync(result, () => undefined, () => undefined)
  }

  const sendCancel = (run: ActiveRun<Handle>) => {
    if (
      !isCurrent(run)
      || run.cancelSent
      || run.cancelInFlight
      || run.jobId === null
    ) return false
    run.cancelInFlight = true
    let result: unknown
    try {
      result = transport.cancel(run.jobId)
    } catch {
      run.cancelInFlight = false
      return false
    }
    observeAsync(
      result,
      () => {
        if (!isCurrent(run)) return
        run.cancelInFlight = false
        run.cancelSent = true
      },
      () => {
        if (!isCurrent(run)) return
        run.cancelInFlight = false
      },
    )
    return true
  }

  const failRun = (
    run: ActiveRun<Handle>,
    category: GlobalFlatFoldabilityErrorCategory,
  ) => {
    if (!isCurrent(run)) return
    const jobId = run.jobId
    activeRun = null
    const timeout = run.timer
    run.timer = null
    const summary = summaryFromJob(state.job)
    publish(failedJob(summary, category))
    clearScheduledTimeout(timeout)
    cancelDetachedJobId(jobId)
  }

  const finishRun = (
    run: ActiveRun<Handle>,
    job: GlobalFlatFoldabilityJobDto,
  ) => {
    if (!isCurrent(run)) return
    activeRun = null
    const timeout = run.timer
    run.timer = null
    publish(job)
    clearScheduledTimeout(timeout)
  }

  const schedulePoll = (run: ActiveRun<Handle>): boolean => {
    if (!isCurrent(run) || run.jobId === null || run.timer !== null) {
      return false
    }
    const timeout: ScheduledTimeout<Handle> = {
      handle: undefined,
      handleReady: false,
      clearRequested: false,
      invoked: false,
    }
    run.timer = timeout
    let scheduling = true
    let synchronousInvocationRequested = false

    const invoke = () => {
      if (timeout.invoked) return
      timeout.invoked = true
      if (run.timer === timeout) run.timer = null
      if (!isCurrent(run)) return
      pollRun(run)
    }
    const callback = () => {
      if (scheduling) {
        synchronousInvocationRequested = true
        return
      }
      invoke()
    }

    try {
      timeout.handle = scheduler.setTimeout(
        callback,
        GLOBAL_FLAT_FOLDABILITY_POLL_INTERVAL_MS,
      )
      timeout.handleReady = true
      scheduling = false
      if (timeout.clearRequested) {
        clearScheduledTimeout(timeout)
      } else if (synchronousInvocationRequested) {
        drainSynchronousTimer(invoke)
      }
      return isCurrent(run) && !timeout.clearRequested
    } catch {
      scheduling = false
      if (run.timer === timeout) run.timer = null
      if (!timeout.invoked) failRun(run, 'internal_failure')
      return false
    }
  }

  const acceptJob = (
    run: ActiveRun<Handle>,
    rawJob: unknown,
  ) => {
    if (!isCurrent(run)) return
    const parsed = parseGlobalFlatFoldabilityJobDto(rawJob)
    const previous = state.job
    if (
      !parsed
      || !previous
      || !jobTransitionIsMonotonic(previous, parsed)
    ) {
      failRun(run, 'result_unavailable')
      return
    }

    if (parsed.state !== 'queued' && parsed.state !== 'running') {
      finishRun(run, parsed)
      return
    }

    if (parsed.cancel_requested) {
      run.cancelRequested = true
      run.cancelSent = true
    }
    const next = run.cancelRequested
      ? withCancelRequested(parsed)
      : parsed
    publish(next)
    if (!isCurrent(run)) return
    if (run.cancelRequested && !run.cancelSent) sendCancel(run)
    if (!isCurrent(run)) return
    schedulePoll(run)
  }

  function pollRun(run: ActiveRun<Handle>) {
    if (
      !isCurrent(run)
      || run.jobId === null
      || run.pollInFlight
    ) return
    run.pollInFlight = true
    let result: unknown
    try {
      result = transport.poll(run.jobId)
    } catch (error) {
      run.pollInFlight = false
      failRun(
        run,
        transportErrorCategory(error, 'result_unavailable'),
      )
      return
    }
    observeAsync(
      result,
      (rawJob) => {
        if (!isCurrent(run)) return
        run.pollInFlight = false
        acceptJob(run, rawJob)
      },
      (error) => {
        if (!isCurrent(run)) return
        run.pollInFlight = false
        failRun(
          run,
          transportErrorCategory(error, 'result_unavailable'),
        )
      },
    )
  }

  const receiveBegin = (
    run: ActiveRun<Handle>,
    rawResponse: unknown,
  ) => {
    const response = parseBeginResponse(rawResponse)
    if (!isCurrent(run)) {
      if (response) cancelDetachedJobId(response.jobId)
      return
    }
    if (!response) {
      failRun(run, 'worker_unavailable')
      return
    }
    run.jobId = response.jobId

    const initialJob = parseGlobalFlatFoldabilityJobDto(response.job)
    const previous = state.job
    if (
      !initialJob
      || !previous
      || !jobTransitionIsMonotonic(previous, initialJob)
    ) {
      failRun(run, 'result_unavailable')
      return
    }
    if (
      initialJob.state !== 'queued'
      && initialJob.state !== 'running'
    ) {
      finishRun(run, initialJob)
      return
    }
    if (initialJob.cancel_requested) {
      run.cancelRequested = true
      run.cancelSent = true
    }
    publish(
      run.cancelRequested
        ? withCancelRequested(initialJob)
        : initialJob,
    )
    if (!isCurrent(run)) return

    if (run.cancelRequested) sendCancel(run)
    if (!isCurrent(run)) return
    schedulePoll(run)
  }

  const beginRun = (
    run: ActiveRun<Handle>,
    timeLimitSeconds: GlobalFlatFoldabilityTimePreset,
  ) => {
    if (!isCurrent(run)) return
    let result: unknown
    try {
      result = transport.begin(
        run.context,
        timeLimitSeconds * 1_000,
      )
    } catch (error) {
      failRun(
        run,
        transportErrorCategory(error, 'worker_unavailable'),
      )
      return
    }
    observeAsync(
      result,
      (response) => receiveBegin(run, response),
      (error) => {
        if (isCurrent(run)) {
          failRun(
            run,
            transportErrorCategory(error, 'worker_unavailable'),
          )
        }
      },
    )
  }

  const start = (
    suppliedContext: GlobalFlatFoldabilityContext,
    timeLimitSeconds: GlobalFlatFoldabilityTimePreset,
  ): boolean => {
    if (disposed || !isGlobalFlatFoldabilityTimePreset(timeLimitSeconds)) {
      return false
    }
    const context = snapshotContext(suppliedContext)
    if (!context || generation >= Number.MAX_SAFE_INTEGER) return false

    const oldRun = activeRun
    activeRun = null
    generation += 1
    binding = context
    const run: ActiveRun<Handle> = {
      generation,
      context,
      jobId: null,
      timer: null,
      pollInFlight: false,
      cancelRequested: false,
      cancelInFlight: false,
      cancelSent: false,
    }
    activeRun = run
    publish(queuedJob(false))

    if (oldRun) {
      clearRunTimer(oldRun)
      if (oldRun.jobId !== null) {
        cancelDetachedJobId(oldRun.jobId)
      }
    }
    if (!isCurrent(run)) return false
    beginRun(run, timeLimitSeconds)
    return isCurrent(run)
  }

  const cancel = () => {
    const run = activeRun
    const currentJob = state.job
    if (
      disposed
      || !run
      || !isCurrent(run)
      || !currentJob
      || (currentJob.state !== 'queued' && currentJob.state !== 'running')
    ) return false

    let acceptedIntent = false
    if (!run.cancelRequested) {
      run.cancelRequested = true
      acceptedIntent = true
      publish(withCancelRequested(currentJob))
      if (!isCurrent(run)) return acceptedIntent
    }
    if (run.cancelSent || run.cancelInFlight) return acceptedIntent
    return sendCancel(run) || acceptedIntent
  }

  const invalidate = (
    suppliedContext: GlobalFlatFoldabilityContext,
    forceSnapshotReplacement = false,
  ) => {
    if (disposed) return false
    const currentContext = snapshotContext(suppliedContext)
    if (
      !currentContext
      || !binding
      || (!forceSnapshotReplacement && contextsEqual(binding, currentContext))
      || generation >= Number.MAX_SAFE_INTEGER
    ) return false

    const oldRun = activeRun
    activeRun = null
    generation += 1
    binding = currentContext
    const summary = summaryFromJob(state.job)
    publish(staleJob(summary))

    if (oldRun) {
      clearRunTimer(oldRun)
      if (oldRun.jobId !== null) {
        cancelDetachedJobId(oldRun.jobId)
      }
    }
    return true
  }

  const dispose = () => {
    if (disposed) return
    disposed = true
    const oldRun = activeRun
    activeRun = null
    binding = null
    if (generation < Number.MAX_SAFE_INTEGER) generation += 1
    publish(null)
    if (oldRun) {
      clearRunTimer(oldRun)
      if (oldRun.jobId !== null) cancelDetachedJobId(oldRun.jobId)
    }
  }

  return Object.freeze({
    start,
    cancel,
    invalidate,
    dispose,
    getState: () => state,
  })
}

function resolveOptions<Handle>(
  options: GlobalFlatFoldabilityCoordinatorOptions<Handle>,
): Readonly<{
  transport: ResolvedTransport
  scheduler: GlobalFlatFoldabilityTimeoutScheduler<Handle>
  onState(state: GlobalFlatFoldabilityCoordinatorState): void
}> | null {
  try {
    if (!options || typeof options !== 'object') return null
    const transportOwner = options.transport
    const schedulerOwner = options.scheduler
    const onState = options.onState
    if (
      !transportOwner
      || typeof transportOwner !== 'object'
      || !schedulerOwner
      || typeof schedulerOwner !== 'object'
      || typeof onState !== 'function'
    ) return null
    const begin = transportOwner.begin
    const poll = transportOwner.poll
    const cancel = transportOwner.cancel
    const setTimeout = schedulerOwner.setTimeout
    const clearTimeout = schedulerOwner.clearTimeout
    if (
      typeof begin !== 'function'
      || typeof poll !== 'function'
      || typeof cancel !== 'function'
      || typeof setTimeout !== 'function'
      || typeof clearTimeout !== 'function'
    ) return null
    return Object.freeze({
      transport: Object.freeze({
        begin: (
          context: GlobalFlatFoldabilityContext,
          timeLimitMs: number,
        ) => Reflect.apply(begin, transportOwner, [context, timeLimitMs]),
        poll: (jobId: string) =>
          Reflect.apply(poll, transportOwner, [jobId]),
        cancel: (jobId: string) =>
          Reflect.apply(cancel, transportOwner, [jobId]),
      }),
      scheduler: Object.freeze({
        setTimeout: (callback: () => void, delayMs: number) =>
          Reflect.apply(setTimeout, schedulerOwner, [callback, delayMs]) as Handle,
        clearTimeout: (handle: Handle) =>
          Reflect.apply(clearTimeout, schedulerOwner, [handle]) as void,
      }),
      onState: (nextState: GlobalFlatFoldabilityCoordinatorState) =>
        Reflect.apply(onState, options, [nextState]) as void,
    })
  } catch {
    return null
  }
}

function parseBeginResponse(value: unknown): ParsedBeginResponse | null {
  try {
    if (!isPlainObject(value)) return null
    const keys = Reflect.ownKeys(value)
    if (keys.length !== 2) return null
    let jobId: unknown
    let job: unknown
    let hasJobId = false
    let hasJob = false
    for (const key of keys) {
      if (typeof key !== 'string') return null
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) {
        return null
      }
      if (key === 'jobId') {
        hasJobId = true
        jobId = descriptor.value
      } else if (key === 'job') {
        hasJob = true
        job = descriptor.value
      } else {
        return null
      }
    }
    if (!hasJobId || !hasJob || !validJobId(jobId)) return null
    return Object.freeze({ jobId, job })
  } catch {
    return null
  }
}

function snapshotContext(value: unknown): GlobalFlatFoldabilityContext | null {
  try {
    if (!isPlainObject(value)) return null
    const keys = Reflect.ownKeys(value)
    if (
      keys.length !== 4
      || !keys.includes('projectInstanceId')
      || !keys.includes('projectId')
      || !keys.includes('revision')
      || !keys.includes('foldModelFingerprint')
    ) return null
    const instanceDescriptor = Object.getOwnPropertyDescriptor(value, 'projectInstanceId')
    const projectDescriptor = Object.getOwnPropertyDescriptor(value, 'projectId')
    const revisionDescriptor = Object.getOwnPropertyDescriptor(value, 'revision')
    const fingerprintDescriptor = Object.getOwnPropertyDescriptor(
      value,
      'foldModelFingerprint',
    )
    if (
      !instanceDescriptor
      || !projectDescriptor
      || !revisionDescriptor
      || !fingerprintDescriptor
      || !instanceDescriptor.enumerable
      || !projectDescriptor.enumerable
      || !revisionDescriptor.enumerable
      || !fingerprintDescriptor.enumerable
      || !('value' in instanceDescriptor)
      || !('value' in projectDescriptor)
      || !('value' in revisionDescriptor)
      || !('value' in fingerprintDescriptor)
      || !validProjectId(instanceDescriptor.value)
      || !validProjectId(projectDescriptor.value)
      || !validRevision(revisionDescriptor.value)
      || !validFoldModelFingerprint(fingerprintDescriptor.value)
    ) return null
    return Object.freeze({
      projectInstanceId: instanceDescriptor.value,
      projectId: projectDescriptor.value,
      revision: revisionDescriptor.value,
      foldModelFingerprint: fingerprintDescriptor.value,
    })
  } catch {
    return null
  }
}

function observeAsync(
  value: unknown,
  fulfilled: (value: unknown) => void,
  rejected: (reason: unknown) => void,
) {
  try {
    void Promise.resolve(value).then(
      (resolved) => {
        try {
          fulfilled(resolved)
        } catch (error) {
          rejected(error)
        }
      },
      (reason) => {
        try {
          rejected(reason)
        } catch {
          // Rejection handling cannot escape into an unhandled promise.
        }
      },
    ).catch(() => undefined)
  } catch (error) {
    try {
      rejected(error)
    } catch {
      // Hostile thenables and observers remain contained.
    }
  }
}

function transportErrorCategory(
  error: unknown,
  fallback: GlobalFlatFoldabilityErrorCategory,
): GlobalFlatFoldabilityErrorCategory {
  try {
    if (!(error instanceof GlobalFlatFoldabilityNativeError)) return fallback
    switch (error.category) {
      case 'invalid_request':
      case 'snapshot_unavailable':
      case 'worker_unavailable':
      case 'result_unavailable':
      case 'internal_failure':
        return error.category
      default:
        return fallback
    }
  } catch {
    return fallback
  }
}

function jobTransitionIsMonotonic(
  previous: GlobalFlatFoldabilityJobDto,
  next: GlobalFlatFoldabilityJobDto,
) {
  if (previous.state !== 'queued' && previous.state !== 'running') return false
  if (next.state !== 'queued' && next.state !== 'running') {
    return summaryIsMonotonic(
      previous.progress,
      terminalSummary(next),
    )
  }
  if (previous.state === 'running' && next.state === 'queued') return false
  const previousProgress = previous.progress
  const nextProgress = next.progress
  const previousPhaseRank = PHASE_RANK[previousProgress.phase]
  const nextPhaseRank = PHASE_RANK[nextProgress.phase]
  const phaseAdvanced = nextPhaseRank > previousPhaseRank
  if (
    nextPhaseRank < previousPhaseRank
    || nextProgress.completed_work < previousProgress.completed_work
    || nextProgress.elapsed_ms < previousProgress.elapsed_ms
    || !countsAreMonotonic(previousProgress.counts, nextProgress.counts)
  ) return false
  if (!phaseAdvanced) {
    if (
      previousProgress.total_work !== null
      && (
        nextProgress.total_work === null
        || nextProgress.total_work < previousProgress.total_work
      )
    ) return false
  }
  return true
}

function summaryIsMonotonic(
  previous: Readonly<{
    elapsed_ms: number
    counts: GlobalFlatFoldabilityCounts
  }>,
  next: GlobalFlatFoldabilitySummary,
) {
  return next.elapsed_ms >= previous.elapsed_ms
    && countsAreMonotonic(previous.counts, next.counts)
}

function countsAreMonotonic(
  previous: GlobalFlatFoldabilityCounts,
  next: GlobalFlatFoldabilityCounts,
) {
  return next.face_count >= previous.face_count
    && next.overlap_cell_count >= previous.overlap_cell_count
    && next.constraint_count >= previous.constraint_count
    && next.search_node_count >= previous.search_node_count
}

function terminalSummary(
  job: Exclude<
    GlobalFlatFoldabilityJobDto,
    { state: 'queued' | 'running' }
  >,
) {
  return job.state === 'completed' ? job.result.summary : job.summary
}

function withCancelRequested(
  job: Extract<
    GlobalFlatFoldabilityJobDto,
    { state: 'queued' | 'running' }
  >,
): Extract<
  GlobalFlatFoldabilityJobDto,
  { state: 'queued' | 'running' }
> {
  if (job.cancel_requested) return job
  return Object.freeze({
    state: job.state,
    cancel_requested: true,
    progress: job.progress,
  })
}

function summaryFromJob(
  job: GlobalFlatFoldabilityJobDto | null,
): GlobalFlatFoldabilitySummary {
  if (!job) return emptySummary()
  const source = job.state === 'queued' || job.state === 'running'
    ? job.progress
    : job.state === 'completed'
      ? job.result.summary
      : job.summary
  return freezeSummary(source.elapsed_ms, source.counts)
}

function queuedJob(cancelRequested: boolean): GlobalFlatFoldabilityJobDto {
  const counts = freezeCounts({
    face_count: 0,
    overlap_cell_count: 0,
    constraint_count: 0,
    search_node_count: 0,
  })
  return Object.freeze({
    state: 'queued',
    cancel_requested: cancelRequested,
    progress: Object.freeze({
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'capturing',
      completed_work: 0,
      total_work: null,
      elapsed_ms: 0,
      counts,
    }),
  })
}

function failedJob(
  summary: GlobalFlatFoldabilitySummary,
  errorCategory: GlobalFlatFoldabilityErrorCategory,
): GlobalFlatFoldabilityJobDto {
  return Object.freeze({
    state: 'failed',
    summary,
    error_category: errorCategory,
  })
}

function staleJob(
  summary: GlobalFlatFoldabilitySummary,
): GlobalFlatFoldabilityJobDto {
  return Object.freeze({ state: 'stale', summary })
}

function emptySummary() {
  return freezeSummary(0, freezeCounts({
    face_count: 0,
    overlap_cell_count: 0,
    constraint_count: 0,
    search_node_count: 0,
  }))
}

function freezeSummary(
  elapsedMs: number,
  counts: GlobalFlatFoldabilityCounts,
): GlobalFlatFoldabilitySummary {
  return Object.freeze({
    model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    elapsed_ms: elapsedMs,
    counts: freezeCounts(counts),
  })
}

function freezeCounts(
  counts: GlobalFlatFoldabilityCounts,
): GlobalFlatFoldabilityCounts {
  return Object.freeze({
    face_count: counts.face_count,
    overlap_cell_count: counts.overlap_cell_count,
    constraint_count: counts.constraint_count,
    search_node_count: counts.search_node_count,
  })
}

function freezeState(
  value: GlobalFlatFoldabilityCoordinatorState,
) {
  return Object.freeze({ generation: value.generation, job: value.job })
}

function contextsEqual(
  first: GlobalFlatFoldabilityContext,
  second: GlobalFlatFoldabilityContext,
) {
  return first.projectInstanceId === second.projectInstanceId
    && first.projectId === second.projectId
    && first.revision === second.revision
    && first.foldModelFingerprint === second.foldModelFingerprint
}

function validProjectId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= 256
    && value.trim() === value
    && !containsControlCharacter(value)
}

function validRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
    && !Object.is(value, -0)
}

function validFoldModelFingerprint(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/u.test(value)
}

function validJobId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= 256
    && value.trim() === value
    && !containsControlCharacter(value)
}

function isPlainObject(
  value: unknown,
): value is Record<PropertyKey, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function containsControlCharacter(value: string) {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code <= 0x1f || code === 0x7f) return true
  }
  return false
}
