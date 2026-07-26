import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type {
  CreasePatternExportFormat,
  CreasePatternExportPreview,
} from '../src/lib/creaseExport.ts'
import {
  useCreaseExportWorkflow,
  type CreaseExportProjectBinding,
  type CreaseExportWorkflowCopy,
  type CreaseExportWorkflowMessage,
  type CreaseExportWorkflowTransport,
} from '../src/lib/useCreaseExportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const PREPARE_FAILED = workflowMessage('prepare failed')
const CLEANUP_FAILED = workflowMessage('cleanup failed')
const SAVE_FAILED = workflowMessage('save failed')
const CANCELLED = workflowMessage('cancelled')
const PROJECT_CHANGED = workflowMessage('project changed')
const SAVE_CANCELLED_NOTICE = workflowMessage('save cancelled notice')
const SAVE_CANCELLED_STATUS = workflowMessage('save cancelled status')
const COPY: CreaseExportWorkflowCopy = {
  previewRejected: localized('preview rejected'),
  previewReadyJapanese: localized('preview ready: {format}'),
  previewReadyEnglish: localized('preview ready: {format}'),
  cancelled: CANCELLED.text,
  projectChanged: PROJECT_CHANGED.text,
  saveCancelledNotice: SAVE_CANCELLED_NOTICE.text,
  saveCancelledStatus: SAVE_CANCELLED_STATUS.text,
}

function localized(english: string) {
  return Object.freeze({ ja: english, en: english })
}

function workflowMessage(
  english: string,
  variables?: Readonly<Record<string, string | number>>,
): CreaseExportWorkflowMessage {
  return Object.freeze({
    text: localized(english),
    variables,
  })
}

function binding(
  revision = 7,
  projectId = 'project-1',
): CreaseExportProjectBinding {
  return { project_id: projectId, revision }
}

function previewFixture(
  format: CreasePatternExportFormat = 'fold',
  overrides: Partial<CreasePatternExportPreview> = {},
): CreasePatternExportPreview {
  return {
    export_id: `export-${format}`,
    expected_project_id: 'project-1',
    expected_revision: 7,
    format,
    format_summary: format.toUpperCase(),
    suggested_file_name: `project.${format}`,
    byte_count: 128,
    vertex_count: 4,
    edge_count: 5,
    assignment_counts: {
      boundary: 4,
      mountain: 1,
      valley: 0,
      auxiliary: 0,
      cut: 0,
    },
    has_cuts: false,
    warnings: [],
    ...overrides,
  }
}

function setup(options: Readonly<{
  current?: CreaseExportProjectBinding | null
  preview?: CreaseExportWorkflowTransport['preview']
  save?: CreaseExportWorkflowTransport['save']
  cancel?: CreaseExportWorkflowTransport['cancel']
}> = {}) {
  let current = options.current === undefined ? binding() : options.current
  let operationActive = false
  let fileOperation: 'crease_export' | null = null
  const previewRequest = vi.fn(options.preview ?? (async (
    _projectId: string,
    _revision: number,
    format: CreasePatternExportFormat,
  ) => ({ preview: previewFixture(format) })))
  const saveRequest = vi.fn(options.save ?? (async () => ({ canceled: false })))
  const cancelRequest = vi.fn(options.cancel ?? (async () => undefined))
  const setOperationBusy = vi.fn((busy: boolean) => {
    operationActive = busy
  })
  const setFileOperation = vi.fn((operation: 'crease_export' | null) => {
    fileOperation = operation
  })
  const cancelInteraction = vi.fn()
  const onStatus = vi.fn()
  const savedMessage = vi.fn((preview: CreasePatternExportPreview) =>
    workflowMessage('saved', { fileName: preview.suggested_file_name }))
  const scheduleFocus = vi.fn((callback: () => void) => callback())
  const transport = {
    preview: previewRequest,
    save: saveRequest,
    cancel: cancelRequest,
  } as unknown as CreaseExportWorkflowTransport
  const hook = renderHook(() => useCreaseExportWorkflow({
    locale: 'en',
    copy: COPY,
    getCurrentSnapshot: () => current,
    operationActive: () => operationActive,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
    onStatus,
    prepareFailedMessage: PREPARE_FAILED,
    cleanupFailedMessage: CLEANUP_FAILED,
    saveFailedMessage: SAVE_FAILED,
    savedMessage,
    transport,
    scheduleFocus,
  }))
  const button = document.createElement('button')
  const focus = vi.spyOn(button, 'focus')
  hook.result.current.buttonRef.current = button

  return {
    ...hook,
    previewRequest,
    saveRequest,
    cancelRequest,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
    onStatus,
    savedMessage,
    scheduleFocus,
    focus,
    get operationActive() {
      return operationActive
    },
    get fileOperation() {
      return fileOperation
    },
    setCurrent(value: CreaseExportProjectBinding | null) {
      current = value
    },
    setOperationActive(value: boolean) {
      operationActive = value
    },
  }
}

