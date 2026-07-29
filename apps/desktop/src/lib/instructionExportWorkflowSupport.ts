import {
  beginInstructionExportGeneration,
  cancelInstructionExport,
  getInstructionExportProgress,
  matchesProjectOccGuard,
  previewInstructionExport,
  saveInstructionExport,
} from './coreClient.ts'
import {
  instructionExportErrorMessage,
  type InstructionExportFormat,
} from './instructionExport.ts'
import { localizedInstructionExportFormatLabel } from './appPresentation.ts'
import {
  formatLocalizedText,
  type LocalizedText,
  type MessageVariables,
} from './i18n.ts'

export type InstructionExportWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type InstructionExportProjectBinding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

export type InstructionExportWorkflowCopy = Readonly<{
  previewReadyJapanese: LocalizedText
  previewReadyEnglish: LocalizedText
  prepareFailed: LocalizedText
  prepareStatusFailed: LocalizedText
  progressFailed: LocalizedText
  stopping: LocalizedText
  stopped: LocalizedText
  alreadyFinished: LocalizedText
  cancelled: LocalizedText
  cancelFailed: LocalizedText
  cancelStatusFailed: LocalizedText
  projectChanged: LocalizedText
  saveCancelledNotice: LocalizedText
  saveCancelledStatus: LocalizedText
  saved: LocalizedText
  saveFailed: LocalizedText
  saveStatusFailed: LocalizedText
}>

export type InstructionExportWorkflowTransport = Readonly<{
  begin: typeof beginInstructionExportGeneration
  preview: typeof previewInstructionExport
  progress: typeof getInstructionExportProgress
  save: typeof saveInstructionExport
  cancel: typeof cancelInstructionExport
}>

export const DEFAULT_INSTRUCTION_EXPORT_WORKFLOW_TRANSPORT:
InstructionExportWorkflowTransport = Object.freeze({
  begin: beginInstructionExportGeneration,
  preview: previewInstructionExport,
  progress: getInstructionExportProgress,
  save: saveInstructionExport,
  cancel: cancelInstructionExport,
})

export function instructionExportWorkflowMessage(
  text: LocalizedText,
  variables?: MessageVariables,
): InstructionExportWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function instructionExportWorkflowErrorMessage(
  error: unknown,
  text: LocalizedText,
): InstructionExportWorkflowMessage {
  return instructionExportWorkflowMessage(Object.freeze({
    ja: formatLocalizedText('ja', text, {
      error: instructionExportErrorMessage(error, 'ja'),
    }),
    en: formatLocalizedText('en', text, {
      error: instructionExportErrorMessage(error, 'en'),
    }),
  }))
}

export function matchesInstructionExportBinding(
  expected: InstructionExportProjectBinding,
  current: InstructionExportProjectBinding | null,
) {
  return current !== null && matchesProjectOccGuard({
    expectedProjectInstanceId: expected.project_instance_id,
    expectedProjectId: expected.project_id,
    expectedRevision: expected.revision,
  }, current)
}

export function instructionExportPreviewReadyMessage(
  format: InstructionExportFormat,
  copy: Pick<
    InstructionExportWorkflowCopy,
    'previewReadyJapanese' | 'previewReadyEnglish'
  >,
) {
  return instructionExportWorkflowMessage(Object.freeze({
    ja: formatLocalizedText('ja', copy.previewReadyJapanese, {
      format: localizedInstructionExportFormatLabel(format, 'ja'),
    }),
    en: formatLocalizedText('en', copy.previewReadyEnglish, {
      format: localizedInstructionExportFormatLabel(format, 'en'),
    }),
  }))
}
