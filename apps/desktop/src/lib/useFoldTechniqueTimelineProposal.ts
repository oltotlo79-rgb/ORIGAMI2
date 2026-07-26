import { useMemo, useRef, useState } from 'react'

import {
  appendNamedTechniqueInstructionSteps,
  matchesProjectOccGuard,
  type ProjectSnapshot,
} from './coreClient.ts'
import type { FoldTechniqueFileDocumentV1 } from './foldTechniqueEditor.ts'
import {
  createFoldTechniqueTimelineProposalV1,
  type FoldTechniqueTimelineProposalPreview,
} from './foldTechniqueTimelineProposal.ts'
import {
  FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT,
} from './foldTechniqueTimelineProposalText.ts'
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
      || !matchesProjectOccGuard({
        expectedProjectInstanceId: preview.expectedProjectInstanceId,
        expectedProjectId: preview.expectedProjectId,
        expectedRevision: preview.expectedRevision,
      }, input.snapshot)
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
        ? message(TEXT.timelineCapacityError, {
            required: proposal.requiredSteps,
            available: proposal.availableSteps,
          })
        : proposal.error === 'proposal_size'
          ? message(TEXT.proposalSizeError)
          : message(TEXT.proposalBuildError)
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
    const guard = pending
      ? {
          expectedProjectInstanceId: pending.expectedProjectInstanceId,
          expectedProjectId: pending.expectedProjectId,
          expectedRevision: pending.expectedRevision,
        }
      : null
    if (
      !current
      || !guard
      || !matchesProjectOccGuard(guard, current)
      || input.getCurrentWorkspace()?.document !== pending.sourceDocument
      || input.selectedIndex !== pending.techniqueIndex
    ) {
      setError(message(TEXT.staleProposalError))
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
        if (!matchesProjectOccGuard({
          expectedProjectInstanceId: pending.expectedProjectInstanceId,
          expectedProjectId: pending.expectedProjectId,
          expectedRevision: pending.expectedRevision,
        }, {
          project_instance_id: projectInstanceId,
          project_id: projectId,
          revision,
        })) return Promise.reject(new Error('stale named-technique proposal'))
        return appendProposal(
          {
            expectedProjectInstanceId: projectInstanceId,
            expectedProjectId: projectId,
            expectedRevision: revision,
          },
          pending.preview.proposal,
        )
      })
    } catch {
      succeeded = false
    }
    if (!completeOwnedRequest(requestGateRef.current, requestId)) return
    setBusy(false)
    if (!succeeded) {
      setError(message(TEXT.appendFailed))
      return
    }
    const opener = openerRef.current
    openerRef.current = null
    setPreview(null)
    input.onStatus(message(
      TEXT.appendSucceeded,
      { technique: pending.preview.techniqueName },
    ))
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
