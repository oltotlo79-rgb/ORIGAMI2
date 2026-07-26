import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  INSTRUCTION_EXPORT_PROFILE,
  INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  createInstructionExportError,
  type InstructionExportFormat,
  type InstructionExportPreview,
} from '../src/lib/instructionExport.ts'
import {
  instructionExportPreviewReadyMessage,
  instructionExportWorkflowErrorMessage,
  instructionExportWorkflowMessage,
} from '../src/lib/instructionExportWorkflowSupport.ts'
import {
  INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT,
  createInstructionExportCleanupRegistry,
} from '../src/lib/instructionExportCleanupRegistry.ts'
import {
  useInstructionExportWorkflow,
  type InstructionExportProjectBinding,
  type InstructionExportWorkflowCopy,
  type InstructionExportWorkflowTransport,
} from '../src/lib/useInstructionExportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const NEVER = new Promise<void>(() => undefined)
const COPY: InstructionExportWorkflowCopy = {
  previewReadyJapanese: localized('ready {format}'),
  previewReadyEnglish: localized('ready {format}'),
  prepareFailed: localized('prepare failed: {error}'),
  prepareStatusFailed: localized('prepare status: {error}'),
  progressFailed: localized('progress failed: {error}'),
  stopping: localized('stopping'),
  stopped: localized('stopped'),
  alreadyFinished: localized('already finished'),
  cancelled: localized('cancelled'),
  cancelFailed: localized('cancel failed: {error}'),
  cancelStatusFailed: localized('cancel status: {error}'),
  projectChanged: localized('project changed'),
  saveCancelledNotice: localized('save cancelled notice'),
  saveCancelledStatus: localized('save cancelled status'),
  saved: localized('saved {fileName}'),
  saveFailed: localized('save failed: {error}'),
  saveStatusFailed: localized('save status: {error}'),
}

function localized(english: string) {
  return Object.freeze({ ja: english, en: english })
}

function binding(
  revision = 7,
  projectInstanceId = 'instance-1',
  projectId = 'project-1',
): InstructionExportProjectBinding {
  return {
    project_instance_id: projectInstanceId,
    project_id: projectId,
    revision,
  }
}

function generationFixture(exportId = 'export-1') {
  return {
    export_id: exportId,
    profile: INSTRUCTION_EXPORT_PROFILE,
  } as const
}

function previewFixture(
  format: InstructionExportFormat = 'pdf',
  overrides: Partial<InstructionExportPreview> = {},
): InstructionExportPreview {
  return {
    export_id: 'export-1',
    expected_project_id: 'project-1',
    expected_revision: 7,
    format,
    profile: INSTRUCTION_EXPORT_PROFILE,
    projection_profile: INSTRUCTION_EXPORT_PROJECTION_PROFILE,
    format_summary: format === 'pdf' ? 'PDF 1.7' : 'SVG images ZIP',
    suggested_file_name: format === 'pdf'
      ? 'project-instructions.pdf'
      : 'project-instructions.zip',
    byte_count: 4096,
    step_count: 4,
    page_count: 4,
    caution_count: 0,
    warnings: [],
    ...overrides,
  }
}

