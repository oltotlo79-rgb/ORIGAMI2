import { describe, expect, it, vi } from 'vitest'

import {
  createImportPreviewCleanupRegistry,
} from '../src/lib/importWorkflowSupport.ts'

describe('import preview cleanup registry', () => {
  it('single-flights in-flight cleanup and retains failures until retry succeeds', async () => {
    const pending = deferred<void>()
    const cleanupFailure = new Error('cleanup failed')
    const cancel = vi.fn()
      .mockImplementationOnce(() => pending.promise)
      .mockRejectedValueOnce(cleanupFailure)
      .mockResolvedValueOnce(undefined)
    const registry = createImportPreviewCleanupRegistry()

    const first = registry.cancel(cancel, 'preview-1', 'preview-1')
    const joined = registry.cancel(cancel, 'preview-1')
    expect(registry.pendingIds()).toEqual(['preview-1'])
    expect(cancel).not.toHaveBeenCalled()

    pending.resolve(undefined)
    await Promise.all([first, joined])
    expect(cancel).toHaveBeenCalledExactlyOnceWith('preview-1')
    expect(registry.pendingIds()).toEqual([])

    const failure = await registry.cancel(cancel, 'preview-2')
    expect(failure).toBe(cleanupFailure)
    expect(registry.pendingIds()).toEqual(['preview-2'])

    expect(await registry.cancel(cancel, ...registry.pendingIds())).toBeNull()
    expect(cancel).toHaveBeenLastCalledWith('preview-2')
    expect(registry.pendingIds()).toEqual([])
  })

  it('settles a successfully consumed preview without issuing cancel I/O', async () => {
    const cancel = vi.fn(async () => undefined)
    const registry = createImportPreviewCleanupRegistry()
    registry.settle('preview-1')

    expect(registry.hasDisposed('preview-1')).toBe(true)
    expect(await registry.cancel(cancel, 'preview-1')).toBeNull()
    expect(cancel).not.toHaveBeenCalled()
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
