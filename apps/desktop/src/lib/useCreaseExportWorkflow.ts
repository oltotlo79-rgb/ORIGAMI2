import { useRef, useState } from 'react'

import {
  cancelCreasePatternExport,
  previewCreasePatternExport,
  saveCreasePatternExport,
} from './coreClient.ts'
import type {
  CreasePatternExportFormat,
  CreasePatternExportPreview,
} from './creaseExport.ts'
import { localizedCreaseExportFormatLabel } from './appPresentation.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from './i18n.ts'

export type CreaseExportWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type CreaseExportProjectBinding = Readonly<{
  project_id: string
  revision: number
}>

export type CreaseExportWorkflowCopy = Readonly<{
  previewRejected: LocalizedText
  previewReadyJapanese: LocalizedText
  previewReadyEnglish: LocalizedText
  cancelled: LocalizedText
  projectChanged: LocalizedText
  saveCancelledNotice: LocalizedText
  saveCancelledStatus: LocalizedText
}>

export type CreaseExportWorkflowTransport = Readonly<{
  preview: typeof previewCreasePatternExport
  save: typeof saveCreasePatternExport
  cancel: typeof cancelCreasePatternExport
}>

const DEFAULT_TRANSPORT: CreaseExportWorkflowTransport = Object.freeze({
  preview: previewCreasePatternExport,
  save: saveCreasePatternExport,
  cancel: cancelCreasePatternExport,
})

function message(
  text: LocalizedText,
  variables?: MessageVariables,
): CreaseExportWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function useCreaseExportWorkflow(input: Readonly<{
  locale: Locale
  copy: CreaseExportWorkflowCopy
  getCurrentSnapshot: () => CreaseExportProjectBinding | null
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'crease_export' | null) => void
  cancelInteraction: () => void
  onStatus: (status: CreaseExportWorkflowMessage) => void
  prepareFailedMessage: CreaseExportWorkflowMessage
  cleanupFailedMessage: CreaseExportWorkflowMessage
  saveFailedMessage: CreaseExportWorkflowMessage
  savedMessage: (
    preview: CreasePatternExportPreview,
  ) => CreaseExportWorkflowMessage
  transport?: CreaseExportWorkflowTransport
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [open, setOpen] = useState(false)
  const [format, setFormat] = useState<CreasePatternExportFormat>('fold')
  const [preview, setPreview] =
    useState<CreasePatternExportPreview | null>(null)
  const [error, setError] =
    useState<CreaseExportWorkflowMessage | null>(null)
  const [notice, setNotice] =
    useState<CreaseExportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function restoreButtonFocus() {
    scheduleFocus(() => buttonRef.current?.focus())
  }

  async function prepare(nextFormat: CreasePatternExportFormat) {
    const current = input.getCurrentSnapshot()
    if (!current || input.operationActive()) return

    const requestId = ++requestIdRef.current
    input.setOperationBusy(true)
    input.setFileOperation('crease_export')
    setPreview(null)
    setError(null)
    setNotice(null)
    input.cancelInteraction()
    try {
      const response = await transport.preview(
        current.project_id,
        current.revision,
        nextFormat,
      )
      if (requestId !== requestIdRef.current) {
        await transport.cancel(response.preview.export_id).catch(() => undefined)
        return
      }
      const latest = input.getCurrentSnapshot()
      const nextPreview = response.preview
      if (
        !latest
        || nextPreview.format !== nextFormat
        || nextPreview.expected_project_id !== current.project_id
        || nextPreview.expected_revision !== current.revision
        || latest.project_id !== current.project_id
        || latest.revision !== current.revision
      ) {
        await transport.cancel(nextPreview.export_id).catch(() => undefined)
        throw new Error(selectLocalizedText(
          input.locale,
          input.copy.previewRejected,
        ))
      }
      setPreview(nextPreview)
      input.onStatus(message({
        ja: formatLocalizedText('ja', input.copy.previewReadyJapanese, {
          format: localizedCreaseExportFormatLabel(nextPreview.format, 'ja'),
        }),
        en: formatLocalizedText('en', input.copy.previewReadyEnglish, {
          format: localizedCreaseExportFormatLabel(nextPreview.format, 'en'),
        }),
      }))
    } catch {
      if (requestId !== requestIdRef.current) return
      setError(input.prepareFailedMessage)
      input.onStatus(input.prepareFailedMessage)
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
    setFormat('fold')
    setPreview(null)
    setError(null)
    setNotice(null)
    void prepare('fold')
  }

  function changeFormat(nextFormat: CreasePatternExportFormat) {
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
      await transport.cancel(pendingPreview.export_id)
      setOpen(false)
      setPreview(null)
      setError(null)
      setNotice(null)
      input.onStatus(message(input.copy.cancelled))
      restoreButtonFocus()
    } catch {
      setError(input.cleanupFailedMessage)
      input.onStatus(input.cleanupFailedMessage)
    } finally {
      input.setOperationBusy(false)
    }
  }

  async function save(warningsAcknowledged: boolean) {
    const current = input.getCurrentSnapshot()
    const pendingPreview = preview
    if (!current || !pendingPreview || input.operationActive()) return
    if (
      current.project_id !== pendingPreview.expected_project_id
      || current.revision !== pendingPreview.expected_revision
    ) {
      setError(message(input.copy.projectChanged))
      return
    }

    input.setOperationBusy(true)
    input.setFileOperation('crease_export')
    setError(null)
    setNotice(null)
    try {
      const response = await transport.save(
        pendingPreview.export_id,
        current.project_id,
        current.revision,
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
      input.onStatus(input.savedMessage(pendingPreview))
      restoreButtonFocus()
    } catch {
      setError(input.saveFailedMessage)
      input.onStatus(input.saveFailedMessage)
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
