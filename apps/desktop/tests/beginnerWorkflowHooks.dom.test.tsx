import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import {
  createRef,
  useRef,
  type Dispatch,
  type SetStateAction,
} from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import type {
  BeginnerSkeletonEndpointInputV1,
  BeginnerSkeletonEndpointResponseV1,
} from '../src/lib/beginnerSkeletonEndpointClient.ts'
import { useBeginnerCandidateWorkflow } from '../src/lib/useBeginnerCandidateWorkflow.ts'
import { useBeginnerEditorState } from '../src/lib/useBeginnerEditorState.ts'
import { useBeginnerRecognitionWorkflow } from '../src/lib/useBeginnerRecognitionWorkflow.ts'
import { useBeginnerReferenceWorkflow } from '../src/lib/useBeginnerReferenceWorkflow.ts'

afterEach(cleanup)

const COPY = {
  ja: '確認',
  en: 'Confirm',
} as const

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept
    reject = decline
  })
  return { promise, resolve, reject }
}

function snapshot(
  revision = 1,
  instanceId = '11111111-1111-4111-8111-111111111111',
): ProjectSnapshot {
  return {
    project_instance_id: instanceId,
    project_id: '22222222-2222-4222-8222-222222222222',
    revision,
    fold_model_fingerprint: 'a'.repeat(64),
    beginner_design_profile: {
      schema_version: 1,
      preset: 'balanced',
      shape_fidelity_weight: 35,
      foldability_weight: 35,
      step_count_weight: 15,
      paper_efficiency_weight: 15,
      generation_constraints: {
        schema_version: 1,
        maximum_steps: 60,
        detail_level: 'standard',
        target_category: null,
        target_parts: [],
        skeleton_segments: [],
        protrusions: [],
        bulge_targets: [],
        target_asset: null,
        allowed_techniques: ['valley_fold', 'mountain_fold'],
      },
    },
    underlays: {
      schema_version: 1,
      underlays: [{
        id: 'underlay-1',
        asset: 'asset-1',
        opacity: 1,
        visible: true,
        locked: false,
        transform: {
          translate_x: 0,
          translate_y: 0,
          scale_x: 1,
          scale_y: 1,
          rotation_degrees: 0,
        },
      }],
    },
  } as ProjectSnapshot
}

function candidateResponse(project: ProjectSnapshot) {
  return {
    project_instance_id: project.project_instance_id,
    project_id: project.project_id,
    revision: project.revision,
    requested_candidate_count: 1,
    candidates: [],
    generation_status: 'missing_target_category',
    generated_plans: [],
    plan_assessments: [],
  }
}

function CandidateHarness({
  project,
  transport,
  subscribe,
  progressEnabled = true,
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
  subscribe: ReturnType<typeof vi.fn>
  progressEnabled?: boolean
}) {
  const current = useRef(project)
  current.current = project
  const workflow = useBeginnerCandidateWorkflow({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    runNativeEdit: async () => true,
    confirm: () => true,
    copy: {
      applyPlan: COPY,
      saveSymmetric: COPY,
      appendInstructions: COPY,
    },
    transport: transport as never,
    createGenerationId: () => 'candidate-generation',
    consensusProgressEnabled: progressEnabled,
    subscribeConsensusProgress: subscribe,
  })
  return (
    <>
      <button onClick={() => workflow.requestBeginnerCandidates(1)}>
        request candidate
      </button>
      <output data-testid="candidate-busy">
        {String(workflow.beginnerCandidateBusy)}
      </output>
      <output data-testid="candidate-result">
        {workflow.beginnerCandidates ? 'ready' : 'empty'}
      </output>
    </>
  )
}

function recognitionEditor() {
  const ignore = vi.fn() as Dispatch<SetStateAction<never>>
  return {
    beginnerDesignFormRef: createRef<HTMLFormElement>(),
    setBeginnerPartTotal: ignore,
    setBeginnerSkeletonSegments: ignore,
    setBeginnerBodyOutline: ignore,
    setBeginnerBodyOutlineMode: ignore,
    setBeginnerProtrusions: ignore,
  }
}

function RecognitionHarness({
  project,
  transport,
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
}) {
  const current = useRef(project)
  current.current = project
  const editor = useRef(recognitionEditor()).current
  const workflow = useBeginnerRecognitionWorkflow({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    operationBlocked: () => false,
    runNativeEdit: async () => true,
    confirm: () => true,
    copy: {
      copyOutline: COPY,
      applyParts: COPY,
      copyProposal: COPY,
      overrideLowConfidence: COPY,
    },
    editor: editor as never,
    onMissingReference: vi.fn(),
    onRecognitionReady: vi.fn(),
    onRecognitionFailure: vi.fn(),
    onProposalCopied: vi.fn(),
    transport: transport as never,
    scheduleDebounce: () => 1,
    cancelDebounce: vi.fn(),
  })
  return (
    <>
      <form ref={editor.beginnerDesignFormRef}>
        <select
          name="target_reference_underlay"
          defaultValue="underlay-1"
        >
          <option value="underlay-1">reference</option>
        </select>
      </form>
      <button onClick={() => workflow.requestBeginnerRecognition('marker')}>
        recognize
      </button>
      <output data-testid="recognition-busy">
        {String(workflow.beginnerRecognitionBusy)}
      </output>
      <output data-testid="recognition-result">
        {workflow.beginnerRecognitionProposal ? 'ready' : 'empty'}
      </output>
    </>
  )
}

