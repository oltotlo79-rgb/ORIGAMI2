import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const { CurrentNonFlatLayerOrderViewer } = await import(
  '../src/components/CurrentNonFlatLayerOrderViewer'
)

function uuid(seed: number) {
  return `00000000-0000-4000-8000-${seed.toString(16).padStart(12, '0')}`
}

const INSTANCE = uuid(1)
const PROJECT = uuid(2)
const FACE_A = uuid(11)
const FACE_B = uuid(12)
const EDGE_1 = uuid(21)
const FINGERPRINT = 'd'.repeat(64)

const ONE = { sign: 'positive', numeratorMagnitudeHex: '01', denominatorMagnitudeHex: '01' }
const ZERO = { sign: 'zero', numeratorMagnitudeHex: '', denominatorMagnitudeHex: '01' }
const affine = () => ({
  m00: { ...ONE }, m01: { ...ZERO }, m10: { ...ZERO },
  m11: { ...ONE }, tx: { ...ZERO }, ty: { ...ZERO },
})
const point = (u: number, v: number) => ({
  u: u === 0 ? { ...ZERO } : { ...ONE, numeratorMagnitudeHex: u.toString(16).padStart(2, '0') },
  v: v === 0 ? { ...ZERO } : { ...ONE, numeratorMagnitudeHex: v.toString(16).padStart(2, '0') },
})

const PLANE_AXES: Record<string, [string, string]> = {
  x: ['y', 'z'],
  y: ['x', 'z'],
  z: ['x', 'y'],
}

type ViewOptions = Readonly<{
  cells?: 0 | 1
  droppedAxis?: 'x' | 'y' | 'z'
  hugeMagnitude?: boolean
}>

function viewResponse(options: ViewOptions = {}) {
  const cells = options.cells ?? 1
  const axis = options.droppedAxis ?? 'z'
  const plane = PLANE_AXES[axis] as [string, string]
  const first = affine()
  if (options.hugeMagnitude) {
    first.m00 = {
      sign: 'positive',
      numeratorMagnitudeHex: `01${'ff'.repeat(64)}`,
      denominatorMagnitudeHex: '01',
    }
  }
  return {
    version: 1,
    modelId: 'native_stacked_fold_non_flat_planar_order_v1',
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 12,
    foldModelFingerprintSha256: FINGERPRINT,
    pose: {
      modelId: 'tree_absolute_hinge_angles_v1',
      generation: '7',
      fixedFaceId: FACE_A,
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: 73.5 }],
    },
    faces: [
      {
        faceId: FACE_A,
        faceKeySha256: 'a'.repeat(64),
        worldOuterBoundaryXyzMm: [[0, 0, 0], [10, 0, 0], [10, 5, 2]],
        projection: {
          droppedWorldAxis: axis, planeAxes: [...plane],
          sourceToPlaneProjectionExact: first,
        },
      },
      {
        faceId: FACE_B,
        faceKeySha256: 'b'.repeat(64),
        worldOuterBoundaryXyzMm: [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
        projection: {
          droppedWorldAxis: axis, planeAxes: [...plane],
          sourceToPlaneProjectionExact: affine(),
        },
      },
    ],
    cells: cells === 0 ? [] : [
      {
        cellKeySha256: 'c'.repeat(64),
        exactBoundarySha256: 'a'.repeat(64),
        lowerFaceId: FACE_A,
        upperFaceId: FACE_B,
        projection: {
          droppedWorldAxis: axis, planeAxes: [...plane],
          roundedBoundaryUvMm: [[1, 3], [5, 3], [1, 8]],
          exactBoundaryUv: [point(1, 3), point(5, 3), point(1, 8)],
        },
      },
    ],
    work: {
      testedFacePairs: 1,
      materialFaceCount: 2,
      sourceOverlapCellsAuthenticated: 0,
      overlapCellCount: cells === 0 ? 0 : 1,
      facePairOrderCount: cells === 0 ? 0 : 1,
      worldBoundaryPointCount: 6,
      exactBoundaryPointCount: cells === 0 ? 0 : 3,
    },
    readOnly: true,
    authorizesProjectMutation: false,
  }
}

type Source = {
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprintSha256: string
  appliedPose: {
    state: string
    projectId: string
    revision: number
    fixedFaceId: string | null
    hingeAngles: { edgeId: string; angleDegrees: number }[]
  }
}

