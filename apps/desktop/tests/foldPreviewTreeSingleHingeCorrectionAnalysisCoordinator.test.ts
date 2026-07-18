import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  createFoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep,
  type FoldPreviewTreeSingleHingeCorrectionAnalysisRun,
} from '../src/lib/foldPreviewTreeSingleHingeCorrectionAnalysisCoordinator.ts'
import {
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
  type FoldPreviewTreeSingleHingeStaticCandidatePathPresentation,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCandidatePathPresentation.ts'
import {
  FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
} from '../src/lib/foldPreviewTreeSingleHingeCorrectionAnalysisRequest.ts'

test('start clears old output before deferring factory and one step per frame', () => {
  const scheduler = manualScheduler()
  const states: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState[] =
    []
  let factoryCalls = 0
  const budgets: number[] = []
  const steps: FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep[] = [
    pending('static_candidate_analysis'),
    pending('candidate_path_analysis'),
    noCandidate('candidate_path_analysis'),
  ]
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: (state) => states.push(state),
  })

  assert.equal(coordinator.start({
    createJob: () => {
      factoryCalls += 1
      return {
        step: (budget) => {
          budgets.push(budget)
          return steps.shift()
        },
        cancel: () => undefined,
      }
    },
    validateTerminalLease: () => true,
  }), true)
  assert.equal(factoryCalls, 0)
  assert.deepEqual(coordinator.getState(), {
    version: 'tree_single_hinge_correction_analysis_coordinator_v1',
    generation: 1,
    status: 'working',
    phase: 'preparing',
  })
  assert.equal(scheduler.pendingCount(), 1)

  scheduler.runNext()
  assert.equal(factoryCalls, 1)
  assert.deepEqual(budgets, [])
  assert.equal(scheduler.pendingCount(), 1)

  scheduler.runNext()
  assert.deepEqual(budgets, [1])
  assert.equal(coordinator.getState().status, 'working')
  assert.equal(
    statePhase(coordinator.getState()),
    'static_candidate_analysis',
  )
  scheduler.runNext()
  assert.deepEqual(budgets, [1, 1])
  assert.equal(
    statePhase(coordinator.getState()),
    'candidate_path_analysis',
  )
  scheduler.runNext()
  assert.deepEqual(budgets, [1, 1, 1])
  assert.equal(coordinator.getState().status, 'no_candidate')
  assert.equal(scheduler.pendingCount(), 0)
  assert.equal(states[0]?.status, 'working')
})

test('repeated pending steps notify only when the working phase changes', () => {
  const scheduler = manualScheduler()
  const states: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState[] =
    []
  let steps = 0
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: (state) => states.push(state),
  })
  coordinator.start({
    createJob: () => jobWithSteps([
      pending('static_candidate_analysis'),
      pending('static_candidate_analysis'),
      pending('static_candidate_analysis'),
      pending('candidate_path_preparation'),
      pending('candidate_path_preparation'),
      noCandidate('candidate_path_analysis'),
    ], () => { steps += 1 }),
    validateTerminalLease: () => true,
  })

  scheduler.runAll()
  assert.equal(steps, 6)
  assert.deepEqual(
    states
      .filter((state) => state.status === 'working')
      .map((state) => state.status === 'working' ? state.phase : null),
    [
      'preparing',
      'static_candidate_analysis',
      'candidate_path_preparation',
    ],
  )
  assert.equal(states.at(-1)?.status, 'no_candidate')
})

test('certified state is a frozen detached authority-free snapshot', () => {
  const scheduler = manualScheduler()
  const source = presentation()
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: () => undefined,
  })
  coordinator.start(runWithSteps([
    certified(source),
  ]))

  scheduler.runAll()
  const state = coordinator.getState()
  assert.equal(state.status, 'certified')
  assert.ok(Object.isFrozen(state))
  if (state.status !== 'certified') assert.fail('expected certified')
  assert.equal(state.presentation, source)
  assert.ok(Object.isFrozen(state.presentation))
  assert.ok(Object.isFrozen(state.presentation.identity))
  assert.equal(
    Object.hasOwn(state.presentation as object, 'certificate'),
    false,
  )
  assert.equal(
    Object.hasOwn(state.presentation.identity as object, 'ownerToken'),
    false,
  )
  assert.equal(
    Object.hasOwn(state.presentation.safety as object, 'requestLease'),
    false,
  )
  assert.equal(state.presentation.badgeText, 'safe candidate')
  assert.deepEqual(
    Object.keys(state).sort(),
    ['generation', 'presentation', 'status', 'version'],
  )
})

