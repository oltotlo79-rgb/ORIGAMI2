import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { StackedFoldPanel } from '../src/components/StackedFoldPanel'
import type { ProjectSnapshot } from '../src/lib/coreClient'

const transport = vi.hoisted(() => ({
  preview: vi.fn(),
  apply: vi.fn(),
  cancel: vi.fn(),
  cancelRead: vi.fn(),
  registry: vi.fn(),
  progress: null as null | ((value: any) => void),
}))

vi.mock('../src/lib/coreClient', async (importOriginal) => ({
  ...await importOriginal<typeof import('../src/lib/coreClient')>(),
  proposeCurrentStackedFoldRead: transport.preview,
  applyStackedFoldTransaction: transport.apply,
  cancelStackedFoldTransactionPreview: transport.cancel,
  cancelCurrentStackedFoldReadV1: transport.cancelRead,
  readLiveHingeRegistryV1: transport.registry,
  listenStackedFoldReadProgressV1: vi.fn(async (callback) => {
    transport.progress = callback
    return () => {
      transport.progress = null
    }
  }),
}))

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'
const token = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

const snapshot = {
  project_instance_id: instance,
  project_id: project,
  revision: 3,
} as ProjectSnapshot

const ready = {
  guardModelId: 'native_flat_stacked_fold_read_guard_v1',
  proposalModelId: 'native_linear_stacked_fold_read_proposal_v1',
  materialMapModelId: 'native_flat_stacked_fold_material_map_v1',
  binding: {
    projectInstanceId: instance,
    projectId: project,
    sourceRevision: 3,
    poseGeneration: 1,
    layerOrderGeneration: 1,
  },
  support: 'bit_exact_flat_endpoint_tree',
  crossedCells: [{
    cellKeySha256: 'c'.repeat(64),
    bottomToTopFaces: [project, project],
    boundaryWorld: [[0, 0, 0], [20, 0, 0], [20, 0, -10], [0, 0, -10]],
  }],
  targetFaces: [project],
  materialSegments: [{
    faceId: project,
    start: [1, 2],
    end: [3, 4],
    fixedSide: 'left',
    assignment: 'mountain',
  }],
  topologyProof: {
    targetFingerprintSha256: 'a'.repeat(64),
    targetVertexCount: 5,
    targetEdgeCount: 6,
    targetBoundaryVertexCount: 4,
    lineageRecordCount: 2,
    sourceEdgeSubdivisionCount: 1,
    expectedCreaseSubdivisionCount: 1,
    targetMaterialFaceCount: 3,
    targetHingeCount: 2,
  },
  liveGraphHingeAngles: [
    { edge: project, initialAngleDegrees: 0 },
    { edge: token, initialAngleDegrees: 0 },
  ],
  endpointCollision: {
    expectedPairCount: 3,
    separatedPairCount: 0,
    touchingPairCount: 0,
    allowedPairCount: 3,
    penetratingPairCount: 0,
    indeterminatePairCount: 0,
    hasBlockingHold: false,
  },
  continuousPath: {
    modelId: 'stacked_fold_bounded_path_diagnostic_v1',
    continuousCertificateModelId: 'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1',
    paperThicknessMm: 0.1,
    sampledPoseCount: 2,
    sampledNonblockingPoseCount: 2,
    intervalLeafCount: 8,
    intervalPairWork: 8,
    intervalCandidateLimit: 2048,
    positiveEndpointCandidateCount: 64,
    positiveEndpointExactPairCalls: 0,
    positiveEndpointCandidateLimit: 120,
    closureRequired: false,
    closureLeafCount: 0,
    closurePairWork: 0,
    firstClosureFailureAngleDegrees: null,
    firstSampledBlockingAngleDegrees: null,
    requestedAngleDegrees: 180,
    continuousClearanceCertified: true,
    safeStopAngleDegrees: 180,
    authorizesProjectMutation: false,
  },
  certifiedPathGraph: null,
  flatEndpointLayerOrder: {
    applicable: true,
    certified: true,
    materialFaceCount: 3,
    overlapCellCount: 1,
  },
  transactionProposal: {
    transactionToken: token,
    sourceProjectId: project,
    sourceRevision: 3,
    targetRevision: 4,
    sourceFingerprintSha256: 'b'.repeat(64),
    targetFingerprintSha256: 'a'.repeat(64),
    readyForAtomicApply: true,
    failureClasses: [],
    authorizesProjectMutation: true,
    addedVertexCount: 1,
    addedEdgeCount: 2,
    mountainCreaseCount: 1,
    valleyCreaseCount: 0,
    timelineStepCount: 1,
    timelineCompleteHingeAngleCount: 2,
    requestedAngleDegrees: 180,
  },
  work: {
    scannedCells: 0,
    totalBoundaryVertices: 4,
    totalLayerRecords: 2,
    orientationTests: 1,
    exactArithmeticOperations: 1,
    maximumExactIntegerBits: 64,
    totalExactIntegerBits: 64,
    retainedCells: 1,
    retainedTargetFaces: 1,
  },
  authorizesProjectMutation: false,
  authorizesApplyStackedFold: false,
}

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

