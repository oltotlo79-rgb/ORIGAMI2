import {
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { BeginnerCandidateControls } from '../src/components/BeginnerCandidateControls.tsx'
import { BeginnerCandidateResults } from '../src/components/BeginnerCandidateResults.tsx'
import type { ProjectSnapshot } from '../src/lib/coreClient.ts'

afterEach(cleanup)

function candidateWorkflow(overrides: Record<string, unknown> = {}) {
  return {
    beginnerCandidates: null,
    beginnerCandidateBusy: false,
    beginnerCandidateApplyBusy: false,
    beginnerCandidateRequestStatus: 'idle',
    consensusProgress: {
      processed_assets: 0,
      total_assets: 0,
      processed_pairs: 0,
      total_pairs: 0,
    },
    selectedConsensusPair: null,
    setSelectedConsensusPair: vi.fn(),
    beginnerSymmetricEstimate: null,
    beginnerSymmetricScale: 25,
    setBeginnerSymmetricScale: vi.fn(),
    beginnerSymmetricSpacing: 35,
    setBeginnerSymmetricSpacing: vi.fn(),
    requestBeginnerCandidates: vi.fn(),
    cancelConsensusAnalysis: vi.fn(),
    requestBeginnerSymmetricEstimate: vi.fn(),
    confirmBeginnerSymmetricEstimate: vi.fn(),
    excludeBeginnerConsensusAsset: vi.fn(),
    confirmAndApplyBeginnerPlan: vi.fn(),
    ...overrides,
  }
}

function gridCandidate(applyAllowed: boolean) {
  return {
    point: {
      id: 0,
      scale_percent: 20,
      spacing_percent: 40,
      detail_level: 'standard',
    },
    primary_score: 1_000,
    plan: {
      crease_pattern: { vertices: [], edges: [{ id: 'edge-1' }] },
      instruction_codes: [],
    },
    assessment: {
      proof_scope: 'sufficient',
      apply_allowed: applyAllowed,
      shape_approximation_score: null,
      shape_difference_reason: null,
    },
    local_proof_scope: 'necessary',
    global_proof_scope: 'sufficient',
    complexity_score: 10,
    paper_efficiency_score: 90,
    scale_deviation_penalty: 0,
    spacing_deviation_penalty: 0,
    detail_mismatch_penalty: 0,
    outcome_reason: 'necessary_conditions_satisfied',
    contour_witness: {
      body_contour_points: 0,
      local_bindings: [],
      generic_feature_bindings: [],
      skeleton_branch_bindings: [],
      skeleton_tree_authority_sha256: [],
      witnessed_vertices: 0,
      witnessed_creases: 0,
      topology_authority_hash: [],
      max_contour_error_millionths: 0,
    },
    refinement_iterations: 0,
    strict_improvements: 0,
    refinement_starts: 1,
  }
}

function gridWorkflow(overrides: Record<string, unknown> = {}) {
  return {
    beginnerGrid: null,
    beginnerGridSelectedPointId: null,
    setBeginnerGridSelectedPointId: vi.fn(),
    beginnerGridBusy: false,
    beginnerGridApplyBusy: false,
    beginnerGridRequestStatus: 'idle',
    beginnerGridProgress: {
      enumerated: 0,
      globalChecked: 0,
      refined: 0,
    },
    beginnerGridButtonRef: { current: null },
    requestBeginnerGrid: vi.fn(),
    cancelBeginnerGrid: vi.fn(),
    confirmAndApplyBeginnerGridCandidate: vi.fn(),
    ...overrides,
  }
}

function renderControls(
  candidate: ReturnType<typeof candidateWorkflow>,
  grid: ReturnType<typeof gridWorkflow>,
) {
  return render(
    <BeginnerCandidateControls
      locale="en"
      coreBusy={false}
      recoveryBlocking={false}
      skeletonTreeStatus="tree"
      candidateWorkflow={candidate as never}
      gridWorkflow={grid as never}
    />,
  )
}

function snapshot(): ProjectSnapshot {
  return {
    beginner_design_profile: {
      shape_fidelity_weight: 35,
      foldability_weight: 35,
      step_count_weight: 15,
      paper_efficiency_weight: 15,
      generation_constraints: {
        protrusions: [],
      },
    },
  } as ProjectSnapshot
}

function readyCandidateResponse() {
  const vertexA = '11111111-1111-4111-8111-111111111111'
  const vertexB = '22222222-2222-4222-8222-222222222222'
  const edge = '33333333-3333-4333-8333-333333333333'
  return {
    requested_candidate_count: 3,
    generation_status: 'ready',
    candidates: [{
      kind: 'recommended',
      rank: 1,
      total_score: 90,
      shape_score: 90,
      foldability_score: 90,
      step_count_score: 90,
      paper_efficiency_score: 90,
      target_approximation_score: 90,
    }],
    generated_plans: [{
      kind: 'diagonal_fold',
      crease_pattern: {
        vertices: [
          { id: vertexA, position: { x: 0, y: 0 } },
          { id: vertexB, position: { x: 1, y: 1 } },
        ],
        edges: [{
          id: edge,
          start: vertexA,
          end: vertexB,
          kind: 'valley',
        }],
      },
      instruction_codes: ['diagonal_fold'],
      target_parts: [],
      skeleton_segments: [],
      target_asset: null,
    }],
    plan_assessments: [{
      proof_scope: 'necessary',
      apply_allowed: true,
      reason: 'necessary_conditions_satisfied',
      shape_approximation_score: null,
      shape_difference_reason: null,
      component_shape_comparison: null,
    }],
    multi_reference_fusion: null,
    reference_consensus_analysis: null,
  }
}

describe('beginner candidate accessibility and authority states', () => {
  it('announces generation progress with a separately operable cancel control', () => {
    const cancel = vi.fn()
    renderControls(candidateWorkflow({
      beginnerCandidateBusy: true,
      beginnerCandidateRequestStatus: 'running',
      consensusProgress: {
        processed_assets: 1,
        total_assets: 2,
        processed_pairs: 0,
        total_pairs: 1,
      },
      cancelConsensusAnalysis: cancel,
    }), gridWorkflow())

    const status = screen.getByRole('status')
    expect(status.textContent).toContain('assets 1/2')
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.getAttribute('aria-atomic')).toBe('true')
    fireEvent.click(screen.getByRole('button', {
      name: 'Cancel consensus analysis',
    }))
    expect(cancel).toHaveBeenCalledOnce()
  })

  it('announces cancellation, failure, and empty results while hiding stale grid authority', () => {
    const view = renderControls(candidateWorkflow({
      beginnerCandidateRequestStatus: 'cancelled',
    }), gridWorkflow({
      beginnerGrid: { candidates: [gridCandidate(true)] },
      beginnerGridRequestStatus: 'failed',
    }))

    const cancelled = screen.getByText(
      'Candidate generation was cancelled. Previous candidate authority was discarded.',
    )
    expect(cancelled.getAttribute('role')).toBe('status')
    const failed = screen.getByRole('alert')
    expect(failed.textContent).toContain('Grid evaluation failed')
    expect(failed.getAttribute('aria-live')).toBe('assertive')
    expect(screen.queryByText('Revalidate and apply selected candidate'))
      .toBeNull()

    view.rerender(
      <BeginnerCandidateControls
        locale="en"
        coreBusy={false}
        recoveryBlocking={false}
        skeletonTreeStatus="tree"
        candidateWorkflow={candidateWorkflow({
          beginnerCandidateRequestStatus: 'empty',
        }) as never}
        gridWorkflow={gridWorkflow({
          beginnerGridRequestStatus: 'empty',
        }) as never}
      />,
    )
    expect(screen.getByText(
      'Candidate generation returned no applicable candidates.',
    ).getAttribute('role')).toBe('status')
    expect(screen.getByText(
      'Grid evaluation returned no applicable candidates.',
    ).getAttribute('aria-live')).toBe('polite')
  })

  it('uses one live region for a returned empty candidate response', () => {
    const emptyResponse = {
      ...readyCandidateResponse(),
      generation_status: 'missing_target_category',
      candidates: [],
      generated_plans: [],
      plan_assessments: [],
    }
    const workflow = candidateWorkflow({
      beginnerCandidates: emptyResponse,
      beginnerCandidateRequestStatus: 'empty',
    })
    render(
      <>
        <BeginnerCandidateControls
          locale="en"
          coreBusy={false}
          recoveryBlocking={false}
          skeletonTreeStatus="tree"
          candidateWorkflow={workflow as never}
          gridWorkflow={gridWorkflow() as never}
        />
        <BeginnerCandidateResults
          locale="en"
          snapshot={snapshot()}
          coreBusy={false}
          recoveryBlocking={false}
          candidateWorkflow={workflow as never}
        />
      </>,
    )

    const statuses = screen.getAllByRole('status')
    expect(statuses).toHaveLength(1)
    expect(statuses[0]?.textContent).toContain(
      'Save an animal or insect target category first',
    )
  })

  it('blocks grid apply when proof scope and apply authority do not both allow it', () => {
    const apply = vi.fn()
    renderControls(candidateWorkflow(), gridWorkflow({
      beginnerGrid: {
        evaluated_grid_points: 27,
        grid_hash: [],
        candidates: [gridCandidate(false)],
      },
      beginnerGridSelectedPointId: 0,
      beginnerGridRequestStatus: 'ready',
      confirmAndApplyBeginnerGridCandidate: apply,
    }))

    expect(screen.getByText('Blocked')).toBeTruthy()
    expect(screen.getByRole('region', {
      name: 'Top 3 from the 27-design search',
    }).getAttribute('aria-live')).toBe('polite')
    const selectedApply = screen.getByRole('button', {
      name: 'Revalidate and apply selected candidate',
    }) as HTMLButtonElement
    expect(selectedApply.disabled).toBe(true)
    fireEvent.click(selectedApply)
    expect(apply).not.toHaveBeenCalled()
    expect(screen.queryByRole('button', {
      name: 'Revalidate and apply this design',
    })).toBeNull()
  })

  it('announces and blocks a candidate whose apply flag lacks sufficient proof', () => {
    const apply = vi.fn()
    render(
      <BeginnerCandidateResults
        locale="en"
        snapshot={snapshot()}
        coreBusy={false}
        recoveryBlocking={false}
        candidateWorkflow={candidateWorkflow({
          beginnerCandidates: readyCandidateResponse(),
          beginnerCandidateRequestStatus: 'ready',
          confirmAndApplyBeginnerPlan: apply,
        }) as never}
      />,
    )

    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain(
      'This candidate is unavailable to apply.',
    )
    expect(alert.getAttribute('aria-live')).toBe('assertive')
    const button = screen.getByRole('button', {
      name: 'Review and apply this bounded generated candidate',
    }) as HTMLButtonElement
    expect(button.disabled).toBe(true)
    fireEvent.click(button)
    expect(apply).not.toHaveBeenCalled()
  })
})
