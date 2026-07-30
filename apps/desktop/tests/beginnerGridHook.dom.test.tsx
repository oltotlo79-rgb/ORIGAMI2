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

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import {
  evaluateBeginnerParameterGrid,
  type BeginnerDesignProfileV1,
  type ProjectSnapshot,
} from '../src/lib/coreClient.ts'
import { useBeginnerParameterGridWorkflow } from '../src/lib/useBeginnerParameterGridWorkflow.ts'

afterEach(() => {
  cleanup()
  nativeInvoke.mockReset()
})

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const GRID_GENERATION_ID = '33333333-3333-4333-8333-333333333333'
const GRID_AUTHORITY_ID = '44444444-4444-4444-8444-444444444444'
const CANONICAL_GRID_HASH_V1 = [
  224, 59, 9, 238, 119, 51, 70, 177,
  12, 139, 19, 69, 142, 139, 157, 2,
  55, 85, 134, 120, 49, 93, 4, 65,
  125, 141, 52, 157, 74, 39, 236, 192,
] as const

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

function gridResponse(
  project: ProjectSnapshot,
  authorityToken = '44444444-4444-4444-8444-444444444444',
) {
  return {
    request_generation_id: '33333333-3333-4333-8333-333333333333',
    authority_token: authorityToken,
    project_instance_id: project.project_instance_id,
    project_id: project.project_id,
    revision: project.revision,
    evaluated_grid_points: 27,
    global_checked_candidates: 3,
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

function contractUuid(namespace: number, index: number) {
  return `${namespace.toString(16).padStart(8, '0')}`
    + `-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
}

function genericGridProfile(
  protrusionIds: readonly number[] = [7, 9],
): BeginnerDesignProfileV1 {
  return {
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
      target_category: 'custom_object',
      custom_object_display_name: 'Grid contract target',
      target_parts: [],
      skeleton_segments: [
        {
          id: 10,
          start: { x_tenths_mm: 0, y_tenths_mm: 0 },
          end: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
          thickness_tenths_mm: 10,
        },
        {
          id: 20,
          start: { x_tenths_mm: 1_000, y_tenths_mm: 0 },
          end: { x_tenths_mm: 1_000, y_tenths_mm: 500 },
          thickness_tenths_mm: 10,
        },
      ],
      protrusions: protrusionIds.map((id, index) => ({
        id,
        count: 1,
        length_tenths_mm: 100,
        thickness_tenths_mm: 10,
        position_tenths_mm:
          [index * 10, 0, 0] as [number, number, number],
        direction_milli: [1_000, 0, 0] as [number, number, number],
        symmetry: 'none',
        curvature_degrees: 0,
        joint: 'fixed',
        motion_degrees: [0, 0] as [number, number],
        side: 'either',
        priority: 50,
      })),
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }
}

function genericGridContractResponse(
  profile: BeginnerDesignProfileV1 = genericGridProfile(),
) {
  const rootVertexId = contractUuid(1, 1)
  const firstVertexId = contractUuid(1, 2)
  const secondVertexId = contractUuid(1, 3)
  const firstEdgeId = contractUuid(2, 1)
  const supportVertexIds = Array.from(
    { length: 4 },
    (_, index) => contractUuid(3, index + 1),
  )
  const supportEdgeIds = Array.from(
    { length: 4 },
    (_, index) => contractUuid(4, index + 1),
  )
  const treeVertexIds = Array.from(
    { length: 3 },
    (_, index) => contractUuid(5, index + 1),
  )
  const sourceIds = (
    profile.generation_constraints.protrusions ?? []
  ).map((protrusion) => protrusion.id)
  const skeletonSegments =
    profile.generation_constraints.skeleton_segments.map((segment) => ({
      ...segment,
      start: { ...segment.start },
      end: { ...segment.end },
    }))
  const plan = {
    schema_version: 1,
    kind: 'composite_generic_target_base',
    crease_pattern: {
      vertices: [
        { id: rootVertexId, position: { x: 0, y: 0 } },
        { id: firstVertexId, position: { x: 1, y: 0 } },
        { id: secondVertexId, position: { x: 0, y: 1 } },
        ...supportVertexIds.map((id, index) => ({
          id,
          position: { x: index + 2, y: 2 },
        })),
        ...treeVertexIds.map((id, index) => ({
          id,
          position: { x: index + 10, y: 10 },
        })),
      ],
      edges: [
        ...supportEdgeIds.map((id, index) => ({
          id,
          start: rootVertexId,
          end: supportVertexIds[index]!,
          kind: 'valley',
        })),
        {
          id: firstEdgeId,
          start: rootVertexId,
          end: firstVertexId,
          kind: 'valley',
        },
        {
          id: contractUuid(2, 2),
          start: rootVertexId,
          end: secondVertexId,
          kind: 'valley',
        },
        {
          id: contractUuid(6, 1),
          start: treeVertexIds[0]!,
          end: treeVertexIds[1]!,
          kind: 'auxiliary',
        },
        {
          id: contractUuid(6, 2),
          start: treeVertexIds[1]!,
          end: treeVertexIds[2]!,
          kind: 'auxiliary',
        },
      ],
    },
    instruction_codes: [
      'bounded_tree_river_axial_v1:4000000,1000000',
      'bounded_radial_corner_support_v1:added=4:covered=4',
      'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
      'bounded_tree_paper_orientation_v1:horizontal',
    ],
    target_parts: [],
    skeleton_segments: skeletonSegments,
    target_asset: null,
  }
  const assessment = {
    kind: plan.kind,
    expected_candidate_edge_id: supportEdgeIds[0]!,
    proof_scope: 'sufficient',
    apply_allowed: true,
    reason: 'native_fold_path_certified',
    shape_approximation_score: null,
    shape_difference_reason: null,
    component_shape_comparison: null,
  }
  return {
    request_generation_id: GRID_GENERATION_ID,
    authority_token: GRID_AUTHORITY_ID,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 1,
    grid_hash: Array.from(CANONICAL_GRID_HASH_V1),
    evaluated_grid_points: 27,
    global_checked_candidates: 3,
    refinement_iterations: 0,
    candidates: [{
      point: {
        id: 0,
        scale_percent: 10,
        spacing_percent: 20,
        detail_level: 'simple',
      },
      primary_score: 690,
      plan,
      assessment,
      local_proof_scope: 'necessary',
      global_proof_scope: 'sufficient',
      complexity_score: 90,
      paper_efficiency_score: 50,
      scale_deviation_penalty: 150,
      spacing_deviation_penalty: 150,
      detail_mismatch_penalty: 10,
      outcome_reason: assessment.reason,
      contour_witness: {
        body_contour_points: 0,
        local_bindings: [],
        generic_feature_bindings: sourceIds.map((protrusionId, index) => ({
          protrusion_id: protrusionId,
          generated_feature_id: index + 1,
          endpoint_count: 1,
          crease_start: 4 + index,
          crease_authority_sha256: Array(32).fill(4 + index),
          skeleton_segment_id: index === 0 ? 10 : 20,
          skeleton_endpoint: index === 0 ? 'start' : 'end',
          mount_distance_squared_tenths_mm: 0,
        })),
        skeleton_branch_bindings: [
          {
            segment_id: 10,
            parent_segment_id: null,
            parent_endpoint: null,
            child_endpoint: null,
            generated_feature_ids: [1],
          },
          {
            segment_id: 20,
            parent_segment_id: 10,
            parent_endpoint: 'end',
            child_endpoint: 'start',
            generated_feature_ids: [2],
          },
        ],
        skeleton_tree_authority_sha256: Array(32).fill(2),
        witnessed_vertices: 0,
        witnessed_creases: 0,
        topology_authority_hash: Array(32).fill(3),
        max_contour_error_millionths: 0,
      },
      refinement_iterations: 0,
      strict_improvements: 0,
      refinement_starts: 1,
    }],
  }
}

function evaluateGridContract(
  response: unknown,
  profile: BeginnerDesignProfileV1 = genericGridProfile(),
) {
  nativeInvoke.mockResolvedValueOnce(response)
  return evaluateBeginnerParameterGrid(
    PROJECT_ID,
    1,
    INSTANCE_ID,
    GRID_GENERATION_ID,
    profile,
  )
}

function sparseSha256() {
  const value: number[] = []
  value.length = 32
  return value
}

function GridHarness({
  project,
  transport,
  runNativeEdit,
  startPolling,
  stopPolling,
  scheduleFocus = (callback) => callback(),
}: {
  project: ProjectSnapshot
  transport: Record<string, unknown>
  runNativeEdit: ReturnType<typeof vi.fn>
  startPolling: (callback: () => void) => number
  stopPolling: ReturnType<typeof vi.fn>
  scheduleFocus?: (callback: () => void) => void
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
    scheduleFocus,
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
      <button onClick={
        workflow.invalidateBeginnerGridForProjectReplacement
      }>
        replace project
      </button>
      <output data-testid="grid-busy">{String(workflow.beginnerGridBusy)}</output>
      <output data-testid="grid-apply-busy">
        {String(workflow.beginnerGridApplyBusy)}
      </output>
      <output data-testid="grid-status">
        {workflow.beginnerGridRequestStatus}
      </output>
      <output data-testid="grid-progress">
        {workflow.beginnerGridProgress.enumerated}
      </output>
      <output data-testid="grid-global-checked">
        {workflow.beginnerGridProgress.globalChecked}
      </output>
      <output data-testid="grid-refined">
        {workflow.beginnerGridProgress.refined}
      </output>
      {workflow.beginnerGrid?.candidates[0] && (
        <button
          disabled={workflow.beginnerGridApplyBusy}
          onClick={() => workflow.confirmAndApplyBeginnerGridCandidate(
            workflow.beginnerGrid!.candidates[0]!,
          )}
        >
          apply grid
        </button>
      )}
    </>
  )
}

describe('beginner parameter-grid response binding contract', () => {
  it('rejects a generic binding that aggregates multiple endpoints', async () => {
    const profile = genericGridProfile()
    const response = genericGridContractResponse(profile)
    const witness = response.candidates[0]!.contour_witness
    witness.generic_feature_bindings[0]!.endpoint_count = 2
    witness.generic_feature_bindings.splice(1, 1)
    witness.skeleton_branch_bindings[1]!.generated_feature_ids = []

    await expect(evaluateGridContract(response, profile)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('preserves singleton source IDs and rejects replacement IDs', async () => {
    const profile = genericGridProfile([7, 9])
    const admitted = await evaluateGridContract(
      genericGridContractResponse(profile),
      profile,
    )

    expect(
      admitted.candidates[0]!.contour_witness.generic_feature_bindings.map(
        (binding) => binding.protrusion_id,
      ),
    ).toEqual([7, 9])

    const replaced = genericGridContractResponse(profile)
    replaced.candidates[0]!.contour_witness
      .generic_feature_bindings[0]!.protrusion_id = 1
    replaced.candidates[0]!.contour_witness
      .generic_feature_bindings[1]!.protrusion_id = 2
    await expect(evaluateGridContract(replaced, profile)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('snapshots and freezes all admitted grid SHA evidence', async () => {
    const profile = genericGridProfile()
    const response = genericGridContractResponse(profile)
    const sourceWitness = response.candidates[0]!.contour_witness
    const admitted = await evaluateGridContract(response, profile)
    const witness = admitted.candidates[0]!.contour_witness

    expect(Object.isFrozen(
      witness.skeleton_tree_authority_sha256,
    )).toBe(true)
    expect(Object.isFrozen(witness.topology_authority_hash)).toBe(true)
    expect(Object.isFrozen(
      witness.generic_feature_bindings[0]!.crease_authority_sha256,
    )).toBe(true)

    sourceWitness.skeleton_tree_authority_sha256[0] = 9
    sourceWitness.topology_authority_hash[0] = 9
    sourceWitness.generic_feature_bindings[0]!
      .crease_authority_sha256[0] = 9
    expect(witness.skeleton_tree_authority_sha256[0]).toBe(2)
    expect(witness.topology_authority_hash[0]).toBe(3)
    expect(
      witness.generic_feature_bindings[0]!.crease_authority_sha256[0],
    ).toBe(4)
  })

  const invalidShaCases: ReadonlyArray<readonly [
    string,
    (response: ReturnType<typeof genericGridContractResponse>) => void,
  ]> = [
    ['negative skeleton SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .skeleton_tree_authority_sha256[0] = -1
    }],
    ['256 skeleton SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .skeleton_tree_authority_sha256[0] = 256
    }],
    ['sparse skeleton SHA', (response) => {
      response.candidates[0]!.contour_witness
        .skeleton_tree_authority_sha256 = sparseSha256()
    }],
    ['negative topology SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .topology_authority_hash[0] = -1
    }],
    ['256 topology SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .topology_authority_hash[0] = 256
    }],
    ['sparse topology SHA', (response) => {
      response.candidates[0]!.contour_witness
        .topology_authority_hash = sparseSha256()
    }],
    ['negative feature SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .generic_feature_bindings[0]!.crease_authority_sha256[0] = -1
    }],
    ['256 feature SHA byte', (response) => {
      response.candidates[0]!.contour_witness
        .generic_feature_bindings[0]!.crease_authority_sha256[0] = 256
    }],
    ['sparse feature SHA', (response) => {
      response.candidates[0]!.contour_witness
        .generic_feature_bindings[0]!.crease_authority_sha256 = sparseSha256()
    }],
  ]

  it.each(invalidShaCases)('rejects %s', async (_label, mutate) => {
    const profile = genericGridProfile()
    const response = genericGridContractResponse(profile)
    mutate(response)

    await expect(evaluateGridContract(response, profile)).rejects.toThrow(
      'invalid beginner parameter grid response',
    )
  })

  it('retains the expected-profile snapshot across pending evaluation', async () => {
    const profile = genericGridProfile([7, 9])
    const response = genericGridContractResponse(profile)
    const nativeResult = deferred<unknown>()
    nativeInvoke.mockReturnValueOnce(nativeResult.promise)

    const pending = evaluateBeginnerParameterGrid(
      PROJECT_ID,
      1,
      INSTANCE_ID,
      GRID_GENERATION_ID,
      profile,
    )
    profile.generation_constraints.protrusions![0]!.id = 1
    nativeResult.resolve(response)

    const admitted = await pending
    expect(
      admitted.candidates[0]!.contour_witness.generic_feature_bindings.map(
        (binding) => binding.protrusion_id,
      ),
    ).toEqual([7, 9])
  })
})

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
    expect(transport.evaluate).toHaveBeenCalledWith(
      project.project_id,
      project.revision,
      project.project_instance_id,
      'grid-generation',
      project.beginner_design_profile,
    )
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
    expect(screen.getByTestId('grid-progress').textContent).toBe('0')
    expect(screen.getByTestId('grid-status').textContent).toBe('cancelled')
    await act(() => {
      evaluation.resolve(gridResponse(project))
      return evaluation.promise
    })
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()
  })

  it('invalidates a running grid request on project replacement', async () => {
    const project = snapshot()
    const evaluation = deferred<Record<string, unknown>>()
    const cancel = vi.fn(async () => undefined)
    const stopPolling = vi.fn()
    render(
      <GridHarness
        project={project}
        transport={{
          evaluate: vi.fn(() => evaluation.promise),
          progress: vi.fn(),
          cancel,
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 7}
        stopPolling={stopPolling}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    expect(screen.getByTestId('grid-status').textContent).toBe('running')
    fireEvent.click(screen.getByRole('button', { name: 'replace project' }))
    expect(cancel).toHaveBeenCalledWith('grid-generation')
    expect(stopPolling).toHaveBeenCalledWith(7)
    expect(screen.getByTestId('grid-status').textContent).toBe('idle')
    await act(() => {
      evaluation.resolve(gridResponse(project))
      return evaluation.promise
    })
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()
  })

  it('cancels grid work and ignores late results after unmount', async () => {
    const project = snapshot()
    const evaluation = deferred<Record<string, unknown>>()
    const cancel = vi.fn(async () => undefined)
    const stopPolling = vi.fn()
    const view = render(
      <GridHarness
        project={project}
        transport={{
          evaluate: vi.fn(() => evaluation.promise),
          progress: vi.fn(),
          cancel,
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 7}
        stopPolling={stopPolling}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    view.unmount()
    expect(cancel).toHaveBeenCalledWith('grid-generation')
    expect(stopPolling).toHaveBeenCalledWith(7)
    await act(() => {
      evaluation.resolve(gridResponse(project))
      return evaluation.promise
    })
  })

  it('reports empty and failed evaluations without exposing apply authority', async () => {
    const project = snapshot()
    const stopPolling = vi.fn()
    const baseTransport = {
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
    }
    const view = render(
      <GridHarness
        project={project}
        transport={{
          ...baseTransport,
          evaluate: vi.fn(async () => ({
            ...gridResponse(project),
            candidates: [],
          })),
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 1}
        stopPolling={stopPolling}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await waitFor(() => expect(
      screen.getByTestId('grid-status').textContent,
    ).toBe('empty'))
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()

    view.rerender(
      <GridHarness
        project={project}
        transport={{
          ...baseTransport,
          evaluate: vi.fn(async () => {
            throw new Error('grid failed')
          }),
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 1}
        stopPolling={stopPolling}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await waitFor(() => expect(
      screen.getByTestId('grid-status').textContent,
    ).toBe('failed'))
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()

    view.rerender(
      <GridHarness
        project={project}
        transport={{
          ...baseTransport,
          evaluate: vi.fn(async () => gridResponse(project, '')),
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 1}
        stopPolling={stopPolling}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await waitFor(() => expect(
      screen.getByTestId('grid-status').textContent,
    ).toBe('failed'))
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()

    view.rerender(
      <GridHarness
        project={project}
        transport={{
          ...baseTransport,
          evaluate: vi.fn(async () => gridResponse(project)),
        }}
        runNativeEdit={vi.fn(async () => true)}
        startPolling={() => 1}
        stopPolling={stopPolling}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await screen.findByRole('button', { name: 'apply grid' })
    expect(screen.getByTestId('grid-status').textContent).toBe('ready')
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

  it('retains the registry authority token through the apply workflow', async () => {
    const project = snapshot()
    const response = gridResponse(project)
    const apply = vi.fn(async () => snapshot(2))
    const transport = {
      evaluate: vi.fn(async () => response),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply,
    }
    const runNativeEdit = vi.fn(async (
      edit: (
        projectId: string,
        revision: number,
        projectInstanceId: string,
      ) => Promise<ProjectSnapshot>,
    ) => {
      await edit(
        project.project_id,
        project.revision,
        project.project_instance_id,
      )
      return true
    })
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    fireEvent.click(await screen.findByRole('button', { name: 'apply grid' }))

    await waitFor(() => expect(apply).toHaveBeenCalledOnce())
    expect(apply.mock.calls[0]?.[3]).toMatchObject({
      request_generation_id: response.request_generation_id,
      authority_token: response.authority_token,
    })
    await waitFor(() => expect(
      screen.queryByRole('button', { name: 'apply grid' }),
    ).toBeNull())
  })

  it('applies only the newest registry token after grid replacement', async () => {
    const project = snapshot()
    const oldResponse = gridResponse(
      project,
      '44444444-4444-4444-8444-444444444444',
    )
    const currentResponse = gridResponse(
      project,
      '55555555-5555-4555-8555-555555555555',
    )
    const apply = vi.fn(async () => snapshot(2))
    const evaluate = vi.fn()
      .mockResolvedValueOnce(oldResponse)
      .mockResolvedValueOnce(currentResponse)
    const transport = {
      evaluate,
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply,
    }
    const runNativeEdit = vi.fn(async (
      edit: (
        projectId: string,
        revision: number,
        projectInstanceId: string,
      ) => Promise<ProjectSnapshot>,
    ) => {
      await edit(
        project.project_id,
        project.revision,
        project.project_instance_id,
      )
      return true
    })
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await screen.findByRole('button', { name: 'apply grid' })
    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await waitFor(() => expect(evaluate).toHaveBeenCalledTimes(2))
    fireEvent.click(screen.getByRole('button', { name: 'apply grid' }))

    await waitFor(() => expect(apply).toHaveBeenCalledOnce())
    expect(apply.mock.calls[0]?.[3]).toMatchObject({
      authority_token: currentResponse.authority_token,
    })
    expect(apply.mock.calls[0]?.[3]).not.toMatchObject({
      authority_token: oldResponse.authority_token,
    })
  })

  it('coalesces rapid apply attempts and resets apply state on success', async () => {
    const project = snapshot()
    const applied = deferred<ProjectSnapshot>()
    const apply = vi.fn(() => applied.promise)
    const transport = {
      evaluate: vi.fn(async () => gridResponse(project)),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply,
    }
    const runNativeEdit = vi.fn(async (
      edit: (
        projectId: string,
        revision: number,
        projectInstanceId: string,
      ) => Promise<ProjectSnapshot>,
    ) => {
      await edit(
        project.project_id,
        project.revision,
        project.project_instance_id,
      )
      return true
    })
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    const applyButton = await screen.findByRole('button', {
      name: 'apply grid',
    })
    fireEvent.click(applyButton)
    fireEvent.click(applyButton)

    await waitFor(() => expect(apply).toHaveBeenCalledOnce())
    expect(screen.getByTestId('grid-apply-busy').textContent).toBe('true')
    await act(() => {
      applied.resolve(snapshot(2))
      return applied.promise
    })
    await waitFor(() => expect(
      screen.getByTestId('grid-apply-busy').textContent,
    ).toBe('false'))
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()
  })

  it('does not restore focus or publish state after a late unmounted apply', async () => {
    const project = snapshot()
    const applied = deferred<boolean>()
    const scheduleFocus = vi.fn((callback: () => void) => callback())
    const transport = {
      evaluate: vi.fn(async () => gridResponse(project)),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply: vi.fn(async () => snapshot(2)),
    }
    const view = render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={vi.fn(() => applied.promise)}
        startPolling={() => 1}
        stopPolling={vi.fn()}
        scheduleFocus={scheduleFocus}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    fireEvent.click(await screen.findByRole('button', {
      name: 'apply grid',
    }))
    view.unmount()
    await act(() => {
      applied.resolve(true)
      return applied.promise
    })
    expect(scheduleFocus).not.toHaveBeenCalled()
  })

  it('resets single-consume apply state after a failed native edit', async () => {
    const project = snapshot()
    const transport = {
      evaluate: vi.fn(async () => gridResponse(project)),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply: vi.fn(async () => snapshot(2)),
    }
    const runNativeEdit = vi.fn(async () => false)
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    const applyButton = await screen.findByRole('button', {
      name: 'apply grid',
    })
    fireEvent.click(applyButton)
    await waitFor(() => expect(
      screen.getByTestId('grid-apply-busy').textContent,
    ).toBe('false'))
    expect((applyButton as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(applyButton)
    await waitFor(() => expect(runNativeEdit).toHaveBeenCalledTimes(2))
  })

  it('absorbs a rejected native edit and permits a clean retry', async () => {
    const project = snapshot()
    const transport = {
      evaluate: vi.fn(async () => gridResponse(project)),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply: vi.fn(async () => snapshot(2)),
    }
    const runNativeEdit = vi.fn()
      .mockRejectedValueOnce(new Error('native edit failed'))
      .mockResolvedValueOnce(false)
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={runNativeEdit}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    const applyButton = await screen.findByRole('button', {
      name: 'apply grid',
    })
    fireEvent.click(applyButton)
    await waitFor(() => expect(
      screen.getByTestId('grid-apply-busy').textContent,
    ).toBe('false'))
    expect((applyButton as HTMLButtonElement).disabled).toBe(false)

    fireEvent.click(applyButton)
    await waitFor(() => expect(runNativeEdit).toHaveBeenCalledTimes(2))
  })

  it('uses response progress values and clears them on project replacement', async () => {
    const project = snapshot()
    const response = {
      ...gridResponse(project),
      evaluated_grid_points: 19,
      global_checked_candidates: 2,
      refinement_iterations: 4,
    }
    const transport = {
      evaluate: vi.fn(async () => response),
      progress: vi.fn(),
      cancel: vi.fn(async () => undefined),
      apply: vi.fn(async () => snapshot(2)),
    }
    render(
      <GridHarness
        project={project}
        transport={transport}
        runNativeEdit={vi.fn(async () => false)}
        startPolling={() => 1}
        stopPolling={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'evaluate grid' }))
    await screen.findByRole('button', { name: 'apply grid' })
    expect(screen.getByTestId('grid-progress').textContent).toBe('19')
    expect(screen.getByTestId('grid-global-checked').textContent).toBe('2')
    expect(screen.getByTestId('grid-refined').textContent).toBe('4')

    fireEvent.click(screen.getByRole('button', { name: 'replace project' }))
    expect(screen.queryByRole('button', { name: 'apply grid' })).toBeNull()
    expect(screen.getByTestId('grid-progress').textContent).toBe('0')
    expect(screen.getByTestId('grid-global-checked').textContent).toBe('0')
    expect(screen.getByTestId('grid-refined').textContent).toBe('0')
  })
})
