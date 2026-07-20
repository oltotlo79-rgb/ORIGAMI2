import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { StackedFoldPanel } from '../src/components/StackedFoldPanel'
import type { ProjectSnapshot } from '../src/lib/coreClient'

const transport = vi.hoisted(() => ({
  preview: vi.fn(),
  apply: vi.fn(),
  cancel: vi.fn(),
}))

vi.mock('../src/lib/coreClient', async (importOriginal) => ({
  ...await importOriginal<typeof import('../src/lib/coreClient')>(),
  proposeCurrentStackedFoldRead: transport.preview,
  applyStackedFoldTransaction: transport.apply,
  cancelStackedFoldTransactionPreview: transport.cancel,
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
    targetMaterialFaceCount: 2,
    targetHingeCount: 2,
  },
  endpointCollision: {
    expectedPairCount: 0,
    separatedPairCount: 0,
    touchingPairCount: 0,
    allowedPairCount: 0,
    penetratingPairCount: 0,
    indeterminatePairCount: 0,
    hasBlockingHold: false,
  },
  continuousPath: {
    modelId: 'stacked_fold_bounded_path_diagnostic_v1',
    continuousCertificateModelId: 'stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1',
    paperThicknessMm: 0,
    sampledPoseCount: 2,
    sampledNonblockingPoseCount: 2,
    intervalLeafCount: 8,
    intervalPairWork: 8,
    intervalCandidateLimit: 2048,
    firstSampledBlockingAngleDegrees: null,
    requestedAngleDegrees: 180,
    continuousClearanceCertified: true,
    safeStopAngleDegrees: 180,
    authorizesProjectMutation: false,
  },
  flatEndpointLayerOrder: {
    applicable: true,
    certified: true,
    materialFaceCount: 2,
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
    timelineCompleteHingeAngleCount: 1,
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

describe('StackedFoldPanel', () => {
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
    expect(screen.getByText('stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1')).toBeTruthy()
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