test('lease must be exact true before factory, after work, and at terminal publish', () => {
  for (const invalidLease of [false, 1, new Boolean(true)]) {
    const scheduler = manualScheduler()
    let factoryCalls = 0
    const coordinator = requiredCoordinator({
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      onState: () => undefined,
    })
    coordinator.start({
      createJob: () => {
        factoryCalls += 1
        return jobWithSteps([
          noCandidate('static_candidate_analysis'),
        ])
      },
      validateTerminalLease: () => invalidLease as boolean,
    })
    scheduler.runNext()
    assert.equal(factoryCalls, 0)
    assert.equal(coordinator.getState().status, 'stale')
  }

  const scheduler = manualScheduler()
  let leaseCalls = 0
  let cancellations = 0
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: () => undefined,
  })
  coordinator.start({
    createJob: () => ({
      step: () => certified(presentation()),
      cancel: () => { cancellations += 1 },
    }),
    validateTerminalLease: () => {
      leaseCalls += 1
      return leaseCalls < 4
    },
  })
  scheduler.runAll()
  assert.equal(coordinator.getState().status, 'stale')
  assert.equal(cancellations, 1)
})

test('superseding, invalidating, and disposing revoke forced old callbacks', () => {
  const scheduler = manualScheduler()
  let oldSteps = 0
  let oldCancellations = 0
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: () => undefined,
  })
  coordinator.start({
    createJob: () => ({
      step: () => {
        oldSteps += 1
        return noCandidate('static_candidate_analysis')
      },
      cancel: () => { oldCancellations += 1 },
    }),
    validateTerminalLease: () => true,
  })
  const oldFactoryHandle = scheduler.onlyPendingHandle()
  assert.equal(coordinator.start(runWithSteps([
    noCandidate('static_candidate_analysis'),
  ])), true)
  assert.equal(oldCancellations, 0)
  scheduler.force(oldFactoryHandle)
  assert.equal(oldSteps, 0)
  scheduler.runAll()
  assert.equal(coordinator.getState().status, 'no_candidate')
  assert.equal(coordinator.getState().generation, 2)

  coordinator.start({
    createJob: () => ({
      step: () => pending('candidate_path_preparation'),
      cancel: () => { oldCancellations += 1 },
    }),
    validateTerminalLease: () => true,
  })
  scheduler.runNext()
  const workHandle = scheduler.onlyPendingHandle()
  coordinator.invalidate()
  assert.equal(coordinator.getState().status, 'stale')
  assert.equal(oldCancellations, 1)
  scheduler.force(workHandle)
  assert.equal(coordinator.getState().status, 'stale')

  coordinator.start(runWithSteps([
    noCandidate('static_candidate_analysis'),
  ]))
  const disposeHandle = scheduler.onlyPendingHandle()
  coordinator.dispose()
  coordinator.dispose()
  assert.equal(coordinator.getState().status, 'idle')
  assert.equal(coordinator.start(runWithSteps([])), false)
  scheduler.force(disposeHandle)
  assert.equal(coordinator.getState().status, 'idle')
})

test('new working state is authoritative before old cancellation re-enters', () => {
  const scheduler = manualScheduler()
  let coordinator:
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator
  const cancellationObservations: Array<
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState
  > = []
  coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: () => undefined,
  })
  coordinator.start({
    createJob: () => ({
      step: () => pending('static_candidate_preparation'),
      cancel: () => {
        cancellationObservations.push(coordinator.getState())
        coordinator.invalidate()
      },
    }),
    validateTerminalLease: () => true,
  })
  scheduler.runNext()

  assert.equal(coordinator.start(runWithSteps([
    noCandidate('static_candidate_analysis'),
  ])), false)
  assert.equal(cancellationObservations.length, 1)
  assert.equal(cancellationObservations[0]?.status, 'working')
  assert.equal(cancellationObservations[0]?.generation, 2)
  assert.equal(coordinator.getState().status, 'stale')
  assert.equal(coordinator.getState().generation, 3)
  scheduler.runAll()
  assert.equal(coordinator.getState().status, 'stale')
})

