import {
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from 'react'

import {
  applyBeginnerParameterGridCandidate,
  beginnerGeneratedPlanAssessmentAllowsApplyV1,
  cancelBeginnerParameterGrid,
  evaluateBeginnerParameterGrid,
  getBeginnerParameterGridProgress,
  type BeginnerGridEvaluationResponse,
  type ProjectSnapshot,
} from './coreClient.ts'
import {
  finishBeginnerGridCancellation,
  runBeginnerGridApplyWorkflow,
} from './beginnerGridWorkflow.ts'
import type { LocalizedText } from './i18n.ts'
import {
  beginnerProjectBinding,
  matchesBeginnerProjectBinding,
  type BeginnerNativeEditRunner,
} from './beginnerWorkflowSupport.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

type GridProgress = Readonly<{
  enumerated: number
  globalChecked: number
  refined: number
}>

type BeginnerGridRequestStatus =
  | 'idle'
  | 'running'
  | 'ready'
  | 'empty'
  | 'cancelled'
  | 'failed'

type GridTransport = Readonly<{
  evaluate: typeof evaluateBeginnerParameterGrid
  progress: typeof getBeginnerParameterGridProgress
  cancel: typeof cancelBeginnerParameterGrid
  apply: typeof applyBeginnerParameterGridCandidate
}>

const EMPTY_PROGRESS: GridProgress = Object.freeze({
  enumerated: 0,
  globalChecked: 0,
  refined: 0,
})

const DEFAULT_TRANSPORT: GridTransport = Object.freeze({
  evaluate: evaluateBeginnerParameterGrid,
  progress: getBeginnerParameterGridProgress,
  cancel: cancelBeginnerParameterGrid,
  apply: applyBeginnerParameterGridCandidate,
})

export function useBeginnerParameterGridWorkflow(input: Readonly<{
  getCurrentSnapshot: () => ProjectSnapshot | null
  skeletonTreeStatus: string
  runNativeEdit: BeginnerNativeEditRunner
  confirm: (message: LocalizedText) => boolean
  applyConfirmation: LocalizedText
  transport?: GridTransport
  createGenerationId?: () => string
  startPolling?: (callback: () => void) => number
  stopPolling?: (handle: number) => void
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [beginnerGrid, setBeginnerGrid] =
    useState<BeginnerGridEvaluationResponse | null>(null)
  const gridAuthorityRef =
    useRef<BeginnerGridEvaluationResponse | null>(null)
  const [
    beginnerGridSelectedPointId,
    setBeginnerGridSelectedPointId,
  ] = useState<number | null>(null)
  const [beginnerGridBusy, setBeginnerGridBusy] = useState(false)
  const [beginnerGridApplyBusy, setBeginnerGridApplyBusy] = useState(false)
  const [
    beginnerGridRequestStatus,
    setBeginnerGridRequestStatus,
  ] = useState<BeginnerGridRequestStatus>('idle')
  const [beginnerGridProgress, setBeginnerGridProgress] =
    useState<GridProgress>(EMPTY_PROGRESS)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestRef = useRef(0)
  const applyRequestRef = useRef(0)
  const generationRef = useRef<string | null>(null)
  const pollRef = useRef<number | null>(null)
  const busyRef = useRef(false)
  const applyBusyRef = useRef(false)
  const mountedRef = useRef(true)
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const createGenerationId =
    input.createGenerationId ?? (() => crypto.randomUUID())
  const startPolling = input.startPolling
    ?? ((callback: () => void) => window.setInterval(callback, 50))
  const stopPolling = input.stopPolling
    ?? ((handle: number) => window.clearInterval(handle))
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function stopProgressPolling() {
    if (pollRef.current === null) return
    stopPolling(pollRef.current)
    pollRef.current = null
  }

  function cancelGeneration() {
    const generationId = generationRef.current
    generationRef.current = null
    if (generationId) {
      void transport.cancel(generationId).catch(() => undefined)
    }
  }

  function restoreFocus() {
    if (!mountedRef.current) return
    scheduleFocus(() => buttonRef.current?.focus())
  }

  function invalidateBeginnerGridForProjectReplacement() {
    requestRef.current += 1
    applyRequestRef.current += 1
    busyRef.current = false
    applyBusyRef.current = false
    stopProgressPolling()
    cancelGeneration()
    setBeginnerGridBusy(false)
    setBeginnerGridApplyBusy(false)
    setBeginnerGridRequestStatus('idle')
    gridAuthorityRef.current = null
    setBeginnerGrid(null)
    setBeginnerGridSelectedPointId(null)
    setBeginnerGridProgress(EMPTY_PROGRESS)
  }

  const cleanupGridOnUnmount = useEffectEvent(() => {
    mountedRef.current = false
    requestRef.current += 1
    applyRequestRef.current += 1
    busyRef.current = false
    applyBusyRef.current = false
    gridAuthorityRef.current = null
    stopProgressPolling()
    cancelGeneration()
  })

  useEffect(() => {
    mountedRef.current = true
    return () => cleanupGridOnUnmount()
  }, [])

  function requestBeginnerGrid() {
    if (
      busyRef.current
      || applyBusyRef.current
      || input.skeletonTreeStatus !== 'tree'
    ) return
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++requestRef.current
    const generationId = createGenerationId()
    generationRef.current = generationId
    busyRef.current = true
    setBeginnerGridProgress(EMPTY_PROGRESS)
    gridAuthorityRef.current = null
    setBeginnerGrid(null)
    setBeginnerGridSelectedPointId(null)
    setBeginnerGridRequestStatus('running')
    setBeginnerGridBusy(true)
    stopProgressPolling()
    pollRef.current = startPolling(() => {
      void transport.progress(generationId).then((progress) => {
        if (
          requestId !== requestRef.current
          || generationRef.current !== generationId
        ) return
        setBeginnerGridProgress((currentProgress) => ({
          enumerated: Math.max(
            currentProgress.enumerated,
            progress.enumerated_grid_points,
          ),
          globalChecked: Math.max(
            currentProgress.globalChecked,
            progress.global_checked_candidates,
          ),
          refined: Math.max(
            currentProgress.refined,
            progress.refinement_iterations,
          ),
        }))
      }).catch(() => undefined)
    })
    void transport.evaluate(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
      generationId,
      current.beginner_design_profile,
    ).then((response) => {
      if (
        requestId === requestRef.current
        && generationRef.current === generationId
        && matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        && matchesBeginnerProjectBinding(
          response,
          input.getCurrentSnapshot(),
        )
      ) {
        const firstCandidate = isCanonicalNonNilUuid(
          response.authority_token,
        )
          ? response.candidates[0]
          : undefined
        gridAuthorityRef.current = firstCandidate ? response : null
        setBeginnerGrid(firstCandidate ? response : null)
        setBeginnerGridSelectedPointId(firstCandidate?.point.id ?? null)
        setBeginnerGridRequestStatus(
          firstCandidate
            ? 'ready'
            : response.candidates.length === 0
              ? 'empty'
              : 'failed',
        )
        setBeginnerGridProgress({
          enumerated: response.evaluated_grid_points,
          globalChecked: response.global_checked_candidates,
          refined: response.refinement_iterations,
        })
      }
    }).catch(() => {
      if (
        requestId === requestRef.current
        && generationRef.current === generationId
      ) {
        gridAuthorityRef.current = null
        setBeginnerGrid(null)
        setBeginnerGridSelectedPointId(null)
        setBeginnerGridRequestStatus('failed')
      }
    }).finally(() => {
      if (
        requestId === requestRef.current
        && generationRef.current === generationId
      ) {
        stopProgressPolling()
        generationRef.current = null
        busyRef.current = false
        setBeginnerGridBusy(false)
      }
    })
  }

  function cancelBeginnerGrid() {
    requestRef.current += 1
    busyRef.current = false
    stopProgressPolling()
    cancelGeneration()
    setBeginnerGridBusy(false)
    setBeginnerGridProgress(EMPTY_PROGRESS)
    setBeginnerGridRequestStatus('cancelled')
    gridAuthorityRef.current = null
    finishBeginnerGridCancellation(
      () => {
        setBeginnerGrid(null)
        setBeginnerGridSelectedPointId(null)
      },
      restoreFocus,
    )
  }

  function confirmAndApplyBeginnerGridCandidate(
    candidate: BeginnerGridEvaluationResponse['candidates'][number],
  ) {
    const grid = beginnerGrid
    const current = input.getCurrentSnapshot()
    if (
      !grid
      || !current
      || applyBusyRef.current
      || gridAuthorityRef.current !== grid
      || !isCanonicalNonNilUuid(grid.authority_token)
      || !grid.candidates.includes(candidate)
      || !beginnerGeneratedPlanAssessmentAllowsApplyV1(
        candidate.assessment,
      )
      || !matchesBeginnerProjectBinding(grid, current)
    ) return
    const expectedProfile = current.beginner_design_profile
    const applyRequestId = ++applyRequestRef.current
    applyBusyRef.current = true
    setBeginnerGridApplyBusy(true)
    void runBeginnerGridApplyWorkflow({
      confirm: () => input.confirm(input.applyConfirmation),
      apply: () => input.runNativeEdit((
        projectId,
        revision,
        projectInstanceId,
      ) => transport.apply(
        projectId,
        revision,
        projectInstanceId,
        grid,
        expectedProfile,
        candidate,
      )),
      clearPreview: () => {
        if (
          !mountedRef.current
          || applyRequestRef.current !== applyRequestId
        ) return
        requestRef.current += 1
        gridAuthorityRef.current = null
        setBeginnerGrid(null)
        setBeginnerGridSelectedPointId(null)
        setBeginnerGridRequestStatus('idle')
      },
      restoreFocus: () => {
        if (applyRequestRef.current === applyRequestId) restoreFocus()
      },
    }).catch(() => false).finally(() => {
      if (applyRequestRef.current === applyRequestId) {
        applyBusyRef.current = false
        if (mountedRef.current) setBeginnerGridApplyBusy(false)
      }
    })
  }

  return {
    beginnerGrid,
    beginnerGridSelectedPointId,
    setBeginnerGridSelectedPointId,
    beginnerGridBusy,
    beginnerGridApplyBusy,
    beginnerGridRequestStatus,
    beginnerGridProgress,
    beginnerGridButtonRef: buttonRef,
    requestBeginnerGrid,
    cancelBeginnerGrid,
    confirmAndApplyBeginnerGridCandidate,
    invalidateBeginnerGridForProjectReplacement,
  } as const
}
