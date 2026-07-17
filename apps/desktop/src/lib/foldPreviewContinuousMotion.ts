export type FoldPreviewContinuousPointDecision<Blocker = unknown> =
  | Readonly<{ kind: 'safe' }>
  | Readonly<{ kind: 'blocked'; blocker?: Blocker }>
  | Readonly<{ kind: 'indeterminate'; reason: string }>

export type FoldPreviewContinuousIntervalDecision =
  | Readonly<{ kind: 'clear' }>
  | Readonly<{ kind: 'unresolved' }>
  | Readonly<{ kind: 'indeterminate'; reason: string }>

export type FoldPreviewContinuousMotionStats = Readonly<{
  intervalTests: number
  pointTests: number
  pointCacheHits: number
  maximumDepthReached: number
}>

export type FoldPreviewContinuousMotionResult<Blocker = unknown> =
  | Readonly<{
      kind: 'clear'
      certifiedSafeThrough: 1
      stopTime: 1
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'blocked'
      certifiedSafeThrough: number
      stopTime: number
      unsafeBracket: readonly [number, number]
      blocker?: Blocker
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'indeterminate'
      certifiedSafeThrough: number
      stopTime: number
      unresolvedBracket: readonly [number, number]
      reason: string
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'cancelled'
      certifiedSafeThrough: number
      stats: FoldPreviewContinuousMotionStats
    }>

export type FoldPreviewContinuousMotionStep<Blocker = unknown> =
  | Readonly<{
      kind: 'pending'
      certifiedSafeThrough: number
      stats: FoldPreviewContinuousMotionStats
    }>
  | FoldPreviewContinuousMotionResult<Blocker>

export type FoldPreviewContinuousMotionJob<Blocker = unknown> = Readonly<{
  /**
   * Performs at most `workBudget` interval-certificate calls. Point checks are
   * cached and bounded indirectly by the binary interval tree.
   */
  step(workBudget: number): FoldPreviewContinuousMotionStep<Blocker>
  /** Permanently prevents further callback execution unless already terminal. */
  cancel(): void
}>

export type FoldPreviewContinuousMotionOptions = Readonly<{
  maxDepth?: number
  maxIntervalTests?: number
  minTimeSpan?: number
}>

export type FoldPreviewContinuousMotionCallbacks<Blocker = unknown> = Readonly<{
  evaluatePoint(time: number): FoldPreviewContinuousPointDecision<Blocker>
  certifyInterval(
    startTime: number,
    endTime: number,
  ): FoldPreviewContinuousIntervalDecision
}>

type Interval = Readonly<{
  start: number
  end: number
  depth: number
}>

type ResolvedOptions = Readonly<{
  maxDepth: number
  maxIntervalTests: number
  minTimeSpan: number
}>

const DEFAULT_MAX_DEPTH = 24
const DEFAULT_MAX_INTERVAL_TESTS = 2_048
const DEFAULT_MIN_TIME_SPAN = 2 ** -24

/**
 * Creates a resumable, chronological interval-subdivision job.
 *
 * Point samples can locate a blocking or unknown time, but never certify the
 * path between samples. Only `certifyInterval(...).kind === 'clear'` advances
 * `certifiedSafeThrough`; exhausted or malformed work therefore fails closed.
 */
