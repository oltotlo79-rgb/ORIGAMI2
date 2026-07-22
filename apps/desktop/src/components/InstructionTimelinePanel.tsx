import {
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import {
  addInstructionStep,
  moveInstructionStep,
  splitInstructionStep,
  mergeAdjacentInstructionSteps,
  removeInstructionStep,
  replaceInstructionStepPose,
  updateInstructionStepMetadata,
  type InstructionVisual,
  type ProjectSnapshot,
} from '../lib/coreClient'
import type { FoldPreviewAppliedPoseSnapshot } from '../lib/foldPreviewAppliedPose'
import { pathCertificateEndpointsMatch } from '../lib/pathCertificateIntegrity'
import {
  DEFAULT_INSTRUCTION_DURATION_MS,
  INSTRUCTION_APPLICATION_TIMEOUT_MS,
  MAX_INSTRUCTION_CAUTION_CHARACTERS,
  MAX_INSTRUCTION_DESCRIPTION_CHARACTERS,
  MAX_INSTRUCTION_STEPS,
  MAX_INSTRUCTION_TITLE_CHARACTERS,
  MAX_INSTRUCTION_DURATION_MS,
  MIN_INSTRUCTION_DURATION_MS,
  createInstructionPlaybackPlan,
  createInstructionInterpolatedStep,
  createInstructionPlaybackState,
  createInstructionPoseDraft,
  createInstructionTimelinePresentation,
  formatInstructionDuration,
  instructionCaptureStatusText,
  instructionEditorErrorText,
  instructionPoseMatchesApplied,
  instructionTimelineNoticeText,
  parseInstructionVisual,
  reduceInstructionPlayback,
  resolveInstructionPoseApplicationObservation,
  validateInstructionMetadata,
  type InstructionCaptureStatus,
  type InstructionEditorError,
  type InstructionPlaybackStopReason,
  type InstructionStepPresentation,
  type InstructionTimelineNotice,
} from '../lib/instructionTimeline'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocalizedText,
} from '../lib/i18n'

type InstructionEditorState = {
  stepId: string
  title: string
  description: string
  caution: string
  durationMs: string
  visualJson: string
}

type InstructionTimelinePanelProps = {
  snapshot: ProjectSnapshot | null
  appliedPose: FoldPreviewAppliedPoseSnapshot | null
  currentCamera?: NonNullable<InstructionVisual['camera']> | null
  poseModelKey: string | null
  manualPoseChangeSequence: number
  coreBusy: boolean
  benchmarkActive: boolean
  fileOperationActive: boolean
  exportAvailable: boolean
  exportButtonRef: RefObject<HTMLButtonElement | null>
  animationExportButtonRef: RefObject<HTMLButtonElement | null>
  inert?: boolean
  runNativeEdit(
    action: (
      projectId: string,
      revision: number,
      projectInstanceId: string,
    ) => Promise<ProjectSnapshot>,
  ): Promise<boolean>
  applyStepPose(step: InstructionStepPresentation): boolean
  onExport(): void
  onAnimationExport(): void
}

type PathCertificateDisplay =
  | Readonly<{
      kind: 'verified'
      shortBinding: string
      shortSource: string
      shortTarget: string
      shortModelBinding: string
      transitionCount: number
    }>
  | Readonly<{ kind: 'mismatch' | 'text-only' }>

const PATH_CERTIFICATE_MARKER = '経路証明 SHA-256:'
const EMPTY_INSTRUCTION_STEPS: readonly InstructionStepPresentation[] = Object.freeze([])

function shortHash(bytes: readonly number[]): string {
  return `${bytes.slice(0, 6).map((byte) => byte.toString(16).padStart(2, '0')).join('')}…`
}

function createPathCertificateDisplay(
  step: InstructionStepPresentation,
): PathCertificateDisplay | null {
  const reference = step.visual.path_certificate_reference_v1
  const hasCertificateText = step.description.includes(PATH_CERTIFICATE_MARKER)
  if (!reference) return hasCertificateText ? { kind: 'text-only' } : null

  const binding = reference.binding_sha256
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('')
  const expectedText = `${PATH_CERTIFICATE_MARKER} ${binding} / 元モデル SHA-256: ${step.pose.source_model_fingerprint}`
  if (!step.description.includes(expectedText)) return { kind: 'mismatch' }

  return {
    kind: 'verified',
    shortBinding: shortHash(reference.binding_sha256),
    shortSource: shortHash(reference.source_pose_sha256),
    shortTarget: shortHash(reference.target_pose_sha256),
    shortModelBinding: shortHash(reference.source_model_binding_sha256),
    transitionCount: reference.transition_count,
  }
}