test('state observer re-entry cannot schedule another step for a revoked run', () => {
  const scheduler = manualScheduler()
  let coordinator:
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator
  let steps = 0
  coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: (state) => {
      if (
        state.status === 'working'
        && state.phase === 'static_candidate_analysis'
      ) coordinator.invalidate()
    },
  })
  coordinator.start({
    createJob: () => ({
      step: () => {
        steps += 1
        return pending('static_candidate_analysis')
      },
      cancel: () => undefined,
    }),
    validateTerminalLease: () => true,
  })
  scheduler.runAll()
  assert.equal(steps, 1)
  assert.equal(coordinator.getState().status, 'stale')
  assert.equal(scheduler.pendingCount(), 0)
})

test('lease-validator re-entry cannot invoke a revoked factory or step', () => {
  {
    const scheduler = manualScheduler()
    let coordinator:
      FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator
    let factoryCalls = 0
    coordinator = requiredCoordinator({
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      onState: () => undefined,
    })
    coordinator.start({
      createJob: () => {
        factoryCalls += 1
        return jobWithSteps([
          noCandidate('static_candidate_analysis'),
        ])
      },
      validateTerminalLease: () => {
        coordinator.invalidate()
        return true
      },
    })
    scheduler.runAll()
    assert.equal(factoryCalls, 0)
    assert.equal(coordinator.getState().status, 'stale')
  }

  {
    const scheduler = manualScheduler()
    let coordinator:
      FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator
    let validationCalls = 0
    let stepCalls = 0
    coordinator = requiredCoordinator({
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      onState: () => undefined,
    })
    coordinator.start({
      createJob: () => ({
        step: () => {
          stepCalls += 1
          return noCandidate('static_candidate_analysis')
        },
        cancel: () => undefined,
      }),
      validateTerminalLease: () => {
        validationCalls += 1
        if (validationCalls === 3) coordinator.invalidate()
        return true
      },
    })
    scheduler.runAll()
    assert.equal(stepCalls, 0)
    assert.equal(coordinator.getState().status, 'stale')
  }
})

test('synchronous scheduling and late handles do not orphan work', () => {
  let nextHandle = 1
  const cancelled: number[] = []
  const states: string[] = []
  const coordinator = requiredCoordinator({
    schedule: (callback) => {
      const handle = nextHandle
      nextHandle += 1
      callback()
      return handle
    },
    cancel: (handle) => { cancelled.push(handle) },
    onState: (state) => { states.push(state.status) },
  })
  let steps = 0
  assert.equal(coordinator.start({
    createJob: () => ({
      step: (budget) => {
        assert.equal(budget, 1)
        steps += 1
        return noCandidate('static_candidate_analysis')
      },
      cancel: () => undefined,
    }),
    validateTerminalLease: () => true,
  }), true)
  assert.equal(steps, 1)
  assert.equal(coordinator.getState().status, 'no_candidate')
  assert.deepEqual(states, ['working', 'no_candidate'])
  assert.deepEqual(cancelled, [])
})

test('a synchronous scheduler trampolines long pending runs', () => {
  let nextHandle = 0
  let steps = 0
  const coordinator = requiredCoordinator({
    schedule: (callback) => {
      callback()
      nextHandle += 1
      return nextHandle
    },
    cancel: () => undefined,
    onState: () => undefined,
  })
  assert.equal(coordinator.start({
    createJob: () => ({
      step: () => {
        steps += 1
        return steps <= 20_000
          ? pending('candidate_path_analysis')
          : noCandidate('candidate_path_analysis')
      },
      cancel: () => undefined,
    }),
    validateTerminalLease: () => true,
  }), true)

  assert.equal(steps, 20_001)
  assert.equal(coordinator.getState().status, 'no_candidate')
})

test('candidate path exhaustion uncertainty remains indeterminate', () => {
  const scheduler = manualScheduler()
  const coordinator = requiredCoordinator({
    schedule: scheduler.schedule,
    cancel: scheduler.cancel,
    onState: () => undefined,
  })
  coordinator.start(runWithSteps([{
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
    kind: 'indeterminate',
    phase: 'candidate_path_analysis',
    reason: 'candidate_path_exhausted_indeterminate',
    safety: analysisSafety(),
  }]))

  scheduler.runAll()
  const state = coordinator.getState()
  assert.equal(state.status, 'indeterminate')
  assert.equal(
    stateReason(state),
    'candidate_path_exhausted_indeterminate',
  )
})