function referenceEditor() {
  return {
    beginnerDesignFormRef: createRef<HTMLFormElement>(),
    setBeginnerBodyOutline: vi.fn(),
    setBeginnerBodyOutlineMode: vi.fn(),
    setBeginnerProtrusions: vi.fn(),
    setBeginnerSkeletonSegments: vi.fn(),
    setBeginnerComponentBridgeOverride: vi.fn(),
  }
}

function ReferenceHarness({
  project,
  transport,
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
}) {
  const current = useRef(project)
  current.current = project
  const workflow = useBeginnerReferenceWorkflow({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    runNativeEdit: async () => true,
    confirm: () => true,
    copy: {
      applySuggestion: COPY,
      copyEstimatedBridges: COPY,
    },
    editor: referenceEditor() as never,
    transport: transport as never,
  })
  return (
    <>
      <button onClick={workflow.toggleBeginnerReferenceModelPreview}>
        preview reference
      </button>
      <output data-testid="reference-result">
        {workflow.beginnerReferenceGeometry ? 'ready' : 'empty'}
      </output>
    </>
  )
}

function skeletonEndpointResponse(
  endXTenthsMm: number,
): BeginnerSkeletonEndpointResponseV1 {
  return {
    start_tenths_mm: [0, 0],
    end_tenths_mm: [endXTenthsMm, 0],
  } as BeginnerSkeletonEndpointResponseV1
}

function EditorHarness({
  project,
  resolveEndpoint,
}: {
  project: ProjectSnapshot
  resolveEndpoint: (
    input: BeginnerSkeletonEndpointInputV1,
  ) => Promise<BeginnerSkeletonEndpointResponseV1>
}) {
  const current = useRef(project)
  current.current = project
  const form = useRef<HTMLFormElement>(null)
  const editor = useBeginnerEditorState({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    getSelectedFaceId: () => null,
    resolveSkeletonEndpoint: resolveEndpoint,
  })
  return (
    <>
      <form ref={form}>
        <input name="skeleton_start_x_mm" defaultValue="0" />
        <input name="skeleton_start_y_mm" defaultValue="0" />
        <input name="skeleton_length_mm" defaultValue="10" />
        <input name="skeleton_angle_degrees" defaultValue="0" />
        <input name="skeleton_thickness_mm" defaultValue="1" />
        <button
          type="button"
          onClick={() => {
            if (form.current) editor.addBeginnerSkeletonSegment(form.current)
          }}
        >
          add skeleton
        </button>
      </form>
      <output data-testid="editor-skeleton-endpoints">
        {editor.beginnerSkeletonSegments
          .map((segment) => segment.end.x_tenths_mm)
          .join(',')}
      </output>
    </>
  )
}

