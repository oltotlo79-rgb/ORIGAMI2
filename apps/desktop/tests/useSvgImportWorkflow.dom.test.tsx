import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import {
  formatLocalizedText,
} from '../src/lib/i18n.ts'
import {
  importWorkflowError,
  importWorkflowMessage,
} from '../src/lib/importWorkflowSupport.ts'
import type {
  SvgImportPreview,
  SvgImportSettings,
  SvgImportSettingsDraft,
  SvgImportSettingsValidation,
} from '../src/lib/svgImport.ts'
import {
  useSvgImportWorkflow,
  type SvgImportWorkflowCopy,
  type SvgImportWorkflowTransport,
} from '../src/lib/useSvgImportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const COPY: SvgImportWorkflowCopy = {
  missingPreview: localized('missing preview'),
  cancelled: localized('cancelled'),
  reviewReady: localized('review ready'),
  validationReadyJapanese: localized('validated {width} x {height}'),
  validationReadyEnglish: localized('validated {width} x {height}'),
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

function preview(importId = 'svg-import-1'): SvgImportPreview {
  return {
    import_id: importId,
    file_name: 'sample.svg',
    suggested_name: 'Sample',
    default_mm_per_unit: 1,
    root_view_box: { x: 0, y: 0, width: 100, height: 80 },
    root_physical_size: {
      width_millimetres: 100,
      height_millimetres: 80,
      width_unit: 'mm',
      height_unit: 'mm',
    },
    source_segment_count: 4,
    style_groups: [{
      group_id: 0,
      element_count: 1,
      segment_count: 4,
      stroke: '#000000',
      stroke_color: '#000000',
      dash_array: null,
      line_cap: 'round',
      classes: [],
      layer: null,
      representative_id: null,
      semantic_hint: 'boundary',
    }],
    boundary_candidates: [],
    preview_vertices: [],
    preview_edges: [],
    preview_truncated: false,
    warnings: [],
  }
}

function draft(
  importId = 'svg-import-1',
  mappings = { '0': 'boundary' as const },
): SvgImportSettingsDraft {
  return {
    importId,
    mmPerUnit: 1,
    boundaryCandidateId: null,
    mappings,
  }
}

function validation(
  overrides: Partial<SvgImportSettingsValidation> = {},
): SvgImportSettingsValidation {
  return {
    validation_id: 'validation-1',
    preview_id: 'svg-import-1',
    expected_project_id: 'project-1',
    expected_revision: 7,
    millimeters_per_unit: 1,
    boundary_candidate_id: null,
    width_mm: 100,
    height_mm: 80,
    has_cuts: false,
    ...overrides,
  }
}

function settings(
  overrides: Partial<SvgImportSettings> = {},
): SvgImportSettings {
  return {
    importId: 'svg-import-1',
    validationId: 'validation-1',
    name: 'Imported SVG',
    mmPerUnit: 1,
    boundaryCandidateId: null,
    boundaryConfirmed: true,
    mappings: { '0': 'boundary' },
    warningsAcknowledged: true,
    cuttingAllowedConfirmed: false,
    ...overrides,
  }
}

