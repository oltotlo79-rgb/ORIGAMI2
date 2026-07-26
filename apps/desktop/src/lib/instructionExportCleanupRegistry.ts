export type InstructionExportCancelTransport =
  (exportId: string) => Promise<void>

export const INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT = 128

export type InstructionExportCleanupRegistry = Readonly<{
  pendingExportIds: () => readonly string[]
  hasPending: (exportId: string) => boolean
  hasDisposed: (exportId: string) => boolean
  settle: (...exportIds: readonly (string | null)[]) => void
  cancel: (
    transport: InstructionExportCancelTransport,
    ...exportIds: readonly (string | null)[]
  ) => Promise<unknown | null>
  discard: (
    transport: InstructionExportCancelTransport,
    ...exportIds: readonly string[]
  ) => Promise<void>
}>

export function createInstructionExportCleanupRegistry():
InstructionExportCleanupRegistry {
  const cancellationById = new Map<string, Promise<void>>()
  // Native generations use fresh UUID v4 lifetime identities. Recent
  // tombstones catch a contract violation fail-closed without unbounded
  // growth; a collision outside this hard bound is unreachable under that
  // native contract. Failed/in-flight cleanup lives separately and is never
  // evicted.
  const disposedExportIds: string[] = []
  const pendingCleanupIds = new Set<string>()

  function rememberDisposed(exportId: string) {
    if (disposedExportIds.includes(exportId)) return
    disposedExportIds.push(exportId)
    if (
      disposedExportIds.length > INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT
    ) disposedExportIds.shift()
  }

  function cancelOnce(
    transport: InstructionExportCancelTransport,
    exportId: string,
  ): Promise<void> {
    if (disposedExportIds.includes(exportId)) {
      pendingCleanupIds.delete(exportId)
      return Promise.resolve()
    }
    const pending = cancellationById.get(exportId)
    if (pending) return pending
    // Register cleanup before starting transport I/O. The active-close path
    // releases the UI operation lock while cancellation is still in flight,
    // so a new generation must discover and join this promise.
    pendingCleanupIds.add(exportId)
    const cancellation = Promise.resolve()
      .then(() => transport(exportId))
      .then(() => {
        rememberDisposed(exportId)
        pendingCleanupIds.delete(exportId)
      })
      .finally(() => {
        if (cancellationById.get(exportId) === cancellation) {
          cancellationById.delete(exportId)
        }
      })
    cancellationById.set(exportId, cancellation)
    return cancellation
  }

  async function cancel(
    transport: InstructionExportCancelTransport,
    ...exportIds: readonly (string | null)[]
  ) {
    const uniqueExportIds = [...new Set(exportIds.filter(
      (exportId): exportId is string => exportId !== null,
    ))]
    const results = await Promise.allSettled(uniqueExportIds.map(
      (exportId) => cancelOnce(transport, exportId),
    ))
    let firstFailure: unknown | null = null
    results.forEach((result, index) => {
      if (result.status !== 'rejected') return
      const exportId = uniqueExportIds[index]
      if (
        exportId !== undefined
        && !disposedExportIds.includes(exportId)
      ) pendingCleanupIds.add(exportId)
      // cancelOnce registered the opaque ID before transport I/O and removes
      // it only on success. Reassert it here in case unrelated explicit
      // settlement raced transport failure; a tombstone is the only authority
      // that may suppress the retry gate.
      firstFailure ??= result.reason
    })
    return firstFailure
  }

  return Object.freeze({
    pendingExportIds: () => Object.freeze([...pendingCleanupIds]),
    hasPending: (exportId: string) => pendingCleanupIds.has(exportId),
    hasDisposed: (exportId: string) => disposedExportIds.includes(exportId),
    settle: (...exportIds: readonly (string | null)[]) => {
      for (const exportId of exportIds) {
        if (exportId === null) continue
        pendingCleanupIds.delete(exportId)
        rememberDisposed(exportId)
      }
    },
    cancel,
    discard: async (
      transport: InstructionExportCancelTransport,
      ...exportIds: readonly string[]
    ) => {
      await cancel(transport, ...exportIds)
    },
  })
}
