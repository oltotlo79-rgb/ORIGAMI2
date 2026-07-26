import { useRef, useState } from 'react'

import {
  cancelInstructionMeshAnimation,
  matchesProjectOccGuard,
  previewInstructionMeshAnimation,
  saveInstructionMeshAnimation,
} from './coreClient.ts'
import type { MeshAnimationPreviewResponse } from './meshAnimationExport.ts'
import type {
  LocalizedText,
  MessageVariables,
} from './i18n.ts'

export type MeshAnimationExportWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type MeshAnimationExportProjectBinding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

export type MeshAnimationExportWorkflowCopy = Readonly<{
  prepareFailed: LocalizedText
  cleanupFailed: LocalizedText
  projectChanged: LocalizedText
  saveCancelledNotice: LocalizedText
  saved: LocalizedText
  saveFailed: LocalizedText
}>

export type MeshAnimationExportWorkflowTransport = Readonly<{
  preview: typeof previewInstructionMeshAnimation
  save: typeof saveInstructionMeshAnimation
  cancel: typeof cancelInstructionMeshAnimation
}>

const DEFAULT_TRANSPORT: MeshAnimationExportWorkflowTransport = Object.freeze({
  preview: previewInstructionMeshAnimation,
  save: saveInstructionMeshAnimation,
  cancel: cancelInstructionMeshAnimation,
})

function message(
  text: LocalizedText,
  variables?: MessageVariables,
): MeshAnimationExportWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function useMeshAnimationExportWorkflow(input: Readonly<{
  copy: MeshAnimationExportWorkflowCopy
  getCurrentSnapshot: () => MeshAnimationExportProjectBinding | null
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'mesh_animation_export' | null) => void
  onStatus: (status: MeshAnimationExportWorkflowMessage) => void
  transport?: MeshAnimationExportWorkflowTransport
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [open, setOpen] = useState(false)
  const [preview, setPreview] =
    useState<MeshAnimationPreviewResponse | null>(null)
  const [error, setError] =
    useState<MeshAnimationExportWorkflowMessage | null>(null)
  const [notice, setNotice] =
    useState<MeshAnimationExportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function restoreButtonFocus() {
    scheduleFocus(() => buttonRef.current?.focus())
  }

  async function prepare() {
    const current = input.getCurrentSnapshot()
    if (!current || input.operationActive()) return
    const requestId = ++requestIdRef.current
    input.setOperationBusy(true)
    input.setFileOperation('mesh_animation_export')
    setPreview(null)
    setError(null)
    setNotice(null)
    try {
      const nextPreview = await transport.preview({
        expectedProjectInstanceId: current.project_instance_id,
        expectedProjectId: current.project_id,
        expectedRevision: current.revision,
      })
      if (requestId !== requestIdRef.current) {
        await transport.cancel(nextPreview.exportId).catch(() => undefined)
        return
      }
      const latest = input.getCurrentSnapshot()
      if (
        !latest
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: nextPreview.projectInstanceId,
          expectedProjectId: nextPreview.projectId,
          expectedRevision: nextPreview.revision,
        }, latest)
      ) {
        await transport.cancel(nextPreview.exportId).catch(() => undefined)
        throw new Error('stale animation preview')
      }
      setPreview(nextPreview)
    } catch {
      if (requestId !== requestIdRef.current) return
      const safeError = message(input.copy.prepareFailed)
      setError(safeError)
      input.onStatus(safeError)
    } finally {
      if (requestId === requestIdRef.current) {
        input.setFileOperation(null)
        input.setOperationBusy(false)
      }
    }
  }

  function begin() {
    if (!input.getCurrentSnapshot() || input.operationActive()) return
    setOpen(true)
    void prepare()
  }

  async function close() {
    if (input.operationActive()) return
    const pendingPreview = preview
    requestIdRef.current += 1
    if (pendingPreview) {
      input.setOperationBusy(true)
      try {
        await transport.cancel(pendingPreview.exportId)
      } catch {
        setError(message(input.copy.cleanupFailed))
        return
      } finally {
        input.setOperationBusy(false)
      }
    }
    setOpen(false)
    setPreview(null)
    setError(null)
    setNotice(null)
    restoreButtonFocus()
  }

  async function save() {
    const pendingPreview = preview
    const current = input.getCurrentSnapshot()
    if (!pendingPreview || !current || input.operationActive()) return
    if (
      !matchesProjectOccGuard({
        expectedProjectInstanceId: pendingPreview.projectInstanceId,
        expectedProjectId: pendingPreview.projectId,
        expectedRevision: pendingPreview.revision,
      }, current)
    ) {
      setError(message(input.copy.projectChanged))
      return
    }
    input.setOperationBusy(true)
    input.setFileOperation('mesh_animation_export')
    setError(null)
    setNotice(null)
    try {
      const response = await transport.save({
        exportId: pendingPreview.exportId,
        expectedProjectInstanceId: pendingPreview.projectInstanceId,
        expectedProjectId: pendingPreview.projectId,
        expectedRevision: pendingPreview.revision,
        expectedSourceFingerprint: pendingPreview.sourceFingerprint,
      })
      if (response.canceled) {
        setNotice(message(input.copy.saveCancelledNotice))
        return
      }
      setOpen(false)
      setPreview(null)
      input.onStatus(message(input.copy.saved, {
        fileName: pendingPreview.suggestedFileName,
      }))
      restoreButtonFocus()
    } catch {
      const safeError = message(input.copy.saveFailed)
      setError(safeError)
      input.onStatus(safeError)
    } finally {
      input.setFileOperation(null)
      input.setOperationBusy(false)
    }
  }

  return {
    open,
    preview,
    error,
    notice,
    buttonRef,
    prepare,
    begin,
    close,
    save,
  } as const
}