function setup(options: Readonly<{
  current?: InstructionExportProjectBinding | null
  available?: boolean
  begin?: InstructionExportWorkflowTransport['begin']
  preview?: InstructionExportWorkflowTransport['preview']
  progress?: InstructionExportWorkflowTransport['progress']
  save?: InstructionExportWorkflowTransport['save']
  cancel?: InstructionExportWorkflowTransport['cancel']
  waitForPoll?: () => Promise<void>
}> = {}) {
  let current = options.current === undefined ? binding() : options.current
  let available = options.available ?? true
  let operationActive = false
  let fileOperation: 'instruction_export' | null = null
  const beginRequest = vi.fn(options.begin ?? (async () => generationFixture()))
  const previewRequest = vi.fn(options.preview ?? (async (
    exportId: string,
    _projectId: string,
    _revision: number,
    format: InstructionExportFormat,
  ) => ({
    preview: previewFixture(format, { export_id: exportId }),
  })))
  const progressRequest = vi.fn(options.progress ?? (async (exportId: string) => ({
    export_id: exportId,
    phase: 'ready' as const,
  })))
  const saveRequest = vi.fn(options.save ?? (async () => ({ canceled: false })))
  const cancelRequest = vi.fn(options.cancel ?? (async () => undefined))
  const setOperationBusy = vi.fn((busy: boolean) => {
    operationActive = busy
  })
  const setFileOperation = vi.fn((
    operation: 'instruction_export' | null,
  ) => {
    fileOperation = operation
  })
  const cancelInteraction = vi.fn()
  const onStatus = vi.fn()
  const waitForPoll = vi.fn(options.waitForPoll ?? (() => NEVER))
  const scheduleFocus = vi.fn((callback: () => void) => callback())
  const transport = {
    begin: beginRequest,
    preview: previewRequest,
    progress: progressRequest,
    save: saveRequest,
    cancel: cancelRequest,
  } as unknown as InstructionExportWorkflowTransport
  const hook = renderHook(() => useInstructionExportWorkflow({
    copy: COPY,
    getCurrentSnapshot: () => current,
    exportAvailable: () => available,
    operationActive: () => operationActive,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
    onStatus,
    transport,
    waitForPoll,
    scheduleFocus,
  }))
  const button = document.createElement('button')
  const focus = vi.spyOn(button, 'focus')
  hook.result.current.buttonRef.current = button

  return {
    ...hook,
    beginRequest,
    previewRequest,
    progressRequest,
    saveRequest,
    cancelRequest,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
    onStatus,
    waitForPoll,
    scheduleFocus,
    focus,
    get operationActive() {
      return operationActive
    },
    get fileOperation() {
      return fileOperation
    },
    setCurrent(value: InstructionExportProjectBinding | null) {
      current = value
    },
    setAvailable(value: boolean) {
      available = value
    },
    setOperationActive(value: boolean) {
      operationActive = value
    },
  }
}