describe('useCreaseExportWorkflow', () => {
  it('prepares the default format under one owned operation', async () => {
    const context = setup()

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.previewRequest).toHaveBeenCalledExactlyOnceWith(
      'project-1',
      7,
      'fold',
    )
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.format).toBe('fold')
    expect(context.result.current.preview?.export_id).toBe('export-fold')
    expect(context.result.current.error).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.cancelInteraction).toHaveBeenCalledOnce()
    expect(context.setFileOperation.mock.calls).toEqual([
      ['crease_export'],
      [null],
    ])
    expect(context.setOperationBusy.mock.calls).toEqual([[true], [false]])
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(context.onStatus).toHaveBeenCalledOnce()
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith(
      workflowMessage('preview ready: FOLD 1.2'),
    )
  })

  it('cancels a mismatched preview and releases every operation flag', async () => {
    const privateFailure = new Error('C:\\private\\project.ori2')
    const context = setup({
      preview: async () => ({
        preview: previewFixture('fold', {
          export_id: 'mismatched',
          expected_revision: 8,
        }),
      }),
      cancel: async () => {
        throw privateFailure
      },
    })

    act(() => context.result.current.begin())
    await waitFor(() =>
      expect(context.result.current.error).toBe(PREPARE_FAILED))

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('mismatched')
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith(PREPARE_FAILED)
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
  })

  it('lets only the newest request publish and cancels a late stale preview', async () => {
    const first = deferred<{ preview: CreasePatternExportPreview }>()
    const previewRequest = vi.fn()
      .mockImplementationOnce(() => first.promise)
      .mockResolvedValueOnce({ preview: previewFixture('svg') })
    const context = setup({
      preview: previewRequest as CreaseExportWorkflowTransport['preview'],
    })

    act(() => context.result.current.begin())
    expect(context.operationActive).toBe(true)

    context.setOperationActive(false)
    act(() => context.result.current.changeFormat('svg'))
    await waitFor(() =>
      expect(context.result.current.preview?.format).toBe('svg'))

    await act(async () => {
      first.resolve({
        preview: previewFixture('fold', { export_id: 'late-fold' }),
      })
      await first.promise
    })

    expect(context.result.current.preview?.format).toBe('svg')
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('late-fold')
    expect(context.onStatus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('rejects stale save binding without invoking the transport', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    context.setCurrent(binding(8))

    await act(() => context.result.current.save(true))

    expect(context.saveRequest).not.toHaveBeenCalled()
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(PROJECT_CHANGED)
    expect(context.setOperationBusy).toHaveBeenCalledTimes(2)
  })

  it('keeps a canceled save retryable and closes only after a later success', async () => {
    const saveRequest = vi.fn()
      .mockResolvedValueOnce({ canceled: true })
      .mockResolvedValueOnce({ canceled: false })
    const context = setup({
      save: saveRequest as CreaseExportWorkflowTransport['save'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.save(false))

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.notice).not.toBeNull()
    expect(context.focus).not.toHaveBeenCalled()

    await act(() => context.result.current.save(true))

    expect(context.saveRequest.mock.calls).toEqual([
      ['export-fold', 'project-1', 7, false],
      ['export-fold', 'project-1', 7, true],
    ])
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.savedMessage).toHaveBeenCalledExactlyOnceWith(
      previewFixture('fold'),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.scheduleFocus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('cancels an admitted preview before closing and restores toolbar focus', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-fold')
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(CANCELLED)
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.scheduleFocus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
  })

  it('keeps failed cleanup visible and releases the shared operation lock', async () => {
    const context = setup({
      cancel: async () => {
        throw new Error('native cleanup detail')
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toBe(CLEANUP_FAILED)
    expect(context.onStatus).toHaveBeenLastCalledWith(CLEANUP_FAILED)
    expect(context.focus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
  })

  it('keeps failed saves retryable and clears the file-operation marker', async () => {
    const context = setup({
      save: async () => {
        throw new Error('C:\\private\\save-target.fold')
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.save(true))

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toBe(SAVE_FAILED)
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(SAVE_FAILED)
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((accept) => {
    resolve = accept
  })
  return { promise, resolve }
}
