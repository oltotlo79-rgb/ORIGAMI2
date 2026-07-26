import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { FoldPreviewAppliedPoseSnapshot } from '../src/lib/foldPreviewAppliedPose.ts'
import type {
  StaticMeshExportFormat,
  StaticMeshExportPreview,
} from '../src/lib/staticMeshExport.ts'
import {
  useStaticMeshExportWorkflow,
  type StaticMeshExportProjectBinding,
  type StaticMeshExportWorkflowCopy,
  type StaticMeshExportWorkflowMessage,
  type StaticMeshExportWorkflowTransport,
} from '../src/lib/useStaticMeshExportWorkflow.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const COPY: StaticMeshExportWorkflowCopy = {
  previewReady: localized('preview ready'),
  prepareFailed: localized('prepare failed'),
  cancelled: localized('cancelled'),
  cleanupFailed: localized('cleanup failed'),
  projectChanged: localized('project changed'),
  saveCancelledNotice: localized('save cancelled notice'),
  saveCancelledStatus: localized('save cancelled status'),
  saved: localized('saved {fileName}'),
  saveFailed: localized('save failed'),
}

function localized(english: string) {
  return Object.freeze({ ja: english, en: english })
}

function workflowMessage(
  text: keyof typeof COPY,
  variables?: Readonly<Record<string, string | number>>,
): StaticMeshExportWorkflowMessage {
  return Object.freeze({ text: COPY[text], variables })
}

function binding(
  revision = 7,
  projectInstanceId = 'instance-1',
  projectId = 'project-1',
): StaticMeshExportProjectBinding {
  return {
    project_instance_id: projectInstanceId,
    project_id: projectId,
    revision,
  }
}

function pose(
  overrides: Partial<FoldPreviewAppliedPoseSnapshot> = {},
): FoldPreviewAppliedPoseSnapshot {
  return {
    projectId: 'project-1',
    revision: 7,
    fixedFaceId: 'face-1',
    hingeAngles: [{ edgeId: 'edge-1', angleDegrees: 42 }],
    state: 'stable',
    ...overrides,
  }
}

function previewFixture(
  format: StaticMeshExportFormat = 'obj',
  overrides: Partial<StaticMeshExportPreview> = {},
): StaticMeshExportPreview {
  return {
    exportId: `export-${format}`,
    projectInstanceId: 'instance-1',
    projectId: 'project-1',
    revision: 7,
    sourceFingerprint: 'a'.repeat(64),
    poseGeneration: '5',
    format,
    formatSummary: format.toUpperCase(),
    suggestedFileName: `project-pose.${format}`,
    byteCount: 256,
    paperThicknessMm: 0,
    faceCount: 1,
    vertexCount: 3,
    triangleCount: 1,
    geometryProfile: 'authenticated_mid_surface_triangle_mesh_v1',
    sourceUnit: 'millimeter',
    encodedUnit: format === 'glb' ? 'meter' : 'millimeter',
    sourceAxis: 'right-handed X-right Y-forward Z-up',
    encodedAxis: format === 'glb'
      ? 'glTF 2.0 right-handed -X-right Y-up Z-forward'
      : 'right-handed X-right Y-forward Z-up',
    warnings: [
      'mid_surface_only',
      'no_thickness_solid',
      'no_textures_animation',
      'no_project_semantics',
    ],
    printability: {
      status: 'not_applicable',
      watertight: false,
      consistentlyOriented: false,
      nonzeroVolume: false,
      noDuplicateTriangles: false,
      noDegenerateTriangles: false,
      conservativeSelfIntersectionClear: false,
      connectedComponentCount: 1,
      checkedEdgeCount: 0,
      checkedTrianglePairCount: 0,
      limitations: ['format_not_covered', 'manifold_only_not_printability'],
    },
    ...overrides,
  }
}

