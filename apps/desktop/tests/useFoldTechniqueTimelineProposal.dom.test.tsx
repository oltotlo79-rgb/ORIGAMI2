import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import { createInitialFoldTechniqueDocumentV1 } from '../src/lib/foldTechniqueEditor.ts'
import {
  FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT,
} from '../src/lib/foldTechniqueTimelineProposalText.ts'
import {
  useFoldTechniqueTimelineProposal,
  type FoldTechniqueTimelineWorkspace,
} from '../src/lib/useFoldTechniqueTimelineProposal.ts'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const documentFixture = createInitialFoldTechniqueDocumentV1()
const workspace: FoldTechniqueTimelineWorkspace = {
  document: documentFixture,
  dirty: false,
}

function snapshot(revision = 1, instructionStepCount = 0): ProjectSnapshot {
  return {
    project_instance_id: 'instance-1',
    project_id: 'project-1',
    revision,
    instruction_timeline: {
      steps: Array.from({ length: instructionStepCount }),
    },
  } as unknown as ProjectSnapshot
}

function setup(options: Readonly<{
  runResult?: boolean
  currentSnapshot?: ProjectSnapshot | null
  appendError?: unknown
}> = {}) {
  let currentSnapshot = options.currentSnapshot === undefined
    ? snapshot()
    : options.currentSnapshot
  let currentWorkspace: FoldTechniqueTimelineWorkspace | null = workspace
  const appendProposal = vi.fn(async () => {
    if (options.appendError !== undefined) throw options.appendError
    return snapshot(2)
  })
  const runNativeEdit = vi.fn(async (action) => {
    await action('project-1', 1, 'instance-1')
    return options.runResult ?? true
  })
  const onStatus = vi.fn()
  const scheduleFocus = vi.fn((callback: () => void) => callback())
  const opener = document.createElement('button')
  const focus = vi.spyOn(opener, 'focus')
  const hook = renderHook(
    ({ renderedSnapshot, selectedIndex }) => useFoldTechniqueTimelineProposal({
      locale: 'en',
      snapshot: renderedSnapshot,
      workspace,
      selectedIndex,
      nativeCoreAvailable: () => true,
      getCurrentSnapshot: () => currentSnapshot,
      getCurrentWorkspace: () => currentWorkspace,
      coreOperationActive: () => false,
      foldTechniqueBusy: () => false,
      runNativeEdit,
      onStatus,
      appendProposal,
      scheduleFocus,
    }),
    { initialProps: { renderedSnapshot: snapshot(), selectedIndex: 0 } },
  )
  return {
    ...hook,
    appendProposal,
    runNativeEdit,
    onStatus,
    opener,
    focus,
    setCurrentSnapshot(value: ProjectSnapshot | null) {
      currentSnapshot = value
    },
    setCurrentWorkspace(value: FoldTechniqueTimelineWorkspace | null) {
      currentWorkspace = value
    },
  }
}

describe('useFoldTechniqueTimelineProposal', () => {
  it('reports capacity with the exact required and available variables', () => {
    const context = setup({ currentSnapshot: snapshot(1, 512) })

    act(() => context.result.current.previewSelected(context.opener))

    expect(context.result.current.preview).toBeNull()
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith({
      text: TEXT.timelineCapacityError,
      variables: { required: 3, available: 0 },
    })
    expect(context.runNativeEdit).not.toHaveBeenCalled()
  })

  it('confirms one admitted proposal and closes only after success', async () => {
    const context = setup()
    act(() => context.result.current.previewSelected(context.opener))
    expect(context.result.current.preview?.preview.ok).toBe(true)

    await act(() => context.result.current.confirmProposal())

    expect(context.appendProposal).toHaveBeenCalledOnce()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.busy).toBe(false)
    expect(context.onStatus).toHaveBeenCalledExactlyOnceWith({
      text: TEXT.appendSucceeded,
      variables: { technique: 'New folding technique' },
    })
    expect(context.focus).toHaveBeenCalledOnce()
  })

  it('marks changed project state stale and rejects confirmation unchanged', async () => {
    const context = setup()
    act(() => context.result.current.previewSelected(context.opener))
    context.setCurrentSnapshot(snapshot(2))
    context.rerender({ renderedSnapshot: snapshot(2), selectedIndex: 0 })
    expect(context.result.current.stale).toBe(true)

    await act(() => context.result.current.confirmProposal())

    expect(context.runNativeEdit).not.toHaveBeenCalled()
    expect(context.appendProposal).not.toHaveBeenCalled()
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual({
      text: TEXT.staleProposalError,
      variables: undefined,
    })
  })

  it('cancels without mutation and restores the opener focus', () => {
    const context = setup()
    act(() => context.result.current.previewSelected(context.opener))
    act(() => context.result.current.closePreview())

    expect(context.runNativeEdit).not.toHaveBeenCalled()
    expect(context.result.current.preview).toBeNull()
    expect(context.result.current.error).toBeNull()
    expect(context.focus).toHaveBeenCalledOnce()
  })

  it('keeps the preview open and reports failure without a success status', async () => {
    const context = setup({ runResult: false })
    act(() => context.result.current.previewSelected(context.opener))

    await act(() => context.result.current.confirmProposal())

    expect(context.appendProposal).toHaveBeenCalledOnce()
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.busy).toBe(false)
    expect(context.result.current.error).toEqual({
      text: TEXT.appendFailed,
      variables: undefined,
    })
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.focus).not.toHaveBeenCalled()
  })

  it('never exposes a native append error and keeps the preview open', async () => {
    const nativeError = new Error(
      'native storage path C:\\private\\project.oripa failed',
    )
    const context = setup({ appendError: nativeError })
    act(() => context.result.current.previewSelected(context.opener))

    await act(() => context.result.current.confirmProposal())

    expect(context.appendProposal).toHaveBeenCalledOnce()
    expect(context.result.current.preview).not.toBeNull()
    expect(context.result.current.error).toEqual({
      text: TEXT.appendFailed,
      variables: undefined,
    })
    expect(context.result.current.error?.text.en).not.toContain(
      nativeError.message,
    )
    expect(context.onStatus).not.toHaveBeenCalled()
    expect(context.focus).not.toHaveBeenCalled()
  })
})
