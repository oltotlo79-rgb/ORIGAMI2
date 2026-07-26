import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { MeshAnimationPreviewResponse } from '../src/lib/meshAnimationExport.ts'
import {
  useMeshAnimationExportWorkflow,
  type MeshAnimationExportProjectBinding,
  type MeshAnimationExportWorkflowCopy,
  type MeshAnimationExportWorkflowMessage,
  type MeshAnimationExportWorkflowTransport,
} from '../src/lib/useMeshAnimationExportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const COPY: MeshAnimationExportWorkflowCopy = {
  prepareFailed: localized('prepare failed'),
  cleanupFailed: localized('cleanup failed'),
  projectChanged: localized('project changed'),
  saveCancelledNotice: localized('save cancelled notice'),
  saved: localized('saved {fileName}'),
  saveFailed: localized('save failed'),
}

function localized(english: string) {
  return Object.freeze({ ja: english, en: english })
}

function workflowMessage(
  text: keyof typeof COPY,
  variables?: Readonly<Record<string, string | number>>,
): MeshAnimationExportWorkflowMessage {
  return Object.freeze({ text: COPY[text], variables })
}

function binding(
  revision = 7,
  projectInstanceId = 'instance-1',
  projectId = 'project-1',
): MeshAnimationExportProjectBinding {
  return {
    project_instance_id: projectInstanceId,
    project_id: projectId,
    revision,
  }
}

function previewFixture(
  overrides: Partial<MeshAnimationPreviewResponse> = {},
): MeshAnimationPreviewResponse {
  return {
    exportId: 'animation-export-1',
    projectInstanceId: 'instance-1',
    projectId: 'project-1',
    revision: 7,
    sourceFingerprint: 'a'.repeat(64),
    frameCount: 12,
    vertexCount: 16,
    triangleCount: 8,
    durationSeconds: 2.5,
    byteCount: 4096,
    mediaType: 'model/gltf-binary',
    fileExtension: 'glb',
    suggestedFileName: 'project-animation.glb',
    ...overrides,
  }
}

