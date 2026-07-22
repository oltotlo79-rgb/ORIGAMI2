import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { StackedFoldPanel } from '../src/components/StackedFoldPanel'
import type { ProjectSnapshot } from '../src/lib/coreClient'

const transport = vi.hoisted(() => ({
  preview: vi.fn(),
  basicPreview: vi.fn(),
  cyclePreview: vi.fn(),
  apply: vi.fn(),
  namedApply: vi.fn(),
  reverseApply: vi.fn(),
  accordionApply: vi.fn(),
  sinkApply: vi.fn(),
  layerApply: vi.fn(),
  cancel: vi.fn(),
  cancelRead: vi.fn(),
  registry: vi.fn(),
  progress: null as null | ((value: any) => void),
  cycleProgress: null as null | ((value: any) => void),
}))

vi.mock('../src/lib/coreClient', async (importOriginal) => ({
  ...await importOriginal<typeof import('../src/lib/coreClient')>(),
  proposeCurrentStackedFoldRead: transport.preview,
  previewNamedBasicFoldTimeline: transport.basicPreview,
  proposeCurrentCyclePoseV1: transport.cyclePreview,
  applyStackedFoldTransaction: transport.apply,
  applyNamedBookFoldTransaction: transport.namedApply,
  applyNamedReverseFoldTransaction: transport.reverseApply,
  applyNamedAccordionFoldTransaction: transport.accordionApply,
  applyNamedSinkFoldTransaction: transport.sinkApply,
  applyNamedLayerSelectiveTransaction: transport.layerApply,
  cancelStackedFoldTransactionPreview: transport.cancel,
  cancelCurrentStackedFoldReadV1: transport.cancelRead,
  readLiveHingeRegistryV1: transport.registry,
  listenStackedFoldReadProgressV1: vi.fn(async (callback) => {
    transport.progress = callback
    return () => {
      transport.progress = null
    }
  }),
  listenCurrentCyclePoseProgressV1: vi.fn(async (callback) => {
    transport.cycleProgress = callback
    return () => {
      transport.cycleProgress = null
    }
  }),
}))

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'
const token = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const basicTimelinePreview = {
  schemaVersion: 1 as const,
  transactionToken: token,
  projectInstanceId: instance,
  projectId: project,
  revision: 3,
  sourceModelFingerprint: 'a'.repeat(64),
  fixedFace: project,
  foldEdge: 'edge',
  assignment: 'mountain' as const,
  techniqueKind: 'mountain' as const,
  previewBindingSha256: 'b'.repeat(64),
  timeline: { steps: [] },
}

const snapshot = {
  project_instance_id: instance,
  project_id: project,
  revision: 3,
} as ProjectSnapshot

