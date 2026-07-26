import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import type {
  FoldImportPreview,
  FoldImportSettings,
} from '../src/lib/foldImport.ts'
import {
  importWorkflowError,
  importWorkflowMessage,
} from '../src/lib/importWorkflowSupport.ts'
import {
  useFoldImportWorkflow,
  type FoldImportWorkflowCopy,
  type FoldImportWorkflowTransport,
} from '../src/lib/useFoldImportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const COPY: FoldImportWorkflowCopy = {
  missingPreview: localized('missing preview'),
  cancelled: localized('cancelled'),
  reviewReady: localized('review ready'),
  imported: localized('imported {name}'),
}

function localized(value: string) {
  return Object.freeze({ ja: value, en: value })
}

function project(
  revision = 7,
  projectInstanceId = 'instance-1',
  projectId = 'project-1',
  isDirty = false,
): ProjectSnapshot {
  return {
    project_instance_id: projectInstanceId,
    project_id: projectId,
    revision,
    is_dirty: isDirty,
    name: 'Current',
  } as ProjectSnapshot
}

function preview(importId = 'fold-import-1'): FoldImportPreview {
  return {
    import_id: importId,
    file_name: 'sample.fold',
    suggested_name: 'Sample',
    file_spec: '1.2',
    frame_unit: 'mm',
    default_mm_per_unit: 1,
    vertex_count: 3,
    edge_count: 3,
    boundary_edge_count: 3,
    boundary_candidates: [{
      id: 0,
      source: 'assigned_boundary',
      edge_indices: [0, 1, 2],
    }],
    fixed_boundary_candidate_id: 0,
    assignments: [{ assignment: 'M', count: 1 }],
    preview_vertices: [],
    preview_edges: [],
    preview_truncated: false,
    warnings: [],
  }
}

function settings(importId = 'fold-import-1'): FoldImportSettings {
  return {
    importId,
    name: 'Imported FOLD',
    mmPerUnit: 1,
    mappings: { M: 'mountain' },
    boundaryCandidateId: 0,
  }
}

function setup(options: Readonly<{
  current?: ProjectSnapshot | null
  preview?: FoldImportWorkflowTransport['preview']
  apply?: FoldImportWorkflowTransport['apply']
  cancel?: FoldImportWorkflowTransport['cancel']
  confirmReplace?: (message: string) => boolean
  onApplied?: (snapshot: ProjectSnapshot) => void
}> = {}) {
  let current = options.current === undefined ? project() : options.current
  let operationActive = false
  let fileOperation: 'fold_import' | null = null
  const previewRequest = vi.fn(options.preview ?? (async () => ({
    canceled: false,
    preview: preview(),
  })))
  const applyRequest = vi.fn(options.apply ?? (async () => ({
    ...project(0, 'instance-2', 'project-2'),
    name: 'Imported FOLD',
  })))
  const cancelRequest = vi.fn(options.cancel ?? (async () => undefined))
  const confirmReplace = vi.fn(options.confirmReplace ?? (() => true))
  const onApplied = vi.fn(options.onApplied ?? (() => undefined))
  const onStatus = vi.fn()
  const cancelInteraction = vi.fn()
  const focusSchedule = vi.fn((callback: () => void) => callback())
  const setOperationBusy = vi.fn((busy: boolean) => {
    operationActive = busy
  })
  const setFileOperation = vi.fn((operation: 'fold_import' | null) => {
    fileOperation = operation
  })
  const transport = {
    preview: previewRequest,
    apply: applyRequest,
    cancel: cancelRequest,
  } as unknown as FoldImportWorkflowTransport
  const hook = renderHook(() => useFoldImportWorkflow({
    locale: 'en',
    copy: COPY,
    getCurrentSnapshot: () => current,
    operationActive: () => operationActive,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
    onStatus,
    onApplied,
    transport,
    confirmReplace,
    scheduleFocus: focusSchedule,
  }))
  const button = document.createElement('button')
  const focus = vi.spyOn(button, 'focus')
  hook.result.current.buttonRef.current = button

  return {
    ...hook,
    previewRequest,
    applyRequest,
    cancelRequest,
    confirmReplace,
    onApplied,
    onStatus,
    cancelInteraction,
    setOperationBusy,
    setFileOperation,
    focusSchedule,
    focus,
    get operationActive() {
      return operationActive
    },
    get fileOperation() {
      return fileOperation
    },
    setCurrent(next: ProjectSnapshot | null) {
      current = next
    },
  }
}

