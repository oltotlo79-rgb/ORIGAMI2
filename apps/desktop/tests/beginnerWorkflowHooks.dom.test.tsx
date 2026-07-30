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
import { BeginnerProtrusionEditor } from '../src/components/BeginnerProtrusionEditor.tsx'

afterEach(cleanup)

const CANDIDATE_GENERATION_ID =
  '33333333-3333-4333-8333-333333333333'

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

function applicableCandidateResponse(project: ProjectSnapshot) {
  return {
    ...candidateResponse(project),
    generation_status: 'ready',
    generated_plans: [{
      kind: 'diagonal_fold',
      crease_pattern: {
        vertices: [],
        edges: [{
          id: '44444444-4444-4444-8444-444444444444',
          start: '55555555-5555-4555-8555-555555555555',
          end: '66666666-6666-4666-8666-666666666666',
          kind: 'valley',
        }],
      },
      instruction_codes: [],
      target_parts: [],
      skeleton_segments: [],
      target_asset: null,
    }],
    plan_assessments: [{
      proof_scope: 'sufficient',
      apply_allowed: true,
    }],
  }
}

function CandidateHarness({
  project,
  transport,
  subscribe,
  progressEnabled = true,
  runNativeEdit = vi.fn(async () => true),
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
  subscribe: ReturnType<typeof vi.fn>
  progressEnabled?: boolean
  runNativeEdit?: ReturnType<typeof vi.fn>
}) {
  const current = useRef(project)
  current.current = project
  const workflow = useBeginnerCandidateWorkflow({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    runNativeEdit,
    confirm: () => true,
    copy: {
      applyPlan: COPY,
      saveSymmetric: COPY,
      appendInstructions: COPY,
    },
    transport: transport as never,
    createGenerationId: () => CANDIDATE_GENERATION_ID,
    consensusProgressEnabled: progressEnabled,
    subscribeConsensusProgress: subscribe,
  })
  return (
    <>
      <button onClick={() => workflow.requestBeginnerCandidates(1)}>
        request candidate
      </button>
      <button onClick={workflow.cancelConsensusAnalysis}>
        cancel candidate
      </button>
      <output data-testid="candidate-busy">
        {String(workflow.beginnerCandidateBusy)}
      </output>
      <output data-testid="candidate-apply-busy">
        {String(workflow.beginnerCandidateApplyBusy)}
      </output>
      <output data-testid="candidate-result">
        {workflow.beginnerCandidates ? 'ready' : 'empty'}
      </output>
      <output data-testid="candidate-status">
        {workflow.beginnerCandidateRequestStatus}
      </output>
      {workflow.beginnerCandidates?.generated_plans[0] && (
        <button
          disabled={workflow.beginnerCandidateApplyBusy}
          onClick={() => workflow.confirmAndApplyBeginnerPlan(
            workflow.beginnerCandidates!.generated_plans[0]!.kind,
            workflow.beginnerCandidates!
              .generated_plans[0]!.crease_pattern.edges[0]!.id,
          )}
        >
          apply candidate
        </button>
      )}
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

function ManualProtrusionHarness({
  project,
}: {
  project: ProjectSnapshot
}) {
  const current = useRef(project)
  current.current = project
  const editor = useBeginnerEditorState({
    snapshot: project,
    getCurrentSnapshot: () => current.current,
    getSelectedFaceId: () => null,
  })
  return (
    <>
      <form>
        <BeginnerProtrusionEditor
          locale="en"
          coreBusy={false}
          editor={editor}
        />
      </form>
      <output data-testid="manual-protrusions">
        {JSON.stringify(editor.beginnerProtrusions)}
      </output>
    </>
  )
}

function snapshotWithProtrusionIds(ids: readonly number[]) {
  const project = snapshot()
  project.beginner_design_profile.generation_constraints.protrusions =
    ids.map((id) => ({
      id,
      count: 1,
      length_tenths_mm: 200,
      thickness_tenths_mm: 20,
      position_tenths_mm: [0, 0, 0],
      direction_milli: [1_000, 0, 0],
      symmetry: 'none',
      curvature_degrees: 0,
      joint: 'fixed',
      motion_degrees: [0, 0],
      side: 'either',
      priority: 50,
    }))
  return project
}

describe('beginner workflow hook race boundaries', () => {
  it('adds only supported symmetry counts and preserves sparse ascending IDs', async () => {
    render(<ManualProtrusionHarness project={snapshotWithProtrusionIds([2, 7])} />)
    await waitFor(() => expect(
      JSON.parse(screen.getByTestId('manual-protrusions').textContent ?? '[]')
        .map(({ id }: { id: number }) => id),
    ).toEqual([2, 7]))

    expect(
      (screen.getByLabelText('Symmetry', {
        selector: 'select[name="protrusion_symmetry"]',
      }) as HTMLSelectElement).value,
    ).toBe('bilateral')
    fireEvent.change(screen.getByLabelText('Count', {
      selector: 'input[name="protrusion_count"]',
    }), {
      target: { value: '3' },
    })
    fireEvent.click(screen.getByRole('button', {
      name: 'Add protrusion target',
    }))
    expect(
      JSON.parse(screen.getByTestId('manual-protrusions').textContent ?? '[]')
        .map(({ id }: { id: number }) => id),
    ).toEqual([2, 7])

    fireEvent.change(screen.getByLabelText('Symmetry', {
      selector: 'select[name="protrusion_symmetry"]',
    }), {
      target: { value: 'radial' },
    })
    fireEvent.click(screen.getByRole('button', {
      name: 'Add protrusion target',
    }))
    await waitFor(() => {
      const targets = JSON.parse(
        screen.getByTestId('manual-protrusions').textContent ?? '[]',
      )
      expect(targets.map(({ id }: { id: number }) => id)).toEqual([2, 7, 8])
      expect(targets[2]).toMatchObject({
        id: 8,
        count: 3,
        symmetry: 'radial',
      })
    })
  })

  it('uses a sorted ID gap only when the u16 maximum prevents append', async () => {
    render(<ManualProtrusionHarness
      project={snapshotWithProtrusionIds([2, 65_535])}
    />)
    await waitFor(() => expect(
      JSON.parse(screen.getByTestId('manual-protrusions').textContent ?? '[]')
        .map(({ id }: { id: number }) => id),
    ).toEqual([2, 65_535]))
    fireEvent.click(screen.getByRole('button', {
      name: 'Add protrusion target',
    }))
    await waitFor(() => expect(
      JSON.parse(screen.getByTestId('manual-protrusions').textContent ?? '[]')
        .map(({ id }: { id: number }) => id),
    ).toEqual([1, 2, 65_535]))
  })

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
    expect(transport.evaluate).toHaveBeenCalledWith(
      first.project_id,
      first.revision,
      first.project_instance_id,
      1,
      CANDIDATE_GENERATION_ID,
      first.beginner_design_profile,
    )
    expect(screen.getByTestId('candidate-busy').textContent).toBe('true')
    expect(screen.getByTestId('candidate-status').textContent).toBe('running')
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
    expect(screen.getByTestId('candidate-status').textContent).toBe('empty')

    const nextPending = deferred<ReturnType<typeof candidateResponse>>()
    transport.evaluate.mockImplementationOnce(() => nextPending.promise)
    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
    expect(screen.getByTestId('candidate-status').textContent).toBe('running')
    view.rerender(
      <CandidateHarness
        project={snapshot(2)}
        transport={transport}
        subscribe={subscribe}
      />,
    )
    await waitFor(() => expect(cancelConsensus).toHaveBeenCalledWith(
      CANDIDATE_GENERATION_ID,
    ))
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
    expect(screen.getByTestId('candidate-status').textContent).toBe('idle')
    await act(() => {
      nextPending.resolve(candidateResponse(first))
      return nextPending.promise
    })
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
  })

  it('discards prior authority on cancellation and reports evaluation failure', async () => {
    const project = snapshot()
    const pending = deferred<ReturnType<typeof candidateResponse>>()
    const cancelConsensus = vi.fn(async () => undefined)
    const transport = {
      evaluate: vi.fn(() => pending.promise),
      cancelConsensus,
    }
    const subscribe = vi.fn(async () => vi.fn())
    const view = render(
      <CandidateHarness
        project={project}
        transport={transport}
        subscribe={subscribe}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    fireEvent.click(screen.getByRole('button', { name: 'cancel candidate' }))
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')
    expect(screen.getByTestId('candidate-status').textContent)
      .toBe('cancelled')
    expect(cancelConsensus).toHaveBeenCalledWith(
      CANDIDATE_GENERATION_ID,
    )

    view.rerender(
      <CandidateHarness
        project={project}
        transport={{
          evaluate: vi.fn(async () => {
            throw new Error('evaluation failed')
          }),
          cancelConsensus,
        }}
        subscribe={subscribe}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    await waitFor(() => expect(
      screen.getByTestId('candidate-status').textContent,
    ).toBe('failed'))
    expect(screen.getByTestId('candidate-result').textContent).toBe('empty')

    view.rerender(
      <CandidateHarness
        project={project}
        transport={{
          evaluate: vi.fn(async () => candidateResponse(project)),
          cancelConsensus,
        }}
        subscribe={subscribe}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    await waitFor(() => expect(
      screen.getByTestId('candidate-status').textContent,
    ).toBe('empty'))
  })

  it('single-consumes candidate apply authority across rapid attempts', async () => {
    const project = snapshot()
    const applied = deferred<boolean>()
    const runNativeEdit = vi.fn(() => applied.promise)
    const transport = {
      evaluate: vi.fn(async () => applicableCandidateResponse(project)),
      cancelConsensus: vi.fn(async () => undefined),
    }
    const view = render(
      <CandidateHarness
        project={project}
        transport={transport}
        subscribe={vi.fn(async () => vi.fn())}
        runNativeEdit={runNativeEdit}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    const applyButton = await screen.findByRole('button', {
      name: 'apply candidate',
    })
    fireEvent.click(applyButton)
    fireEvent.click(applyButton)

    expect(runNativeEdit).toHaveBeenCalledOnce()
    expect(screen.getByTestId('candidate-apply-busy').textContent).toBe('true')
    await act(() => {
      applied.resolve(true)
      return applied.promise
    })
    await waitFor(() => expect(
      screen.getByTestId('candidate-apply-busy').textContent,
    ).toBe('false'))
    expect(screen.queryByRole('button', { name: 'apply candidate' })).toBeNull()
    expect(screen.getByTestId('candidate-status').textContent).toBe('idle')
    view.unmount()
  })

  it('cancels candidate authority and ignores a late response on unmount', async () => {
    const project = snapshot()
    const pending = deferred<ReturnType<typeof candidateResponse>>()
    const cancelConsensus = vi.fn(async () => undefined)
    const view = render(
      <CandidateHarness
        project={project}
        transport={{
          evaluate: vi.fn(() => pending.promise),
          cancelConsensus,
        }}
        subscribe={vi.fn(async () => vi.fn())}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'request candidate' }))
    view.unmount()
    expect(cancelConsensus).toHaveBeenCalledWith(
      CANDIDATE_GENERATION_ID,
    )
    await act(() => {
      pending.resolve(candidateResponse(project))
      return pending.promise
    })
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