describe('useInstructionExportWorkflow', () => {
  it('single-flights concurrent cleanup for one opaque export identity', async () => {
    const pending = deferred<void>()
    const cancelRequest = vi.fn(() => pending.promise)
    const registry = createInstructionExportCleanupRegistry()
    const first = registry.cancel(cancelRequest, 'export-1', 'export-1')
    const second = registry.discard(cancelRequest, 'export-1')
    await waitFor(() => expect(cancelRequest).toHaveBeenCalledOnce())
    expect(registry.pendingExportIds()).toEqual(['export-1'])

    pending.resolve(undefined)
    await Promise.all([first, second])

    expect(cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(registry.pendingExportIds()).toEqual([])
  })

  it('bounds completed tombstones without ever evicting unresolved cleanup', async () => {
    const registry = createInstructionExportCleanupRegistry()
    for (
      let index = 0;
      index < INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT;
      index += 1
    ) {
      registry.settle(`disposed-${index}`)
    }
    expect(registry.hasDisposed('disposed-0')).toBe(true)
    expect(registry.hasDisposed(
      `disposed-${INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT - 1}`,
    )).toBe(true)

    registry.settle(`disposed-${INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT}`)
    expect(registry.hasDisposed('disposed-0')).toBe(false)
    expect(registry.hasDisposed('disposed-1')).toBe(true)
    expect(registry.hasDisposed(
      `disposed-${INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT}`,
    )).toBe(true)

    const unresolvedIds = Array.from(
      { length: INSTRUCTION_EXPORT_DISPOSED_CACHE_LIMIT + 1 },
      (_, index) => `pending-${index}`,
    )
    const cleanupFailure = new Error('fixed cleanup failure')
    await registry.cancel(async () => {
      throw cleanupFailure
    }, ...unresolvedIds)
    expect(registry.pendingExportIds()).toEqual(unresolvedIds)
  })

  it('rejects a native identity reused after its lifetime was settled', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.save(true))

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.error).not.toBeNull())

    expect(context.beginRequest).toHaveBeenCalledTimes(2)
    expect(context.previewRequest).toHaveBeenCalledOnce()
    expect(context.cancelRequest).not.toHaveBeenCalled()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowErrorMessage(
        createInstructionExportError('document_contract_invalid'),
        COPY.prepareFailed,
      ),
    )
  })

  it('prepares PDF from the exact current binding under one owned operation', async () => {
    const context = setup()

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.beginRequest).toHaveBeenCalledOnce()
    expect(context.previewRequest).toHaveBeenCalledExactlyOnceWith(
      'export-1',
      'project-1',
      7,
      'pdf',
    )
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.format).toBe('pdf')
    expect(context.result.current.preview).toEqual(previewFixture())
    expect(context.result.current.phase).toBe('ready')
    expect(context.result.current.generationActive).toBe(false)
    expect(context.result.current.error).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.cancelInteraction).toHaveBeenCalledOnce()
    expect(context.setOperationBusy.mock.calls).toEqual([[true], [false]])
    expect(context.setFileOperation.mock.calls).toEqual([
      ['instruction_export'],
      [null],
    ])
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith(
      instructionExportPreviewReadyMessage('pdf', COPY),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('does not begin without a project, fold preview, or free operation lock', () => {
    const missingProject = setup({ current: null })
    act(() => missingProject.result.current.begin())
    expect(missingProject.result.current.open).toBe(false)
    expect(missingProject.beginRequest).not.toHaveBeenCalled()

    const missingPreview = setup({ available: false })
    act(() => missingPreview.result.current.begin())
    expect(missingPreview.result.current.open).toBe(false)
    expect(missingPreview.beginRequest).not.toHaveBeenCalled()

    const busy = setup()
    busy.setOperationActive(true)
    act(() => busy.result.current.begin())
    expect(busy.result.current.open).toBe(false)
    expect(busy.beginRequest).not.toHaveBeenCalled()
  })

  it('cancels a generation if the project instance changes before preview', async () => {
    const pending = deferred<ReturnType<typeof generationFixture>>()
    const context = setup({
      begin: (() => pending.promise) as InstructionExportWorkflowTransport['begin'],
    })
    act(() => context.result.current.begin())
    context.setCurrent(binding(7, 'instance-2'))

    await act(async () => {
      pending.resolve(generationFixture())
      await pending.promise
    })
    await waitFor(() => expect(context.cancelRequest).toHaveBeenCalledOnce())

    const projectChanged = createInstructionExportError('project_changed')
    expect(context.previewRequest).not.toHaveBeenCalled()
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowErrorMessage(
        projectChanged,
        COPY.prepareFailed,
      ),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('retains failed preview cleanup and blocks new generations until retry succeeds', async () => {
    const previewFailure = new Error('C:\\private\\preview-source.ori2')
    const cleanupFailure = new Error('C:\\private\\generated.pdf')
    const beginRequest = vi.fn()
      .mockResolvedValueOnce(generationFixture('export-1'))
      .mockResolvedValueOnce(generationFixture('export-2'))
    const previewRequest = vi.fn()
      .mockRejectedValueOnce(previewFailure)
      .mockResolvedValueOnce({
        preview: previewFixture('pdf', { export_id: 'export-2' }),
      })
    const cancelRequest = vi.fn()
      .mockRejectedValueOnce(cleanupFailure)
      .mockRejectedValueOnce(cleanupFailure)
      .mockResolvedValueOnce(undefined)
    const context = setup({
      begin: beginRequest as InstructionExportWorkflowTransport['begin'],
      preview: previewRequest as InstructionExportWorkflowTransport['preview'],
      cancel: cancelRequest as InstructionExportWorkflowTransport['cancel'],
    })

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.error).not.toBeNull())

    expect(context.beginRequest).toHaveBeenCalledOnce()
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(context.result.current.notice).toEqual(
      instructionExportWorkflowErrorMessage(
        cleanupFailure,
        COPY.cancelFailed,
      ),
    )
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
    expect(JSON.stringify(context.result.current.notice)).not.toContain('private')

    await act(() => context.result.current.prepare('pdf'))

    expect(context.beginRequest).toHaveBeenCalledOnce()
    expect(context.cancelRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).not.toBeNull()

    await act(() => context.result.current.prepare('pdf'))
    await waitFor(() =>
      expect(context.result.current.preview?.export_id).toBe('export-2'))

    expect(context.cancelRequest.mock.calls).toEqual([
      ['export-1'],
      ['export-1'],
      ['export-1'],
    ])
    expect(context.beginRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.notice).toBeNull()
    expect(context.operationActive).toBe(false)
  })

  it('cancels the old format once before replacement and ignores its late response', async () => {
    const firstPreview = deferred<{ preview: InstructionExportPreview }>()
    const beginRequest = vi.fn()
      .mockResolvedValueOnce(generationFixture('export-1'))
      .mockResolvedValueOnce(generationFixture('export-2'))
    const previewRequest = vi.fn()
      .mockImplementationOnce(() => firstPreview.promise)
      .mockResolvedValueOnce({
        preview: previewFixture('svg_zip', { export_id: 'export-2' }),
      })
    const context = setup({
      begin: beginRequest as InstructionExportWorkflowTransport['begin'],
      preview: previewRequest as InstructionExportWorkflowTransport['preview'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledOnce())

    context.setOperationActive(false)
    act(() => context.result.current.changeFormat('svg_zip'))
    await waitFor(() =>
      expect(context.result.current.preview?.export_id).toBe('export-2'))

    await act(async () => {
      firstPreview.resolve({
        preview: previewFixture('pdf', { export_id: 'export-1' }),
      })
      await firstPreview.promise
    })

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(context.beginRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.format).toBe('svg_zip')
    expect(context.result.current.preview?.export_id).toBe('export-2')
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith(
      instructionExportPreviewReadyMessage('svg_zip', COPY),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('fails closed when old-format cleanup fails and retries before replacement', async () => {
    const cleanupFailure = new Error('C:\\private\\old-export.pdf')
    const beginRequest = vi.fn()
      .mockResolvedValueOnce(generationFixture('export-1'))
      .mockResolvedValueOnce(generationFixture('export-2'))
    const previewRequest = vi.fn((
      exportId: string,
      _projectId: string,
      _revision: number,
      format: InstructionExportFormat,
    ) => Promise.resolve({
      preview: previewFixture(format, { export_id: exportId }),
    }))
    const cancelRequest = vi.fn()
      .mockRejectedValueOnce(cleanupFailure)
      .mockResolvedValueOnce(undefined)
    const context = setup({
      begin: beginRequest as InstructionExportWorkflowTransport['begin'],
      preview: previewRequest as InstructionExportWorkflowTransport['preview'],
      cancel: cancelRequest as InstructionExportWorkflowTransport['cancel'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    act(() => context.result.current.changeFormat('svg_zip'))
    await waitFor(() => expect(context.result.current.error).not.toBeNull())

    expect(context.beginRequest).toHaveBeenCalledOnce()
    expect(context.result.current.format).toBe('svg_zip')
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.notice).toEqual(
      instructionExportWorkflowErrorMessage(
        cleanupFailure,
        COPY.cancelFailed,
      ),
    )

    await act(() => context.result.current.prepare('svg_zip'))
    await waitFor(() =>
      expect(context.result.current.preview?.export_id).toBe('export-2'))

    expect(context.cancelRequest.mock.calls).toEqual([
      ['export-1'],
      ['export-1'],
    ])
    expect(context.beginRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.notice).toBeNull()
  })

  it('cancels both distinct opaque IDs from an invalid preview exactly once', async () => {
    const contractError = createInstructionExportError(
      'document_contract_invalid',
    )
    const context = setup({
      preview: async () => ({
        preview: previewFixture('pdf', { export_id: 'foreign-export' }),
      }),
    })

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.error).not.toBeNull())

    expect(context.cancelRequest.mock.calls).toEqual([
      ['foreign-export'],
      ['export-1'],
    ])
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowErrorMessage(
        contractError,
        COPY.prepareFailed,
      ),
    )
    expect(context.result.current.notice).toBeNull()
    expect(context.operationActive).toBe(false)
  })

  it('publishes bounded progress and stops polling at ready', async () => {
    const pendingPreview = deferred<{ preview: InstructionExportPreview }>()
    const progressRequest = vi.fn()
      .mockResolvedValueOnce({
        export_id: 'export-1',
        phase: 'analyzing_topology',
      })
      .mockResolvedValueOnce({
        export_id: 'export-1',
        phase: 'ready',
      })
    const context = setup({
      preview: (() => pendingPreview.promise) as InstructionExportWorkflowTransport['preview'],
      progress: progressRequest as InstructionExportWorkflowTransport['progress'],
      waitForPoll: async () => undefined,
    })
    act(() => context.result.current.begin())

    await waitFor(() => expect(context.progressRequest).toHaveBeenCalledTimes(2))
    expect(context.progressRequest.mock.calls).toEqual([
      ['export-1'],
      ['export-1'],
    ])
    expect(context.result.current.phase).toBe('ready')
    expect(context.result.current.generationActive).toBe(true)

    await act(async () => {
      pendingPreview.resolve({ preview: previewFixture() })
      await pendingPreview.promise
    })
    expect(context.result.current.generationActive).toBe(false)
    expect(context.result.current.preview).not.toBeNull()
  })

  it('keeps polling failures fixed and redacted while preview can still finish', async () => {
    const pendingPreview = deferred<{ preview: InstructionExportPreview }>()
    const progressFailure = new Error('C:\\private\\progress.log')
    const context = setup({
      preview: (() => pendingPreview.promise) as InstructionExportWorkflowTransport['preview'],
      progress: (async () => {
        throw progressFailure
      }) as InstructionExportWorkflowTransport['progress'],
      waitForPoll: async () => undefined,
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.notice).not.toBeNull())

    expect(context.result.current.notice).toEqual(
      instructionExportWorkflowErrorMessage(
        progressFailure,
        COPY.progressFailed,
      ),
    )
    expect(JSON.stringify(context.result.current.notice)).not.toContain('private')

    await act(async () => {
      pendingPreview.resolve({ preview: previewFixture() })
      await pendingPreview.promise
    })
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.notice).not.toBeNull()
    expect(context.operationActive).toBe(false)
  })

  it('stops active generation, restores focus, and deduplicates its late preview', async () => {
    const pendingPreview = deferred<{ preview: InstructionExportPreview }>()
    const pollWait = deferred<void>()
    const context = setup({
      preview: (() => pendingPreview.promise) as InstructionExportWorkflowTransport['preview'],
      waitForPoll: () => pollWait.promise,
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledOnce())

    await act(() => context.result.current.close())

    expect(context.result.current.open).toBe(false)
    expect(context.result.current.generationActive).toBe(false)
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(context.onStatus.mock.calls.slice(-2)).toEqual([
      [instructionExportWorkflowMessage(COPY.stopping)],
      [instructionExportWorkflowMessage(COPY.stopped)],
    ])
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()

    await act(async () => {
      pollWait.resolve(undefined)
      await pollWait.promise
    })
    expect(context.progressRequest).not.toHaveBeenCalled()

    await act(async () => {
      pendingPreview.resolve({ preview: previewFixture() })
      await pendingPreview.promise
    })
    expect(context.cancelRequest).toHaveBeenCalledOnce()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toBeNull()
  })

  it('retains failed active cleanup and admits no next generation until retry succeeds', async () => {
    const pendingPreview = deferred<{ preview: InstructionExportPreview }>()
    const cleanupRetry = deferred<void>()
    const cancelFailure = new Error('C:\\private\\active-generation.pdf')
    let generationCount = 0
    const context = setup({
      begin: async () => generationFixture(`export-${++generationCount}`),
      preview: (async (exportId, _projectId, _revision, format) => {
        if (exportId === 'export-1') return pendingPreview.promise
        return {
          preview: previewFixture(format, { export_id: exportId }),
        }
      }) as InstructionExportWorkflowTransport['preview'],
      cancel: vi.fn()
        .mockRejectedValueOnce(cancelFailure)
        .mockImplementationOnce(() => cleanupRetry.promise),
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledOnce())

    await act(() => context.result.current.close())

    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus.mock.calls.slice(-2)).toEqual([
      [instructionExportWorkflowMessage(COPY.stopping)],
      [instructionExportWorkflowMessage(COPY.alreadyFinished)],
    ])
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()

    context.beginRequest.mockClear()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.cancelRequest).toHaveBeenCalledTimes(2))
    expect(context.beginRequest).not.toHaveBeenCalled()

    await act(async () => {
      cleanupRetry.resolve(undefined)
      await cleanupRetry.promise
    })
    await waitFor(() => expect(context.beginRequest).toHaveBeenCalledOnce())
    await waitFor(() => expect(context.result.current.preview?.export_id).toBe(
      'export-2',
    ))

    await act(async () => {
      pendingPreview.resolve({ preview: previewFixture() })
      await pendingPreview.promise
    })
    expect(context.cancelRequest).toHaveBeenCalledTimes(2)
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.error).toBeNull()
  })

  it('cancels an admitted preview before closing', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-1')
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      instructionExportWorkflowMessage(COPY.cancelled),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
  })

  it('keeps failed close visible, retains binding, and allows a later save', async () => {
    const cancelFailure = new Error('C:\\private\\cancel-target.pdf')
    let generationCount = 0
    const context = setup({
      begin: async () => generationFixture(`export-${++generationCount}`),
      cancel: async () => {
        throw cancelFailure
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowErrorMessage(
        cancelFailure,
        COPY.cancelFailed,
      ),
    )
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
    expect(context.focus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)

    await act(() => context.result.current.save(true))

    expect(context.saveRequest).toHaveBeenCalledExactlyOnceWith(
      'export-1',
      'project-1',
      7,
      true,
    )
    expect(context.result.current.open).toBe(false)
    expect(context.focus).toHaveBeenCalledOnce()

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview?.export_id).toBe(
      'export-2',
    ))
    expect(context.beginRequest).toHaveBeenCalledTimes(2)
    expect(context.cancelRequest).toHaveBeenCalledOnce()
  })

  it('rejects an ABA project-instance replacement before save', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    context.setCurrent(binding(7, 'instance-2'))

    await act(() => context.result.current.save(true))

    expect(context.saveRequest).not.toHaveBeenCalled()
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowMessage(COPY.projectChanged),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('keeps a canceled save retryable and forwards exact warning authority', async () => {
    const saveRequest = vi.fn()
      .mockResolvedValueOnce({ canceled: true })
      .mockResolvedValueOnce({ canceled: false })
    const context = setup({
      save: saveRequest as InstructionExportWorkflowTransport['save'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    const admittedPreview = context.result.current.preview

    await act(() => context.result.current.save(false))

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).toBe(admittedPreview)
    expect(context.result.current.notice).toEqual(
      instructionExportWorkflowMessage(COPY.saveCancelledNotice),
    )
    expect(context.onStatus).toHaveBeenLastCalledWith(
      instructionExportWorkflowMessage(COPY.saveCancelledStatus),
    )
    expect(context.focus).not.toHaveBeenCalled()

    await act(() => context.result.current.save(true))

    expect(context.saveRequest.mock.calls).toEqual([
      ['export-1', 'project-1', 7, false],
      ['export-1', 'project-1', 7, true],
    ])
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      instructionExportWorkflowMessage(COPY.saved, {
        fileName: 'project-instructions.pdf',
      }),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.scheduleFocus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('keeps save failure retryable and clears every operation marker', async () => {
    const saveFailure = new Error('C:\\private\\save-target.pdf')
    const context = setup({
      save: async () => {
        throw saveFailure
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.save(true))

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      instructionExportWorkflowErrorMessage(
        saveFailure,
        COPY.saveFailed,
      ),
    )
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      instructionExportWorkflowErrorMessage(
        saveFailure,
        COPY.saveStatusFailed,
      ),
    )
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
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
