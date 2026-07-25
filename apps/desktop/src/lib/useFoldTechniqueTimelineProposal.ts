import { useMemo, useRef, useState } from 'react'

import {
  appendNamedTechniqueInstructionSteps,
  type ProjectSnapshot,
} from './coreClient.ts'
import type { FoldTechniqueFileDocumentV1 } from './foldTechniqueEditor.ts'
import {
  createFoldTechniqueTimelineProposalV1,
  type FoldTechniqueTimelineProposalPreview,
} from './foldTechniqueTimelineProposal.ts'
import type {
  Locale,
  LocalizedText,
  MessageVariables,
} from './i18n.ts'
import {
  completeOwnedRequest,
  createOwnedRequestGate,
  ownedRequestActive,
  tryBeginOwnedRequest,
} from './ownedRequestGate.ts'

export type FoldTechniqueTimelineMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type FoldTechniqueTimelineWorkspace = Readonly<{
  document: FoldTechniqueFileDocumentV1
  dirty: boolean
}>

export type FoldTechniqueTimelinePreviewState = Readonly<{
  preview: Extract<FoldTechniqueTimelineProposalPreview, { ok: true }>
  sourceDocument: FoldTechniqueFileDocumentV1
  techniqueIndex: number
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
}>

type NativeEditRunner = (
  action: (
    projectId: string,
    revision: number,
    projectInstanceId: string,
  ) => Promise<ProjectSnapshot>,
) => Promise<boolean>

function message(
  text: LocalizedText,
  variables?: MessageVariables,
): FoldTechniqueTimelineMessage {
  return Object.freeze({ text, variables })
}

export function useFoldTechniqueTimelineProposal(input: Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot | null
  workspace: FoldTechniqueTimelineWorkspace | null
  selectedIndex: number
  nativeCoreAvailable: () => boolean
  getCurrentSnapshot: () => ProjectSnapshot | null
  getCurrentWorkspace: () => FoldTechniqueTimelineWorkspace | null
  coreOperationActive: () => boolean
  foldTechniqueBusy: () => boolean
  runNativeEdit: NativeEditRunner
  onStatus: (status: FoldTechniqueTimelineMessage) => void
  appendProposal?: typeof appendNamedTechniqueInstructionSteps
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [preview, setPreview] =
    useState<FoldTechniqueTimelinePreviewState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<FoldTechniqueTimelineMessage | null>(null)
  const openerRef = useRef<HTMLButtonElement | null>(null)
  const requestGateRef = useRef(createOwnedRequestGate())
  const appendProposal =
    input.appendProposal ?? appendNamedTechniqueInstructionSteps
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  const stale = useMemo(() => Boolean(
    preview
    && (
      !input.snapshot
      || input.snapshot.project_instance_id
        !== preview.expectedProjectInstanceId
      || input.snapshot.project_id
        !== preview.expectedProjectId
      || input.snapshot.revision
        !== preview.expectedRevision
      || input.workspace?.document
        !== preview.sourceDocument
      || input.selectedIndex
        !== preview.techniqueIndex
    ),
  ), [input.selectedIndex, input.snapshot, input.workspace?.document, preview])

  function previewSelected(opener: HTMLButtonElement) {
    const workspace = input.getCurrentWorkspace()
    const current = input.getCurrentSnapshot()
    if (
      !workspace
      || !current
      || input.coreOperationActive()
      || input.foldTechniqueBusy()
      || ownedRequestActive(requestGateRef.current)
      || !input.nativeCoreAvailable()
    ) return
    const proposal = createFoldTechniqueTimelineProposalV1(
      workspace.document,
      input.selectedIndex,
      input.locale,
      current.instruction_timeline.steps.length,
    )
    if (!proposal.ok) {
      const status = proposal.error === 'timeline_capacity'
        ? message({
            ja: '折り手順の上限内に追加できません（必要 {required}、空き {available}）。',
            en: 'The proposal does not fit in the instruction limit (requires {required}, {available} available).',
          }, {
            required: proposal.requiredSteps,
            available: proposal.availableSteps,
          })
        : proposal.error === 'proposal_size'
          ? message({
              ja: '折り技法の説明案が安全な入力サイズ上限を超えています。',
              en: 'The fold-technique proposal exceeds the safe input-size limit.',
            })
          : message({
              ja: '選択中の折り技法から説明案を作成できませんでした。',
              en: 'Could not build a proposal from the selected fold technique.',
            })
      input.onStatus(status)
      return
    }
    openerRef.current = opener
    setError(null)
    setPreview({
      preview: proposal,
      sourceDocument: workspace.document,
      techniqueIndex: input.selectedIndex,
      expectedProjectInstanceId: current.project_instance_id,
      expectedProjectId: current.project_id,
      expectedRevision: current.revision,
    })
  }

  function closePreview() {
    if (ownedRequestActive(requestGateRef.current)) return
    const opener = openerRef.current
    openerRef.current = null
    setPreview(null)
    setError(null)
    scheduleFocus(() => opener?.focus())
  }

  async function confirmProposal() {
    const pending = preview
    const current = input.getCurrentSnapshot()
    if (
      !pending
      || ownedRequestActive(requestGateRef.current)
    ) return
    if (
      !current
      || current.project_instance_id !== pending.expectedProjectInstanceId
      || current.project_id !== pending.expectedProjectId
      || current.revision !== pending.expectedRevision
      || input.getCurrentWorkspace()?.document !== pending.sourceDocument
      || input.selectedIndex !== pending.techniqueIndex
    ) {
      setError(message({
        ja: 'プロジェクトまたは選択中の技法が変わりました。案を閉じて作り直してください。',
        en: 'The project or selected technique changed. Close and rebuild the proposal.',
      }))
      return
    }

    const requestId = tryBeginOwnedRequest(requestGateRef.current)
    if (requestId === null) return
    setBusy(true)
    setError(null)
    let succeeded = false
    try {
      succeeded = await input.runNativeEdit((
        projectId,
        revision,
        projectInstanceId,
      ) => {
        if (
          projectInstanceId !== pending.expectedProjectInstanceId
          || projectId !== pending.expectedProjectId
          || revision !== pending.expectedRevision
        ) return Promise.reject(new Error('stale named-technique proposal'))
        return appendProposal(
          projectId,
          revision,
          projectInstanceId,
          pending.preview.proposal,
        )
      })
    } catch {
      succeeded = false
    }
    if (!completeOwnedRequest(requestGateRef.current, requestId)) return
    setBusy(false)
    if (!succeeded) {
      setError(message({
        ja: '説明ステップを追加できませんでした。プロジェクトは変更されていません。',
        en: 'Could not append the description steps. The project was not changed.',
      }))
      return
    }
    const opener = openerRef.current
    openerRef.current = null
    setPreview(null)
    input.onStatus(message({
      ja: '「{technique}」から説明専用の折り手順を追加しました。1回のUndoで戻せます。',
      en: 'Added description-only steps from “{technique}”. One Undo removes the complete addition.',
    }, { technique: pending.preview.techniqueName }))
    scheduleFocus(() => opener?.focus())
  }

  return {
    preview,
    busy,
    error,
    stale,
    previewSelected,
    closePreview,
    confirmProposal,
  } as const
}
