import {
  matchesProjectOccGuard,
  type ProjectSnapshot,
} from './coreClient.ts'
import {
  appErrorLocalizedText,
  type AppErrorCode,
} from './appMessages.ts'
import type {
  LocalizedText,
  MessageVariables,
} from './i18n.ts'

export type ImportWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type ImportWorkflowProjectBinding = Readonly<Pick<
  ProjectSnapshot,
  'project_instance_id' | 'project_id' | 'revision'
>>

export type ImportPreviewCancelTransport =
  (previewId: string) => Promise<void>

export type ImportPreviewCleanupRegistry = Readonly<{
  pendingIds: () => readonly string[]
  hasDisposed: (previewId: string) => boolean
  settle: (previewId: string) => void
  cancel: (
    transport: ImportPreviewCancelTransport,
    ...previewIds: readonly (string | null)[]
  ) => Promise<unknown | null>
}>

const DISPOSED_IMPORT_PREVIEW_LIMIT = 64

export function importWorkflowMessage(
  text: LocalizedText,
  variables?: MessageVariables,
): ImportWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function importWorkflowError(
  code: AppErrorCode,
): ImportWorkflowMessage {
  return importWorkflowMessage(appErrorLocalizedText(code))
}

export function importWorkflowBinding(
  snapshot: ImportWorkflowProjectBinding,
): ImportWorkflowProjectBinding {
  return Object.freeze({
    project_instance_id: snapshot.project_instance_id,
    project_id: snapshot.project_id,
    revision: snapshot.revision,
  })
}

export function matchesImportWorkflowBinding(
  expected: ImportWorkflowProjectBinding,
  current: ImportWorkflowProjectBinding | null,
) {
  return current !== null && matchesProjectOccGuard({
    expectedProjectInstanceId: expected.project_instance_id,
    expectedProjectId: expected.project_id,
    expectedRevision: expected.revision,
  }, current)
}

export function createImportPreviewCleanupRegistry():
ImportPreviewCleanupRegistry {
  const cancellationById = new Map<string, Promise<void>>()
  const pendingIds = new Set<string>()
  const disposedIds: string[] = []

  function rememberDisposed(previewId: string) {
    if (disposedIds.includes(previewId)) return
    disposedIds.push(previewId)
    if (disposedIds.length > DISPOSED_IMPORT_PREVIEW_LIMIT) {
      disposedIds.shift()
    }
  }

  function cancelOnce(
    transport: ImportPreviewCancelTransport,
    previewId: string,
  ) {
    if (disposedIds.includes(previewId)) {
      pendingIds.delete(previewId)
      return Promise.resolve()
    }
    const existing = cancellationById.get(previewId)
    if (existing) return existing
    pendingIds.add(previewId)
    const cancellation = Promise.resolve()
      .then(() => transport(previewId))
      .then(() => {
        pendingIds.delete(previewId)
        rememberDisposed(previewId)
      })
      .finally(() => {
        if (cancellationById.get(previewId) === cancellation) {
          cancellationById.delete(previewId)
        }
      })
    cancellationById.set(previewId, cancellation)
    return cancellation
  }

  async function cancel(
    transport: ImportPreviewCancelTransport,
    ...previewIds: readonly (string | null)[]
  ) {
    const uniqueIds = [...new Set(previewIds.filter(
      (previewId): previewId is string => previewId !== null,
    ))]
    const results = await Promise.allSettled(
      uniqueIds.map((previewId) => cancelOnce(transport, previewId)),
    )
    let firstFailure: unknown | null = null
    results.forEach((result, index) => {
      if (result.status !== 'rejected') return
      const previewId = uniqueIds[index]
      if (
        previewId !== undefined
        && !disposedIds.includes(previewId)
      ) pendingIds.add(previewId)
      firstFailure ??= result.reason
    })
    return firstFailure
  }

  return Object.freeze({
    pendingIds: () => Object.freeze([...pendingIds]),
    hasDisposed: (previewId: string) => disposedIds.includes(previewId),
    settle: (previewId: string) => {
      pendingIds.delete(previewId)
      rememberDisposed(previewId)
    },
    cancel,
  })
}