test('FoldPreview keeps correction analysis terminal-bound and analysis-only', () => {
  const source = readFileSync(
    new URL('../src/components/FoldPreview.tsx', import.meta.url),
    'utf8',
  )

  const terminalPublication = sourceSection(
    source,
    'const publishTreeRunnerState = (',
    'requestTreeMotionTarget = (',
  )
  const blockedGuardIndex = terminalPublication.indexOf(
    "runnerState.status === 'blocked' && evidenceContext",
  )
  const analysisStartIndex = terminalPublication.indexOf(
    'startTreeCorrectionAnalysis(',
  )
  const evidenceReleaseIndex = terminalPublication.indexOf(
    'binding.activeEvidenceContext = null',
  )
  assert.ok(blockedGuardIndex >= 0)
  assert.ok(analysisStartIndex > blockedGuardIndex)
  assert.ok(evidenceReleaseIndex > analysisStartIndex)

  const requestBoundary = sourceSectionFromLast(
    source,
    'requestTreeMotionTarget = (',
    'const prepareTreeMotionRuntime = (',
  )
  const runtimeTransitionIndex = requestBoundary.indexOf(
    'const runtimePlan = transitionFoldPreviewTreeMotionRuntime(',
  )
  const invalidationIndex = requestBoundary.indexOf(
    'if (runtimePlan) invalidateCorrectionAnalysis()',
  )
  const runtimeExecutionIndex = requestBoundary.indexOf(
    'executeTreeRuntimePlan(binding, runtimePlan)',
  )
  assert.ok(runtimeTransitionIndex >= 0)
  assert.ok(invalidationIndex > runtimeTransitionIndex)
  assert.ok(runtimeExecutionIndex > invalidationIndex)

  const correctionData = sourceSection(
    source,
    'data-correction-status={',
    'data-angle-mode={',
  )
  for (const attribute of [
    'data-correction-status',
    'data-correction-phase',
    'data-correction-analysis-only',
    'data-correction-scene-applied',
    'data-correction-auto-applicable',
  ]) {
    assert.match(correctionData, new RegExp(`${attribute}=\\{`, 'u'))
  }
  const analysisOnlyAttribute = sourceSection(
    correctionData,
    'data-correction-analysis-only={',
    'data-correction-scene-applied={',
  )
  assert.match(analysisOnlyAttribute, /\?\s*true\s*:\s*undefined/u)
  assert.doesNotMatch(analysisOnlyAttribute, /\?\s*false\s*:/u)
  const sceneAppliedAttribute = sourceSection(
    correctionData,
    'data-correction-scene-applied={',
    'data-correction-auto-applicable={',
  )
  assert.match(sceneAppliedAttribute, /\?\s*false\s*:\s*undefined/u)
  const autoApplicableAttribute = sourceSection(
    correctionData,
    'data-correction-auto-applicable={',
    'data-correction-runtime-request-bound={',
  )
  assert.match(autoApplicableAttribute, /\?\s*false\s*:\s*undefined/u)

  const correctionBadge = sourceSection(
    source,
    'className={`fold-preview-correction',
    '</span>',
  )
  assert.match(correctionBadge, /correctionAnalysisView\.badgeClass/u)
  assert.match(correctionBadge, /correctionAnalysisView\.badgeText/u)
  const correctionLiveRegion = sourceSection(
    source,
    'previewAvailable && treeCorrectionAnalysisAvailable ? (',
    '{previewAvailable ? (',
  )
  assert.match(
    correctionLiveRegion,
    /aria-live="polite"[\s\S]*correctionAnalysisView\.liveText/u,
  )

  const analysisStartBoundary = sourceSection(
    source,
    'const startTreeCorrectionAnalysis = (',
    'const failTreeMotion = (',
  )
  assert.match(analysisStartBoundary, /coordinator\.start/u)
  assert.match(
    analysisStartBoundary,
    /createFoldPreviewTreeSingleHingeCorrectionAnalysisJob/u,
  )
  assert.doesNotMatch(
    analysisStartBoundary,
    /\b(?:updatePose|executeTreeRuntimePlan|executeTreeRuntimeCommand|applyTreeRunnerAngle|onCommitHingeFoldAngleRef)\b/u,
  )
  assert.doesNotMatch(
    analysisStartBoundary,
    /\banalysisOnly\s*:\s*false\b/u,
  )
})