export function createFoldPreviewContinuousMotionJob<Blocker = unknown>(
  callbacks: FoldPreviewContinuousMotionCallbacks<Blocker>,
  options: FoldPreviewContinuousMotionOptions = {},
): FoldPreviewContinuousMotionJob<Blocker> | null {
  let evaluatePointCallback: FoldPreviewContinuousMotionCallbacks<Blocker>['evaluatePoint']
  let certifyIntervalCallback: FoldPreviewContinuousMotionCallbacks<Blocker>['certifyInterval']
  let resolvedOptions: ResolvedOptions | null
  try {
    if (
      !callbacks
      || typeof callbacks.evaluatePoint !== 'function'
      || typeof callbacks.certifyInterval !== 'function'
    ) return null
    evaluatePointCallback = callbacks.evaluatePoint
    certifyIntervalCallback = callbacks.certifyInterval
    resolvedOptions = resolveOptions(options)
  } catch {
    return null
  }
  if (!resolvedOptions) return null

  let cancelled = false
  let stepping = false
  let initialized = false
  let certifiedSafeThrough = 0
  let intervalTests = 0
  let pointTests = 0
  let pointCacheHits = 0
  let maximumDepthReached = 0
  let terminal: FoldPreviewContinuousMotionResult<Blocker> | null = null
  const intervals: Interval[] = [{ start: 0, end: 1, depth: 0 }]
  const pointCache = new Map<number, FoldPreviewContinuousPointDecision<Blocker>>()

  const stats = (): FoldPreviewContinuousMotionStats => Object.freeze({
    intervalTests,
    pointTests,
    pointCacheHits,
    maximumDepthReached,
  })

  const finish = (result: FoldPreviewContinuousMotionResult<Blocker>) => {
    terminal = Object.freeze(result)
    return terminal
  }

  const finishIndeterminate = (
    interval: readonly [number, number],
    reason: string,
  ) => finish({
    kind: 'indeterminate',
    certifiedSafeThrough,
    stopTime: certifiedSafeThrough,
    unresolvedBracket: Object.freeze([...interval]) as readonly [number, number],
    reason,
    stats: stats(),
  })

  const finishCancelled = () => finish({
    kind: 'cancelled',
    certifiedSafeThrough,
    stats: stats(),
  })

  const evaluatePoint = (time: number) => {
    const cached = pointCache.get(time)
    if (cached) {
      pointCacheHits += 1
      return cached
    }
    pointTests += 1
    let decision: FoldPreviewContinuousPointDecision<Blocker>
    try {
      decision = normalizePointDecision<Blocker>(evaluatePointCallback(time))
    } catch {
      decision = Object.freeze({
        kind: 'indeterminate',
        reason: 'point_callback_error',
      })
    }
    pointCache.set(time, decision)
    return decision
  }

  const certifyInterval = (interval: Interval) => {
    intervalTests += 1
    maximumDepthReached = Math.max(maximumDepthReached, interval.depth)
    let decision: FoldPreviewContinuousIntervalDecision
    try {
      decision = normalizeIntervalDecision(
        certifyIntervalCallback(interval.start, interval.end),
      )
    } catch {
      decision = Object.freeze({
        kind: 'indeterminate',
        reason: 'interval_callback_error',
      })
    }
    return decision
  }

  const initialize = () => {
    if (initialized) return null
    initialized = true
    const initial = evaluatePoint(0)
    if (cancelled) return finishCancelled()
    if (initial.kind === 'safe') return null
    if (initial.kind === 'blocked') {
      return finish({
        kind: 'blocked',
        certifiedSafeThrough: 0,
        stopTime: 0,
        unsafeBracket: Object.freeze([0, 0]),
        ...('blocker' in initial ? { blocker: initial.blocker } : {}),
        stats: stats(),
      })
    }
    return finishIndeterminate([0, 0], initial.reason)
  }

  const runStep = (workBudget: number): FoldPreviewContinuousMotionStep<Blocker> => {
    if (terminal) return terminal
    if (cancelled) return finishCancelled()
    if (!Number.isSafeInteger(workBudget) || workBudget <= 0) {
      const next = intervals.at(-1)
      return finishIndeterminate(
        next ? [next.start, next.end] : [certifiedSafeThrough, 1],
        'invalid_work_budget',
      )
    }
    const initialResult = initialize()
    if (initialResult) return initialResult

    let processed = 0
    while (processed < workBudget) {
      if (cancelled) return finishCancelled()
      const interval = intervals.pop()
      if (!interval) {
        certifiedSafeThrough = 1
        return finish({
          kind: 'clear',
          certifiedSafeThrough: 1,
          stopTime: 1,
          stats: stats(),
        })
      }
      if (intervalTests >= resolvedOptions.maxIntervalTests) {
        const endpoint = evaluatePoint(interval.end)
        if (cancelled) return finishCancelled()
        if (endpoint.kind === 'blocked') {
          return finish({
            kind: 'blocked',
            certifiedSafeThrough,
            stopTime: certifiedSafeThrough,
            unsafeBracket: Object.freeze([
              interval.start,
              interval.end,
            ]),
            ...('blocker' in endpoint ? { blocker: endpoint.blocker } : {}),
            stats: stats(),
          })
        }
        if (endpoint.kind === 'indeterminate') {
          return finishIndeterminate(
            [interval.start, interval.end],
            endpoint.reason,
          )
        }
        return finishIndeterminate(
          [interval.start, interval.end],
          'work_limit',
        )
      }
      if (interval.start !== certifiedSafeThrough) {
        return finishIndeterminate(
          [interval.start, interval.end],
          'chronology_error',
        )
      }

      const intervalDecision = certifyInterval(interval)
      processed += 1
      if (cancelled) return finishCancelled()
      if (intervalDecision.kind === 'indeterminate') {
        return finishIndeterminate(
          [interval.start, interval.end],
          intervalDecision.reason,
        )
      }
      if (intervalDecision.kind === 'clear') {
        const cachedConflict = firstCachedConflict(
          pointCache,
          interval.start,
          interval.end,
        )
        if (cachedConflict) {
          return finishIndeterminate(
            [interval.start, interval.end],
            'contradictory_interval_certificate',
          )
        }
        if (interval.end === 1 && intervals.length === 0) {
          const target = evaluatePoint(1)
          if (cancelled) return finishCancelled()
          if (target.kind === 'blocked') {
            return finish({
              kind: 'blocked',
              certifiedSafeThrough,
              stopTime: certifiedSafeThrough,
              unsafeBracket: Object.freeze([interval.start, 1]),
              ...('blocker' in target ? { blocker: target.blocker } : {}),
              stats: stats(),
            })
          }
          if (target.kind === 'indeterminate') {
            return finishIndeterminate(
              [interval.start, 1],
              target.reason,
            )
          }
          certifiedSafeThrough = 1
          return finish({
            kind: 'clear',
            certifiedSafeThrough: 1,
            stopTime: 1,
            stats: stats(),
          })
        }
        certifiedSafeThrough = interval.end
        continue
      }

      const span = interval.end - interval.start
      const atLimit = interval.depth >= resolvedOptions.maxDepth
        || span <= resolvedOptions.minTimeSpan
      if (atLimit) {
        const endpoint = evaluatePoint(interval.end)
        if (cancelled) return finishCancelled()
        if (endpoint.kind === 'blocked') {
          return finish({
            kind: 'blocked',
            certifiedSafeThrough,
            stopTime: certifiedSafeThrough,
            unsafeBracket: Object.freeze([
              interval.start,
              interval.end,
            ]),
            ...('blocker' in endpoint ? { blocker: endpoint.blocker } : {}),
            stats: stats(),
          })
        }
        if (endpoint.kind === 'indeterminate') {
          return finishIndeterminate(
            [interval.start, interval.end],
            endpoint.reason,
          )
        }
        return finishIndeterminate(
          [interval.start, interval.end],
          'uncertified_interval',
        )
      }

      const midpoint = interval.start + span / 2
      if (
        !Number.isFinite(midpoint)
        || midpoint <= interval.start
        || midpoint >= interval.end
      ) {
        return finishIndeterminate(
          [interval.start, interval.end],
          'numerical_subdivision',
        )
      }
      const midpointDecision = evaluatePoint(midpoint)
      if (cancelled) return finishCancelled()
      if (midpointDecision.kind === 'indeterminate') {
        // The first unknown time cannot be later than this midpoint. Search
        // the earlier half before reporting the cached unknown endpoint.
        intervals.push({
          start: interval.start,
          end: midpoint,
          depth: interval.depth + 1,
        })
        maximumDepthReached = Math.max(
          maximumDepthReached,
          interval.depth + 1,
        )
        continue
      }
      const childDepth = interval.depth + 1
      maximumDepthReached = Math.max(maximumDepthReached, childDepth)
      if (midpointDecision.kind === 'blocked') {
        // The first unsafe time cannot be later than this midpoint. Keep only
        // the earlier half and retain the cached blocking endpoint.
        intervals.push({
          start: interval.start,
          end: midpoint,
          depth: childDepth,
        })
      } else {
        // LIFO stack: push the later half first so the earlier half is always
        // processed before any later time can advance the safe lower bound.
        intervals.push(
          { start: midpoint, end: interval.end, depth: childDepth },
          { start: interval.start, end: midpoint, depth: childDepth },
        )
      }
    }

    if (intervals.length === 0) {
      return finishIndeterminate(
        [certifiedSafeThrough, 1],
        'missing_target_validation',
      )
    }
    return Object.freeze({
      kind: 'pending',
      certifiedSafeThrough,
      stats: stats(),
    })
  }

  const step = (workBudget: number): FoldPreviewContinuousMotionStep<Blocker> => {
    if (stepping) {
      // A callback must not recursively advance the same mutable search. Mark
      // the outer invocation cancelled so it cannot publish a partial result.
      cancelled = true
      return Object.freeze({
        kind: 'cancelled',
        certifiedSafeThrough,
        stats: stats(),
      })
    }
    stepping = true
    try {
      return runStep(workBudget)
    } finally {
      stepping = false
    }
  }

  return Object.freeze({
    step,
    cancel() {
      if (!terminal) cancelled = true
    },
  })
}