describe('useFoldImportWorkflow', () => {
  it('previews under one operation and preserves exact marker/status behavior', async () => {
    const context = setup()

    act(() => {
      void context.result.current.begin()
    })
    expect(context.operationActive).toBe(true)
    expect(context.fileOperation).toBe('fold_import')
    expect(context.cancelInteraction).toHaveBeenCalledOnce()
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.previewRequest).toHaveBeenCalledOnce()
    expect(context.result.current.preview?.import_id).toBe('fold-import-1')
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.reviewReady),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(context.setFileOperation.mock.calls).toEqual([
      ['fold_import'],
      [null],
    ])
  })

  it('keeps picker cancellation and malformed preview failures fixed', async () => {
    const canceled = setup({
      preview: async () => ({ canceled: true, preview: null }),
    })
    act(() => {
      void canceled.result.current.begin()
    })
    await waitFor(() => expect(canceled.operationActive).toBe(false))
    expect(canceled.result.current.preview).toBeNull()
    expect(canceled.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.cancelled),
    )

    cleanup()
    const malformed = setup({
      preview: async () => ({ canceled: false, preview: null }),
    })
    act(() => {
      void malformed.result.current.begin()
    })
    await waitFor(() => expect(malformed.operationActive).toBe(false))
    expect(malformed.result.current.preview).toBeNull()
    expect(malformed.result.current.error).toEqual(
      importWorkflowError('fold_read_failed'),
    )
    expect(malformed.onStatus).toHaveBeenLastCalledWith(
      importWorkflowError('fold_read_failed'),
    )

    cleanup()
    const contradictory = setup({
      preview: async () => ({ canceled: true, preview: preview() }),
    })
    act(() => {
      void contradictory.result.current.begin()
    })
    await waitFor(() => expect(contradictory.operationActive).toBe(false))
    expect(contradictory.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'fold-import-1',
    )
    expect(contradictory.result.current.preview).toBeNull()
    expect(contradictory.result.current.error).toEqual(
      importWorkflowError('fold_read_failed'),
    )
  })

  it('cancels a stale preview and gates the next picker until cleanup succeeds', async () => {
    const pendingPreview = deferred<{
      canceled: boolean
      preview: FoldImportPreview | null
    }>()
    const cleanupRetry = deferred<void>()
    const cancelFailure = new Error('private cleanup path')
    const cancelRequest = vi.fn()
      .mockRejectedValueOnce(cancelFailure)
      .mockImplementationOnce(() => cleanupRetry.promise)
    let previewCount = 0
    const context = setup({
      preview: async () => {
        previewCount += 1
        return previewCount === 1
          ? pendingPreview.promise
          : { canceled: false, preview: preview('fold-import-2') }
      },
      cancel: cancelRequest,
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledOnce())
    context.setCurrent(project(7, 'instance-2'))

    await act(async () => {
      pendingPreview.resolve({ canceled: false, preview: preview() })
      await pendingPreview.promise
    })
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_cleanup_failed'),
    )
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'fold-import-1',
    )

    context.setCurrent(project())
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.cancelRequest).toHaveBeenCalledTimes(2))
    expect(context.previewRequest).toHaveBeenCalledOnce()

    await act(async () => {
      cleanupRetry.resolve(undefined)
      await cleanupRetry.promise
    })
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(context.result.current.preview?.import_id).toBe(
      'fold-import-2',
    ))
  })

  it('keeps the dialog and binding retryable when close cleanup fails', async () => {
    const cancelFailure = new Error('C:\\private\\sample.fold')
    const cancelRequest = vi.fn()
      .mockRejectedValueOnce(cancelFailure)
      .mockResolvedValueOnce(undefined)
    const context = setup({ cancel: cancelRequest })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_cleanup_failed'),
    )
    expect(context.focus).not.toHaveBeenCalled()
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')

    await act(() => context.result.current.close())
    expect(context.cancelRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.cancelled),
    )
  })

  it('rejects wrong opaque identity and project-instance ABA before apply', async () => {
    const context = setup()
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.apply(settings('other-import')))
    expect(context.applyRequest).not.toHaveBeenCalled()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_import_failed'),
    )

    context.setCurrent(project(7, 'instance-2'))
    await act(() => context.result.current.apply(settings()))
    expect(context.applyRequest).not.toHaveBeenCalled()
    expect(context.confirmReplace).not.toHaveBeenCalled()
    expect(context.result.current.preview).not.toBeNull()

    await act(() => context.result.current.close())
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'fold-import-1',
    )
    expect(context.result.current.preview).toBeNull()
  })

  it('rejects a disposed preview identity reused by a later native picker', async () => {
    const context = setup()
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.close())

    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.operationActive).toBe(false))

    expect(context.previewRequest).toHaveBeenCalledTimes(2)
    expect(context.cancelRequest).toHaveBeenCalledOnce()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_read_failed'),
    )
  })

  it('serializes close against apply while opaque cleanup is in flight', async () => {
    const pendingCancel = deferred<void>()
    const context = setup({
      cancel: async () => pendingCancel.promise,
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    act(() => {
      void context.result.current.close()
    })
    await waitFor(() => expect(context.cancelRequest).toHaveBeenCalledOnce())
    await act(() => context.result.current.apply(settings()))
    expect(context.applyRequest).not.toHaveBeenCalled()

    await act(async () => {
      pendingCancel.resolve(undefined)
      await pendingCancel.promise
    })
    expect(context.result.current.preview).toBeNull()
    expect(context.operationActive).toBe(false)
  })

  it('defers dirty confirmation and applies the exact preview-bound DTO', async () => {
    const callOrder: string[] = []
    const context = setup({
      current: project(7, 'instance-1', 'project-1', true),
      confirmReplace: () => {
        callOrder.push('confirm')
        return true
      },
      apply: async () => {
        callOrder.push('native')
        return {
          ...project(0, 'instance-2', 'project-2'),
          name: 'Imported FOLD',
        }
      },
      onApplied: () => {
        callOrder.push('snapshot')
      },
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.apply(settings()))

    expect(context.confirmReplace).toHaveBeenCalledOnce()
    expect(context.applyRequest).toHaveBeenCalledExactlyOnceWith(
      'project-1',
      7,
      settings(),
    )
    expect(callOrder).toEqual(['confirm', 'native', 'snapshot'])
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.imported, { name: 'Imported FOLD' }),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.cancelInteraction).toHaveBeenCalledTimes(2)
    expect(context.operationActive).toBe(false)
  })

  it('keeps a native apply failure retryable and clears every busy marker', async () => {
    const context = setup({
      apply: async () => {
        throw new Error('C:\\private\\import.fold')
      },
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.apply(settings()))

    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_import_failed'),
    )
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(context.focus).not.toHaveBeenCalled()
  })

  it('closes consumed authority but retains fixed failure when UI adoption throws', async () => {
    const context = setup({
      onApplied: () => {
        throw new Error('C:\\private\\ui-adoption.fold')
      },
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.apply(settings()))

    expect(context.applyRequest).toHaveBeenCalledOnce()
    expect(context.cancelRequest).not.toHaveBeenCalled()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('fold_import_failed'),
    )
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowError('fold_import_failed'),
    )
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
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