export function InstructionTimelinePanel({
  snapshot,
  appliedPose,
  currentCamera = null,
  poseModelKey,
  manualPoseChangeSequence,
  coreBusy,
  benchmarkActive,
  fileOperationActive,
  exportAvailable,
  exportButtonRef,
  animationExportButtonRef,
  inert,
  runNativeEdit,
  applyStepPose,
  onExport,
  onAnimationExport,
}: InstructionTimelinePanelProps) {
  const locale = useLocale()
  const presentation = useMemo(
    () => createInstructionTimelinePresentation(
      snapshot?.instruction_timeline,
      snapshot?.fold_model_fingerprint,
    ),
    [snapshot?.fold_model_fingerprint, snapshot?.instruction_timeline],
  )
  const [selectedStepId, setSelectedStepId] = useState<string | null>(null)
  const [editor, setEditor] = useState<InstructionEditorState | null>(null)
  const [editorError, setEditorError] = useState<InstructionEditorError | null>(null)
  const [notice, setNotice] = useState<InstructionTimelineNotice | null>(null)
  const [playback, setPlayback] = useState(createInstructionPlaybackState)
  const playbackRef = useRef(playback)
  playbackRef.current = playback
  const applyAttemptRef = useRef<string | null>(null)
  const applyObservationRef = useRef<FoldPreviewAppliedPoseSnapshot | null>(null)
  const playbackModelKeyRef = useRef<string | null>(null)
  const previousManualPoseChangeRef = useRef(manualPoseChangeSequence)
  const currentAppliedPoseRef = useRef<FoldPreviewAppliedPoseSnapshot | null>(null)
  const animationActiveRef = useRef(false)
  const animationWasUsedRef = useRef(false)

  const steps = presentation.kind === 'ready'
    ? presentation.steps
    : EMPTY_INSTRUCTION_STEPS
  const finalPhysicalStepId = steps.findLast((step) => !step.declarativeOnly)?.id ?? null
  const firstPhysicalStep = steps.find((step) => !step.declarativeOnly)
  const selectedStep = presentation.kind === 'ready' && selectedStepId
    ? presentation.stepsById.get(selectedStepId) ?? null
    : null
  const selectedProofDisplay = selectedStep
    ? createPathCertificateDisplay(selectedStep)
    : null
  const [proofEndpointValidation, setProofEndpointValidation] = useState<Readonly<{
    step: InstructionStepPresentation
    status: 'checking' | 'valid' | 'invalid'
  }> | null>(null)
  useEffect(() => {
    let active = true
    if (selectedStep) setProofEndpointValidation({ step: selectedStep, status: 'checking' })
    if (!selectedStep || selectedProofDisplay?.kind !== 'verified') return () => { active = false }
    const index = steps.findIndex((step) => step.id === selectedStep.id)
    void pathCertificateEndpointsMatch(steps[index - 1], selectedStep).then((matches) => {
      if (active) setProofEndpointValidation({
        step: selectedStep,
        status: matches ? 'valid' : 'invalid',
      })
    }, () => {
      if (active) setProofEndpointValidation({ step: selectedStep, status: 'invalid' })
    })
    return () => { active = false }
  }, [selectedStep, selectedProofDisplay?.kind, steps])
  const hasStructuredCertificates = steps.some(
    (step) => step.visual.path_certificate_reference_v1 != null,
  )
  const [certificateExportValidation, setCertificateExportValidation] = useState<Readonly<{
    steps: readonly InstructionStepPresentation[]
    status: 'checking' | 'valid' | 'invalid'
  }> | null>(null)
  useEffect(() => {
    let active = true
    if (!hasStructuredCertificates) {
      setCertificateExportValidation({ steps, status: 'valid' })
      return () => { active = false }
    }
    setCertificateExportValidation({ steps, status: 'checking' })
    const checks = steps.map(async (step, index) => {
      if (step.visual.path_certificate_reference_v1 == null) return true
      if (createPathCertificateDisplay(step)?.kind !== 'verified') return false
      return pathCertificateEndpointsMatch(steps[index - 1], step)
    })
    void Promise.all(checks).then((results) => {
      if (active) setCertificateExportValidation({
        steps,
        status: results.every(Boolean) ? 'valid' : 'invalid',
      })
    }, () => {
      if (active) setCertificateExportValidation({ steps, status: 'invalid' })
    })
    return () => { active = false }
  }, [hasStructuredCertificates, steps])
  const certificateExportBlocked = hasStructuredCertificates
    && (certificateExportValidation?.steps !== steps
      || certificateExportValidation.status !== 'valid')
  const captureDraft = useMemo(() => {
    if (
      !snapshot
      || !appliedPose
      || appliedPose.projectId !== snapshot.project_id
      || appliedPose.revision !== snapshot.revision
    ) return null
    return createInstructionPoseDraft(
      appliedPose,
      snapshot.fold_model_fingerprint,
    )
  }, [appliedPose, snapshot])
  const currentAppliedPose = snapshot
    && appliedPose?.projectId === snapshot.project_id
    && appliedPose.revision === snapshot.revision
      ? appliedPose
      : null
  currentAppliedPoseRef.current = currentAppliedPose
  const selectedPoseIsDisplayed = Boolean(
    selectedStep
    && !selectedStep.stale
    && instructionPoseMatchesApplied(selectedStep.pose, currentAppliedPose),
  )
  const playbackActive = playback.status === 'applying'
    || playback.status === 'holding'
  const timelineAvailable = presentation.kind === 'ready'
  const editingDisabled = coreBusy || benchmarkActive || fileOperationActive
    || !snapshot || !timelineAvailable
  const noticeText = notice
    ? instructionTimelineNoticeText(notice, locale)
    : ''

  const cancelPlayback = useCallback((reason: InstructionPlaybackStopReason) => {
    setPlayback((current) => reduceInstructionPlayback(current, {
      kind: 'cancel',
      reason,
    }))
  }, [])

  useEffect(() => {
    if (!selectedStepId) return
    if (presentation.kind === 'ready' && presentation.stepsById.has(selectedStepId)) {
      return
    }
    setSelectedStepId(null)
    setEditor(null)
    setEditorError(null)
  }, [presentation, selectedStepId])

  useEffect(() => {
    if (!selectedStep) {
      setEditor(null)
      setEditorError(null)
      return
    }
    setEditor({
      stepId: selectedStep.id,
      title: selectedStep.title,
      description: selectedStep.description,
      caution: selectedStep.caution,
      durationMs: String(selectedStep.durationMs),
      visualJson: JSON.stringify(selectedStep.visual, null, 2),
    })
    setEditorError(null)
  }, [selectedStep])

  useEffect(() => {
    setPlayback((current) => {
      if (current.status !== 'applying' && current.status !== 'holding') return current
      if (!snapshot || snapshot.project_id !== current.plan.projectId) {
        return reduceInstructionPlayback(current, {
          kind: 'cancel',
          reason: 'project_changed',
        })
      }
      if (snapshot.revision !== current.plan.revision) {
        return reduceInstructionPlayback(current, {
          kind: 'cancel',
          reason: 'revision_changed',
        })
      }
      if (
        snapshot.fold_model_fingerprint !== current.plan.modelFingerprint
        || poseModelKey !== playbackModelKeyRef.current
      ) {
        return reduceInstructionPlayback(current, {
          kind: 'cancel',
          reason: 'model_changed',
        })
      }
      return current
    })
  }, [poseModelKey, snapshot])

  useEffect(() => {
    if (previousManualPoseChangeRef.current !== manualPoseChangeSequence) {
      previousManualPoseChangeRef.current = manualPoseChangeSequence
      cancelPlayback('manual_pose')
    }
  }, [cancelPlayback, manualPoseChangeSequence])

  useEffect(() => {
    if (benchmarkActive) cancelPlayback('benchmark')
  }, [benchmarkActive, cancelPlayback])

  useEffect(() => {
    if (coreBusy) cancelPlayback('revision_changed')
  }, [cancelPlayback, coreBusy])

  useEffect(() => {
    if (fileOperationActive) cancelPlayback('file_operation')
  }, [cancelPlayback, fileOperationActive])

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') cancelPlayback('hidden')
    }
    document.addEventListener('visibilitychange', handleVisibilityChange)
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange)
  }, [cancelPlayback])

  useEffect(() => () => {
    playbackRef.current = reduceInstructionPlayback(playbackRef.current, {
      kind: 'cancel',
      reason: 'disposed',
    })
  }, [])

  useEffect(() => {
    if (playback.status !== 'applying') {
      animationActiveRef.current = false
      applyAttemptRef.current = null
      applyObservationRef.current = null
      return
    }
    const attemptKey = `${playback.sequence}:${playback.cursor}:${playback.target.id}`
    if (applyAttemptRef.current === attemptKey) return
    applyAttemptRef.current = attemptKey
    const animationStartPose = currentAppliedPoseRef.current
    applyObservationRef.current = animationStartPose
    animationWasUsedRef.current = false
    setSelectedStepId(playback.target.id)
    const animatedStart = createInstructionInterpolatedStep(
      playback.target,
      animationStartPose,
      0,
    )
    if (animatedStart && playback.target.durationMs > 0) {
      animationActiveRef.current = true
      animationWasUsedRef.current = true
      let initialApplied = false
      try {
        initialApplied = applyStepPose(animatedStart)
      } catch {
        initialApplied = false
      }
      if (!initialApplied) {
        animationActiveRef.current = false
        setPlayback((current) => reduceInstructionPlayback(current, {
          kind: 'apply_failed',
        }))
        return
      }
      let elapsedMs = 0
      let previousFrameTime: number | null = null
      let frame = 0
      const animate = (now: number) => {
        if (
          playbackRef.current.status !== 'applying'
          || playbackRef.current.sequence !== playback.sequence
          || playbackRef.current.target.id !== playback.target.id
        ) {
          animationActiveRef.current = false
          return
        }
        if (previousFrameTime !== null) {
          const frameDelta = now - previousFrameTime
          elapsedMs += Number.isFinite(frameDelta) && frameDelta > 0
            ? frameDelta
            : 1_000 / 60
        }
        previousFrameTime = now
        const progress = Math.min(1, Math.max(
          0,
          elapsedMs / playback.target.durationMs,
        ))
        const step = createInstructionInterpolatedStep(
          playback.target,
          applyObservationRef.current,
          progress,
        )
        let applied = false
        try {
          applied = step !== null && applyStepPose(step)
        } catch {
          applied = false
        }
        if (!applied) {
          animationActiveRef.current = false
          setPlayback((current) => reduceInstructionPlayback(current, {
            kind: 'apply_failed',
          }))
          return
        }
        frame = window.requestAnimationFrame(animate)
      }
      frame = window.requestAnimationFrame(animate)
      const completionTimer = window.setTimeout(() => {
        window.cancelAnimationFrame(frame)
        let finalApplied = false
        try {
          finalApplied = applyStepPose(playback.target)
        } catch {
          finalApplied = false
        }
        animationActiveRef.current = false
        if (!finalApplied) {
          setPlayback((current) => reduceInstructionPlayback(current, {
            kind: 'apply_failed',
          }))
          return
        }
        setPlayback((current) => {
          const now = performance.now()
          return reduceInstructionPlayback(
            reduceInstructionPlayback(current, {
              kind: 'pose_applied',
              stepId: playback.target.id,
              now,
              animated: true,
            }),
            { kind: 'tick', now },
          )
        })
      }, playback.target.durationMs)
      return () => {
        animationActiveRef.current = false
        window.cancelAnimationFrame(frame)
        window.clearTimeout(completionTimer)
      }
    }
    let applied = false
    try {
      applied = !playback.target.stale && applyStepPose(playback.target)
    } catch {
      applied = false
    }
    if (!applied) {
      setPlayback((current) => reduceInstructionPlayback(current, {
        kind: 'apply_failed',
      }))
    }
  }, [applyStepPose, playback])

  useEffect(() => {
    if (
      playback.status !== 'applying'
      || playback.target.stale
      || !snapshot
      || snapshot.project_id !== playback.plan.projectId
      || snapshot.revision !== playback.plan.revision
    ) return
    const observation = resolveInstructionPoseApplicationObservation(
      playback.target.pose,
      applyObservationRef.current,
      currentAppliedPose,
    )
    if (animationActiveRef.current) return
    if (observation === 'acknowledge') {
      setPlayback((current) => reduceInstructionPlayback(current, {
        kind: 'pose_applied',
        stepId: playback.target.id,
        now: performance.now(),
        animated: animationWasUsedRef.current,
      }))
      return
    }
    if (observation === 'fail') {
      setPlayback((current) => reduceInstructionPlayback(current, {
        kind: 'apply_failed',
      }))
    }
  }, [currentAppliedPose, playback, snapshot])

  useEffect(() => {
    if (playback.status !== 'applying') return
    const sequence = playback.sequence
    const stepId = playback.target.id
    const handle = window.setTimeout(() => {
      setPlayback((current) => (
        current.status === 'applying'
        && current.sequence === sequence
        && current.target.id === stepId
          ? reduceInstructionPlayback(current, { kind: 'apply_failed' })
          : current
      ))
    }, Math.max(
      INSTRUCTION_APPLICATION_TIMEOUT_MS,
      playback.target.durationMs + 5_000,
    ))
    return () => window.clearTimeout(handle)
  }, [playback])

  useEffect(() => {
    if (playback.status !== 'holding') return
    const delay = Math.max(0, playback.holdUntil - performance.now())
    const handle = window.setTimeout(() => {
      setPlayback((current) => reduceInstructionPlayback(current, {
        kind: 'tick',
        now: performance.now(),
      }))
    }, delay)
    return () => window.clearTimeout(handle)
  }, [playback])

  useEffect(() => {
    setNotice({ kind: 'playback', state: playback })
  }, [playback])

  async function addCurrentPose() {
    if (
      editingDisabled
      || !snapshot
      || presentation.kind !== 'ready'
      || presentation.steps.length >= MAX_INSTRUCTION_STEPS
      || !captureDraft
    ) return
    cancelPlayback('revision_changed')
    const previousIds = new Set(presentation.steps.map(({ id }) => id))
    let addedStepId: string | null = null
    const title = formatLocalizedText(locale, TEXT.defaultStepTitle, {
      step: presentation.steps.length + 1,
    })
    const succeeded = await runNativeEdit(async (projectId, revision, projectInstanceId) => {
      const response = await addInstructionStep(
        projectId,
        revision,
        projectInstanceId,
        title,
        '',
        '',
        DEFAULT_INSTRUCTION_DURATION_MS,
        captureDraft.fixedFace,
        captureDraft.hingeAngles,
      )
      const nextPresentation = createInstructionTimelinePresentation(
        response.instruction_timeline,
        response.fold_model_fingerprint,
      )
      if (nextPresentation.kind === 'ready') {
        const added = nextPresentation.steps.filter(({ id }) => !previousIds.has(id))
        if (added.length === 1) addedStepId = added[0]?.id ?? null
      }
      return response
    })
    if (!succeeded) {
      setNotice({ kind: 'add_failed' })
      return
    }
    if (addedStepId) setSelectedStepId(addedStepId)
    setNotice({ kind: 'added', title })
  }

  async function saveMetadata(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (
      editingDisabled
      || !editor
      || !selectedStep
      || editor.stepId !== selectedStep.id
    ) return
    const metadata = validateInstructionMetadata({
      title: editor.title,
      description: editor.description,
      caution: editor.caution,
      durationMs: Number(editor.durationMs),
    })
    if (!metadata) {
      setEditorError('invalid_metadata')
      return
    }
    let visual: InstructionVisual | null
    try {
      visual = parseInstructionVisual(JSON.parse(editor.visualJson))
    } catch {
      setEditorError('invalid_metadata')
      return
    }
    if (!visual) {
      setEditorError('invalid_metadata')
      return
    }
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      updateInstructionStepMetadata(
        projectId,
        revision,
        projectInstanceId,
        selectedStep.id,
        metadata.title,
        metadata.description,
        metadata.caution,
        metadata.durationMs,
        visual,
      ))
    setEditorError(succeeded ? null : 'update_failed')
    setNotice(succeeded
      ? { kind: 'updated', title: metadata.title }
      : { kind: 'update_failed' })
  }

  function captureCurrentCamera() {
    if (editingDisabled || !editor || !currentCamera) return
    try {
      const visual = parseInstructionVisual(JSON.parse(editor.visualJson))
      if (!visual) throw new Error('invalid visual')
      setEditor({
        ...editor,
        visualJson: JSON.stringify({ ...visual, camera: currentCamera }, null, 2),
      })
      setEditorError(null)
    } catch {
      setEditorError('invalid_metadata')
    }
  }

  async function replaceSelectedPose() {
    if (
      editingDisabled
      || !selectedStep
      || selectedStep.declarativeOnly
      || !captureDraft
    ) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      replaceInstructionStepPose(
        projectId,
        revision,
        projectInstanceId,
        selectedStep.id,
        captureDraft.fixedFace,
        captureDraft.hingeAngles,
      ))
    setNotice(succeeded
      ? { kind: 'pose_updated', title: selectedStep.title }
      : { kind: 'pose_update_failed' })
  }

  async function deleteSelectedStep() {
    if (editingDisabled || !selectedStep) return
    if (!window.confirm(formatLocalizedText(locale, TEXT.deleteConfirmation, {
      title: selectedStep.title,
    }))) return
    cancelPlayback('revision_changed')
    const deletedId = selectedStep.id
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      removeInstructionStep(projectId, revision, projectInstanceId, deletedId))
    if (!succeeded) {
      setNotice({ kind: 'delete_failed' })
      return
    }
    setSelectedStepId(null)
    setNotice({ kind: 'deleted', title: selectedStep.title })
  }

  async function moveSelectedStep(targetIndex: number) {
    if (
      editingDisabled
      || !selectedStep
      || targetIndex < 0
      || targetIndex >= steps.length
      || targetIndex === selectedStep.index
    ) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      moveInstructionStep(
        projectId,
        revision,
        projectInstanceId,
        selectedStep.id,
        targetIndex,
      ))
    setNotice(succeeded ? { kind: 'moved' } : { kind: 'move_failed' })
  }

  async function splitSelectedStep() {
    if (editingDisabled || !selectedStep) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      splitInstructionStep(projectId, revision, projectInstanceId, selectedStep.id))
    setNotice(succeeded ? { kind: 'split' } : { kind: 'move_failed' })
  }

  async function mergeSelectedWithNext() {
    const next = selectedStep ? steps[selectedStep.index + 1] : undefined
    if (editingDisabled || !selectedStep || !next) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      mergeAdjacentInstructionSteps(projectId, revision, projectInstanceId, selectedStep.id, next.id))
    setNotice(succeeded ? { kind: 'merged' } : { kind: 'move_failed' })
  }

  function showStepPose(step: InstructionStepPresentation) {
    cancelPlayback('manual_pose')
    if (step.declarativeOnly) {
      setNotice({ kind: 'declarative_playback_unsupported' })
      return
    }
    if (step.stale) {
      setNotice({ kind: 'stale_pose' })
      return
    }
    let applied = false
    try {
      applied = applyStepPose(step)
    } catch {
      applied = false
    }
    if (!applied) {
      setNotice({ kind: 'pose_apply_failed' })
      return
    }
    setSelectedStepId(step.id)
    setNotice({ kind: 'pose_applying', title: step.title })
  }

  function startOrStopPlayback() {
    if (playbackActive) {
      cancelPlayback('canceled')
      return
    }
    if (
      steps.length > 0
      && steps.every((step) => step.declarativeOnly)
    ) {
      setNotice({ kind: 'declarative_playback_unsupported' })
      return
    }
    if (!snapshot || !poseModelKey) {
      setNotice({ kind: 'model_required' })
      return
    }
    const plan = createInstructionPlaybackPlan(
      snapshot.project_id,
      snapshot.revision,
      presentation,
    )
    if (!plan) {
      setNotice({ kind: 'no_steps' })
      return
    }
    const selectedIndex = selectedStep
      && !selectedStep.stale
      && !selectedStep.declarativeOnly
        ? Math.max(
            0,
            plan.steps.findIndex((step) => step.id === selectedStep.id),
          )
        : 0
    playbackModelKeyRef.current = poseModelKey
    applyAttemptRef.current = null
    setPlayback((current) => reduceInstructionPlayback(current, {
      kind: 'start',
      plan,
      startIndex: selectedIndex,
    }))
  }

  const captureStatus = instructionCaptureStatusText(
    describeCaptureStatus(snapshot, appliedPose, captureDraft !== null),
    locale,
  )

  return (
    <section
      id="instruction-timeline-panel"
      className="timeline panel"
      inert={inert}
    >
      <div className="timeline-controls">
        <button
          type="button"
          aria-label={selectLocalizedText(
            locale,
            steps[0]?.declarativeOnly
              ? TEXT.showFirstPhysicalStep
              : TEXT.showFirstStep,
          )}
          disabled={
            coreBusy
            || !firstPhysicalStep
            || firstPhysicalStep.stale
          }
          onClick={() => {
            if (firstPhysicalStep) showStepPose(firstPhysicalStep)
          }}
        >
          |◀
        </button>
        <button
          type="button"
          aria-label={selectLocalizedText(
            locale,
            playbackActive ? TEXT.stopPlayback : TEXT.playFromSelection,
          )}
          aria-pressed={playbackActive}
          disabled={coreBusy || benchmarkActive || steps.length === 0}
          onClick={startOrStopPlayback}
        >
          {playbackActive ? '■' : '▶'}
        </button>
        <strong>{selectLocalizedText(locale, TEXT.heading)}</strong>
        <span>
          {formatInstructionStepCount(steps.length, locale)}
          {presentation.kind === 'ready'
            ? formatLocalizedText(locale, TEXT.totalDuration, {
                duration: formatInstructionDuration(
                  presentation.totalDurationMs,
                  locale,
                ),
              })
            : ''}
        </span>
        <small>
          {selectLocalizedText(locale, TEXT.endpointSafety)}
        </small>
        <button
          ref={exportButtonRef}
          type="button"
          className="instruction-export-button"
          disabled={
            coreBusy
            || benchmarkActive
            || fileOperationActive
            || !exportAvailable
            || steps.length === 0
            || steps.some((step) => step.stale)
            || certificateExportBlocked
          }
          title={
            steps.some((step) => step.stale)
              ? selectLocalizedText(locale, TEXT.exportStaleTitle)
              : certificateExportBlocked
                ? (locale === 'ja'
                    ? '構造化経路証明の再検証が完了していないため書き出せません。'
                    : 'Export is unavailable until structured path certificates are revalidated.')
                : selectLocalizedText(locale, TEXT.exportTitle)
          }
          onClick={onExport}
        >
          {selectLocalizedText(locale, TEXT.exportAction)}
        </button>
        <button
          ref={animationExportButtonRef}
          type="button"
          className="instruction-export-button"
          disabled={
            coreBusy
            || benchmarkActive
            || fileOperationActive
            || !exportAvailable
            || steps.length === 0
            || steps.some((step) => step.stale)
            || certificateExportBlocked
          }
          onClick={onAnimationExport}
        >
          {locale === 'ja' ? 'GLBアニメーション' : 'GLB animation'}
        </button>
      </div>
      <div className="instruction-timeline-body">
        {presentation.kind === 'invalid' ? (
          <p className="instruction-timeline-error" role="alert">
            {selectLocalizedText(locale, TEXT.invalidTimeline)}
          </p>
        ) : (
          <>
            <div
              className="timeline-track"
              aria-label={selectLocalizedText(locale, TEXT.timelineList)}
            >
              {steps.map((step) => {
                const selected = step.id === selectedStepId
                const displayed = !step.stale
                  && instructionPoseMatchesApplied(step.pose, currentAppliedPose)
                return (
                  <button
                    type="button"
                    key={step.id}
                    className={[
                      'step',
                      selected ? 'selected' : '',
                      displayed ? 'is-displayed' : '',
                      step.stale ? 'is-stale' : '',
                    ].filter(Boolean).join(' ')}
                    aria-pressed={selected}
                    aria-current={displayed ? 'step' : undefined}
                    onClick={() => setSelectedStepId(step.id)}
                  >
                    <span>
                      {step.index + 1}. {step.title}
                      {step.id === finalPhysicalStepId
                        ? ` · ${locale === 'ja' ? '完成形サムネイル' : 'Completed-form thumbnail'}`
                        : ''}
                    </span>
                    <small>
                      {step.stale
                        ? selectLocalizedText(locale, TEXT.needsUpdate)
                        : step.declarativeOnly
                          ? selectLocalizedText(locale, TEXT.descriptionOnly)
                        : displayed
                          ? selectLocalizedText(locale, TEXT.shownIn3d)
                          : formatInstructionDuration(step.durationMs, locale)}
                    </small>
                  </button>
                )
              })}
              <button
                type="button"
                className="step add"
                disabled={
                  editingDisabled
                  || !captureDraft
                  || steps.length >= MAX_INSTRUCTION_STEPS
                }
                title={captureStatus}
                onClick={() => void addCurrentPose()}
              >
                {selectLocalizedText(locale, TEXT.addCurrentPose)}
              </button>
            </div>
            <div className="instruction-editor-region">
              {noticeText && (
                <p className="instruction-notice" aria-hidden="true">
                  {noticeText}
                </p>
              )}
              {selectedStep && editor ? (
                <form className="instruction-editor" onSubmit={(event) => void saveMetadata(event)}>
                  <label>
                    <span>{selectLocalizedText(locale, TEXT.titleLabel)}</span>
                    <input
                      value={editor.title}
                      maxLength={MAX_INSTRUCTION_TITLE_CHARACTERS}
                      disabled={editingDisabled}
                      onChange={(event) => setEditor({
                        ...editor,
                        title: event.currentTarget.value,
                      })}
                    />
                  </label>
                  <label>
                    <span>{selectLocalizedText(locale, TEXT.descriptionLabel)}</span>
                    <textarea
                      value={editor.description}
                      maxLength={MAX_INSTRUCTION_DESCRIPTION_CHARACTERS}
                      rows={2}
                      disabled={editingDisabled}
                      onChange={(event) => setEditor({
                        ...editor,
                        description: event.currentTarget.value,
                      })}
                    />
                  </label>
                  {selectedProofDisplay?.kind === 'verified'
                    && proofEndpointValidation?.step === selectedStep
                    && proofEndpointValidation.status === 'valid' && (
                    <aside className="instruction-notice" aria-label={locale === 'ja'
                      ? '構造化経路証明'
                      : 'Structured path certificate'}>
                      <strong>{locale === 'ja' ? '構造化経路証明' : 'Structured path certificate'}</strong>
                      <div>{locale === 'ja' ? '出力前確認（読み取り専用）' : 'Pre-export review (read-only)'}</div>
                      <div>{locale === 'ja' ? '証明指紋' : 'Certificate fingerprint'}: {selectedProofDisplay.shortBinding}</div>
                      <div>{locale === 'ja' ? '検証区間' : 'Verified transitions'}: {selectedProofDisplay.transitionCount}</div>
                      <div>{locale === 'ja' ? '始点姿勢' : 'Source pose'}: {selectedProofDisplay.shortSource}</div>
                      <div>{locale === 'ja' ? '終点姿勢' : 'Target pose'}: {selectedProofDisplay.shortTarget}</div>
                      <div>{locale === 'ja' ? '元モデル束縛' : 'Source-model binding'}: {selectedProofDisplay.shortModelBinding}</div>
                      <small>{locale === 'ja'
                        ? '保存済みDTOの識別情報です。折り図出力時に直前姿勢・現在姿勢・元モデルへ再照合します。'
                        : 'Saved DTO identity; diagram export rechecks the previous pose, current pose, and source model.'}</small>
                    </aside>
                  )}
                  {selectedProofDisplay?.kind === 'verified'
                    && proofEndpointValidation?.step === selectedStep
                    && proofEndpointValidation.status === 'invalid' && (
                    <p className="instruction-timeline-error" role="alert">
                      {locale === 'ja'
                        ? '証明の元モデルまたは姿勢端点が構造化データと一致しません。書き出しは拒否されます。'
                        : 'The source model or pose endpoints do not match the structured certificate. Export will be rejected.'}
                    </p>
                  )}
                  {selectedProofDisplay?.kind === 'mismatch' && (
                    <p className="instruction-timeline-error" role="alert">
                      {locale === 'ja'
                        ? '証明説明が構造化データと一致しません。書き出しは拒否されます。'
                        : 'The certificate description does not match the structured data. Export will be rejected.'}
                    </p>
                  )}
                  {selectedProofDisplay?.kind === 'text-only' && (
                    <p className="instruction-timeline-error" role="alert">
                      {locale === 'ja'
                        ? '構造化証明データがないため、この説明文は証明として扱いません。'
                        : 'This description is not treated as proof because structured certificate data is absent.'}
                    </p>
                  )}
                  <label>
                    <span>{selectLocalizedText(locale, TEXT.cautionLabel)}</span>
                    <textarea
                      value={editor.caution}
                      maxLength={MAX_INSTRUCTION_CAUTION_CHARACTERS}
                      rows={2}
                      disabled={editingDisabled}
                      onChange={(event) => setEditor({
                        ...editor,
                        caution: event.currentTarget.value,
                      })}
                    />
                  </label>
                  <label className="instruction-duration-field">
                    <span>{selectLocalizedText(locale, TEXT.durationLabel)}</span>
                    <span>
                      <input
                        type="number"
                        min={MIN_INSTRUCTION_DURATION_MS}
                        max={MAX_INSTRUCTION_DURATION_MS}
                        step="100"
                        value={editor.durationMs}
                        disabled={editingDisabled}
                        onChange={(event) => setEditor({
                          ...editor,
                          durationMs: event.currentTarget.value,
                        })}
                      />
                      ms
                    </span>
                  </label>
                  <label>
                    <span>{selectLocalizedText(locale, TEXT.visualLabel)}</span>
                    <textarea
                      value={editor.visualJson}
                      rows={10}
                      spellCheck={false}
                      disabled={editingDisabled}
                      onChange={(event) => setEditor({
                        ...editor,
                        visualJson: event.currentTarget.value,
                      })}
                    />
                    <small>{selectLocalizedText(locale, TEXT.visualHelp)}</small>
                  </label>
                  <button
                    type="button"
                    disabled={editingDisabled || !currentCamera}
                    aria-label={selectLocalizedText(locale, TEXT.captureCamera)}
                    onClick={captureCurrentCamera}
                  >
                    {selectLocalizedText(locale, TEXT.captureCamera)}
                  </button>
                  <div className="instruction-editor-actions">
                    <button type="submit" disabled={editingDisabled}>
                      {selectLocalizedText(locale, TEXT.saveMetadata)}
                    </button>
                    <button
                      type="button"
                      disabled={
                        editingDisabled
                        || selectedStep.stale
                        || selectedStep.declarativeOnly
                        || !poseModelKey
                      }
                      onClick={() => showStepPose(selectedStep)}
                    >
                      {selectLocalizedText(locale, TEXT.showIn3d)}
                    </button>
                    <button
                      type="button"
                      disabled={
                        editingDisabled
                        || selectedStep.declarativeOnly
                        || !captureDraft
                      }
                      title={captureStatus}
                      onClick={() => void replaceSelectedPose()}
                    >
                      {selectLocalizedText(locale, TEXT.updateCurrentPose)}
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === 0}
                      onClick={() => void moveSelectedStep(0)}
                    >
                      {selectLocalizedText(locale, TEXT.moveFirst)}
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === 0}
                      onClick={() => void moveSelectedStep(selectedStep.index - 1)}
                    >
                      {selectLocalizedText(locale, TEXT.moveEarlier)}
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === steps.length - 1}
                      onClick={() => void moveSelectedStep(selectedStep.index + 1)}
                    >
                      {selectLocalizedText(locale, TEXT.moveLater)}
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === steps.length - 1}
                      onClick={() => void moveSelectedStep(steps.length - 1)}
                    >
                      {selectLocalizedText(locale, TEXT.moveLast)}
                    </button>
                    <button type="button" disabled={editingDisabled || selectedStep.declarativeOnly || selectedStep.durationMs < 200}
                      onClick={() => void splitSelectedStep()}>
                      {locale === 'ja' ? '手順を分割' : 'Split step'}
                    </button>
                    <button type="button" disabled={editingDisabled || selectedStep.declarativeOnly || selectedStep.index === steps.length - 1}
                      onClick={() => void mergeSelectedWithNext()}>
                      {locale === 'ja' ? '次の手順と結合' : 'Merge with next'}
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={editingDisabled}
                      onClick={() => void deleteSelectedStep()}
                    >
                      {selectLocalizedText(locale, TEXT.deleteAction)}
                    </button>
                  </div>
                  {selectedStep.stale && (
                    <p className="instruction-stale-guidance">
                      {selectLocalizedText(locale, TEXT.staleGuidance)}
                    </p>
                  )}
                  {selectedStep.declarativeOnly && (
                    <p className="instruction-stale-guidance">
                      {selectLocalizedText(locale, TEXT.declarativeGuidance)}
                    </p>
                  )}
                  {selectedPoseIsDisplayed && (
                    <p className="instruction-current-pose">
                      {selectLocalizedText(locale, TEXT.currentPose)}
                    </p>
                  )}
                  {editorError && (
                    <p className="instruction-editor-error" role="alert">
                      {instructionEditorErrorText(editorError, locale)}
                    </p>
                  )}
                </form>
              ) : (
                <p className="instruction-empty-editor">
                  {steps.length === 0
                    ? formatLocalizedText(locale, TEXT.emptyTimeline, {
                        captureStatus,
                      })
                    : selectLocalizedText(locale, TEXT.selectStep)}
                </p>
              )}
            </div>
          </>
        )}
        <p className="visually-hidden" aria-live="polite" aria-atomic="true">
          {noticeText}
        </p>
      </div>
    </section>
  )
}

