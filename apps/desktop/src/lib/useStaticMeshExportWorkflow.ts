import { useRef, useState } from 'react'

import {
  cancelStaticMeshExport,
  matchesProjectOccGuard,
  previewStaticMeshExport,
  saveStaticMeshExport,
} from './coreClient.ts'
import {
  foldPreviewAppliedPoseKey,
  type FoldPreviewAppliedPoseSnapshot,
} from './foldPreviewAppliedPose.ts'
import type {
  StaticMeshExportFormat,
  StaticMeshExportPreview,
} from './staticMeshExport.ts'
import type {
  LocalizedText,
  MessageVariables,
} from './i18n.ts'

export type StaticMeshExportWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type StaticMeshExportProjectBinding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

export type StaticMeshExportWorkflowCopy = Readonly<{
  previewReady: LocalizedText
  prepareFailed: LocalizedText
  cancelled: LocalizedText
  cleanupFailed: LocalizedText
  projectChanged: LocalizedText
  saveCancelledNotice: LocalizedText
  saveCancelledStatus: LocalizedText
  saved: LocalizedText
  saveFailed: LocalizedText
}>

export type StaticMeshExportWorkflowTransport = Readonly<{
  preview: typeof previewStaticMeshExport
  save: typeof saveStaticMeshExport
  cancel: typeof cancelStaticMeshExport
}>

const DEFAULT_TRANSPORT: StaticMeshExportWorkflowTransport = Object.freeze({
  preview: previewStaticMeshExport,
  save: saveStaticMeshExport,
  cancel: cancelStaticMeshExport,
})

function message(
  text: LocalizedText,
  variables?: MessageVariables,
): StaticMeshExportWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function useStaticMeshExportWorkflow(input: Readonly<{
  copy: StaticMeshExportWorkflowCopy
  getCurrentSnapshot: () => StaticMeshExportProjectBinding | null
  getCurrentPose: () => FoldPreviewAppliedPoseSnapshot | null
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'mesh_export' | null) => void
  cancelInteraction: () => void
  onStatus: (status: StaticMeshExportWorkflowMessage) => void
  transport?: StaticMeshExportWorkflowTransport
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [open, setOpen] = useState(false)
  const [format, setFormat] = useState<StaticMeshExportFormat>('obj')
  const [preview, setPreview] = useState<StaticMeshExportPreview | null>(null)
  const [error, setError] =
    useState<StaticMeshExportWorkflowMessage | null>(null)
  const [notice, setNotice] =
    useState<StaticMeshExportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function restoreButtonFocus() {
    scheduleFocus(() => buttonRef.current?.focus())
  }

  async function prepare(nextFormat: StaticMeshExportFormat) {
    const current = input.getCurrentSnapshot()
    const pose = input.getCurrentPose()
    const sourcePoseKey = foldPreviewAppliedPoseKey(pose)
    if (
      !current
      || !pose
      || pose.state === 'running'
      || !sourcePoseKey
      || pose.projectId !== current.project_id
      || pose.revision !== current.revision
      || input.operationActive()
    ) return

    const requestId = ++requestIdRef.current
    input.setOperationBusy(true)
    input.setFileOperation('mesh_export')
    setPreview(null)
    setError(null)
    setNotice(null)
    input.cancelInteraction()
    try {
      const response = await transport.preview(
        current.project_instance_id,
        current.project_id,
        current.revision,
        nextFormat,
      )
      if (requestId !== requestIdRef.current) {
        await transport.cancel(response.preview.exportId).catch(() => undefined)
        return
      }
      const latest = input.getCurrentSnapshot()
      const latestPose = input.getCurrentPose()
      const nextPreview = response.preview
      if (
        !latest
        || nextPreview.format !== nextFormat
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: nextPreview.projectInstanceId,
          expectedProjectId: nextPreview.projectId,
          expectedRevision: nextPreview.revision,
        }, current)
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: current.project_instance_id,
          expectedProjectId: current.project_id,
          expectedRevision: current.revision,
        }, latest)
        || foldPreviewAppliedPoseKey(latestPose) !== sourcePoseKey
        || latestPose?.state === 'running'
      ) {
        await transport.cancel(nextPreview.exportId).catch(() => undefined)
        throw new Error('stale static-mesh preview')
      }
      setPreview(nextPreview)
      input.onStatus(message(input.copy.previewReady))
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
    const current = input.getCurrentSnapshot()
    const pose = input.getCurrentPose()
    if (
      !current
      || !pose
      || pose.state === 'running'
      || pose.projectId !== current.project_id
      || pose.revision !== current.revision
      || input.operationActive()
    ) return
    setOpen(true)
    setFormat('obj')
    setPreview(null)
    setError(null)
    setNotice(null)
    void prepare('obj')
  }

  function changeFormat(nextFormat: StaticMeshExportFormat) {
    if (nextFormat === format || input.operationActive()) return
    setFormat(nextFormat)
    void prepare(nextFormat)
  }

  async function close() {
    if (input.operationActive()) return
    const pendingPreview = preview
    requestIdRef.current += 1
    if (!pendingPreview) {
      setOpen(false)
      setError(null)
      setNotice(null)
      restoreButtonFocus()
      return
    }

    input.setOperationBusy(true)
    try {
      await transport.cancel(pendingPreview.exportId)
      setOpen(false)
      setPreview(null)
      setError(null)
      setNotice(null)
      input.onStatus(message(input.copy.cancelled))
      restoreButtonFocus()
    } catch {
      const safeError = message(input.copy.cleanupFailed)
      setError(safeError)
      input.onStatus(safeError)
    } finally {
      input.setOperationBusy(false)
    }
  }

  async function save(warningsAcknowledged: boolean) {
    const current = input.getCurrentSnapshot()
    const pendingPreview = preview
    if (!current || !pendingPreview || input.operationActive()) return
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
    input.setFileOperation('mesh_export')
    setError(null)
    setNotice(null)
    try {
      const response = await transport.save(
        pendingPreview,
        warningsAcknowledged,
      )
      if (response.canceled) {
        setNotice(message(input.copy.saveCancelledNotice))
        input.onStatus(message(input.copy.saveCancelledStatus))
        return
      }
      setOpen(false)
      setPreview(null)
      setNotice(null)
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
    format,
    preview,
    error,
    notice,
    buttonRef,
    prepare,
    begin,
    changeFormat,
    close,
    save,
  } as const
}