test('malformed inputs and internal failures close to indeterminate', () => {
  assert.equal(
    createFoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator({
      schedule: null as never,
      cancel: () => undefined,
      onState: () => undefined,
    }),
    null,
  )

  const cases: Array<{
    run: FoldPreviewTreeSingleHingeCorrectionAnalysisRun
    reason: string
  }> = [
    {
      run: {
        createJob: () => { throw new Error('factory') },
        validateTerminalLease: () => true,
      },
      reason: 'job_factory_error',
    },
    {
      run: {
        createJob: () => null,
        validateTerminalLease: () => true,
      },
      reason: 'job_factory_returned_null',
    },
    {
      run: {
        createJob: () => ({ step: null, cancel: null }) as never,
        validateTerminalLease: () => true,
      },
      reason: 'job_factory_returned_malformed_job',
    },
    {
      run: {
        createJob: () => ({
          step: () => { throw new Error('step') },
          cancel: () => undefined,
        }),
        validateTerminalLease: () => true,
      },
      reason: 'job_step_error',
    },
    {
      run: runWithSteps([{
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
        kind: 'pending',
        status: 'working',
        phase: 'unknown',
        safety: analysisSafety(),
      } as never]),
      reason: 'malformed_job_step',
    },
  ]
  for (const { run, reason } of cases) {
    const scheduler = manualScheduler()
    const coordinator = requiredCoordinator({
      schedule: scheduler.schedule,
      cancel: scheduler.cancel,
      onState: () => { throw new Error('observer') },
    })
    assert.doesNotThrow(() => {
      coordinator.start(run)
      scheduler.runAll()
    })
    const state = coordinator.getState()
    assert.equal(state.status, 'indeterminate')
    assert.equal(stateReason(state), reason)
  }

  const coordinator = requiredCoordinator({
    schedule: () => { throw new Error('scheduler') },
    cancel: () => undefined,
    onState: () => undefined,
  })
  assert.equal(coordinator.start(runWithSteps([])), false)
  assert.equal(
    stateReason(coordinator.getState()),
    'scheduler_error',
  )
})

function requiredCoordinator<ScheduledHandle>(
  options: {
    schedule(callback: () => void): ScheduledHandle
    cancel(handle: ScheduledHandle): void
    onState(
      state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
    ): void
  },
) {
  const coordinator =
    createFoldPreviewTreeSingleHingeCorrectionAnalysisCoordinator(options)
  assert.ok(coordinator)
  return coordinator
}

function runWithSteps(
  values: FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep[],
): FoldPreviewTreeSingleHingeCorrectionAnalysisRun {
  return {
    createJob: () => jobWithSteps(values),
    validateTerminalLease: () => true,
  }
}

function deepFreeze<T>(value: T): T {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) {
    return value
  }
  for (const key of Reflect.ownKeys(value)) {
    deepFreeze((value as Record<PropertyKey, unknown>)[key])
  }
  return Object.freeze(value)
}

function jobWithSteps(
  values: FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep[],
  onStep: () => void = () => undefined,
) {
  const remaining = [...values]
  return {
    step: (_budget: number) => {
      onStep()
      return remaining.shift() as
        FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep
    },
    cancel: () => undefined,
  }
}

function pending(
  phase: FoldPreviewTreeSingleHingeCorrectionAnalysisJobPhase,
): FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep {
  return {
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
    kind: 'pending',
    status: 'working',
    phase,
    safety: analysisSafety(),
  }
}

function noCandidate(
  exhaustedPhase:
    | 'static_candidate_analysis'
    | 'candidate_path_analysis',
): FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep {
  return {
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
    kind: 'no_candidate',
    exhaustedPhase,
    safety: analysisSafety(),
  }
}

function certified(
  value: FoldPreviewTreeSingleHingeStaticCandidatePathPresentation,
): FoldPreviewTreeSingleHingeCorrectionAnalysisJobStep {
  return {
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_CORRECTION_ANALYSIS_JOB_VERSION,
    kind: 'certified',
    presentation: value,
    safety: analysisSafety(),
  }
}

function analysisSafety() {
  return {
    analysisOnly: true as const,
    sceneApplied: false as const,
    autoApplicable: false as const,
  }
}