beforeEach(() => {
  transport.progress = null
  transport.cancelRead.mockResolvedValue(undefined)
  transport.registry.mockResolvedValue({
    version: 1,
    projectInstanceId: instance,
    projectId: project,
    revision: 12,
    poseGeneration: 4,
    graphFingerprintSha256: 'a'.repeat(64),
    entries: [
      { edge: project, initialAngleDegrees: 10 },
      { edge: token, initialAngleDegrees: 20 },
    ],
    authorizesProjectMutation: false,
  })
})

describe('StackedFoldPanel', () => {
  it('offers cooperative cancellation while a bounded path read is pending', async () => {
    transport.preview.mockReturnValue(new Promise(() => undefined))
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await waitFor(() => expect(transport.progress).not.toBeNull())
    const request = transport.preview.mock.calls[0]?.[0]
    transport.progress?.({
      version: 1,
      requestId: request.progressRequestId,
      exploredStateCount: 2,
      evaluatedTransitionCount: 3,
      stateLimit: 32,
      transitionLimit: 64,
      authorizesProjectMutation: false,
    })
    expect((await screen.findByRole('status')).textContent).toBe(
      'Explored states 2/32; transitions 3/64',
    )
    fireEvent.click(await screen.findByRole('button', {
      name: 'Cancel path analysis',
    }))
    expect(transport.cancelRead).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('status')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Apply stacked fold' })).toBeNull()
  })

  it('bootstraps canonical linear candidate entries from the read-only live registry', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    const requested = await screen.findByLabelText(`Requested angle ${project}`)
    fireEvent.change(requested, { target: { value: '30' } })
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await waitFor(() => expect(transport.preview).toHaveBeenCalledWith(expect.objectContaining({
      linearCandidateV1: {
        version: 1,
        entries: [
          { edge: project, initialAngleDegrees: 10, requestedAngleDegrees: 30 },
          { edge: token, initialAngleDegrees: 20, requestedAngleDegrees: 20 },
        ],
      },
    })))
  })

  it('passes an explicitly authored versioned cycle schedule to native proof', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    const schedule = {
      version: 1,
      entries: [{
        edge: token,
        uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }],
        numeratorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        requestedAngleDegrees: 90,
      }],
    }
    fireEvent.change(screen.getByLabelText('Cycle path definition (JSON, cyclic patterns only)'), {
      target: { value: JSON.stringify(schedule) },
    })
    expect(await screen.findByRole('status')).toHaveProperty(
      'textContent',
      'Bounded schedule: 1/64 hinges; at most 9 coefficients each',
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByText('Target faces')
    expect(transport.preview).toHaveBeenCalledWith(expect.objectContaining({
      cycleScheduleV1: schedule,
    }))
  })

  it('rejects an unbounded or malformed half-angle draft before native transport', async () => {
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    fireEvent.change(screen.getByLabelText('Cycle path definition (JSON, cyclic patterns only)'), {
      target: { value: JSON.stringify({
        version: 1,
        entries: [{
          edge: token,
          uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }],
          numeratorPowerCoefficients: [{ numerator: 1, denominator: 0 }],
          denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
          requestedAngleDegrees: 90,
        }],
      }) },
    })
    expect(await screen.findByRole('status')).toHaveProperty(
      'textContent',
      'Invalid schedule. Denominators must be positive integers, coefficients 1–9 each, and angles 0–180°.',
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByRole('alert')
    expect(transport.preview).not.toHaveBeenCalled()
  })

  it('keeps a closure-certified graph transaction ready and exposes bounded closure work', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue({
      ...ready,
      continuousPath: {
        ...ready.continuousPath,
        modelId: 'stacked_fold_bounded_path_diagnostic_v1',
        continuousCertificateModelId: 'stacked_fold_cycle_interval_zero_thickness_continuous_certificate_v1',
        paperThicknessMm: 0,
        closureRequired: true,
        closureLeafCount: 12,
        closurePairWork: 7,
        requestedAngleDegrees: 180,
        safeStopAngleDegrees: 180,
      },
      transactionProposal: {
        ...ready.transactionProposal,
        requestedAngleDegrees: 180,
      },
    })
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByText('Closure leaves')
    expect(screen.getByText('12')).toBeTruthy()
    expect(screen.getByText('Closure pair work')).toBeTruthy()
    expect(screen.getByText('7')).toBeTruthy()
    const apply = screen.getByRole('button', { name: 'Apply stacked fold' })
    expect((apply as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(screen.getByRole('checkbox'))
    expect((apply as HTMLButtonElement).disabled).toBe(false)
  })

  it('shows every certified graph edge as read-only evidence and focuses its related hinge', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.apply.mockResolvedValue(13)
    transport.preview.mockResolvedValue({
      ...ready,
      certifiedPathGraph: {
        modelId: 'bounded_certified_pose_graph_path_v1',
        version: 1,
        sourceFingerprintSha256: '1'.repeat(64),
        targetFingerprintSha256: '2'.repeat(64),
        exploredStateCount: 2,
        evaluatedTransitionCount: 1,
        edges: [{
          sourceFingerprintSha256: '1'.repeat(64),
          targetFingerprintSha256: '2'.repeat(64),
          scheduleCertificateSha256: '3'.repeat(64),
          collisionCertificateSha256: '4'.repeat(64),
          closureCertificateSha256: '5'.repeat(64),
          hinges: [project],
        }],
        authorizesProjectMutation: false,
      },
      transactionProposal: {
        ...ready.transactionProposal,
        timelineStepCount: 1,
      },
    })
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const path = await screen.findByRole('region', {
      name: 'Certified candidate path',
    })
    expect(path.textContent).toContain('read-only preview')
    expect(path.textContent).toContain('3'.repeat(64))
    expect(path.textContent).toContain('4'.repeat(64))
    expect(path.textContent).toContain('5'.repeat(64))
    fireEvent.click(screen.getByRole('button', {
      name: /Select related hinge/u,
    }))
    expect(document.activeElement?.getAttribute('id')).toBe(
      `stacked-fold-proof-hinge-${project}`,
    )
    const apply = screen.getByRole('button', { name: 'Apply stacked fold' })
    expect((apply as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(screen.getByRole('checkbox'))
    expect((apply as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(apply)
    await waitFor(() => expect(transport.apply).toHaveBeenCalledWith(token))
    expect(transport.apply).toHaveBeenCalledTimes(1)
  })

  it('uses the selected canvas line and applies only after explicit confirmation', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    transport.apply.mockResolvedValue(4)
    const refreshed = { ...snapshot, revision: 4 } as ProjectSnapshot
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn().mockResolvedValue(refreshed)}
        onApplied={onApplied}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    expect((await screen.findAllByText('Certified')).length).toBe(2)
    expect(screen.getByText('stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1')).toBeTruthy()
    expect(screen.getByRole('img', { name: 'Exploded front/back layer stack' })).toBeTruthy()
    expect(screen.getByRole('button', { name: /Back \/ bottom/ })).toBeTruthy()
    const front = screen.getByRole('button', { name: /Front \/ top/ })
    fireEvent.mouseEnter(front)
    fireEvent.click(front)
    expect(front.getAttribute('aria-pressed')).toBe('true')
    expect(transport.preview).toHaveBeenCalledWith(expect.objectContaining({
      first: [1, 0, -2],
      second: [3, 0, -4],
    }))
    const apply = screen.getByRole('button', { name: 'Apply stacked fold' })
    expect((apply as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(screen.getByRole('checkbox'))
    expect((apply as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(apply)
    await waitFor(() => expect(transport.apply).toHaveBeenCalledWith(token))
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
  })

  it('keeps apply disabled when native metadata is not fully certified', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockRejectedValue(new Error('uncertified'))
    render(
      <StackedFoldPanel
        locale="ja"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 0, y: 0 }, end: { x: 1, y: 0 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '安全性を確認' }))
    expect(await screen.findByText('この入力ではnative証明を完成できませんでした。')).toBeTruthy()
    expect(screen.queryByRole('button', { name: '折り重ねを適用' })).toBeNull()
  })

  it('cancels the opaque preview when project authority becomes stale', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    const props = {
      locale: 'en' as const,
      selectedLine: { id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } },
      disabled: false,
      refreshSnapshot: vi.fn(),
      onApplied: vi.fn(),
    }
    const rendered = render(<StackedFoldPanel {...props} snapshot={snapshot} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByRole('button', { name: 'Apply stacked fold' })
    expect(screen.getByText('64 / 120')).toBeTruthy()
    expect(screen.getByText('Positive-thickness exact calls')).toBeTruthy()
    rendered.rerender(
      <StackedFoldPanel {...props} snapshot={{ ...snapshot, revision: 4 } as ProjectSnapshot} />,
    )
    await waitFor(() => expect(transport.cancel).toHaveBeenCalledWith(token))
    expect(screen.queryByRole('button', { name: 'Apply stacked fold' })).toBeNull()
  })

  it('retains a certified token for retry after a pre-commit apply failure', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    transport.apply.mockRejectedValueOnce(new Error('busy')).mockResolvedValueOnce(4)
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })}
        onApplied={onApplied}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByRole('button', { name: 'Apply stacked fold' })
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: 'Apply stacked fold' }))
    expect(await screen.findByText('Apply failed. You can retry with the same certified preview.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Apply stacked fold' }))
    await waitFor(() => expect(transport.apply).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(onApplied).toHaveBeenCalledOnce())
  })

  it('separates a committed apply from refresh failure and retries only refresh', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    transport.apply.mockResolvedValue(4)
    const refresh = vi.fn()
      .mockRejectedValueOnce(new Error('refresh'))
      .mockResolvedValueOnce({ ...snapshot, revision: 4 })
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={refresh}
        onApplied={onApplied}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    await screen.findByRole('button', { name: 'Apply stacked fold' })
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: 'Apply stacked fold' }))
    expect(await screen.findByText('The stacked fold was applied, but the refreshed project could not be loaded.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Retry refresh' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledOnce())
    expect(transport.apply).toHaveBeenCalledTimes(1)
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it.each([
    ['cycle_nonclosing', 'The cyclic hinge endpoint does not close, so apply is disabled.'],
    ['cycle_path_uncertified', 'The cyclic endpoint closes, but its continuous path is uncertified, so apply is disabled.'],
    ['cycle_path_unsupported', 'This input is outside the supported limited linear hinge-path class, so apply is disabled.'],
    ['cycle_path_resource_limit', 'The bounded proof reached its resource limit. This does not claim safety or impossibility, so apply is disabled.'],
    ['cycle_path_no_certified_path', 'No path to the target was found using certified transitions only. This does not claim impossibility.'],
    ['cycle_path_collision', 'The scheduled continuous path could not receive a collision-clearance certificate, so apply is disabled.'],
  ] as const)('shows the bounded cycle failure %s without an apply action', async (reason, copy) => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockRejectedValue({ reason })
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    expect(await screen.findByText(copy)).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Apply stacked fold' })).toBeNull()
  })
})
