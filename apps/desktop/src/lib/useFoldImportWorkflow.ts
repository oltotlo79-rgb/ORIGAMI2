import { useRef, useState } from 'react'

import {
  applyFoldImport,
  cancelFoldImport,
  previewFoldImport,
  type ProjectSnapshot,
} from './coreClient.ts'
import {
  appConfirmationText,
} from './appMessages.ts'
import type { FoldImportPreview, FoldImportSettings } from './foldImport.ts'
import type { Locale, LocalizedText } from './i18n.ts'
import {
  createImportPreviewCleanupRegistry,
  importWorkflowBinding,
  importWorkflowError,
  importWorkflowMessage,
  matchesImportWorkflowBinding,
  type ImportWorkflowMessage,
  type ImportWorkflowProjectBinding,
} from './importWorkflowSupport.ts'

export type FoldImportWorkflowCopy = Readonly<{
  missingPreview: LocalizedText
  cancelled: LocalizedText
  reviewReady: LocalizedText
  imported: LocalizedText
}>

export type FoldImportWorkflowTransport = Readonly<{
  preview: typeof previewFoldImport
  apply: typeof applyFoldImport
  cancel: typeof cancelFoldImport
}>

const DEFAULT_TRANSPORT: FoldImportWorkflowTransport = Object.freeze({
  preview: previewFoldImport,
  apply: applyFoldImport,
  cancel: cancelFoldImport,
})

export function useFoldImportWorkflow(input: Readonly<{
  locale: Locale
  copy: FoldImportWorkflowCopy
  getCurrentSnapshot: () => ProjectSnapshot | null
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'fold_import' | null) => void
  cancelInteraction: () => void
  onStatus: (message: ImportWorkflowMessage) => void
  onApplied: (snapshot: ProjectSnapshot) => void
  transport?: FoldImportWorkflowTransport
  confirmReplace?: (message: string) => boolean
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [preview, setPreview] = useState<FoldImportPreview | null>(null)
  const [error, setError] = useState<ImportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const bindingRef = useRef<ImportWorkflowProjectBinding | null>(null)
  const cleanupRef =
    useRef<ReturnType<typeof createImportPreviewCleanupRegistry> | null>(null)
  cleanupRef.current ??= createImportPreviewCleanupRegistry()
  const cleanup = cleanupRef.current
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const confirmReplace = input.confirmReplace
    ?? ((message: string) => window.confirm(message))
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function restoreButtonFocus() {
    scheduleFocus(() => buttonRef.current?.focus())
  }

  function clearPreview(clearError = true) {
    bindingRef.current = null
    setPreview(null)
    if (clearError) setError(null)
  }

  function rejectWith(code: 'fold_read_failed' | 'fold_cleanup_failed' | 'fold_import_failed') {
    const message = importWorkflowError(code)
    setError(message)
    input.onStatus(message)
  }

  async function begin() {
    const current = input.getCurrentSnapshot()
    if (!current || preview !== null || input.operationActive()) return

    const binding = importWorkflowBinding(current)
    const requestId = ++requestIdRef.current
    let issuedPreviewId: string | null = null
    input.setOperationBusy(true)
    input.setFileOperation('fold_import')
    setError(null)
    input.cancelInteraction()
    try {
      const pendingCleanupError = await cleanup.cancel(
        transport.cancel,
        ...cleanup.pendingIds(),
      )
      if (requestId !== requestIdRef.current) return
      if (pendingCleanupError !== null) {
        rejectWith('fold_cleanup_failed')
        return
      }

      const response = await transport.preview()
      issuedPreviewId = response.preview?.import_id ?? null
      if (requestId !== requestIdRef.current) {
        if (issuedPreviewId) {
          await cleanup.cancel(transport.cancel, issuedPreviewId)
        }
        return
      }
      if (response.canceled) {
        if (issuedPreviewId) {
          const cleanupError = await cleanup.cancel(
            transport.cancel,
            issuedPreviewId,
          )
          rejectWith(
            cleanupError === null
              ? 'fold_read_failed'
              : 'fold_cleanup_failed',
          )
          return
        }
        input.onStatus(importWorkflowMessage(input.copy.cancelled))
        return
      }
      if (!response.preview) {
        throw new Error(input.copy.missingPreview.en)
      }
      if (
        cleanup.hasDisposed(response.preview.import_id)
        || !matchesImportWorkflowBinding(
          binding,
          input.getCurrentSnapshot(),
        )
      ) {
        const cleanupError = await cleanup.cancel(
          transport.cancel,
          response.preview.import_id,
        )
        if (cleanupError !== null) {
          rejectWith('fold_cleanup_failed')
        } else {
          rejectWith('fold_read_failed')
        }
        return
      }

      bindingRef.current = binding
      setPreview(response.preview)
      input.onStatus(importWorkflowMessage(input.copy.reviewReady))
    } catch {
      const cleanupError = issuedPreviewId
        ? await cleanup.cancel(transport.cancel, issuedPreviewId)
        : null
      if (requestId !== requestIdRef.current) return
      rejectWith(
        cleanupError === null ? 'fold_read_failed' : 'fold_cleanup_failed',
      )
    } finally {
      if (requestId === requestIdRef.current) {
        input.setFileOperation(null)
        input.setOperationBusy(false)
      }
    }
  }

  async function close() {
    const pendingPreview = preview
    if (!pendingPreview || input.operationActive()) return

    const requestId = ++requestIdRef.current
    input.setOperationBusy(true)
    try {
      const cleanupError = await cleanup.cancel(
        transport.cancel,
        pendingPreview.import_id,
      )
      if (requestId !== requestIdRef.current) return
      if (cleanupError !== null) {
        rejectWith('fold_cleanup_failed')
        return
      }
      clearPreview()
      input.onStatus(importWorkflowMessage(input.copy.cancelled))
      restoreButtonFocus()
    } finally {
      if (requestId === requestIdRef.current) {
        input.setOperationBusy(false)
      }
    }
  }

  async function apply(settings: FoldImportSettings) {
    const current = input.getCurrentSnapshot()
    const pendingPreview = preview
    const binding = bindingRef.current
    if (
      !current
      || !pendingPreview
      || !binding
      || input.operationActive()
    ) return
    if (
      settings.importId !== pendingPreview.import_id
      || !matchesImportWorkflowBinding(binding, current)
    ) {
      rejectWith('fold_import_failed')
      return
    }
    if (
      current.is_dirty
      && !confirmReplace(appConfirmationText(input.locale, 'replaceWithFold'))
    ) return

    const requestId = ++requestIdRef.current
    let nativeApplied = false
    input.setOperationBusy(true)
    setError(null)
    input.cancelInteraction()
    try {
      const snapshot = await transport.apply(
        binding.project_id,
        binding.revision,
        settings,
      )
      nativeApplied = true
      cleanup.settle(pendingPreview.import_id)
      if (requestId !== requestIdRef.current) return
      if (!matchesImportWorkflowBinding(
        binding,
        input.getCurrentSnapshot(),
      )) {
        throw new Error('project binding changed after FOLD import')
      }
      input.onApplied(snapshot)
      clearPreview()
      input.onStatus(importWorkflowMessage(input.copy.imported, {
        name: snapshot.name,
      }))
      restoreButtonFocus()
    } catch {
      rejectWith('fold_import_failed')
      if (nativeApplied) {
        clearPreview(false)
        restoreButtonFocus()
      }
    } finally {
      if (requestId === requestIdRef.current) {
        input.setOperationBusy(false)
      }
    }
  }

  return {
    preview,
    error,
    buttonRef,
    begin,
    close,
    apply,
  } as const
}
