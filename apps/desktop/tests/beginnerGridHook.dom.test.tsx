import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import { useBeginnerParameterGridWorkflow } from '../src/lib/useBeginnerParameterGridWorkflow.ts'

afterEach(cleanup)

const COPY = { ja: '確認', en: 'Confirm' } as const

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((accept) => {
    resolve = accept
  })
  return { promise, resolve }
}

function snapshot(revision = 1): ProjectSnapshot {
  return {
    project_instance_id: '11111111-1111-4111-8111-111111111111',
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
  } as ProjectSnapshot
}

function gridResponse(project: ProjectSnapshot) {
  return {
    project_instance_id: project.project_instance_id,
    project_id: project.project_id,
    revision: project.revision,
    evaluated_grid_points: 27,
    refinement_iterations: 2,
    grid_hash: [],
    candidates: [{
      point: { id: 0 },
      assessment: {
        proof_scope: 'sufficient',
        apply_allowed: true,
      },
    }],
  }
}

function GridHarness({
  project,
  transport,
  runNativeEdit,
  startPolling,
  stopPolling,
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
  runNativeEdit: ReturnType<typeof vi.fn>
  startPolling: (callback: () => void) => number
  stopPolling: ReturnType<typeof vi.fn>
}) {
  const current = useRef(project)
  current.current = project
  const workflow = useBeginnerParameterGridWorkflow({
    getCurrentSnapshot: () => current.current,
    skeletonTreeStatus: 'tree',
    runNativeEdit,
    confirm: () => true,
    applyConfirmation: COPY,
    transport: transport as never,
    createGenerationId: () => 'grid-generation',
    startPolling,
    stopPolling,
    scheduleFocus: (callback) => callback(),
  })
  return (
    <>
      <button
        ref={workflow.beginnerGridButtonRef}
        onClick={workflow.requestBeginnerGrid}
      >
        evaluate grid
      </button>
      <button onClick={workflow.cancelBeginnerGrid}>cancel grid</button>
      <output data-testid="grid-busy">{String(workflow.beginnerGridBusy)}</output>
      <output data-testid="grid-progress">
        {workflow.beginnerGridProgress.enumerated}
      </output>
      {workflow.beginnerGrid?.candidates[0] && (
        <button onClick={() => workflow.confirmAndApplyBeginnerGridCandidate(
          workflow.beginnerGrid!.candidates[0]!,
        )}>
          apply grid
        </button>
      )}
    </>
  )
}

describe('beginner parameter-grid hook races', () => {
  it('cancels polling, ignores late results, and restores focus', async () => {
    const project = snapshot()
    const evaluation = deferred<Record<string, unknown>>()
    let poll: (() => void) | undefined
    const stopPolling = vi.fn()
    const cancel = vi.fn(async () => undefined)
    const transport = {
      evaluate: vi.fn(() => evaluation.promise),
      progress: vi.fn(async () => ({
        enumerated_grid_points: 5,
        global_checked_candidates: 1,
        refinement_iterations: 0,
      })),
      cancel,
    }
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={(callback) => {
          poll = callback
          return 7
        }}
        stopPolling={stopPolling}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await act(async () => poll?.())
    await waitFor(() => expect(
      screen.getByTestId('grid-progress').textContent,
    ).toBe('5'))
    fireEvent.click(screen.getByRole('button', { name: 'cancel grid' }))
    expect(cancel).toHaveBeenCalledWith('grid-generation')
    expect(stopPolling).toHaveBeenCalledWith(7)
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'evaluate grid' }),
    )
    await act(() => {
      evaluation.resolve(gridResponse(project))
      return evaluation.promise
    })
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()
  })

  it('applies a candidate only while its full OCC binding is live', async () => {
    const first = snapshot()
    const transport = {
      evaluate: vi.fn(async () => gridResponse(first)),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply: vi.fn(async () => snapshot(2)),
    }
    const runNativeEdit = vi.fn(async () => true)
    const view = render(
      <GridHarness
        project={first}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await screen.findByRole('button', { name: 'apply grid' })
    view.rerender(
      <GridHarness
        project={snapshot(2)}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'apply grid' }))
    expect(runNativeEdit).not.toHaveBeenCalled()
  })
})