function setup(options: Readonly<{
  current?: MeshAnimationExportProjectBinding | null
  preview?: MeshAnimationExportWorkflowTransport['preview']
  save?: MeshAnimationExportWorkflowTransport['save']
  cancel?: MeshAnimationExportWorkflowTransport['cancel']
}> = {}) {
  let current = options.current === undefined ? binding() : options.current
  let operationActive = false
  let fileOperation: 'mesh_animation_export' | null = null
  const previewRequest = vi.fn(options.preview ?? (async () => previewFixture()))
  const saveRequest = vi.fn(options.save ?? (async () => ({ canceled: false })))
  const cancelRequest = vi.fn(options.cancel ?? (async () => undefined))
  const setOperationBusy = vi.fn((busy: boolean) => {
    operationActive = busy
  })
  const setFileOperation = vi.fn((
    operation: 'mesh_animation_export' | null,
  ) => {
    fileOperation = operation
  })
  const onStatus = vi.fn()
  const scheduleFocus = vi.fn((callback: () => void) => callback())
  const transport = {
    preview: previewRequest,
    save: saveRequest,
    cancel: cancelRequest,
  } as unknown as MeshAnimationExportWorkflowTransport
  const hook = renderHook(() => useMeshAnimationExportWorkflow({
    copy: COPY,
    getCurrentSnapshot: () => current,
    operationActive: () => operationActive,
    setOperationBusy,
    setFileOperation,
    onStatus,
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
    onStatus,
    scheduleFocus,
    focus,
    get operationActive() {
      return operationActive
    },
    get fileOperation() {
      return fileOperation
    },
    setCurrent(value: MeshAnimationExportProjectBinding | null) {
      current = value
    },
    setOperationActive(value: boolean) {
      operationActive = value
    },
  }
}

describe('useMeshAnimationExportWorkflow', () => {
  it('prepares from the exact current project identity and releases its markers', async () => {
    const context = setup()

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.previewRequest).toHaveBeenCalledExactlyOnceWith({
      expectedProjectInstanceId: 'instance-1',
      expectedProjectId: 'project-1',
      expectedRevision: 7,
    })
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).toEqual(previewFixture())
    expect(context.result.current.error).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.setFileOperation.mock.calls).toEqual([
      ['mesh_animation_export'],
      [null],
    ])
    expect(context.setOperationBusy.mock.calls).toEqual([[true], [false]])
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('does not begin without a current project or while another operation owns the lock', () => {
    const missing = setup({ current: null })
    act(() => missing.result.current.begin())
    expect(missing.result.current.open).toBe(false)
    expect(missing.previewRequest).not.toHaveBeenCalled()

    const busy = setup()
    busy.setOperationActive(true)
    act(() => busy.result.current.begin())
    expect(busy.result.current.open).toBe(false)
    expect(busy.previewRequest).not.toHaveBeenCalled()
  })

  it('cancels a preview when the project is replaced while preparation is pending', async () => {
    const pending = deferred<MeshAnimationPreviewResponse>()
    const context = setup({
      preview: (() => pending.promise) as MeshAnimationExportWorkflowTransport['preview'],
      cancel: async () => {
        throw new Error('C:\\private\\project-animation.glb')
      },
    })
    act(() => context.result.current.begin())
    context.setCurrent(binding(7, 'instance-2'))

    await act(async () => {
      pending.resolve(previewFixture())
      await pending.promise
    })

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'animation-export-1',
    )
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('prepareFailed'))
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('prepareFailed'),
    )
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('publishes only the newest preparation and cancels a late response', async () => {
    const first = deferred<MeshAnimationPreviewResponse>()
    const previewRequest = vi.fn()
      .mockImplementationOnce(() => first.promise)
      .mockResolvedValueOnce(previewFixture({
        exportId: 'animation-export-2',
      }))
    const context = setup({
      preview: previewRequest as MeshAnimationExportWorkflowTransport['preview'],
    })
    act(() => context.result.current.begin())
    context.setOperationActive(false)
    act(() => {
      void context.result.current.prepare()
    })
    await waitFor(() =>
      expect(context.result.current.preview?.exportId).toBe('animation-export-2'))

    await act(async () => {
      first.resolve(previewFixture({ exportId: 'late-animation-export' }))
      await first.promise
    })

    expect(context.result.current.preview?.exportId).toBe('animation-export-2')
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'late-animation-export',
    )
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('rejects a stale save binding without invoking the native transport', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    context.setCurrent(binding(8))

    await act(() => context.result.current.save())

    expect(context.saveRequest).not.toHaveBeenCalled()
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('projectChanged'))
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('saves with the admitted export identity and keeps cancellation retryable', async () => {
    const saveRequest = vi.fn()
      .mockResolvedValueOnce({ canceled: true })
      .mockResolvedValueOnce({ canceled: false })
    const context = setup({
      save: saveRequest as MeshAnimationExportWorkflowTransport['save'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    const admittedPreview = context.result.current.preview

    await act(() => context.result.current.save())

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).toBe(admittedPreview)
    expect(context.result.current.notice).toEqual(
      workflowMessage('saveCancelledNotice'),
    )
    expect(context.focus).not.toHaveBeenCalled()

    await act(() => context.result.current.save())

    const expectedRequest = {
      exportId: 'animation-export-1',
      expectedProjectInstanceId: 'instance-1',
      expectedProjectId: 'project-1',
      expectedRevision: 7,
      expectedSourceFingerprint: 'a'.repeat(64),
    }
    expect(context.saveRequest.mock.calls).toEqual([
      [expectedRequest],
      [expectedRequest],
    ])
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('saved', { fileName: 'project-animation.glb' }),
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

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith(
      'animation-export-1',
    )
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.result.current.notice).toBeNull()
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.scheduleFocus).toHaveBeenCalledOnce()
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('keeps cleanup failure visible and releases the shared operation lock', async () => {
    const context = setup({
      cancel: async () => {
        throw new Error('C:\\private\\discard-animation.glb')
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('cleanupFailed'))
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
    expect(context.focus).not.toHaveBeenCalled()
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('keeps failed saves retryable and always clears the file-operation marker', async () => {
    const context = setup({
      save: async () => {
        throw new Error('C:\\private\\save-target.glb')
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.save())

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('saveFailed'))
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('saveFailed'),
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