function describeCaptureStatus(
  snapshot: ProjectSnapshot | null,
  appliedPose: FoldPreviewAppliedPoseSnapshot | null,
  canCapture: boolean,
): InstructionCaptureStatus {
  if (!snapshot) return 'project_required'
  if (
    !appliedPose
    || appliedPose.projectId !== snapshot.project_id
    || appliedPose.revision !== snapshot.revision
  ) return 'pose_required'
  if (appliedPose.state === 'running') return 'pose_running'
  if (!canCapture) return 'pose_invalid'
  if (appliedPose.state === 'blocked') return 'pose_blocked'
  if (appliedPose.state === 'indeterminate') return 'pose_indeterminate'
  return 'pose_ready'
}

function formatInstructionStepCount(count: number, locale: Locale) {
  const formatted = count.toLocaleString(locale === 'en' ? 'en-US' : 'ja-JP')
  return formatLocalizedText(
    locale,
    locale === 'en' && count === 1 ? TEXT.stepCountOne : TEXT.stepCount,
    { count: formatted },
  )
}

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const TEXT = Object.freeze({
  defaultStepTitle: localized('手順 {step}', 'Step {step}'),
  deleteConfirmation: localized(
    '「{title}」を削除しますか？',
    'Delete “{title}”?',
  ),
  showFirstStep: localized(
    '先頭の手順を3Dに表示',
    'Show the first step in 3D',
  ),
  showFirstPhysicalStep: localized(
    '最初の実姿勢手順を3Dに表示',
    'Show the first physical-pose step in 3D',
  ),
  stopPlayback: localized('再生を停止', 'Stop playback'),
  playFromSelection: localized(
    '選択手順から再生',
    'Play from the selected step',
  ),
  heading: localized('折り手順', 'Folding instructions'),
  stepCount: localized('{count}手順', '{count} steps'),
  stepCountOne: localized('{count}手順', '{count} step'),
  totalDuration: localized('・合計 {duration}', ' · Total {duration}'),
  endpointSafety: localized(
    '保存した姿勢を段階表示します。姿勢間の連続した折り経路の安全性は保証しません。',
    'Shows saved poses step by step. It does not guarantee a safe continuous folding path between poses.',
  ),
  exportStaleTitle: localized(
    '展開図が変わったため、要更新の手順を作り直してください。',
    'The crease pattern changed. Recreate every step that needs updating.',
  ),
  exportTitle: localized(
    '現在の折り手順をPDFまたはSVG画像一式へ書き出します。',
    'Exports the current folding instructions as a PDF or a set of SVG images.',
  ),
  exportAction: localized('折り図を書き出す', 'Export diagrams'),
  invalidTimeline: localized(
    '折り手順データを安全に読み取れないため、編集と再生を停止しました。',
    'Editing and playback were stopped because the folding-step data could not be read safely.',
  ),
  timelineList: localized('折り手順一覧', 'Folding-step list'),
  needsUpdate: localized('要更新', 'Needs update'),
  descriptionOnly: localized('説明専用', 'Description only'),
  shownIn3d: localized('3D表示中', 'Shown in 3D'),
  addCurrentPose: localized(
    '＋ 現在の3D姿勢を追加',
    '＋ Add current 3D pose',
  ),
  titleLabel: localized('タイトル', 'Title'),
  descriptionLabel: localized('説明', 'Description'),
  cautionLabel: localized('注意', 'Caution'),
  durationLabel: localized('表示時間', 'Display time'),
  saveMetadata: localized('説明を保存', 'Save details'),
  captureCamera: localized('現在のカメラを取得', 'Capture current camera'),
  visualLabel: localized('カメラ・矢印・注目箇所・手指ガイド（JSON）', 'Camera, arrows, focus points, and hand guides (JSON)'),
  visualHelp: localized(
    'camera、arrows、focus_pointsに加え、hand_guidesへpinch/hold/push/regripとposition/direction/labelを指定します。',
    'Set camera, arrows, focus_points, and hand_guides with pinch/hold/push/regrip plus position/direction/label.',
  ),
  showIn3d: localized('3Dに表示', 'Show in 3D'),
  updateCurrentPose: localized(
    '現在の3D姿勢で更新',
    'Update with current 3D pose',
  ),
  moveEarlier: localized('← 前へ', '← Earlier'),
  moveLater: localized('次へ →', 'Later →'),
  moveFirst: localized('先頭へ', 'Move to first'),
  moveLast: localized('末尾へ', 'Move to last'),
  deleteAction: localized('削除', 'Delete'),
  staleGuidance: localized(
    '展開図が記録時から変わりました。内容を確認し、現在の3D姿勢で更新すると再生できます。',
    'The crease pattern changed after this step was recorded. Review it and update it with the current 3D pose before playback.',
  ),
  declarativeGuidance: localized(
    '名前付き技法から追加された説明専用ステップです。3D表示・姿勢更新・自動再生・物理的な折り操作は行いません。',
    'This description-only step came from a named technique. It cannot show or update a 3D pose, play automatically, or execute a physical fold.',
  ),
  currentPose: localized(
    'この保存姿勢を3Dに表示中です。',
    'This saved pose is currently shown in 3D.',
  ),
  emptyTimeline: localized(
    '現在の3D姿勢を最初の手順として追加できます。{captureStatus}',
    'Add the current 3D pose as the first step. {captureStatus}',
  ),
  selectStep: localized(
    '手順を選択すると説明・姿勢・順番を編集できます。',
    'Select a step to edit its details, pose, and order.',
  ),
})