function snapshotWithCycleProof(proof: unknown): ProjectSnapshot {
  return {
    ...snapshot,
    instruction_timeline: {
      steps: [{
        id: token, title: 'fold', description: '', caution: '', duration_ms: 1,
        pose: { model: 'absolute_hinge_angles_v1', source_model_fingerprint: 'a'.repeat(64), fixed_face: null, hinge_angles: [] },
        visual: {
          camera: null, arrows: [], focus_points: [], hand_guides: [],
          cycle_layer_order_proof_v1: proof,
        },
      }],
    },
  } as ProjectSnapshot
}

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
  transport.cycleProgress = null
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
  it('selects same-named palette tiles by stable ID and explains unsupported entries bilingually', () => {
    const onSelect = vi.fn()
    const base = {
      snapshot,
      selectedLine: null,
      disabled: false,
      refreshSnapshot: vi.fn(),
      onApplied: vi.fn(),
      namedBookFold: {
        document: { techniques: [] } as any,
        techniqueId: 'tech-b', name: 'Same name', kind: 'mountain' as const,
      },
      namedTechniquePalette: [
        { techniqueId: 'tech-b', name: 'Same name', supported: true },
        { techniqueId: 'tech-a', name: 'Same name', supported: true },
        { techniqueId: 'tech-x', name: 'Unsupported', supported: false },
      ],
      onSelectNamedTechnique: onSelect,
    }
    const view = render(<StackedFoldPanel locale="en" {...base} />)
    expect(screen.getByRole('group', { name: 'Technique palette' })).toBeTruthy()
    const same = screen.getAllByRole('button', { name: 'Same name' })
    expect(same[0]?.getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(same[1]!)
    expect(onSelect).toHaveBeenCalledWith('tech-a')
    const unsupported = screen.getByRole('button', { name: 'Unsupported' }) as HTMLButtonElement
    expect(unsupported.disabled).toBe(true)
    expect(screen.getByText('Unsupported as a certified physical operation.')).toBeTruthy()

    view.rerender(<StackedFoldPanel locale="ja" {...base} />)
    expect(screen.getByRole('group', { name: '技法パレット' })).toBeTruthy()
    expect(screen.getByText('安全な物理操作として未対応です。')).toBeTruthy()
  })

  it('shows saved compiler provenance as read only without exposing its digest', () => {
    const saved = {
      ...snapshot,
      instruction_timeline: { steps: [{
        visual: { named_technique_compiler_v1: {
          version: 1, model_id: 'certified_named_technique_compiler_metadata_v1',
          technique_kind: 'accordion', segment_index: 0, segment_count: 4,
          compiler_output_sha256: Array(32).fill(0x5a),
        } },
      }] },
    } as unknown as ProjectSnapshot
    const { rerender } = render(<StackedFoldPanel locale="en" snapshot={saved}
      selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    expect(screen.getByText('Saved compiler provenance (read only): accordion / 4 steps')).toBeTruthy()
    expect(document.body.textContent).not.toContain('5a5a5a5a')
    rerender(<StackedFoldPanel locale="en" snapshot={snapshot}
      selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    expect(screen.getByText('No saved compiler proof information')).toBeTruthy()
  })

  it('plays bounded read-only compiler steps by buttons and keyboard and drops stale state', async () => {
    transport.cancel.mockResolvedValue(undefined)
    const step = (id: string, title: string, angle: number) => ({
      id, title, description: `${title} detail`, caution: '', duration_ms: 1000,
      visual: { path_certificate_reference_v1: null },
      pose: { model: 'absolute_hinge_angles_v1', source_model_fingerprint: 'a'.repeat(64),
        fixed_face: project, hinge_angles: [{ edge: token, angle_degrees: angle }] },
    })
    const preview = { ...basicTimelinePreview, timeline: { steps: [
      step('step-1', 'Start pose', 0), step('step-2', 'Folded pose', 90),
    ] } } as any
    transport.preview.mockResolvedValue(ready)
    transport.basicPreview.mockResolvedValue(preview)
    const props = {
      locale: 'en' as const, snapshot,
      selectedLine: { id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } },
      disabled: false,
      namedBookFold: { document: { techniques: [] } as any, techniqueId: 'mountain',
        name: 'Mountain fold', kind: 'mountain' as const },
      refreshSnapshot: vi.fn(), onApplied: vi.fn(),
    }
    const { rerender } = render(<StackedFoldPanel {...props} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Preview certified timeline' }))
    const player = await screen.findByRole('status', { name: 'Certified timeline step player' })
    expect(screen.getByRole('heading', { name: 'Start pose' })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Next step' }))
    expect(await screen.findByRole('heading', { name: 'Folded pose' })).toBeTruthy()
    fireEvent.keyDown(player, { key: 'Home' })
    expect(await screen.findByRole('heading', { name: 'Start pose' })).toBeTruthy()
    expect(screen.getByText('Read-only preview; no mutation authority is included.')).toBeTruthy()
    rerender(<StackedFoldPanel {...props} snapshot={{ ...snapshot, revision: 4 } as ProjectSnapshot} />)
    expect(screen.queryByRole('status', { name: 'Certified timeline step player' })).toBeNull()
  })

  it('keeps named timeline preview single-flight and rejects a late stale response', async () => {
    transport.cancel.mockResolvedValue(undefined)
    transport.preview.mockResolvedValue(ready)
    let resolvePreview!: (value: typeof basicTimelinePreview) => void
    transport.basicPreview.mockReturnValue(new Promise((resolve) => { resolvePreview = resolve }))
    const props = {
      locale: 'en' as const, snapshot,
      selectedLine: { id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } },
      disabled: false,
      namedBookFold: { document: { techniques: [] } as any, techniqueId: 'mountain',
        name: 'Mountain fold', kind: 'mountain' as const },
      refreshSnapshot: vi.fn(), onApplied: vi.fn(),
    }
    const { rerender } = render(<StackedFoldPanel {...props} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const previewButton = await screen.findByRole('button', { name: 'Preview certified timeline' })
    fireEvent.click(previewButton)
    fireEvent.click(previewButton)
    expect(transport.basicPreview).toHaveBeenCalledTimes(1)
    expect(previewButton.getAttribute('aria-busy')).toBe('true')
    expect(screen.getByRole('status').textContent).toContain('Building certified timeline')

    rerender(<StackedFoldPanel {...props} namedBookFold={{ ...props.namedBookFold,
      techniqueId: 'valley', kind: 'valley' }} />)
    await waitFor(() => expect(transport.cancel).toHaveBeenCalledWith(token))
    resolvePreview(basicTimelinePreview)
    await waitFor(() => expect(transport.cancel).toHaveBeenCalledTimes(2))
    expect(screen.queryByRole('status', { name: 'Certified timeline step player' })).toBeNull()
    expect(screen.queryByRole('checkbox')).toBeNull()
  })

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
    const apply = screen.getByRole('button', { name: 'Apply stacked fold' })
    expect((apply as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(screen.getByRole('checkbox'))
    expect((apply as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(apply)
    await waitFor(() => expect(transport.apply).toHaveBeenCalledWith(token))
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
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
    expect(screen.getByText('Positive-thickness continuous-path certificate')).toBeTruthy()
    expect(screen.queryByText('stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1')).toBeNull()
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

  it('applies a selected named book fold through the proof-bound native transaction', async () => {
    transport.preview.mockResolvedValue(ready)
    transport.basicPreview.mockResolvedValue(basicTimelinePreview)
    transport.namedApply.mockResolvedValue(4)
    const refreshed = { ...snapshot, revision: 4 } as ProjectSnapshot
    const document = {
      schema: 'origami2_fold_technique_file', version: 1,
      package_id: 'user.test.book', metadata: {}, techniques: [],
    } as any
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="ja"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        namedBookFold={{ document, techniqueId: 'book-fold', name: '山折り', kind: 'mountain' }}
        refreshSnapshot={vi.fn().mockResolvedValue(refreshed)}
        onApplied={onApplied}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '安全性を確認' }))
    const apply = await screen.findByRole('button', { name: '名前付き二つ折りを適用' })
    expect(screen.getByRole('note').textContent).toContain('PDF/SVG折り図')
    expect(apply).toHaveProperty('disabled', true)
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalled())
    expect(transport.namedApply).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(apply)
    fireEvent.click(apply)
    await waitFor(() => expect(transport.namedApply).toHaveBeenCalledWith(
      token, document, 'book-fold', basicTimelinePreview,
    ))
    expect(transport.apply).not.toHaveBeenCalled()
    expect(transport.namedApply).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
  })

  it('keeps the project unchanged when named proof apply rejects stale or tampered authority', async () => {
    transport.preview.mockResolvedValue(ready)
    transport.basicPreview.mockResolvedValue(basicTimelinePreview)
    transport.namedApply.mockRejectedValue(new Error('stale or tampered'))
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        namedBookFold={{
          document: { techniques: [] } as any,
          techniqueId: 'book-fold',
          name: 'Mountain fold',
          kind: 'mountain',
        }}
        refreshSnapshot={vi.fn()}
        onApplied={onApplied}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const apply = await screen.findByRole('button', { name: 'Apply named book fold' })
    fireEvent.click(screen.getByRole('button', { name: 'Preview certified timeline' }))
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalled())
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(apply)
    expect((await screen.findByRole('alert')).textContent).toContain(
      'Apply failed. You can retry with the same certified preview.',
    )
    expect(onApplied).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Apply named book fold' }))
      .toHaveProperty('disabled', false)
  })

  it('keeps a petal fold explicitly unsupported without preview or apply authority', async () => {
    transport.preview.mockResolvedValue(ready)
    transport.cancel.mockResolvedValue(undefined)
    render(<StackedFoldPanel locale="en" snapshot={snapshot}
      selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
      disabled={false}
      namedBookFold={{ document: { techniques: [] } as any, techniqueId: 'petal', name: 'Petal fold', kind: 'petal' }}
      refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    expect((await screen.findByRole('alert')).textContent).toContain('Petal fold remains unsupported')
    const premises = screen.getByLabelText('Missing petal-fold proof premises')
    expect(premises.textContent).toContain('at least 3 in one Graph chain')
    expect(premises.textContent).toContain('Lifted-flap topology authority')
    expect(premises.textContent).toContain('Adjacent-face opening path authority')
    expect(premises.textContent).toContain('Final-flattening endpoint authority')
    expect(premises.textContent).toContain('Continuous layer-order authority')
    expect(premises.textContent).toContain('no preview or apply token is issued')
    expect(screen.queryByRole('button', { name: 'Preview certified timeline' })).toBeNull()
    expect(screen.getByRole('checkbox')).toHaveProperty('disabled', true)
    expect(screen.getByRole('button', { name: 'Apply named book fold' })).toHaveProperty('disabled', true)
    expect(transport.namedApply).not.toHaveBeenCalled()
  })

  it.each(['squash', 'crimp'] as const)(
    'requires a digest-bound two-segment preview before %s apply', async (kind) => {
      const preview = { ...basicTimelinePreview, techniqueKind: kind }
      transport.preview.mockResolvedValue(ready)
      transport.basicPreview.mockResolvedValue(preview)
      transport.namedApply.mockResolvedValue(4)
      const document = { techniques: [] } as any
      render(<StackedFoldPanel locale="en" snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false} namedBookFold={{ document, techniqueId: kind, name: kind, kind }}
        refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })}
        onApplied={vi.fn()} />)
      fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
      const apply = await screen.findByRole('button', { name: 'Apply named book fold' })
      expect(apply).toHaveProperty('disabled', true)
      fireEvent.click(screen.getByRole('button', { name: 'Preview certified timeline' }))
      await waitFor(() => expect(transport.basicPreview).toHaveBeenCalledWith(
        expect.objectContaining({ techniqueKind: kind }),
      ))
      fireEvent.click(screen.getByRole('checkbox'))
      fireEvent.click(apply)
      await waitFor(() => expect(transport.namedApply).toHaveBeenCalledWith(
        token, document, kind, preview,
      ))
    },
  )

  it('routes a named reverse fold only through the two-segment native transaction', async () => {
    transport.preview.mockResolvedValue(ready)
    const reversePreview = { ...basicTimelinePreview, techniqueKind: 'inside_reverse' as const }
    transport.basicPreview.mockResolvedValue(reversePreview)
    transport.namedApply.mockResolvedValue(4)
    const document = { techniques: [] } as any
    render(<StackedFoldPanel
      locale="en" snapshot={snapshot}
      selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
      disabled={false}
      namedBookFold={{ document, techniqueId: 'inside-reverse', name: 'Inside reverse', kind: 'inside_reverse' }}
      refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })}
      onApplied={vi.fn()}
    />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const apply = await screen.findByRole('button', { name: 'Apply named reverse fold' })
    expect(apply).toHaveProperty('disabled', true)
    fireEvent.click(screen.getByRole('button', { name: 'Preview certified timeline' }))
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalledWith(
      expect.objectContaining({ techniqueKind: 'inside_reverse' }),
    ))
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(apply)
    await waitFor(() => expect(transport.reverseApply).toHaveBeenCalledWith(
      token, document, 'inside-reverse',
    ))
    expect(transport.namedApply).not.toHaveBeenCalled()
    expect(transport.apply).not.toHaveBeenCalled()
  })

  it('routes a named accordion only through the ordered multi-segment transaction', async () => {
    transport.preview.mockResolvedValue(ready)
    const accordionPreview = { ...basicTimelinePreview, techniqueKind: 'accordion' as const }
    transport.basicPreview.mockResolvedValue(accordionPreview)
    transport.namedApply.mockResolvedValue(4)
    const document = { techniques: [] } as any
    render(<StackedFoldPanel locale="ja" snapshot={snapshot}
      selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
      disabled={false}
      namedBookFold={{ document, techniqueId: 'accordion', name: '蛇腹折り', kind: 'accordion' }}
      refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })}
      onApplied={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: '安全性を確認' }))
    const apply = await screen.findByRole('button', { name: '名前付き蛇腹折りを適用' })
    expect(apply).toHaveProperty('disabled', true)
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalledWith(
      expect.objectContaining({ techniqueKind: 'accordion' }),
    ))
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(apply)
    await waitFor(() => expect(transport.namedApply).toHaveBeenCalledWith(
      token, document, 'accordion', accordionPreview,
    ))
    expect(transport.apply).not.toHaveBeenCalled()
    expect(transport.accordionApply).not.toHaveBeenCalled()
    expect(transport.reverseApply).not.toHaveBeenCalled()
  })

  it('routes a named sink fold through exactly two certified segments', async () => {
    transport.preview.mockResolvedValue(ready)
    const sinkPreview = { ...basicTimelinePreview, techniqueKind: 'sink' as const }
    transport.basicPreview.mockResolvedValue(sinkPreview)
    transport.namedApply.mockResolvedValue(4)
    const document = { techniques: [] } as any
    render(<StackedFoldPanel locale="en" snapshot={snapshot}
      selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
      disabled={false} namedBookFold={{ document, techniqueId: 'open-sink', name: 'Open sink', kind: 'sink' }}
      refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })} onApplied={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const apply = await screen.findByRole('button', { name: 'Apply named sink fold' })
    fireEvent.click(screen.getByRole('button', { name: 'Preview certified timeline' }))
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalledWith(
      expect.objectContaining({ techniqueKind: 'sink' }),
    ))
    fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(apply)
    await waitFor(() => expect(transport.namedApply).toHaveBeenCalledWith(
      token, document, 'open-sink', sinkPreview,
    ))
    expect(transport.apply).not.toHaveBeenCalled()
    expect(transport.sinkApply).not.toHaveBeenCalled()
  })

  it('routes a layer-selective technique through its proof-bound transaction', async () => {
    transport.preview.mockResolvedValue(ready)
    const layerPreview = { ...basicTimelinePreview, techniqueKind: 'layer_selective' as const }
    transport.basicPreview.mockResolvedValue(layerPreview); transport.namedApply.mockResolvedValue(4)
    const document = { techniques: [] } as any
    render(<StackedFoldPanel locale="en" snapshot={snapshot}
      selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }} disabled={false}
      namedBookFold={{ document, techniqueId: 'layer-select', name: 'Layer select', kind: 'layer_selective' }}
      refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })} onApplied={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const apply = await screen.findByRole('button', { name: 'Apply named layer technique' })
    expect(apply).toHaveProperty('disabled', true)
    fireEvent.click(screen.getByRole('button', { name: 'Preview certified timeline' }))
    await waitFor(() => expect(transport.basicPreview).toHaveBeenCalledWith(
      expect.objectContaining({ techniqueKind: 'layer_selective' }),
    ))
    fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(apply)
    await waitFor(() => expect(transport.layerApply).toHaveBeenCalledWith(
      token, document, 'layer-select',
    ))
    expect(transport.namedApply).not.toHaveBeenCalled()
    expect(transport.apply).not.toHaveBeenCalled()
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
    ['cycle_path_unsupported', 'Static reason: the hinge graph and schedule do not match a certified grid, symmetric-sector, or opposite-axis straight-fold class. Apply is disabled.'],
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

  it('previews, summarizes, explicitly applies, and cancels a current-pose cycle', async () => {
    transport.cyclePreview.mockResolvedValue({
      version: 1,
      transactionToken: token,
      sourceRevision: 3,
      targetRevision: 4,
      closureLeafCount: 1,
      continuousPathCertified: true,
      authorizesProjectMutation: false,
    })
    transport.apply.mockResolvedValue(4)
    transport.cancel.mockResolvedValue(undefined)
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
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    expect(await screen.findByText('Closure intervals')).toBeTruthy()
    await waitFor(() => expect(document.activeElement).toBe(
      screen.getByText('Closure intervals').closest('[role="status"]'),
    ))
    expect(screen.getByText('This preview is read-only. The project is unchanged until you explicitly apply it.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel preview' }))
    await waitFor(() => expect(transport.cancel).toHaveBeenCalledWith(token))
    await waitFor(() => expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Prove from current pose' }),
    ))
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    await screen.findByText('Closure intervals')
    fireEvent.click(screen.getByRole('button', { name: 'Apply certified cycle fold' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
    expect(transport.apply).toHaveBeenCalledWith(token)
  })

  it('authors and applies a six-hinge balloon straight-fold schedule from the UI', async () => {
    const hingeIds = Array.from({ length: 6 }, (_, index) =>
      `018f47a2-4b7a-7cc1-8abc-00000000000${index}`)
    transport.cyclePreview.mockResolvedValue({
      version: 1,
      transactionToken: token,
      sourceRevision: 3,
      targetRevision: 4,
      closureLeafCount: 1,
      closureMaxDepth: 0,
      checkedHingeCount: 6,
      totalHingeCount: 6,
      continuousPathCertified: true,
      authorizesProjectMutation: false,
    })
    transport.apply.mockResolvedValue(4)
    const refreshed = { ...snapshot, revision: 4 } as ProjectSnapshot
    const onApplied = vi.fn()
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: hingeIds[0], start: { x: -100, y: 0 }, end: { x: 100, y: 0 } }}
        disabled={false}
        refreshSnapshot={vi.fn().mockResolvedValue(refreshed)}
        onApplied={onApplied}
      />,
    )
    const schedule = {
      version: 1,
      entries: hingeIds.map((edge, index) => ({
        edge,
        uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }],
        numeratorPowerCoefficients: index === 0 || index === 3
          ? [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 100 }]
          : [{ numerator: 0, denominator: 1 }],
        denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        requestedAngleDegrees: index === 0 || index === 3 ? 2 * Math.atan(0.01) * 180 / Math.PI : 0,
      })),
    }
    fireEvent.change(screen.getByLabelText('Cycle path definition (JSON, cyclic patterns only)'), {
      target: { value: JSON.stringify(schedule) },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    const preview = await screen.findByRole('region', { name: 'Current-pose cycle preview' })
    expect(preview.textContent).toContain('All hinges covered6/6')
    expect(preview.textContent).toContain('Continuous pathCertified')
    fireEvent.click(screen.getByRole('button', { name: 'Apply certified cycle fold' }))
    await waitFor(() => expect(transport.apply).toHaveBeenCalledWith(token))
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
    expect(transport.cyclePreview.mock.calls.at(-1)?.[0].cycleScheduleV1.entries).toHaveLength(6)
  })

  it('does not publish a late cycle preview after rapid replacement', async () => {
    let resolveFirst!: (value: any) => void
    let resolveSecond!: (value: any) => void
    transport.cyclePreview
      .mockReturnValueOnce(new Promise((resolve) => { resolveFirst = resolve }))
      .mockReturnValueOnce(new Promise((resolve) => { resolveSecond = resolve }))
    transport.cancel.mockResolvedValue(undefined)
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
    const schedule = (angle: number) => ({
      version: 1,
      entries: [{
        edge: token,
        uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }],
        numeratorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
        requestedAngleDegrees: angle,
      }],
    })
    const textarea = screen.getByLabelText('Cycle path definition (JSON, cyclic patterns only)')
    fireEvent.change(textarea, { target: { value: JSON.stringify(schedule(90)) } })
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    fireEvent.change(textarea, { target: { value: JSON.stringify(schedule(80)) } })
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    resolveSecond({
      version: 1, transactionToken: project, sourceRevision: 3, targetRevision: 4,
      closureLeafCount: 4, closureMaxDepth: 2, checkedHingeCount: 16, totalHingeCount: 16,
      continuousPathCertified: true, authorizesProjectMutation: false,
      continuousLayerTransportModelId: 'general_multi_face_positive_thickness_cell_transport_v1',
      continuousLayerTransitionCount: 5, continuousLayerPairOrderCount: 1,
      continuousLayerTargetOrderSha256: 'ab'.repeat(32),
      sourceLayerOrder: [{ lowerFace: project, upperFace: token }],
      targetLayerOrder: [{ lowerFace: project, upperFace: token }],
    })
    await waitFor(() => expect(
      screen.getByRole('region', { name: 'Current-pose cycle preview' }).textContent,
    ).toContain('Closure intervals4'))
    expect(screen.getByText('Maximum proof depth').nextElementSibling?.textContent).toBe('2')
    expect(screen.getByText('All hinges covered').nextElementSibling?.textContent).toBe('16/16')
    expect(screen.getByTestId('cycle-layer-transition-count').textContent).toBe('5')
    expect(screen.getByTestId('cycle-layer-order-viewer').textContent).toContain('Target: 1')
    expect(screen.getByText('Layer-order proof hash').nextElementSibling?.textContent)
      .toBe('ab'.repeat(32))
    resolveFirst({
      version: 1, transactionToken: token, sourceRevision: 3, targetRevision: 4,
      closureLeafCount: 99, closureMaxDepth: 7, checkedHingeCount: 4, totalHingeCount: 4,
      continuousPathCertified: true, authorizesProjectMutation: false,
    })
    await waitFor(() => expect(transport.cancel).toHaveBeenCalledWith(token))
    expect(screen.queryByText('99')).toBeNull()
    expect(screen.getByRole('region', { name: 'Current-pose cycle preview' }).textContent)
      .toContain('Closure intervals4')
  })

  it('announces cycle cancellation and allows an immediate retry', async () => {
    transport.cyclePreview.mockReturnValueOnce(new Promise(() => undefined)).mockResolvedValueOnce({
      version: 1, transactionToken: token, sourceRevision: 3, targetRevision: 4,
      closureLeafCount: 3, closureMaxDepth: 2, checkedHingeCount: 12, totalHingeCount: 12,
      continuousPathCertified: true, authorizesProjectMutation: false,
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
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel cycle proof' }))
    expect(await screen.findByText('Cycle proof cancelled. You can retry.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    expect(await screen.findByText('Closure intervals')).toBeTruthy()
    expect(transport.cancelRead).toHaveBeenCalled()
  })

  it('restores a persisted applied layer-order proof from project history', () => {
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={{
          ...snapshot,
          instruction_timeline: {
            steps: [{
              id: token, title: 'fold', description: '', caution: '', duration_ms: 1,
              pose: { model: 'absolute_hinge_angles_v1', source_model_fingerprint: 'a'.repeat(64), fixed_face: null, hinge_angles: [] },
              visual: {
                camera: null, arrows: [], focus_points: [], hand_guides: [],
                cycle_layer_order_proof_v1: {
                  version: 1,
                  model_id: 'native_continuous_layer_transport_certificate_v1',
                  target_order_sha256: Array(32).fill(0xab),
                  transition_count: 5,
                  pairs: [{ lower_face: project, upper_face: token }],
                },
              },
            }],
          },
        }}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn()}
        onApplied={vi.fn()}
      />,
    )
    const viewer = screen.getByTestId('persisted-cycle-layer-order-viewer')
    expect(viewer.textContent).toContain('Transitions: 5')
    expect(viewer.textContent).toContain('ab'.repeat(32))
    expect(screen.getByRole('list', { name: 'Canonical proof pairs (lower to upper)' }).textContent)
      .toContain(`${project} → ${token}`)
    expect(viewer.textContent).toContain('read-only view of the proof persisted')
  })

  it('bounds the applied viewer, admits derived v5 faces, and selects either endpoint', () => {
    const pairs = Array.from({ length: 201 }, (_, index) => ({
      lower_face: `00000000-0000-5000-8000-${index.toString(16).padStart(12, '0')}`,
      upper_face: 'ffffffff-ffff-5fff-8fff-ffffffffffff',
    }))
    render(<StackedFoldPanel locale="en" snapshot={snapshotWithCycleProof({
      version: 1, model_id: 'native_continuous_layer_transport_certificate_v1',
      target_order_sha256: Array(32).fill(0xab), transition_count: 5, pairs,
    })} selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    const list = screen.getByRole('list', { name: 'Canonical proof pairs (lower to upper)' })
    expect(list.querySelectorAll('li')).toHaveLength(200)
    expect(screen.getByText('Showing the first 200; 1 more are omitted.')).toBeTruthy()
    const lower = screen.getByRole('button', { name: pairs[0].lower_face })
    fireEvent.click(lower)
    expect(lower.getAttribute('aria-pressed')).toBe('true')
    const upper = screen.getAllByRole('button', { name: pairs[0].upper_face })[0]
    fireEvent.click(upper)
    expect(upper.getAttribute('aria-pressed')).toBe('true')
  })

  it.each([
    ['non-array hash', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: null, transition_count: 1, pairs: [] }],
    ['unknown key', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: Array(32).fill(1), transition_count: 1, pairs: [], extra: true }],
    ['same face', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: Array(32).fill(1), transition_count: 1, pairs: [{ lower_face: project, upper_face: project }] }],
    ['duplicate pair', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: Array(32).fill(1), transition_count: 1, pairs: [{ lower_face: project, upper_face: token }, { lower_face: project, upper_face: token }] }],
    ['noncanonical order', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: Array(32).fill(1), transition_count: 1, pairs: [{ lower_face: token, upper_face: project }, { lower_face: project, upper_face: token }] }],
    ['oversize', { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1', target_order_sha256: Array(32).fill(1), transition_count: 1, pairs: Array.from({ length: 50_001 }, (_, index) => ({ lower_face: `00000000-0000-5000-8000-${index.toString(16).padStart(12, '0')}`, upper_face: 'ffffffff-ffff-5fff-8fff-ffffffffffff' })) }],
  ])('fails closed for a tampered applied layer proof: %s', (_name, proof) => {
    render(<StackedFoldPanel locale="en" snapshot={snapshotWithCycleProof(proof)}
      selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    expect(screen.queryByTestId('persisted-cycle-layer-order-viewer')).toBeNull()
  })

  it('does not fall back to an older proof when the latest persisted proof is invalid', () => {
    const valid = { version: 1, model_id: 'native_continuous_layer_transport_certificate_v1',
      target_order_sha256: Array(32).fill(1), transition_count: 1,
      pairs: [{ lower_face: project, upper_face: token }] }
    const withTwoSteps = structuredClone(snapshotWithCycleProof(valid)) as any
    withTwoSteps.instruction_timeline.steps.push({
      ...withTwoSteps.instruction_timeline.steps[0], id: project,
      visual: { ...withTwoSteps.instruction_timeline.steps[0].visual,
        cycle_layer_order_proof_v1: { ...valid, pairs: null } },
    })
    render(<StackedFoldPanel locale="en" snapshot={withTwoSteps}
      selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    expect(screen.queryByTestId('persisted-cycle-layer-order-viewer')).toBeNull()
  })

  it('labels the persisted read-only viewer in Japanese', () => {
    render(<StackedFoldPanel locale="ja" snapshot={snapshotWithCycleProof({
      version: 1, model_id: 'native_continuous_layer_transport_certificate_v1',
      target_order_sha256: Array(32).fill(1), transition_count: 1,
      pairs: [{ lower_face: project, upper_face: token }],
    })} selectedLine={null} disabled={false} refreshSnapshot={vi.fn()} onApplied={vi.fn()} />)
    expect(screen.getByRole('region', { name: '適用済み層順ビューアー' })).toBeTruthy()
    expect(screen.getByText('これは適用済みタイムライン手順に保存された証明の読み取り専用表示です。')).toBeTruthy()
  })

  it('blocks duplicate apply and cancels active work with listener cleanup on unmount', async () => {
    let resolveApply!: (value: number) => void
    transport.cyclePreview.mockResolvedValue({
      version: 1, transactionToken: token, sourceRevision: 3, targetRevision: 4,
      closureLeafCount: 4, closureMaxDepth: 2, checkedHingeCount: 16, totalHingeCount: 16,
      continuousPathCertified: true, authorizesProjectMutation: false,
    })
    transport.apply.mockReturnValue(new Promise((resolve) => { resolveApply = resolve }))
    const rendered = render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled={false}
        refreshSnapshot={vi.fn().mockResolvedValue({ ...snapshot, revision: 4 })}
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
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    const apply = await screen.findByRole('button', { name: 'Apply certified cycle fold' })
    fireEvent.click(apply)
    fireEvent.click(apply)
    expect(transport.apply).toHaveBeenCalledTimes(1)
    expect(apply).toHaveProperty('disabled', true)
    resolveApply(4)
    await waitFor(() => expect(transport.apply).toHaveBeenCalledTimes(1))

    transport.cyclePreview.mockReturnValue(new Promise(() => undefined))
    fireEvent.click(screen.getByRole('button', { name: 'Prove from current pose' }))
    rendered.unmount()
    await waitFor(() => expect(transport.cancelRead).toHaveBeenCalled())
    expect(transport.progress).toBeNull()
    expect(transport.cycleProgress).toBeNull()
  })

  it('disables cycle preview controls when the panel is disabled', () => {
    render(
      <StackedFoldPanel
        locale="en"
        snapshot={snapshot}
        selectedLine={{ id: 'edge', start: { x: 1, y: 2 }, end: { x: 3, y: 4 } }}
        disabled
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
          numeratorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
          denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }],
          requestedAngleDegrees: 90,
        }],
      }) },
    })
    expect(screen.getByRole('button', { name: 'Prove from current pose' }))
      .toHaveProperty('disabled', true)
  })
})