describe('beginner workflow hook race boundaries', () => {
  it('does not subscribe to native consensus progress in browser mode', () => {
    const subscribe = vi.fn(async () => vi.fn())
    render(
      <CandidateHarness
        project={snapshot()}
        transport={{}}
        subscribe={subscribe}
        progressEnabled={false}
      />,
    )
    expect(subscribe).not.toHaveBeenCalled()
  })

  it('keeps a candidate across same-OCC objects and cancels on revision change', async () => {
    const first = snapshot()
    const pending = deferred<ReturnType<typeof candidateResponse>>()
    const cancelConsensus = vi.fn(async () => undefined)
    const transport = {
      evaluate: vi.fn(() => pending.promise),
      cancelConsensus,
    }
    const subscribe = vi.fn(async () => vi.fn())
    const view = render(
      <CandidateHarness
        project={first}
        transport={transport}
        subscribe={subscribe}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    expect(screen.getByTestId('candidate-busy').textContent).toBe('true')
    view.rerender(
      <CandidateHarness
        project={{ ...first }}
        transport={transport}
        subscribe={subscribe}
      />,
    )
    expect(screen.getByTestId('candidate-busy').textContent).toBe('true')
    await act(() => {
      pending.resolve(candidateResponse(first))
      return pending.promise
    })
    expect(screen.getByTestId('candidate-result').textContent).toBe('ready')

    const nextPending = deferred<ReturnType<typeof candidateResponse>>()
    transport.evaluate.mockImplementationOnce(() => nextPending.promise)
    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    view.rerender(
      <CandidateHarness
        project={snapshot(2)}
        transport={transport}
        subscribe={subscribe}
      />,
    )
    await waitFor(() => expect(cancelConsensus).toHaveBeenCalledWith(
      'candidate-generation',
    ))
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
    await act(() => {
      nextPending.resolve(candidateResponse(first))
      return nextPending.promise
    })
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
  })

  it('ignores stale recognition and reference responses after project ABA changes', async () => {
    const first = snapshot()
    const recognition = deferred<Record<string, unknown>>()
    const geometry = deferred<Record<string, unknown>>()
    const recognitionTransport = {
      recognizeTarget: vi.fn(() => recognition.promise),
    }
    const referenceTransport = {
      geometry: vi.fn(() => geometry.promise),
    }
    const recognitionView = render(
      <RecognitionHarness project={first} transport={recognitionTransport} />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'recognize' }))
    expect(screen.getByTestId('recognition-busy').textContent).toBe('true')
    recognitionView.rerender(
      <RecognitionHarness
        project={snapshot(1, '33333333-3333-4333-8333-333333333333')}
        transport={recognitionTransport}
      />,
    )
    await act(() => {
      recognition.resolve({
        project_instance_id: first.project_instance_id,
        project_id: first.project_id,
        revision: first.revision,
        source_underlay_id: 'underlay-1',
        source_asset_id: 'asset-1',
        target_parts: [],
        skeleton_segments: [],
        protrusions: [],
      })
      return recognition.promise
    })
    expect(screen.getByTestId('recognition-result').textContent).toBe('empty')
    expect(screen.getByTestId('recognition-busy').textContent).toBe('false')
    recognitionView.unmount()

    const referenceView = render(
      <ReferenceHarness project={first} transport={referenceTransport} />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'preview reference' }))
    referenceView.rerender(
      <ReferenceHarness project={snapshot(2)} transport={referenceTransport} />,
    )
    await act(() => {
      geometry.resolve({
        project_instance_id: first.project_instance_id,
        project_id: first.project_id,
        revision: first.revision,
        positions: [],
        triangle_indices: [],
      })
      return geometry.promise
    })
    expect(screen.getByTestId('reference-result').textContent).toBe('empty')
  })

  it('rejects stale skeleton endpoints across revision and project ABA changes', async () => {
    const firstProject = snapshot()
    const revisionPending = deferred<BeginnerSkeletonEndpointResponseV1>()
    const abaPending = deferred<BeginnerSkeletonEndpointResponseV1>()
    const resolveEndpoint = vi.fn()
      .mockImplementationOnce(() => revisionPending.promise)
      .mockImplementationOnce(() => abaPending.promise)
    const view = render(
      <EditorHarness
        project={firstProject}
        resolveEndpoint={resolveEndpoint}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'add skeleton' }))
    const revisedProject = snapshot(2)
    view.rerender(
      <EditorHarness
        project={revisedProject}
        resolveEndpoint={resolveEndpoint}
      />,
    )
    await act(async () => {
      revisionPending.resolve(skeletonEndpointResponse(10))
      await revisionPending.promise
      await Promise.resolve()
    })
    expect(screen.getByTestId('editor-skeleton-endpoints').textContent).toBe('')

    fireEvent.click(screen.getByRole('button', { name: 'add skeleton' }))
    view.rerender(
      <EditorHarness
        project={snapshot(
          2,
          '33333333-3333-4333-8333-333333333333',
        )}
        resolveEndpoint={resolveEndpoint}
      />,
    )
    view.rerender(
      <EditorHarness
        project={{ ...revisedProject }}
        resolveEndpoint={resolveEndpoint}
      />,
    )
    await act(async () => {
      abaPending.resolve(skeletonEndpointResponse(20))
      await abaPending.promise
      await Promise.resolve()
    })
    expect(screen.getByTestId('editor-skeleton-endpoints').textContent).toBe('')
  })

  it('commits same-revision skeleton endpoint requests in request order', async () => {
    const firstPending = deferred<BeginnerSkeletonEndpointResponseV1>()
    const secondPending = deferred<BeginnerSkeletonEndpointResponseV1>()
    const resolveEndpoint = vi.fn()
      .mockImplementationOnce(() => firstPending.promise)
      .mockImplementationOnce(() => secondPending.promise)
    render(
      <EditorHarness
        project={snapshot()}
        resolveEndpoint={resolveEndpoint}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'add skeleton' }))
    fireEvent.click(screen.getByRole('button', { name: 'add skeleton' }))
    expect(resolveEndpoint).toHaveBeenCalledTimes(2)

    await act(async () => {
      secondPending.resolve(skeletonEndpointResponse(20))
      await secondPending.promise
      await Promise.resolve()
    })
    expect(screen.getByTestId('editor-skeleton-endpoints').textContent).toBe('')

    await act(async () => {
      firstPending.resolve(skeletonEndpointResponse(10))
      await firstPending.promise
      await Promise.resolve()
    })
    expect(screen.getByTestId('editor-skeleton-endpoints').textContent)
      .toBe('10,20')
  })

})