function manualScheduler() {
  let nextHandle = 1
  const callbacks = new Map<number, () => void>()
  const pending = new Set<number>()
  return {
    schedule: (callback: () => void) => {
      const handle = nextHandle
      nextHandle += 1
      callbacks.set(handle, callback)
      pending.add(handle)
      return handle
    },
    cancel: (handle: number) => {
      pending.delete(handle)
    },
    pendingCount: () => pending.size,
    onlyPendingHandle: () => {
      assert.equal(pending.size, 1)
      return [...pending][0] as number
    },
    runNext: () => {
      const handle = [...pending][0]
      assert.ok(handle)
      pending.delete(handle)
      const callback = callbacks.get(handle)
      assert.ok(callback)
      callback()
    },
    runAll: () => {
      let guard = 0
      while (pending.size > 0) {
        guard += 1
        assert.ok(guard < 100)
        const handle = [...pending][0] as number
        pending.delete(handle)
        const callback = callbacks.get(handle)
        assert.ok(callback)
        callback()
      }
    },
    force: (handle: number) => {
      const callback = callbacks.get(handle)
      assert.ok(callback)
      callback()
    },
  }
}

function statePhase(
  state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
) {
  return state.status === 'working' ? state.phase : null
}

function stateReason(
  state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
) {
  return state.status === 'indeterminate' ? state.reason : null
}

function sourceSection(
  source: string,
  startMarker: string,
  endMarker: string,
) {
  const startIndex = source.indexOf(startMarker)
  assert.ok(startIndex >= 0, `missing source marker: ${startMarker}`)
  const endIndex = source.indexOf(
    endMarker,
    startIndex + startMarker.length,
  )
  assert.ok(endIndex > startIndex, `missing source marker: ${endMarker}`)
  return source.slice(startIndex, endIndex)
}

function sourceSectionFromLast(
  source: string,
  startMarker: string,
  endMarker: string,
) {
  const startIndex = source.lastIndexOf(startMarker)
  assert.ok(startIndex >= 0, `missing source marker: ${startMarker}`)
  const endIndex = source.indexOf(
    endMarker,
    startIndex + startMarker.length,
  )
  assert.ok(endIndex > startIndex, `missing source marker: ${endMarker}`)
  return source.slice(startIndex, endIndex)
}

function presentation():
  FoldPreviewTreeSingleHingeStaticCandidatePathPresentation {
  const stats = {
    intervalTests: 2,
    pointTests: 3,
    pointCacheHits: 1,
    maximumDepthReached: 4,
  }
  return deepFreeze({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
    kind: 'certified_static_candidate_path_presentation',
    identity: {
      projectId: 'project',
      revision: 3,
      selectedHingeEdgeId: 'edge',
    },
    candidate: { rank: 1 },
    angles: {
      sourceDegrees: 10,
      targetDegrees: 20,
      deltaDegrees: 10,
      absoluteDeltaDegrees: 10,
      direction: 'increasing',
    },
    continuous: {
      stats,
      aggregateStats: stats,
      precedingAttemptCount: 0,
    },
    staticInteractionSummary: {
      broadPhaseCandidateCount: 1,
      broadPhaseNonAdjacentCandidateCount: 1,
      broadPhaseHingeAdjacentCandidateCount: 0,
      interactionCount: 0,
      allowedHingeInteractionCount: 0,
      trianglePairTests: 1,
      satTests: 1,
      numericalMargin: 0.001,
      fullScanBroadPhaseCandidateCount: 1,
      fullScanExpectedTrianglePairCount: 1,
      fullScanTrianglePairTests: 1,
      fullScanAabbRejectedPairCount: 0,
      fullScanSatTests: 1,
      fullScanSatSeparatedPairCount: 1,
    },
    workBounds: {
      entireStepTimeBounded: false,
      synchronousFactoryPreparation: true,
      synchronousChildJobPreparation: true,
      synchronousResultFinalization: true,
      candidateCount: 1,
      maximumCumulativeIntervalTests: 2,
      maximumCumulativeIntervalPairVisits: 3,
      maximumCumulativePointTriangleTests: 4,
      terminalEvidenceFullScanEnabled: false,
    },
    badgeText: 'safe candidate',
    accessibleText: 'safe candidate details',
    limitation: 'analysis only',
    safety: {
      analysisOnly: true,
      staticCandidateRevalidated: true,
      continuousCandidatePathCertified: true,
      runtimeRequestBound: false,
      activeRequestLeaseBound: false,
      startScenePoseMatched: false,
      sceneApplied: false,
      autoApplicable: false,
    },
  })
}
