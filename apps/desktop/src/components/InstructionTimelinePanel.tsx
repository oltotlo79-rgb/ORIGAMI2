import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  addInstructionStep,
  moveInstructionStep,
  removeInstructionStep,
  replaceInstructionStepPose,
  updateInstructionStepMetadata,
  type ProjectSnapshot,
} from '../lib/coreClient'
import type { FoldPreviewAppliedPoseSnapshot } from '../lib/foldPreviewAppliedPose'
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
  createInstructionPlaybackState,
  createInstructionPoseDraft,
  createInstructionTimelinePresentation,
  instructionPlaybackStatusText,
  instructionPoseMatchesApplied,
  reduceInstructionPlayback,
  resolveInstructionPoseApplicationObservation,
  validateInstructionMetadata,
  type InstructionPlaybackStopReason,
  type InstructionStepPresentation,
} from '../lib/instructionTimeline'

type InstructionEditorState = {
  stepId: string
  title: string
  description: string
  caution: string
  durationMs: string
}

type InstructionTimelinePanelProps = {
  snapshot: ProjectSnapshot | null
  appliedPose: FoldPreviewAppliedPoseSnapshot | null
  poseModelKey: string | null
  manualPoseChangeSequence: number
  coreBusy: boolean
  benchmarkActive: boolean
  fileOperationActive: boolean
  inert?: boolean
  runNativeEdit(
    action: (projectId: string, revision: number) => Promise<ProjectSnapshot>,
  ): Promise<boolean>
  applyStepPose(step: InstructionStepPresentation): boolean
}