/** A fresh source object; every call returns a distinct reference. */
function makeSource(
  overrides: Partial<Source['appliedPose']> = {},
  sourceOverrides: Partial<Omit<Source, 'appliedPose'>> = {},
): Source {
  return {
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 12,
    foldModelFingerprintSha256: FINGERPRINT,
    ...sourceOverrides,
    appliedPose: {
      state: 'stable',
      projectId: PROJECT,
      revision: 12,
      fixedFaceId: FACE_A,
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: 73.5 }],
      ...overrides,
    },
  }
}

const source = makeSource()

/** A promise plus its settle handles, for late-response ordering tests. */
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolveFn, rejectFn) => {
    resolve = resolveFn
    reject = rejectFn
  })
  return { promise, resolve, reject }
}

afterEach(() => {
  cleanup()
  invoke.mockReset()
})

describe('CurrentNonFlatLayerOrderViewer', () => {
  it('UI-LIFE-03 renders both panes read-only without mutation controls', async () => {
    invoke.mockResolvedValue(viewResponse())
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByRole('region', { name: 'Face outlines in world XYZ millimetres' })
    expect(screen.getByRole('region', {
      name: 'Selected cell projection in UV millimetres',
    })).toBeTruthy()
    expect(screen.getByText('Read-only')).toBeTruthy()
    expect(screen.getByText(
      'This view observes the current pose only. It carries no authority to modify the project.',
    )).toBeTruthy()
    expect(screen.getByText('2 faces')).toBeTruthy()
    expect(screen.getByText('1 overlap cells')).toBeTruthy()
    for (const forbidden of ['Apply', 'Commit', 'Adopt', 'Save', 'Delete']) {
      expect(screen.queryByRole('button', { name: forbidden })).toBeNull()
    }
  })

  it('UI-LIFE-01 shows the loading status before the response settles', async () => {
    const gate = deferred<unknown>()
    invoke.mockReturnValue(gate.promise)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    expect(screen.getByRole('status').textContent).toBe('Loading layer order…')
    gate.resolve(viewResponse())
    await screen.findByText('2 faces')
  })

  it('shows the zero-cell warning without claiming a collision-free proof', async () => {
    invoke.mockResolvedValue(viewResponse({ cells: 0 }))
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText(
      'There are no overlap cells to show. This is not a proof that nothing collides.',
    )
  })

  it('UI-LIFE-02 reports absence when the project owns no non-flat evidence', async () => {
    invoke.mockResolvedValue(null)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('No non-flat layer order is bound to the current pose.')
  })

  const FAILURES: readonly (readonly [string, string, string])[] = [
    ['04', 'stale_authority', 'The pose or project changed, so the view is unavailable.'],
    ['05', 'invalid_evidence', 'The layer-order evidence did not satisfy the contract.'],
    ['06', 'resource_limit', 'The layer order exceeds the viewer limits.'],
    ['07', 'internal_failure', 'The layer order could not be read.'],
  ]

  for (const [lifeId, category, message] of FAILURES) {
    it(`UI-LIFE-${lifeId} maps the ${category} category to a closed failure message`, async () => {
      invoke.mockRejectedValue({ version: 1, category })
      render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
      const alert = await screen.findByRole('alert')
      expect(alert.textContent).toBe(message)
    })
  }

  it('UI-LIFE-08 never shows a raw native error string', async () => {
    invoke.mockRejectedValue(new Error('native panic at src/lib.rs:42'))
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toBe('The layer order could not be read.')
    expect(document.body.textContent).not.toContain('src/lib.rs')
    expect(document.body.textContent).not.toContain('panic')
  })

  it('never renders a raw exact big integer', async () => {
    invoke.mockResolvedValue(viewResponse({ hugeMagnitude: true }))
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('2 faces')
    expect(document.body.textContent).not.toContain('ffffffff')
  })

  it('hides itself when no source is bound', () => {
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={null} />)
    expect(screen.queryByRole('region')).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })

  // A legitimate application state that owns no non-flat evidence: the viewer
  // reports absence and never reaches the native boundary.
  const ABSENT_GATES: readonly (readonly [
    string,
    Partial<Source['appliedPose']>,
  ])[] = [
    ['UI-GATE-01 a running pose', { state: 'running' }],
    ['UI-GATE-02 a blocked pose', { state: 'blocked' }],
    ['UI-GATE-03 an indeterminate pose', { state: 'indeterminate' }],
    ['UI-GATE-04 a project ID mismatch', { projectId: uuid(3) }],
    ['UI-GATE-05 a revision mismatch', { revision: 13 }],
    ['UI-GATE-06 a null fixed face', { fixedFaceId: null }],
    ['UI-GATE-14 an empty hinge vector', { hingeAngles: [] }],
    ['UI-GATE-15 a completely flat request hinge vector', {
      hingeAngles: [
        { edgeId: EDGE_1, angleDegrees: 0 },
        { edgeId: uuid(22), angleDegrees: 180 },
      ],
    }],
  ]

  for (const [name, overrides] of ABSENT_GATES) {
    it(`${name} reports absence without invoking the command`, async () => {
      render(
        <CurrentNonFlatLayerOrderViewer
          locale="en"
          source={makeSource(overrides)}
        />,
      )
      await screen.findByText('No non-flat layer order is bound to the current pose.')
      expect(invoke).not.toHaveBeenCalled()
    })
  }

  // A malformed stable request is hostile or corrupt input. It is refused as
  // data-free invalid evidence and never softened into an absence.
  const MALFORMED_POSES: readonly (readonly [
    string,
    Partial<Source['appliedPose']>,
  ])[] = [
    ['UI-GATE-07 a duplicate request hinge', {
      hingeAngles: [
        { edgeId: EDGE_1, angleDegrees: 73.5 },
        { edgeId: EDGE_1, angleDegrees: 12 },
      ],
    }],
    ['UI-GATE-08 an invalid request edge UUID', {
      hingeAngles: [{ edgeId: 'not-a-canonical-edge-id', angleDegrees: 73.5 }],
    }],
    ['UI-GATE-09a a NaN request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: Number.NaN }],
    }],
    ['UI-GATE-09b an Infinity request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: Number.POSITIVE_INFINITY }],
    }],
    ['UI-GATE-09c a -Infinity request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: Number.NEGATIVE_INFINITY }],
    }],
    ['UI-GATE-10 a negative-zero request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: -0 }],
    }],
    ['UI-GATE-11a an above-range request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: 181 }],
    }],
    ['UI-GATE-11b a negative request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: -1 }],
    }],
    ['UI-GATE-12 a request hinge vector above the viewer cap', {
      hingeAngles: Array.from({ length: 4_097 }, (_, index) => ({
        edgeId: uuid(100 + index),
        angleDegrees: 73.5,
      })),
    }],
  ]

  for (const [name, overrides] of MALFORMED_POSES) {
    it(`${name} is refused as invalid evidence without invoking`, async () => {
      render(
        <CurrentNonFlatLayerOrderViewer
          locale="en"
          source={makeSource(overrides)}
        />,
      )
      const alert = await screen.findByRole('alert')
      expect(alert.textContent)
        .toBe('The layer-order evidence did not satisfy the contract.')
      expect(invoke).not.toHaveBeenCalled()
      expect(document.body.textContent).not.toContain(EDGE_1)
    })
  }

  const MALFORMED_ROOTS: readonly (readonly [
    string,
    Partial<Omit<Source, 'appliedPose'>>,
  ])[] = [
    ['UI-GATE-16 an invalid project instance ID', {
      projectInstanceId: 'invalid',
    }],
    ['UI-GATE-17 an invalid project ID', { projectId: 'invalid' }],
    ['UI-GATE-18 an unsafe revision', { revision: Number.MAX_SAFE_INTEGER + 1 }],
    ['UI-GATE-19 an uppercase fingerprint', {
      foldModelFingerprintSha256: 'D'.repeat(64),
    }],
    ['UI-GATE-20 a short fingerprint', {
      foldModelFingerprintSha256: 'd'.repeat(62),
    }],
  ]

  for (const [name, overrides] of MALFORMED_ROOTS) {
    it(`${name} is refused as invalid evidence without invoking`, async () => {
      const forged = makeSource({}, overrides)
      // A root mismatch must never be reported as a project or revision gate.
      forged.appliedPose.projectId = forged.projectId
      forged.appliedPose.revision = forged.revision
      render(<CurrentNonFlatLayerOrderViewer locale="en" source={forged} />)
      const alert = await screen.findByRole('alert')
      expect(alert.textContent)
        .toBe('The layer-order evidence did not satisfy the contract.')
      expect(invoke).not.toHaveBeenCalled()
    })
  }

  it('UI-LIFE-09 refuses an undefined native response as invalid evidence', async () => {
    invoke.mockResolvedValue(undefined)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
  })

  it('never executes a source accessor', async () => {
    let reads = 0
    const hostile = makeSource() as Source & Record<string, unknown>
    Object.defineProperty(hostile, 'appliedPose', {
      configurable: true,
      enumerable: true,
      get: () => {
        reads += 1
        return makeSource().appliedPose
      },
    })
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={hostile}
      />,
    )
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
    expect(reads).toBe(0)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('never reads a request array through its get trap', async () => {
    let reads = 0
    const hostile = makeSource()
    hostile.appliedPose.hingeAngles = new Proxy(
      hostile.appliedPose.hingeAngles,
      {
        get() {
          reads += 1
          throw new Error('request array get trap')
        },
      },
    )
    invoke.mockResolvedValue(viewResponse())
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={hostile}
      />,
    )
    await screen.findByText('2 faces')
    expect(reads).toBe(0)
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('accepts the maximum request hinge count without truncating it', async () => {
    const maximum = Array.from({ length: 4_096 }, (_, index) => ({
      edgeId: uuid(10_000 + index),
      angleDegrees: 73.5,
    }))
    invoke.mockResolvedValue(null)
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={makeSource({ hingeAngles: maximum })}
      />,
    )
    await screen.findByText('No non-flat layer order is bound to the current pose.')
    expect(invoke).toHaveBeenCalledTimes(1)
    const request = invoke.mock.calls[0]?.[1] as {
      request: { expectedAppliedPose: { hingeAngles: unknown[] } }
    }
    expect(request.request.expectedAppliedPose.hingeAngles).toHaveLength(4_096)
  })

  it('never executes a request array index accessor', async () => {
    let reads = 0
    const hostile = makeSource()
    Object.defineProperty(hostile.appliedPose.hingeAngles, '0', {
      configurable: true,
      enumerable: true,
      get: () => {
        reads += 1
        return { edgeId: EDGE_1, angleDegrees: 73.5 }
      },
    })
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={hostile}
      />,
    )
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
    expect(reads).toBe(0)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('never executes a request hinge field accessor', async () => {
    let reads = 0
    const hostile = makeSource()
    Object.defineProperty(hostile.appliedPose.hingeAngles[0]!, 'angleDegrees', {
      configurable: true,
      enumerable: true,
      get: () => {
        reads += 1
        return 73.5
      },
    })
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={hostile}
      />,
    )
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
    expect(reads).toBe(0)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('fails closed for a revoked request source Proxy', async () => {
    const revocable = Proxy.revocable(makeSource(), {})
    revocable.revoke()
    render(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={revocable.proxy}
      />,
    )
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
    expect(invoke).not.toHaveBeenCalled()
  })

  it('UI-BIND-05 rejects a response whose pose binding differs from the request', async () => {
    const forged = viewResponse()
    forged.pose.hingeAngles = [{ edgeId: EDGE_1, angleDegrees: 73.25 }]
    invoke.mockResolvedValue(forged)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toBe('The layer-order evidence did not satisfy the contract.')
  })

  it('UI-BIND-08 rejects a response whose project binding differs from the request', async () => {
    const forged = viewResponse()
    forged.revision = 13
    invoke.mockResolvedValue(forged)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toBe('The layer-order evidence did not satisfy the contract.')
  })

  it('keeps world XYZ and projection UV in separate panes', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { container } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    await screen.findByText('2 faces')
    const world = container.querySelector('.non-flat-layer-viewer-world')
    const projection = container.querySelector('.non-flat-layer-viewer-projection')
    const worldPoints = [...world!.querySelectorAll('polygon')]
      .map((polygon) => polygon.getAttribute('points'))
    const projectionPoints = [...projection!.querySelectorAll('polygon')]
      .map((polygon) => polygon.getAttribute('points'))
    // The world pane projects world XYZ; the projection pane paints the
    // rounded UV boundary verbatim.
    expect(worldPoints).toHaveLength(2)
    expect(projectionPoints).toEqual(['1,3 5,3 1,8'])
    expect(world!.textContent).toContain('X axis')
    expect(projection!.textContent).toContain('U axis')
    expect(world!.textContent).not.toContain('U axis')
  })

  const AXIS_LABELS: readonly (readonly ['x' | 'y' | 'z', string])[] = [
    ['x', 'World X is dropped; U is world Y and V is world Z.'],
    ['y', 'World Y is dropped; U is world X and V is world Z.'],
    ['z', 'World Z is dropped; U is world X and V is world Y.'],
  ]

  for (const [axis, message] of AXIS_LABELS) {
    it(`labels the dropped ${axis} axis`, async () => {
      invoke.mockResolvedValue(viewResponse({ droppedAxis: axis }))
      render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
      await screen.findByText(message)
    })
  }

  it('highlights the lower and upper face of the selected cell', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { container } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    await screen.findByText('2 faces')
    expect(container.querySelectorAll('.non-flat-layer-viewer-face-lower')).toHaveLength(1)
    expect(container.querySelectorAll('.non-flat-layer-viewer-face-upper')).toHaveLength(1)
    const items = [...container.querySelectorAll('.non-flat-layer-viewer-world li')]
    expect(items[0]!.textContent).toContain('Lower face')
    expect(items[1]!.textContent).toContain('Upper face')
  })

  it('selects a face with the pointer and with the keyboard', async () => {
    invoke.mockResolvedValue(viewResponse())
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('2 faces')
    const first = screen.getByRole('button', { name: `Select face ${FACE_A}` })
    const second = screen.getByRole('button', { name: `Select face ${FACE_B}` })
    expect(first.getAttribute('aria-selected')).toBe('true')
    fireEvent.click(second)
    expect(second.getAttribute('aria-selected')).toBe('true')
    fireEvent.keyDown(second, { key: 'ArrowUp' })
    expect(first.getAttribute('aria-selected')).toBe('true')
    fireEvent.keyDown(first, { key: 'End' })
    expect(second.getAttribute('aria-selected')).toBe('true')
    fireEvent.keyDown(second, { key: 'Home' })
    expect(first.getAttribute('aria-selected')).toBe('true')
  })

  it('selects a cell with the keyboard', async () => {
    invoke.mockResolvedValue(viewResponse())
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('2 faces')
    const cell = screen.getByRole('button', { name: `Select cell ${'c'.repeat(64)}` })
    expect(cell.getAttribute('aria-selected')).toBe('true')
    fireEvent.keyDown(cell, { key: 'ArrowDown' })
    expect(cell.getAttribute('aria-selected')).toBe('true')
  })

  it('UI-SRC-01 switches locale without refetching or losing the selection', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    await screen.findByText('2 faces')
    fireEvent.click(screen.getByRole('button', { name: `Select face ${FACE_B}` }))
    expect(invoke).toHaveBeenCalledTimes(1)
    rerender(<CurrentNonFlatLayerOrderViewer locale="ja" source={source} />)
    await waitFor(() => expect(screen.getByText('面 2 件')).toBeTruthy())
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(
      screen.getByRole('button', { name: `面 ${FACE_B} を選択` })
        .getAttribute('aria-selected'),
    ).toBe('true')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await waitFor(() => expect(screen.getByText('2 faces')).toBeTruthy())
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('UI-SRC-02 refetches when the source object changes even with the same values', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  })

  it('UI-SRC-03 refetches when only one hinge angle bit changes', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    await screen.findByText('2 faces')
    rerender(
      <CurrentNonFlatLayerOrderViewer
        locale="en"
        source={makeSource({
          hingeAngles: [{ edgeId: EDGE_1, angleDegrees: 73.50000000000001 }],
        })}
      />,
    )
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
    const request = invoke.mock.calls[1]?.[1] as {
      request: { expectedAppliedPose: { hingeAngles: { angleDegrees: number }[] } }
    }
    expect(request.request.expectedAppliedPose.hingeAngles[0]?.angleDegrees)
      .toBe(73.50000000000001)
  })

  it('UI-SRC-04 drops the previous geometry as soon as the source changes', async () => {
    const first = deferred<unknown>()
    const second = deferred<unknown>()
    invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    first.resolve(viewResponse())
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    expect(screen.queryByText('2 faces')).toBeNull()
    expect(screen.getByRole('status').textContent).toBe('Loading layer order…')
    second.resolve(viewResponse())
    await screen.findByText('2 faces')
  })

  it('UI-SRC-07 hides the viewer synchronously when the source becomes unbound', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={null} />)
    expect(screen.queryByRole('region')).toBeNull()
    expect(screen.queryByText('2 faces')).toBeNull()
  })

  it('does not reuse the previous response for the same semantic pose', async () => {
    const first = deferred<unknown>()
    const second = deferred<unknown>()
    invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    first.resolve(viewResponse())
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    expect(screen.queryByText('2 faces')).toBeNull()
    second.resolve(viewResponse({ cells: 0 }))
    await screen.findByText(
      'There are no overlap cells to show. This is not a proof that nothing collides.',
    )
  })

  it('UI-SRC-05 never lets a late response overwrite a newer request', async () => {
    const first = deferred<unknown>()
    const second = deferred<unknown>()
    invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const sourceA = makeSource()
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={sourceA} />,
    )
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    second.resolve(viewResponse({ cells: 0 }))
    await screen.findByText(
      'There are no overlap cells to show. This is not a proof that nothing collides.',
    )
    first.resolve(viewResponse())
    await Promise.resolve()
    expect(screen.queryByText('1 overlap cells')).toBeNull()
  })

  it('UI-SRC-06 survives an A to B to A reissue without reusing the first response', async () => {
    const a1 = deferred<unknown>()
    const b = deferred<unknown>()
    const a2 = deferred<unknown>()
    invoke
      .mockReturnValueOnce(a1.promise)
      .mockReturnValueOnce(b.promise)
      .mockReturnValueOnce(a2.promise)
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    a1.resolve(viewResponse())
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    b.resolve(viewResponse({ cells: 0 }))
    await screen.findByText(
      'There are no overlap cells to show. This is not a proof that nothing collides.',
    )
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    expect(invoke).toHaveBeenCalledTimes(3)
    expect(screen.getByRole('status').textContent).toBe('Loading layer order…')
    a2.resolve(viewResponse())
    await screen.findByText('2 faces')
  })

  it('UI-SRC-08 does not update state after unmount', async () => {
    const gate = deferred<unknown>()
    invoke.mockReturnValue(gate.promise)
    const { unmount } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    unmount()
    gate.resolve(viewResponse())
    await Promise.resolve()
    expect(screen.queryByText('2 faces')).toBeNull()
  })

  it('UI-SRC-09 does not update state after a rejected unmounted request', async () => {
    const gate = deferred<unknown>()
    invoke.mockReturnValue(gate.promise)
    const { unmount } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={source} />,
    )
    unmount()
    gate.reject({ version: 1, category: 'invalid_evidence' })
    await Promise.resolve()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  /** The next representable f64 above a finite value. */
  function oneBitAbove(value: number) {
    const buffer = new ArrayBuffer(8)
    new Float64Array(buffer)[0] = value
    const bits = new BigUint64Array(buffer)
    bits[0] = (bits[0] as bigint) + 1n
    return new Float64Array(buffer)[0] as number
  }

  const RESPONSE_BINDINGS: readonly (readonly [
    string,
    (value: ReturnType<typeof viewResponse>) => void,
  ])[] = [
    ['UI-BIND-01 a fixed face mismatch', (value) => {
      value.pose.fixedFaceId = FACE_B
    }],
    ['UI-BIND-02 a missing response hinge', (value) => {
      value.pose.hingeAngles = []
    }],
    ['UI-BIND-03 an extra response hinge', (value) => {
      value.pose.hingeAngles = [
        { edgeId: EDGE_1, angleDegrees: 73.5 },
        { edgeId: uuid(22), angleDegrees: 12 },
      ]
    }],
    ['UI-BIND-04 an edge ID mismatch', (value) => {
      value.pose.hingeAngles = [{ edgeId: uuid(22), angleDegrees: 73.5 }]
    }],
    ['UI-BIND-06 a response project instance mismatch', (value) => {
      value.projectInstanceId = uuid(9)
    }],
    ['UI-BIND-07 a response project mismatch', (value) => {
      value.projectId = uuid(9)
    }],
    ['UI-BIND-09 a response fingerprint mismatch', (value) => {
      value.foldModelFingerprintSha256 = 'e'.repeat(64)
    }],
  ]

  for (const [name, mutate] of RESPONSE_BINDINGS) {
    it(`${name} is refused without keeping old geometry`, async () => {
      const forged = viewResponse()
      mutate(forged)
      invoke.mockResolvedValue(forged)
      render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
      const alert = await screen.findByRole('alert')
      expect(alert.textContent)
        .toBe('The layer-order evidence did not satisfy the contract.')
      expect(screen.queryByText('2 faces')).toBeNull()
    })
  }

  it('UI-BIND-05b refuses a response hinge that differs by one bit', async () => {
    const forged = viewResponse()
    forged.pose.hingeAngles = [
      { edgeId: EDGE_1, angleDegrees: oneBitAbove(73.5) },
    ]
    invoke.mockResolvedValue(forged)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent)
      .toBe('The layer-order evidence did not satisfy the contract.')
  })

  it('refetches on demand without changing the source', async () => {
    invoke.mockResolvedValue(viewResponse())
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('2 faces')
    fireEvent.click(screen.getByRole('button', { name: 'Reload' }))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  })
})
