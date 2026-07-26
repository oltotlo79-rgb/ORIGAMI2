import {
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from 'react'

import {
  applyBeginnerParameterGridCandidate,
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

type GridProgress = Readonly<{
  enumerated: number
  globalChecked: number
  refined: number
}>

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
  const [
    beginnerGridSelectedPointId,
    setBeginnerGridSelectedPointId,
  ] = useState<number | null>(null)
  const [beginnerGridBusy, setBeginnerGridBusy] = useState(false)
  const [beginnerGridProgress, setBeginnerGridProgress] =
    useState<GridProgress>(EMPTY_PROGRESS)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestRef = useRef(0)
  const generationRef = useRef<string | null>(null)
  const pollRef = useRef<number | null>(null)
  const busyRef = useRef(false)
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
    scheduleFocus(() => buttonRef.current?.focus())
  }

  function invalidateBeginnerGridForProjectReplacement() {
    requestRef.current += 1
    busyRef.current = false
    stopProgressPolling()
    cancelGeneration()
    setBeginnerGridBusy(false)
    setBeginnerGrid(null)
    setBeginnerGridSelectedPointId(null)
  }

  const cleanupGridOnUnmount = useEffectEvent(() => {
    requestRef.current += 1
    stopProgressPolling()
    cancelGeneration()
  })

  useEffect(() => () => cleanupGridOnUnmount(), [])

  function requestBeginnerGrid() {
    if (busyRef.current || input.skeletonTreeStatus !== 'tree') return
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++requestRef.current
    const generationId = createGenerationId()
    generationRef.current = generationId
    busyRef.current = true
    setBeginnerGridProgress(EMPTY_PROGRESS)
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
        setBeginnerGrid(response)
        setBeginnerGridSelectedPointId(
          response.candidates[0]?.point.id ?? null,
        )
        setBeginnerGridProgress({
          enumerated: 27,
          globalChecked: 3,
          refined: response.refinement_iterations,
        })
      }
    }).catch(() => {
      if (
        requestId === requestRef.current
        && generationRef.current === generationId
      ) setBeginnerGrid(null)
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
      || !grid.candidates.includes(candidate)
      || !matchesBeginnerProjectBinding(grid, current)
    ) return
    const expectedProfile = current.beginner_design_profile
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
        requestRef.current += 1
        setBeginnerGrid(null)
        setBeginnerGridSelectedPointId(null)
      },
      restoreFocus,
    })
  }

  return {
    beginnerGrid,
    beginnerGridSelectedPointId,
    setBeginnerGridSelectedPointId,
    beginnerGridBusy,
    beginnerGridProgress,
    beginnerGridButtonRef: buttonRef,
    requestBeginnerGrid,
    cancelBeginnerGrid,
    confirmAndApplyBeginnerGridCandidate,
    invalidateBeginnerGridForProjectReplacement,
  } as const
}