export function InstructionTimelinePanel({
  snapshot,
  appliedPose,
  poseModelKey,
  manualPoseChangeSequence,
  coreBusy,
  benchmarkActive,
  fileOperationActive,
  inert,
  runNativeEdit,
  applyStepPose,
}: InstructionTimelinePanelProps) {
  const presentation = useMemo(
    () => createInstructionTimelinePresentation(
      snapshot?.instruction_timeline,
      snapshot?.fold_model_fingerprint,
    ),
    [snapshot?.fold_model_fingerprint, snapshot?.instruction_timeline],
  )
  const [selectedStepId, setSelectedStepId] = useState<string | null>(null)
  const [editor, setEditor] = useState<InstructionEditorState | null>(null)
  const [editorError, setEditorError] = useState<string | null>(null)
  const [notice, setNotice] = useState('')
  const [playback, setPlayback] = useState(createInstructionPlaybackState)
  const playbackRef = useRef(playback)
  playbackRef.current = playback
  const applyAttemptRef = useRef<string | null>(null)
  const applyObservationRef = useRef<FoldPreviewAppliedPoseSnapshot | null>(null)
  const playbackModelKeyRef = useRef<string | null>(null)
  const previousManualPoseChangeRef = useRef(manualPoseChangeSequence)

  const steps = presentation.kind === 'ready' ? presentation.steps : []
  const selectedStep = presentation.kind === 'ready' && selectedStepId
    ? presentation.stepsById.get(selectedStepId) ?? null
    : null
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
  const selectedPoseIsDisplayed = Boolean(
    selectedStep
    && !selectedStep.stale
    && instructionPoseMatchesApplied(selectedStep.pose, currentAppliedPose),
  )
  const playbackActive = playback.status === 'applying'
    || playback.status === 'holding'
  const timelineAvailable = presentation.kind === 'ready'
  const editingDisabled = coreBusy || benchmarkActive || !snapshot || !timelineAvailable

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
      applyAttemptRef.current = null
      applyObservationRef.current = null
      return
    }
    const attemptKey = `${playback.sequence}:${playback.cursor}:${playback.target.id}`
    if (applyAttemptRef.current === attemptKey) return
    applyAttemptRef.current = attemptKey
    applyObservationRef.current = currentAppliedPose
    setSelectedStepId(playback.target.id)
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
  }, [applyStepPose, currentAppliedPose, playback])

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
    if (observation === 'acknowledge') {
      setPlayback((current) => reduceInstructionPlayback(current, {
        kind: 'pose_applied',
        stepId: playback.target.id,
        now: performance.now(),
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
    }, INSTRUCTION_APPLICATION_TIMEOUT_MS)
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
    setNotice(instructionPlaybackStatusText(playback))
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
    const title = `手順 ${presentation.steps.length + 1}`
    const succeeded = await runNativeEdit(async (projectId, revision) => {
      const response = await addInstructionStep(
        projectId,
        revision,
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
      setNotice('現在の3D姿勢を手順へ追加できませんでした')
      return
    }
    if (addedStepId) setSelectedStepId(addedStepId)
    setNotice(`「${title}」を追加しました`)
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
      setEditorError(
        `タイトルは必須・改行なし${MAX_INSTRUCTION_TITLE_CHARACTERS}文字以内、`
        + `表示時間は${MIN_INSTRUCTION_DURATION_MS}〜${MAX_INSTRUCTION_DURATION_MS}msです。`,
      )
      return
    }
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision) =>
      updateInstructionStepMetadata(
        projectId,
        revision,
        selectedStep.id,
        metadata.title,
        metadata.description,
        metadata.caution,
        metadata.durationMs,
      ))
    setEditorError(succeeded ? null : '手順の説明を更新できませんでした')
    setNotice(succeeded ? `「${metadata.title}」を更新しました` : '手順を更新できませんでした')
  }

  async function replaceSelectedPose() {
    if (editingDisabled || !selectedStep || !captureDraft) return
    cancelPlayback('revision_changed')
    const succeeded = await runNativeEdit((projectId, revision) =>
      replaceInstructionStepPose(
        projectId,
        revision,
        selectedStep.id,
        captureDraft.fixedFace,
        captureDraft.hingeAngles,
      ))
    setNotice(succeeded
      ? `「${selectedStep.title}」の姿勢を現在の3D表示で更新しました`
      : '手順の姿勢を更新できませんでした')
  }

  async function deleteSelectedStep() {
    if (editingDisabled || !selectedStep) return
    if (!window.confirm(`「${selectedStep.title}」を削除しますか？`)) return
    cancelPlayback('revision_changed')
    const deletedId = selectedStep.id
    const succeeded = await runNativeEdit((projectId, revision) =>
      removeInstructionStep(projectId, revision, deletedId))
    if (!succeeded) {
      setNotice('手順を削除できませんでした')
      return
    }
    setSelectedStepId(null)
    setNotice(`「${selectedStep.title}」を削除しました`)
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
    const succeeded = await runNativeEdit((projectId, revision) =>
      moveInstructionStep(projectId, revision, selectedStep.id, targetIndex))
    setNotice(succeeded ? '手順の順番を変更しました' : '手順を移動できませんでした')
  }

  function showStepPose(step: InstructionStepPresentation) {
    cancelPlayback('manual_pose')
    if (step.stale) {
      setNotice('展開図が変更された手順です。「現在の3D姿勢で更新」してから表示してください')
      return
    }
    let applied = false
    try {
      applied = applyStepPose(step)
    } catch {
      applied = false
    }
    if (!applied) {
      setNotice('この手順の姿勢は現在の3Dモデルへ適用できません')
      return
    }
    setSelectedStepId(step.id)
    setNotice(`「${step.title}」の保存姿勢を3Dへ適用しています`)
  }

  function startOrStopPlayback() {
    if (playbackActive) {
      cancelPlayback('canceled')
      return
    }
    if (!snapshot || !poseModelKey) {
      setNotice('再生できる3Dモデルを準備してください')
      return
    }
    const plan = createInstructionPlaybackPlan(
      snapshot.project_id,
      snapshot.revision,
      presentation,
    )
    if (!plan) {
      setNotice('再生する手順がありません')
      return
    }
    const selectedIndex = selectedStep && !selectedStep.stale
      ? selectedStep.index
      : 0
    playbackModelKeyRef.current = poseModelKey
    applyAttemptRef.current = null
    setPlayback((current) => reduceInstructionPlayback(current, {
      kind: 'start',
      plan,
      startIndex: selectedIndex,
    }))
  }

  const captureStatus = describeCaptureStatus(
    snapshot,
    appliedPose,
    captureDraft !== null,
  )

  return (
    <section className="timeline panel" inert={inert}>
      <div className="timeline-controls">
        <button
          type="button"
          aria-label="先頭の手順を3Dに表示"
          disabled={coreBusy || steps.length === 0 || steps[0]?.stale}
          onClick={() => {
            const first = steps[0]
            if (first) showStepPose(first)
          }}
        >
          |◀
        </button>
        <button
          type="button"
          aria-label={playbackActive ? '再生を停止' : '選択手順から再生'}
          aria-pressed={playbackActive}
          disabled={coreBusy || benchmarkActive || steps.length === 0}
          onClick={startOrStopPlayback}
        >
          {playbackActive ? '■' : '▶'}
        </button>
        <strong>折り手順</strong>
        <span>
          {steps.length.toLocaleString()}手順
          {presentation.kind === 'ready'
            ? `・合計 ${formatDuration(presentation.totalDurationMs)}`
            : ''}
        </span>
        <small>
          保存した姿勢を段階表示します。姿勢間の連続した折り経路の安全性は保証しません。
        </small>
      </div>
      <div className="instruction-timeline-body">
        {presentation.kind === 'invalid' ? (
          <p className="instruction-timeline-error" role="alert">
            折り手順データを安全に読み取れないため、編集と再生を停止しました。
          </p>
        ) : (
          <>
            <div className="timeline-track" aria-label="折り手順一覧">
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
                    <span>{step.index + 1}. {step.title}</span>
                    <small>
                      {step.stale ? '要更新' : displayed ? '3D表示中' : formatDuration(step.durationMs)}
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
                ＋ 現在の3D姿勢を追加
              </button>
            </div>
            <div className="instruction-editor-region">
              {notice && (
                <p className="instruction-notice" aria-hidden="true">
                  {notice}
                </p>
              )}
              {selectedStep && editor ? (
                <form className="instruction-editor" onSubmit={(event) => void saveMetadata(event)}>
                  <label>
                    <span>タイトル</span>
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
                    <span>説明</span>
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
                  <label>
                    <span>注意</span>
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
                    <span>表示時間</span>
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
                  <div className="instruction-editor-actions">
                    <button type="submit" disabled={editingDisabled}>説明を保存</button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.stale || !poseModelKey}
                      onClick={() => showStepPose(selectedStep)}
                    >
                      3Dに表示
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || !captureDraft}
                      title={captureStatus}
                      onClick={() => void replaceSelectedPose()}
                    >
                      現在の3D姿勢で更新
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === 0}
                      onClick={() => void moveSelectedStep(selectedStep.index - 1)}
                    >
                      ← 前へ
                    </button>
                    <button
                      type="button"
                      disabled={editingDisabled || selectedStep.index === steps.length - 1}
                      onClick={() => void moveSelectedStep(selectedStep.index + 1)}
                    >
                      次へ →
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={editingDisabled}
                      onClick={() => void deleteSelectedStep()}
                    >
                      削除
                    </button>
                  </div>
                  {selectedStep.stale && (
                    <p className="instruction-stale-guidance">
                      展開図が記録時から変わりました。内容を確認し、現在の3D姿勢で更新すると再生できます。
                    </p>
                  )}
                  {selectedPoseIsDisplayed && (
                    <p className="instruction-current-pose">この保存姿勢を3Dに表示中です。</p>
                  )}
                  {editorError && <p className="instruction-editor-error" role="alert">{editorError}</p>}
                </form>
              ) : (
                <p className="instruction-empty-editor">
                  {steps.length === 0
                    ? `現在の3D姿勢を最初の手順として追加できます。${captureStatus}`
                    : '手順を選択すると説明・姿勢・順番を編集できます。'}
                </p>
              )}
            </div>
          </>
        )}
        <p className="visually-hidden" aria-live="polite" aria-atomic="true">
          {notice}
        </p>
      </div>
    </section>
  )
}

