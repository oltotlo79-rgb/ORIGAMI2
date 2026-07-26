import { useRef, useState } from 'react'

import {
  applySvgImport,
  cancelSvgImport,
  previewSvgImport,
  validateSvgImportSettings,
  type ProjectSnapshot,
} from './coreClient.ts'
import { appConfirmationText } from './appMessages.ts'
import type {
  SvgImportPreview,
  SvgImportSettings,
  SvgImportSettingsDraft,
  SvgImportSettingsValidation,
  SvgImportMapping,
} from './svgImport.ts'
import {
  formatLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'
import {
  createImportPreviewCleanupRegistry,
  importWorkflowBinding,
  importWorkflowError,
  importWorkflowMessage,
  matchesImportWorkflowBinding,
  type ImportWorkflowMessage,
  type ImportWorkflowProjectBinding,
} from './importWorkflowSupport.ts'

export type SvgImportWorkflowCopy = Readonly<{
  missingPreview: LocalizedText
  cancelled: LocalizedText
  reviewReady: LocalizedText
  validationReadyJapanese: LocalizedText
  validationReadyEnglish: LocalizedText
  imported: LocalizedText
}>

export type SvgImportWorkflowTransport = Readonly<{
  preview: typeof previewSvgImport
  validate: typeof validateSvgImportSettings
  apply: typeof applySvgImport
  cancel: typeof cancelSvgImport
}>

type ValidationAuthority = Readonly<{
  binding: ImportWorkflowProjectBinding
  previewId: string
  validationId: string
  millimetersPerUnit: number
  boundaryCandidateId: number | null
  mappings: readonly (readonly [string, string | undefined])[]
}>

const DEFAULT_TRANSPORT: SvgImportWorkflowTransport = Object.freeze({
  preview: previewSvgImport,
  validate: validateSvgImportSettings,
  apply: applySvgImport,
  cancel: cancelSvgImport,
})

export function useSvgImportWorkflow(input: Readonly<{
  locale: Locale
  copy: SvgImportWorkflowCopy
  getCurrentSnapshot: () => ProjectSnapshot | null
  operationActive: () => boolean
  setOperationBusy: (busy: boolean) => void
  setFileOperation: (operation: 'svg_import' | null) => void
  cancelInteraction: () => void
  onStatus: (message: ImportWorkflowMessage) => void
  onApplied: (snapshot: ProjectSnapshot) => void
  transport?: SvgImportWorkflowTransport
  confirmReplace?: (message: string) => boolean
  scheduleFocus?: (callback: () => void) => void
}>) {
  const [preview, setPreview] = useState<SvgImportPreview | null>(null)
  const [validation, setValidation] =
    useState<SvgImportSettingsValidation | null>(null)
  const [error, setError] = useState<ImportWorkflowMessage | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const requestIdRef = useRef(0)
  const validationRequestIdRef = useRef(0)
  const bindingRef = useRef<ImportWorkflowProjectBinding | null>(null)
  const validationAuthorityRef = useRef<ValidationAuthority | null>(null)
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

  function clearValidation() {
    validationRequestIdRef.current += 1
    validationAuthorityRef.current = null
    setValidation(null)
  }

  function clearPreview(clearError = true) {
    bindingRef.current = null
    setPreview(null)
    clearValidation()
    if (clearError) setError(null)
  }

  function rejectWith(
    code:
      | 'svg_read_failed'
      | 'svg_cleanup_failed'
      | 'svg_boundary_validation_failed'
      | 'svg_import_failed',
  ) {
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
    input.setFileOperation('svg_import')
    setError(null)
    clearValidation()
    input.cancelInteraction()
    try {
      const pendingCleanupError = await cleanup.cancel(
        transport.cancel,
        ...cleanup.pendingIds(),
      )
      if (requestId !== requestIdRef.current) return
      if (pendingCleanupError !== null) {
        rejectWith('svg_cleanup_failed')
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
              ? 'svg_read_failed'
              : 'svg_cleanup_failed',
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
          rejectWith('svg_cleanup_failed')
        } else {
          rejectWith('svg_read_failed')
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
        cleanupError === null ? 'svg_read_failed' : 'svg_cleanup_failed',
      )
    } finally {
      if (requestId === requestIdRef.current) {
        input.setFileOperation(null)
        input.setOperationBusy(false)
      }
    }
  }

  function invalidateValidation() {
    clearValidation()
    setError(null)
  }

  async function validate(settings: SvgImportSettingsDraft) {
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
      rejectWith('svg_boundary_validation_failed')
      return
    }

    const validationRequestId = ++validationRequestIdRef.current
    input.setOperationBusy(true)
    setError(null)
    validationAuthorityRef.current = null
    setValidation(null)
    try {
      const response = await transport.validate(
        binding.project_id,
        binding.revision,
        settings,
      )
      if (validationRequestId !== validationRequestIdRef.current) return
      if (
        bindingRef.current !== binding
        || preview?.import_id !== pendingPreview.import_id
        || !matchesImportWorkflowBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        || response.preview_id !== pendingPreview.import_id
        || response.expected_project_id !== binding.project_id
        || response.expected_revision !== binding.revision
        || !Object.is(response.millimeters_per_unit, settings.mmPerUnit)
        || response.boundary_candidate_id !== settings.boundaryCandidateId
        || !Number.isFinite(response.width_mm)
        || response.width_mm <= 0
        || !Number.isFinite(response.height_mm)
        || response.height_mm <= 0
      ) {
        rejectWith('svg_boundary_validation_failed')
        return
      }
      validationAuthorityRef.current = Object.freeze({
        binding,
        previewId: pendingPreview.import_id,
        validationId: response.validation_id,
        millimetersPerUnit: settings.mmPerUnit,
        boundaryCandidateId: settings.boundaryCandidateId,
        mappings: normalizedMappings(settings.mappings),
      })
      setValidation(response)
      input.onStatus(importWorkflowMessage(Object.freeze({
        ja: formatLocalizedText(
          'ja',
          input.copy.validationReadyJapanese,
          {
            width: response.width_mm.toLocaleString('ja'),
            height: response.height_mm.toLocaleString('ja'),
          },
        ),
        en: formatLocalizedText(
          'en',
          input.copy.validationReadyEnglish,
          {
            width: response.width_mm.toLocaleString('en'),
            height: response.height_mm.toLocaleString('en'),
          },
        ),
      })))
    } catch {
      if (validationRequestId !== validationRequestIdRef.current) return
      rejectWith('svg_boundary_validation_failed')
    } finally {
      input.setOperationBusy(false)
    }
  }

  async function close() {
    const pendingPreview = preview
    if (!pendingPreview || input.operationActive()) return

    const requestId = ++requestIdRef.current
    validationRequestIdRef.current += 1
    input.setOperationBusy(true)
    try {
      const cleanupError = await cleanup.cancel(
        transport.cancel,
        pendingPreview.import_id,
      )
      if (requestId !== requestIdRef.current) return
      if (cleanupError !== null) {
        rejectWith('svg_cleanup_failed')
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

  async function apply(settings: SvgImportSettings) {
    const current = input.getCurrentSnapshot()
    const pendingPreview = preview
    const binding = bindingRef.current
    const authority = validationAuthorityRef.current
    if (
      !current
      || !pendingPreview
      || !binding
      || !authority
      || input.operationActive()
    ) return
    if (
      settings.importId !== pendingPreview.import_id
      || settings.validationId !== authority.validationId
      || authority.previewId !== pendingPreview.import_id
      || !matchesImportWorkflowBinding(binding, current)
      || !matchesImportWorkflowBinding(authority.binding, current)
      || !Object.is(settings.mmPerUnit, authority.millimetersPerUnit)
      || settings.boundaryCandidateId !== authority.boundaryCandidateId
      || !sameMappings(settings.mappings, authority.mappings)
    ) {
      rejectWith('svg_import_failed')
      return
    }
    const replaceDirtyProjectConfirmed = current.is_dirty
    if (
      replaceDirtyProjectConfirmed
      && !confirmReplace(appConfirmationText(input.locale, 'replaceWithSvg'))
    ) return

    const requestId = ++requestIdRef.current
    validationRequestIdRef.current += 1
    let nativeApplied = false
    input.setOperationBusy(true)
    setError(null)
    input.cancelInteraction()
    try {
      const snapshot = await transport.apply(
        binding.project_id,
        binding.revision,
        settings,
        replaceDirtyProjectConfirmed,
      )
      nativeApplied = true
      cleanup.settle(pendingPreview.import_id)
      if (requestId !== requestIdRef.current) return
      if (!matchesImportWorkflowBinding(
        binding,
        input.getCurrentSnapshot(),
      )) {
        throw new Error('project binding changed after SVG import')
      }
      input.onApplied(snapshot)
      clearPreview()
      input.onStatus(importWorkflowMessage(input.copy.imported, {
        name: snapshot.name,
      }))
      restoreButtonFocus()
    } catch {
      rejectWith('svg_import_failed')
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
    validation,
    error,
    buttonRef,
    begin,
    invalidateValidation,
    validate,
    close,
    apply,
  } as const
}

function normalizedMappings(
  mappings: SvgImportMapping,
): readonly (readonly [string, string | undefined])[] {
  return Object.freeze(
    Object.entries(mappings)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([groupId, target]) => Object.freeze([groupId, target] as const)),
  )
}

function sameMappings(
  mappings: SvgImportMapping,
  expected: readonly (readonly [string, string | undefined])[],
) {
  const actual = normalizedMappings(mappings)
  return actual.length === expected.length
    && actual.every(([groupId, target], index) => (
      groupId === expected[index]?.[0]
      && target === expected[index]?.[1]
    ))
}
