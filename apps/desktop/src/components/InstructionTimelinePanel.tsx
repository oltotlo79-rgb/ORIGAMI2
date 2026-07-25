import {
  type DragEvent,
  type KeyboardEvent,
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import {
  addInstructionStep,
  duplicateInstructionStep,
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
  createInstructionOnionSkinRequest,
  type InstructionOnionSkinRequest,
  type OnionSkinDirection,
} from '../lib/instructionOnionSkin'
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
} from '../lib/i18n'
import { INSTRUCTION_TIMELINE_PANEL_TEXT as TEXT } from '../lib/instructionTimelinePanelText.ts'

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
  onOnionSkinChange?(request: InstructionOnionSkinRequest | null): void
  onionSkinStatus?: Readonly<{
    request: InstructionOnionSkinRequest
    state: 'available' | 'unavailable'
  }> | null
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
  onOnionSkinChange,
  onionSkinStatus = null,
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
  const pendingSelectedStepRef = useRef<Readonly<{ id: string; revision: number }> | null>(null)
  const [editor, setEditor] = useState<InstructionEditorState | null>(null)
  const [editorError, setEditorError] = useState<InstructionEditorError | null>(null)
  const [notice, setNotice] = useState<InstructionTimelineNotice | null>(null)
  const [onionSkinDirection, setOnionSkinDirection] =
    useState<'off' | OnionSkinDirection>('off')
  const draggedStepIdRef = useRef<string | null>(null)
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
  const editingDisabled = coreBusy || benchmarkActive || fileOperationActive
    || !snapshot || presentation.kind !== 'ready'
  const onionSkinRequest = useMemo(() => (
    !editingDisabled && snapshot && selectedStep && onionSkinDirection !== 'off'
      && typeof snapshot.fold_model_fingerprint === 'string'
      ? createInstructionOnionSkinRequest({
          projectInstanceId: snapshot.project_instance_id,
          projectId: snapshot.project_id,
          revision: snapshot.revision,
          foldModelFingerprint: snapshot.fold_model_fingerprint,
          steps,
          selectedStepId: selectedStep.id,
          direction: onionSkinDirection,
        })
      : null
  ), [editingDisabled, onionSkinDirection, selectedStep, snapshot, steps])
  useEffect(() => {
    if (!onOnionSkinChange) return
    onOnionSkinChange(onionSkinRequest)
  }, [onOnionSkinChange, onionSkinRequest])
  useEffect(() => () => onOnionSkinChange?.(null), [onOnionSkinChange])
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
  useEffect(() => {
    draggedStepIdRef.current = null
  }, [
    editingDisabled,
    snapshot?.project_instance_id,
    snapshot?.project_id,
    snapshot?.revision,
  ])
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
      if (pendingSelectedStepRef.current?.id === selectedStepId) {
        pendingSelectedStepRef.current = null
      }
      return
    }
    const pending = pendingSelectedStepRef.current
    if (pending?.id === selectedStepId && (snapshot?.revision ?? -1) < pending.revision) return
    pendingSelectedStepRef.current = null
    setSelectedStepId(null)
    setEditor(null)
    setEditorError(null)
  }, [presentation, selectedStepId, snapshot?.revision])

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

  async function duplicateSelectedStep() {
    if (
      editingDisabled
      || !snapshot
      || !selectedStep
      || steps.length >= MAX_INSTRUCTION_STEPS
    ) return
    cancelPlayback('revision_changed')
    const previousIds = new Set(steps.map(({ id }) => id))
    let duplicatedStepId: string | null = null
    let duplicatedRevision: number | null = null
    const succeeded = await runNativeEdit(async (projectId, revision, projectInstanceId) => {
      const response = await duplicateInstructionStep(
        projectId,
        revision,
        projectInstanceId,
        selectedStep.id,
      )
      const next = createInstructionTimelinePresentation(
        response.instruction_timeline,
        response.fold_model_fingerprint,
      )
      if (next.kind === 'ready') {
        const added = next.steps.filter(({ id }) => !previousIds.has(id))
        if (added.length === 1) duplicatedStepId = added[0]?.id ?? null
      }
      duplicatedRevision = response.revision
      return response
    })
    if (!succeeded) {
      setNotice({ kind: 'add_failed' })
      return
    }
    if (duplicatedStepId && duplicatedRevision !== null) {
      pendingSelectedStepRef.current = {
        id: duplicatedStepId,
        revision: duplicatedRevision,
      }
      setSelectedStepId(duplicatedStepId)
    }
    setNotice({ kind: 'added', title: selectedStep.title })
  }

  async function moveStep(stepId: string, targetIndex: number) {
    const source = presentation.kind === 'ready'
      ? presentation.stepsById.get(stepId)
      : undefined
    if (
      editingDisabled
      || !source
      || targetIndex < 0
      || targetIndex >= steps.length
      || targetIndex === source.index
    ) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
      moveInstructionStep(
        projectId,
        revision,
        projectInstanceId,
        source.id,
        targetIndex,
      ))
    setNotice(succeeded ? { kind: 'moved' } : { kind: 'move_failed' })
  }

  async function moveSelectedStep(targetIndex: number) {
    if (!selectedStep) return
    await moveStep(selectedStep.id, targetIndex)
  }

  function handleStepKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    step: InstructionStepPresentation,
  ) {
    if (editingDisabled || !event.altKey) return
    const targetIndex = event.key === 'ArrowLeft' || event.key === 'ArrowUp'
      ? step.index - 1
      : event.key === 'ArrowRight' || event.key === 'ArrowDown'
        ? step.index + 1
        : event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? steps.length - 1
            : null
    if (targetIndex === null || targetIndex < 0 || targetIndex >= steps.length) return
    event.preventDefault()
    setSelectedStepId(step.id)
    void moveStep(step.id, targetIndex)
  }

  function handleStepDrop(event: DragEvent<HTMLButtonElement>, targetIndex: number) {
    event.preventDefault()
    const stepId = draggedStepIdRef.current || event.dataTransfer.getData('text/plain')
    draggedStepIdRef.current = null
    if (
      !stepId
      || editingDisabled
      || presentation.kind !== 'ready'
      || !presentation.stepsById.has(stepId)
    ) return
    setSelectedStepId(stepId)
    void moveStep(stepId, targetIndex)
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
                ? selectLocalizedText(locale, TEXT.certificateExportBlockedTitle)
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
          {selectLocalizedText(locale, TEXT.animationExportAction)}
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
              aria-describedby="instruction-reorder-help"
            >
              <span id="instruction-reorder-help" className="visually-hidden">
                {selectLocalizedText(locale, TEXT.reorderHelp)}
              </span>
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
                    draggable={!editingDisabled && steps.length > 1}
                    data-drop-target-index={step.index}
                    onDragStart={(event) => {
                      if (editingDisabled) {
                        event.preventDefault()
                        return
                      }
                      draggedStepIdRef.current = step.id
                      setSelectedStepId(step.id)
                      event.dataTransfer.effectAllowed = 'move'
                      event.dataTransfer.setData('text/plain', step.id)
                    }}
                    onDragEnd={() => {
                      draggedStepIdRef.current = null
                    }}
                    onDragOver={(event) => {
                      if (!editingDisabled && draggedStepIdRef.current) {
                        event.preventDefault()
                        event.dataTransfer.dropEffect = 'move'
                      }
                    }}
                    onDrop={(event) => handleStepDrop(event, step.index)}
                    onKeyDown={(event) => handleStepKeyDown(event, step)}
                    onClick={() => setSelectedStepId(step.id)}
                  >
                    <span>
                      {step.index + 1}. {step.title}
                      {step.id === finalPhysicalStepId
                        ? selectLocalizedText(locale, TEXT.completedFormThumbnailSuffix)
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
                <>
                <fieldset aria-label={selectLocalizedText(locale, TEXT.onionLegend)}>
                  {(['off', 'previous', 'next'] as const).map((direction) => (
                    <label key={direction}>
                      <input
                        type="radio"
                        name="onion_skin_direction"
                        value={direction}
                        checked={onionSkinDirection === direction}
                        disabled={editingDisabled}
                        onChange={() => setOnionSkinDirection(direction)}
                      />
                      {direction === 'off'
                        ? selectLocalizedText(locale, TEXT.onionOff)
                        : direction === 'previous'
                          ? selectLocalizedText(locale, TEXT.onionPrevious)
                          : selectLocalizedText(locale, TEXT.onionNext)}
                    </label>
                  ))}
                  <p role="status" aria-live="polite">
                    {onionSkinDirection === 'off'
                      ? selectLocalizedText(locale, TEXT.onionHidden)
                      : !onionSkinRequest || (
                        onionSkinStatus?.request === onionSkinRequest
                        && onionSkinStatus.state === 'unavailable'
                      )
                        ? selectLocalizedText(locale, TEXT.onionUnavailable)
                      : onionSkinStatus?.request === onionSkinRequest
                        && onionSkinStatus.state === 'available'
                      ? selectLocalizedText(locale, TEXT.onionAvailable)
                      : selectLocalizedText(locale, TEXT.onionPreparing)}
                  </p>
                </fieldset>
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
                    <aside
                      className="instruction-notice"
                      aria-label={selectLocalizedText(locale, TEXT.pathCertificateHeading)}
                    >
                      <strong>{selectLocalizedText(locale, TEXT.pathCertificateHeading)}</strong>
                      <div>{selectLocalizedText(locale, TEXT.pathCertificateReview)}</div>
                      <div>{selectLocalizedText(locale, TEXT.certificateFingerprintLabel)}: {selectedProofDisplay.shortBinding}</div>
                      <div>{selectLocalizedText(locale, TEXT.verifiedTransitionsLabel)}: {selectedProofDisplay.transitionCount}</div>
                      <div>{selectLocalizedText(locale, TEXT.sourcePoseLabel)}: {selectedProofDisplay.shortSource}</div>
                      <div>{selectLocalizedText(locale, TEXT.targetPoseLabel)}: {selectedProofDisplay.shortTarget}</div>
                      <div>{selectLocalizedText(locale, TEXT.sourceModelBindingLabel)}: {selectedProofDisplay.shortModelBinding}</div>
                      <small>{selectLocalizedText(locale, TEXT.pathCertificateIdentityHelp)}</small>
                    </aside>
                  )}
                  {selectedProofDisplay?.kind === 'verified'
                    && proofEndpointValidation?.step === selectedStep
                    && proofEndpointValidation.status === 'invalid' && (
                    <p className="instruction-timeline-error" role="alert">
                      {selectLocalizedText(locale, TEXT.pathCertificateEndpointMismatch)}
                    </p>
                  )}
                  {selectedProofDisplay?.kind === 'mismatch' && (
                    <p className="instruction-timeline-error" role="alert">
                      {selectLocalizedText(locale, TEXT.pathCertificateDescriptionMismatch)}
                    </p>
                  )}
                  {selectedProofDisplay?.kind === 'text-only' && (
                    <p className="instruction-timeline-error" role="alert">
                      {selectLocalizedText(locale, TEXT.pathCertificateTextOnly)}
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
                      {selectLocalizedText(locale, TEXT.splitAction)}
                    </button>
                    <button type="button" disabled={editingDisabled || selectedStep.declarativeOnly || selectedStep.index === steps.length - 1}
                      onClick={() => void mergeSelectedWithNext()}>
                      {selectLocalizedText(locale, TEXT.mergeWithNextAction)}
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || steps.length >= MAX_INSTRUCTION_STEPS}
                      onClick={() => void duplicateSelectedStep()}
                    >
                      {selectLocalizedText(locale, TEXT.duplicateAction)}
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
                </>
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
  const formatted = count.toLocaleString(locale)
  return formatLocalizedText(
    locale,
    count === 1 ? TEXT.stepCountOne : TEXT.stepCount,
    { count: formatted },
  )
}
