import { useRef, useState } from 'react'

import {
  INSTRUCTION_EXPORT_PROFILE,
  INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  createInstructionExportError,
  type InstructionExportFormat,
  type InstructionExportPhase,
  type InstructionExportPreview,
} from './instructionExport.ts'
import {
  DEFAULT_INSTRUCTION_EXPORT_WORKFLOW_TRANSPORT,
  instructionExportPreviewReadyMessage,
  instructionExportWorkflowErrorMessage,
  instructionExportWorkflowMessage,
  matchesInstructionExportBinding,
  type InstructionExportProjectBinding,
  type InstructionExportWorkflowCopy,
  type InstructionExportWorkflowMessage,
  type InstructionExportWorkflowTransport,
} from './instructionExportWorkflowSupport.ts'
import { createInstructionExportCleanupRegistry } from './instructionExportCleanupRegistry.ts'

export type {
  InstructionExportProjectBinding,
  InstructionExportWorkflowCopy,
  InstructionExportWorkflowMessage,
  InstructionExportWorkflowTransport,
} from './instructionExportWorkflowSupport.ts'

export function useInstructionExportWorkflow(input: Readonly<{
  copy: InstructionExportWorkflowCopy
  getCurrentSnapshot: () => InstructionExportProjectBinding | null
  exportAvailable: () => boolean
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'instruction_export' | null) => void
  cancelInteraction: () => void
  onStatus: (status: InstructionExportWorkflowMessage) => void
  transport?: InstructionExportWorkflowTransport
  waitForPoll?: () => Promise<void>
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [open, setOpen] = useState(false)
  const [format, setFormat] = useState<InstructionExportFormat>('pdf')
  const [preview, setPreview] = useState<InstructionExportPreview | null>(null)
  const [generationActive, setGenerationActive] = useState(false)
  const [phase, setPhase] = useState<InstructionExportPhase>('validating')
  const [error, setError] =
    useState<InstructionExportWorkflowMessage | null>(null)
  const [notice, setNotice] =
    useState<InstructionExportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const generationIdRef = useRef<string | null>(null)
  const previewBindingRef = useRef<InstructionExportProjectBinding | null>(null)
  const cleanupRegistryRef =
    useRef<ReturnType<typeof createInstructionExportCleanupRegistry> | null>(
      null,
    )
  cleanupRegistryRef.current ??= createInstructionExportCleanupRegistry()
  const cleanupRegistry = cleanupRegistryRef.current
  const transport = input.transport
    ?? DEFAULT_INSTRUCTION_EXPORT_WORKFLOW_TRANSPORT
  const waitForPoll = input.waitForPoll
    ?? (() => new Promise<void>((resolve) => window.setTimeout(resolve, 100)))
  const scheduleFocus = input.scheduleFocus
    ?? ((callback: () => void) => requestAnimationFrame(callback))

  function restoreButtonFocus() {
    scheduleFocus(() => buttonRef.current?.focus())
  }

  function ownsGeneration(requestId: number, exportId: string) {
    return requestId === requestIdRef.current
      && generationIdRef.current === exportId
  }

  async function cancelExportIds(
    ...exportIds: readonly (string | null)[]
  ) {
    return cleanupRegistry.cancel(transport.cancel, ...exportIds)
  }

  async function discardExports(...exportIds: readonly string[]) {
    await cleanupRegistry.discard(transport.cancel, ...exportIds)
  }

  async function pollProgress(exportId: string, requestId: number) {
    while (ownsGeneration(requestId, exportId)) {
      try {
        await waitForPoll()
        if (!ownsGeneration(requestId, exportId)) return
        const progress = await transport.progress(exportId)
        if (
          !ownsGeneration(requestId, exportId)
          || progress.export_id !== exportId
        ) return
        setPhase(progress.phase)
        if (progress.phase === 'ready') return
      } catch (pollError) {
        if (!ownsGeneration(requestId, exportId)) return
        setNotice(instructionExportWorkflowErrorMessage(
          pollError,
          input.copy.progressFailed,
        ))
        return
      }
    }
  }

  async function prepare(nextFormat: InstructionExportFormat) {
    const current = input.getCurrentSnapshot()
    if (
      !current
      || !input.exportAvailable()
      || input.operationActive()
    ) return

    const replacedExportIds = [
      ...cleanupRegistry.pendingExportIds(),
      generationIdRef.current,
      preview?.export_id ?? null,
    ]
    const requestId = ++requestIdRef.current
    let issuedExportId: string | null = null
    let responseExportId: string | null = null
    let replacementCleanupError: unknown | null = null
    input.setOperationBusy(true)
    input.setFileOperation('instruction_export')
    setGenerationActive(true)
    setPhase('validating')
    setPreview(null)
    setError(null)
    setNotice(null)
    input.cancelInteraction()
    try {
      replacementCleanupError = await cancelExportIds(...replacedExportIds)
      if (requestId !== requestIdRef.current) return
      if (replacementCleanupError !== null) throw replacementCleanupError
      generationIdRef.current = null
      previewBindingRef.current = null

      const generation = await transport.begin()
      issuedExportId = generation.export_id
      if (
        generation.profile !== INSTRUCTION_EXPORT_PROFILE
        || cleanupRegistry.hasDisposed(generation.export_id)
      ) {
        throw createInstructionExportError('document_contract_invalid')
      }
      if (requestId !== requestIdRef.current) {
        await discardExports(generation.export_id)
        return
      }
      if (!matchesInstructionExportBinding(
        current,
        input.getCurrentSnapshot(),
      )) {
        throw createInstructionExportError('project_changed')
      }

      generationIdRef.current = generation.export_id
      void pollProgress(generation.export_id, requestId)
      const response = await transport.preview(
        generation.export_id,
        current.project_id,
        current.revision,
        nextFormat,
      )
      responseExportId = response.preview.export_id
      if (requestId !== requestIdRef.current) {
        await discardExports(response.preview.export_id, generation.export_id)
        return
      }

      const latest = input.getCurrentSnapshot()
      const nextPreview = response.preview
      if (
        !matchesInstructionExportBinding(current, latest)
        || generationIdRef.current !== generation.export_id
        || nextPreview.export_id !== generation.export_id
        || nextPreview.format !== nextFormat
        || nextPreview.profile !== INSTRUCTION_EXPORT_PROFILE
        || nextPreview.projection_profile
          !== INSTRUCTION_EXPORT_PROJECTION_PROFILE
        || nextPreview.expected_project_id !== current.project_id
        || nextPreview.expected_revision !== current.revision
      ) {
        throw createInstructionExportError('document_contract_invalid')
      }

      previewBindingRef.current = Object.freeze({
        project_instance_id: current.project_instance_id,
        project_id: current.project_id,
        revision: current.revision,
      })
      setPreview(nextPreview)
      setPhase('ready')
      input.onStatus(instructionExportPreviewReadyMessage(
        nextPreview.format,
        input.copy,
      ))
    } catch (prepareError) {
      if (requestId !== requestIdRef.current) {
        if (issuedExportId) {
          await discardExports(responseExportId ?? issuedExportId, issuedExportId)
        }
        return
      }
      const issuedCleanupError = issuedExportId
        ? await cancelExportIds(responseExportId, issuedExportId)
        : null
      if (requestId !== requestIdRef.current) return
      const cleanupError = replacementCleanupError ?? issuedCleanupError
      if (
        issuedExportId
        && cleanupRegistry.hasPending(issuedExportId)
      ) {
        generationIdRef.current = issuedExportId
      } else if (issuedExportId) {
        generationIdRef.current = null
      }
      previewBindingRef.current = null
      setError(instructionExportWorkflowErrorMessage(
        prepareError,
        input.copy.prepareFailed,
      ))
      if (cleanupError !== null) {
        setNotice(instructionExportWorkflowErrorMessage(
          cleanupError,
          input.copy.cancelFailed,
        ))
      }
      input.onStatus(instructionExportWorkflowErrorMessage(
        prepareError,
        input.copy.prepareStatusFailed,
      ))
    } finally {
      if (requestId === requestIdRef.current) {
        setGenerationActive(false)
        input.setFileOperation(null)
        input.setOperationBusy(false)
      }
    }
  }

  function begin() {
    if (
      !input.getCurrentSnapshot()
      || !input.exportAvailable()
      || input.operationActive()
    ) return
    setOpen(true)
    setFormat('pdf')
    setPreview(null)
    setError(null)
    setNotice(null)
    void prepare('pdf')
  }

  function changeFormat(nextFormat: InstructionExportFormat) {
    if (nextFormat === format || input.operationActive()) return
    setFormat(nextFormat)
    void prepare(nextFormat)
  }

  async function close() {
    if (input.operationActive() && !generationActive) return
    const pendingPreview = preview
    const exportIds = [
      ...cleanupRegistry.pendingExportIds(),
      generationIdRef.current,
      pendingPreview?.export_id ?? null,
    ]
    const hasExport = exportIds.some((exportId) => exportId !== null)
    requestIdRef.current += 1
    setGenerationActive(false)

    if (input.operationActive()) {
      generationIdRef.current = null
      previewBindingRef.current = null
      setOpen(false)
      setPreview(null)
      setError(null)
      setNotice(null)
      input.setFileOperation(null)
      input.setOperationBusy(false)
      input.onStatus(instructionExportWorkflowMessage(input.copy.stopping))
      restoreButtonFocus()
      if (hasExport) {
        const cancelError = await cancelExportIds(...exportIds)
        if (cancelError === null) {
          input.onStatus(instructionExportWorkflowMessage(input.copy.stopped))
        } else {
          input.onStatus(instructionExportWorkflowMessage(
            input.copy.alreadyFinished,
          ))
        }
      }
      return
    }

    if (!hasExport) {
      generationIdRef.current = null
      previewBindingRef.current = null
      setOpen(false)
      setError(null)
      setNotice(null)
      restoreButtonFocus()
      return
    }

    input.setOperationBusy(true)
    try {
      const cancelError = await cancelExportIds(...exportIds)
      if (cancelError !== null) throw cancelError
      generationIdRef.current = null
      previewBindingRef.current = null
      setOpen(false)
      setPreview(null)
      setError(null)
      setNotice(null)
      input.onStatus(instructionExportWorkflowMessage(input.copy.cancelled))
      restoreButtonFocus()
    } catch (cancelError) {
      setError(instructionExportWorkflowErrorMessage(
        cancelError,
        input.copy.cancelFailed,
      ))
      input.onStatus(instructionExportWorkflowErrorMessage(
        cancelError,
        input.copy.cancelStatusFailed,
      ))
    } finally {
      input.setOperationBusy(false)
    }
  }

  async function save(warningsAcknowledged: boolean) {
    const current = input.getCurrentSnapshot()
    const pendingPreview = preview
    const previewBinding = previewBindingRef.current
    if (
      !current
      || !pendingPreview
      || !previewBinding
      || input.operationActive()
    ) return
    if (
      !matchesInstructionExportBinding(previewBinding, current)
      || current.project_id !== pendingPreview.expected_project_id
      || current.revision !== pendingPreview.expected_revision
    ) {
      setError(instructionExportWorkflowMessage(input.copy.projectChanged))
      return
    }

    input.setOperationBusy(true)
    input.setFileOperation('instruction_export')
    setError(null)
    setNotice(null)
    try {
      const response = await transport.save(
        pendingPreview.export_id,
        pendingPreview.expected_project_id,
        pendingPreview.expected_revision,
        warningsAcknowledged,
      )
      if (response.canceled) {
        setNotice(instructionExportWorkflowMessage(
          input.copy.saveCancelledNotice,
        ))
        input.onStatus(instructionExportWorkflowMessage(
          input.copy.saveCancelledStatus,
        ))
        return
      }
      cleanupRegistry.settle(pendingPreview.export_id)
      setOpen(false)
      generationIdRef.current = null
      previewBindingRef.current = null
      setPreview(null)
      setNotice(null)
      input.onStatus(instructionExportWorkflowMessage(input.copy.saved, {
        fileName: pendingPreview.suggested_file_name,
      }))
      restoreButtonFocus()
    } catch (saveError) {
      setError(instructionExportWorkflowErrorMessage(
        saveError,
        input.copy.saveFailed,
      ))
      input.onStatus(instructionExportWorkflowErrorMessage(
        saveError,
        input.copy.saveStatusFailed,
      ))
    } finally {
      input.setFileOperation(null)
      input.setOperationBusy(false)
    }
  }

  return {
    open,
    format,
    preview,
    generationActive,
    phase,
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