function describeCaptureStatus(
  snapshot: ProjectSnapshot | null,
  appliedPose: FoldPreviewAppliedPoseSnapshot | null,
  canCapture: boolean,
) {
  if (!snapshot) return 'プロジェクトを読み込んでください。'
  if (
    !appliedPose
    || appliedPose.projectId !== snapshot.project_id
    || appliedPose.revision !== snapshot.revision
  ) return '現在のrevisionの3D表示を準備しています。'
  if (appliedPose.state === 'running') return '3Dの動作が止まってから記録できます。'
  if (!canCapture) return '現在の3D姿勢は手順として安全に読み取れません。'
  if (appliedPose.state === 'blocked') {
    return '衝突境界で実際に停止している表示姿勢を記録します。'
  }
  if (appliedPose.state === 'indeterminate') {
    return '経路判定不能で停止した現在の表示姿勢だけを記録します。'
  }
  return '現在3Dに実際に表示されている姿勢を記録します。'
}

function formatDuration(durationMs: number) {
  const totalSeconds = Math.max(0, durationMs) / 1_000
  if (totalSeconds < 60) {
    return `${totalSeconds.toLocaleString('ja-JP', { maximumFractionDigits: 1 })}秒`
  }
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = Math.floor(totalSeconds % 60)
  return `${minutes}:${String(seconds).padStart(2, '0')}`
}