function setup(options: Readonly<{
  current?: StaticMeshExportProjectBinding | null
  currentPose?: FoldPreviewAppliedPoseSnapshot | null
  preview?: StaticMeshExportWorkflowTransport['preview']
  save?: StaticMeshExportWorkflowTransport['save']
  cancel?: StaticMeshExportWorkflowTransport['cancel']
}> = {}) {
  let current = options.current === undefined ? binding() : options.current
  let currentPose = options.currentPose === undefined ? pose() : options.currentPose
  let operationActive = false
  let fileOperation: 'mesh_export' | null = null
  const previewRequest = vi.fn(options.preview ?? (async (
    _projectInstanceId: string,
    _projectId: string,
    _revision: number,
    format: StaticMeshExportFormat,
  ) => ({ preview: previewFixture(format) })))
  const saveRequest = vi.fn(options.save ?? (async () => ({ canceled: false })))
  const cancelRequest = vi.fn(options.cancel ?? (async () => undefined))
  const setOperationBusy = vi.fn((busy: boolean) => {
    operationActive = busy
  })
  const setFileOperation = vi.fn((operation: 'mesh_export' | null) => {
    fileOperation = operation
  })
  const cancelInteraction = vi.fn()
  const onStatus = vi.fn()
  const scheduleFocus = vi.fn((callback: () => void) => callback())
  const transport = {
    preview: previewRequest,
    save: saveRequest,
    cancel: cancelRequest,
  } as unknown as StaticMeshExportWorkflowTransport
  const hook = renderHook(() => useStaticMeshExportWorkflow({
    copy: COPY,
    getCurrentSnapshot: () => current,
    getCurrentPose: () => currentPose,
    operationActive: () => operationActive,
    setOperationBusy,
    setFileOperation,
    cancelInteraction,
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
    cancelInteraction,
    onStatus,
    scheduleFocus,
    focus,
    get operationActive() {
      return operationActive
    },
    get fileOperation() {
      return fileOperation
    },
    setCurrent(value: StaticMeshExportProjectBinding | null) {
      current = value
    },
    setCurrentPose(value: FoldPreviewAppliedPoseSnapshot | null) {
      currentPose = value
    },
    setOperationActive(value: boolean) {
      operationActive = value
    },
  }
}

describe('useStaticMeshExportWorkflow', () => {
  it('prepares OBJ from the exact current project instance and stable pose', async () => {
    const context = setup()

    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    expect(context.previewRequest).toHaveBeenCalledExactlyOnceWith(
      'instance-1',
      'project-1',
      7,
      'obj',
    )
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.format).toBe('obj')
    expect(context.result.current.preview?.exportId).toBe('export-obj')
    expect(context.cancelInteraction).toHaveBeenCalledOnce()
    expect(context.setFileOperation.mock.calls).toEqual([
      ['mesh_export'],
      [null],
    ])
    expect(context.setOperationBusy.mock.calls).toEqual([[true], [false]])
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith(
      workflowMessage('previewReady'),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('does not open for a running or project-mismatched applied pose', () => {
    const running = setup({ currentPose: pose({ state: 'running' }) })
    act(() => running.result.current.begin())
    expect(running.result.current.open).toBe(false)
    expect(running.previewRequest).not.toHaveBeenCalled()

    const mismatched = setup({
      currentPose: pose({ projectId: 'different-project' }),
    })
    act(() => mismatched.result.current.begin())
    expect(mismatched.result.current.open).toBe(false)
    expect(mismatched.previewRequest).not.toHaveBeenCalled()
  })

  it('cancels a preview with a mismatched project instance and releases flags', async () => {
    const context = setup({
      preview: async () => ({
        preview: previewFixture('obj', {
          exportId: 'mismatched',
          projectInstanceId: 'different-instance',
        }),
      }),
      cancel: async () => {
        throw new Error('C:\\private\\mesh.obj')
      },
    })

    act(() => context.result.current.begin())
    await waitFor(() =>
      expect(context.result.current.error).toEqual(
        workflowMessage('prepareFailed'),
      ))

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('mismatched')
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('prepareFailed'),
    )
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
    expect(JSON.stringify(context.result.current.error)).not.toContain('private')
  })

  it('rejects and cancels a preview after the applied pose identity changes', async () => {
    const pending = deferred<{ preview: StaticMeshExportPreview }>()
    const context = setup({
      preview: (() => pending.promise) as StaticMeshExportWorkflowTransport['preview'],
    })
    act(() => context.result.current.begin())
    context.setCurrentPose(pose({
      hingeAngles: [{ edgeId: 'edge-1', angleDegrees: 84 }],
    }))

    await act(async () => {
      pending.resolve({ preview: previewFixture() })
      await pending.promise
    })

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-obj')
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('prepareFailed'))
    expect(context.operationActive).toBe(false)
  })

  it('lets only the newest format request publish and cancels a late response', async () => {
    const first = deferred<{ preview: StaticMeshExportPreview }>()
    const previewRequest = vi.fn()
      .mockImplementationOnce(() => first.promise)
      .mockResolvedValueOnce({ preview: previewFixture('stl') })
    const context = setup({
      preview: previewRequest as StaticMeshExportWorkflowTransport['preview'],
    })
    act(() => context.result.current.begin())
    context.setOperationActive(false)
    act(() => context.result.current.changeFormat('stl'))
    await waitFor(() =>
      expect(context.result.current.preview?.format).toBe('stl'))

    await act(async () => {
      first.resolve({
        preview: previewFixture('obj', { exportId: 'late-obj' }),
      })
      await first.promise
    })

    expect(context.result.current.preview?.format).toBe('stl')
    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('late-obj')
    expect(context.onStatus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('rejects save after project-instance replacement without native mutation', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    context.setCurrent(binding(7, 'instance-2'))

    await act(() => context.result.current.save(true))

    expect(context.saveRequest).not.toHaveBeenCalled()
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('projectChanged'))
  })

  it('keeps a canceled save retryable and forwards warning acknowledgement', async () => {
    const saveRequest = vi.fn()
      .mockResolvedValueOnce({ canceled: true })
      .mockResolvedValueOnce({ canceled: false })
    const context = setup({
      save: saveRequest as StaticMeshExportWorkflowTransport['save'],
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())
    const admittedPreview = context.result.current.preview

    await act(() => context.result.current.save(false))
    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).toBe(admittedPreview)
    expect(context.result.current.notice).toEqual(
      workflowMessage('saveCancelledNotice'),
    )
    expect(context.focus).not.toHaveBeenCalled()

    await act(() => context.result.current.save(true))

    expect(context.saveRequest.mock.calls).toEqual([
      [admittedPreview, false],
      [admittedPreview, true],
    ])
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('saved', { fileName: 'project-pose.obj' }),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.scheduleFocus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
    expect(context.fileOperation).toBeNull()
  })

  it('cancels an admitted preview before closing and restores focus', async () => {
    const context = setup()
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.close())

    expect(context.cancelRequest).toHaveBeenCalledExactlyOnceWith('export-obj')
    expect(context.result.current.open).toBe(false)
    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(
      workflowMessage('cancelled'),
    )
    expect(context.focus).toHaveBeenCalledOnce()
    expect(context.operationActive).toBe(false)
  })

  it('keeps cleanup failure visible and always releases the shared lock', async () => {
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
    expect(context.result.current.error).toEqual(workflowMessage('cleanupFailed'))
    expect(context.focus).not.toHaveBeenCalled()
    expect(context.operationActive).toBe(false)
  })

  it('keeps failed saves retryable and clears the file-operation marker', async () => {
    const context = setup({
      save: async () => {
        throw new Error('C:\\private\\save-target.stl')
      },
    })
    act(() => context.result.current.begin())
    await waitFor(() => expect(context.result.current.preview).not.toBeNull())

    await act(() => context.result.current.save(true))

    expect(context.result.current.open).toBe(true)
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual(workflowMessage('saveFailed'))
    expect(context.result.current.notice).toBeNull()
    expect(context.onStatus).toHaveBeenLastCalledWith(workflowMessage('saveFailed'))
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