function setup(options: Readonly<{
  current?: ProjectSnapshot | null
  preview?: SvgImportWorkflowTransport['preview']
  validate?: SvgImportWorkflowTransport['validate']
  apply?: SvgImportWorkflowTransport['apply']
  cancel?: SvgImportWorkflowTransport['cancel']
  confirmReplace?: (message: string) => boolean
  onApplied?: (snapshot: ProjectSnapshot) => void
}> = {}) {
  let current = options.current === undefined ? project() : options.current
  let operationActive = false
  let fileOperation: 'svg_import' | null = null
  const previewRequest = vi.fn(options.preview ?? (async () => ({
    canceled: false,
    preview: preview(),
  })))
  const validateRequest = vi.fn(options.validate ?? (async () => validation()))
  const applyRequest = vi.fn(options.apply ?? (async () => ({
    ...project(0, 'instance-2', 'project-2'),
    name: 'Imported SVG',
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
  const setFileOperation = vi.fn((operation: 'svg_import' | null) => {
    fileOperation = operation
  })
  const transport = {
    preview: previewRequest,
    validate: validateRequest,
    apply: applyRequest,
    cancel: cancelRequest,
  } as unknown as SvgImportWorkflowTransport
  const hook = renderHook(() => useSvgImportWorkflow({
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
    validateRequest,
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

describe('useSvgImportWorkflow', () => {
  it('previews under exact SVG markers and handles picker cancellation', async () => {
    const context = setup()
    act(() => {
      void context.result.current.begin()
    })
    expect(context.operationActive).toBe(true)
    expect(context.fileOperation).toBe('svg_import')
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.previewRequest).toHaveBeenCalledOnce()
    expect(context.cancelInteraction).toHaveBeenCalledOnce()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.reviewReady),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()

    cleanup()
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
    const contradictory = setup({
      preview: async () => ({ canceled: true, preview: preview() }),
    })
    act(() => {
      void contradictory.result.current.begin()
    })
    await waitFor(() => expect(contradictory.operationActive).toBe(false))
    expect(contradictory.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'svg-import-1',
    )
    expect(contradictory.result.current.preview).toBeNull()
    expect(contradictory.result.current.error).toEqual(
      importWorkflowError('svg_read_failed'),
    )
  })

  it('rejects and cleans a preview returned for a replaced project instance', async () => {
    const pending = deferred<{
      canceled: boolean
      preview: SvgImportPreview | null
    }>()
    const context = setup({
      preview: async () => pending.promise,
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.previewRequest).toHaveBeenCalledOnce())
    context.setCurrent(project(7, 'instance-2'))

    await act(async () => {
      pending.resolve({ canceled: false, preview: preview() })
      await pending.promise
    })

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'svg-import-1',
    )
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_read_failed'),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('validates the exact preview binding and stores bounded authority', async () => {
    const context = setup()
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.validate(draft()))

    expect(context.validateRequest).toHaveBeenCalledExactlyOnceWith(
      'project-1',
      7,
      draft(),
    )
    expect(context.result.current.validation).toEqual(validation())
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(Object.freeze({
        ja: formatLocalizedText('ja', COPY.validationReadyJapanese, {
          width: (100).toLocaleString('ja'),
          height: (80).toLocaleString('ja'),
        }),
        en: formatLocalizedText('en', COPY.validationReadyEnglish, {
          width: (100).toLocaleString('en'),
          height: (80).toLocaleString('en'),
        }),
      })),
    )
    expect(context.operationActive).toBe(false)
  })

  it('rejects malformed or stale validation responses without granting apply authority', async () => {
    const context = setup({
      validate: async () => validation({
        expected_revision: 8,
        width_mm: Number.POSITIVE_INFINITY,
      }),
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.validate(draft()))
    expect(context.result.current.validation).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_boundary_validation_failed'),
    )

    await act(() => context.result.current.apply(settings()))
    expect(context.applyRequest).not.toHaveBeenCalled()
  })

  it('invalidates a late validation response and never revives its authority', async () => {
    const pending = deferred<SvgImportSettingsValidation>()
    const context = setup({
      validate: async () => pending.promise,
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    act(() => {
      void context.result.current.validate(draft())
    })
    await waitFor(() => expect(context.validateRequest).toHaveBeenCalledOnce())

    act(() => context.result.current.invalidateValidation())
    await act(async () => {
      pending.resolve(validation())
      await pending.promise
    })

    expect(context.result.current.validation).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.operationActive).toBe(false)
    await act(() => context.result.current.apply(settings()))
    expect(context.applyRequest).not.toHaveBeenCalled()
  })

  it('keeps preview and validation retryable when close cleanup fails', async () => {
    const cancelRequest = vi.fn()
      .mockRejectedValueOnce(new Error('C:\\private\\sample.svg'))
      .mockResolvedValueOnce(undefined)
    const context = setup({ cancel: cancelRequest })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.validate(draft()))

    await act(() => context.result.current.close())
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.validation).toEqual(validation())
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_cleanup_failed'),
    )
    expect(context.focus).not.toHaveBeenCalled()
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')

    await act(() => context.result.current.close())
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.validation).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.focus).toHaveBeenCalledOnce()
  })

  it('rejects changed mapping, validation token, and project ABA before apply', async () => {
    const context = setup()
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.validate(draft()))

    await act(() => context.result.current.apply(settings({
      mappings: { '0': 'mountain' },
    })))
    await act(() => context.result.current.apply(settings({
      validationId: 'validation-2',
    })))
    context.setCurrent(project(7, 'instance-2'))
    await act(() => context.result.current.apply(settings()))

    expect(context.applyRequest).not.toHaveBeenCalled()
    expect(context.confirmReplace).not.toHaveBeenCalled()
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_import_failed'),
    )

    await act(() => context.result.current.close())
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'svg-import-1',
    )
    expect(context.result.current.preview).toBeNull()
  })

  it('rejects a disposed SVG preview identity reused by a later picker', async () => {
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
      importWorkflowError('svg_read_failed'),
    )
  })

  it('defers dirty confirmation and applies the exact validated DTO', async () => {
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
          name: 'Imported SVG',
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
    await act(() => context.result.current.validate(draft()))

    await act(() => context.result.current.apply(settings()))

    expect(context.confirmReplace).toHaveBeenCalledOnce()
    expect(context.applyRequest).toHaveBeenCalledExactlyOnceWith(
      'project-1',
      7,
      settings(),
      true,
    )
    expect(callOrder).toEqual(['confirm', 'native', 'snapshot'])
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.validation).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowMessage(COPY.imported, { name: 'Imported SVG' }),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.cancelInteraction).toHaveBeenCalledTimes(2)
    expect(context.operationActive).toBe(false)
  })

  it('keeps native apply failures visible and retryable with no raw details', async () => {
    const context = setup({
      apply: async () => {
        throw new Error('C:\\private\\import.svg')
      },
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.validate(draft()))

    await act(() => context.result.current.apply(settings()))

    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.validation).toEqual(validation())
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_import_failed'),
    )
    expect(JSON.stringify(context.onStatus.mock.calls)).not.toContain('private')
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(context.focus).not.toHaveBeenCalled()
  })

  it('closes consumed SVG authority but retains fixed failure when UI adoption throws', async () => {
    const context = setup({
      onApplied: () => {
        throw new Error('C:\\private\\ui-adoption.svg')
      },
    })
    act(() => {
      void context.result.current.begin()
    })
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    await act(() => context.result.current.validate(draft()))

    await act(() => context.result.current.apply(settings()))

    expect(context.applyRequest).toHaveBeenCalledOnce()
    expect(context.cancelRequest).not.toHaveBeenCalled()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.validation).toBeNull()
    expect(context.result.current.error).toEqual(
      importWorkflowError('svg_import_failed'),
    )
    expect(context.onStatus).toHaveBeenLastCalledWith(
      importWorkflowError('svg_import_failed'),
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