function resolveOptions(
  options: FoldPreviewContinuousMotionOptions,
): ResolvedOptions | null {
  if (!options || typeof options !== 'object') return null
  const maxDepth = options.maxDepth ?? DEFAULT_MAX_DEPTH
  const maxIntervalTests = options.maxIntervalTests
    ?? DEFAULT_MAX_INTERVAL_TESTS
  const minTimeSpan = options.minTimeSpan ?? DEFAULT_MIN_TIME_SPAN
  if (
    !Number.isSafeInteger(maxDepth)
    || maxDepth < 0
    || maxDepth > 52
    || !Number.isSafeInteger(maxIntervalTests)
    || maxIntervalTests <= 0
    || maxIntervalTests > 1_000_000
    || !Number.isFinite(minTimeSpan)
    || minTimeSpan <= 0
    || minTimeSpan > 1
  ) return null
  return { maxDepth, maxIntervalTests, minTimeSpan }
}

function normalizePointDecision<Blocker>(
  value: unknown,
): FoldPreviewContinuousPointDecision<Blocker> {
  if (!value || typeof value !== 'object') {
    return Object.freeze({
      kind: 'indeterminate',
      reason: 'malformed_point_decision',
    })
  }
  const decision = value as Partial<FoldPreviewContinuousPointDecision<Blocker>>
  if (decision.kind === 'safe') return Object.freeze({ kind: 'safe' })
  if (decision.kind === 'blocked') {
    return Object.freeze(
      'blocker' in decision
        ? { kind: 'blocked', blocker: decision.blocker }
        : { kind: 'blocked' },
    )
  }
  if (
    decision.kind === 'indeterminate'
    && validReason(decision.reason)
  ) {
    return Object.freeze({
      kind: 'indeterminate',
      reason: decision.reason,
    })
  }
  return Object.freeze({
    kind: 'indeterminate',
    reason: 'malformed_point_decision',
  })
}

function normalizeIntervalDecision(
  value: unknown,
): FoldPreviewContinuousIntervalDecision {
  if (!value || typeof value !== 'object') {
    return Object.freeze({
      kind: 'indeterminate',
      reason: 'malformed_interval_decision',
    })
  }
  const decision = value as Partial<FoldPreviewContinuousIntervalDecision>
  if (decision.kind === 'clear') return Object.freeze({ kind: 'clear' })
  if (decision.kind === 'unresolved') return Object.freeze({ kind: 'unresolved' })
  if (
    decision.kind === 'indeterminate'
    && validReason(decision.reason)
  ) {
    return Object.freeze({
      kind: 'indeterminate',
      reason: decision.reason,
    })
  }
  return Object.freeze({
    kind: 'indeterminate',
    reason: 'malformed_interval_decision',
  })
}

function firstCachedConflict<Blocker>(
  cache: ReadonlyMap<number, FoldPreviewContinuousPointDecision<Blocker>>,
  start: number,
  end: number,
) {
  for (const [time, decision] of cache) {
    if (time >= start && time <= end && decision.kind !== 'safe') {
      return { time, decision }
    }
  }
  return null
}

function validReason(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}
