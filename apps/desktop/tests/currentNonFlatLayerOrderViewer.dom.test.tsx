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
function makeSource(overrides: Partial<Source['appliedPose']> = {}): Source {
  return {
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 12,
    foldModelFingerprintSha256: FINGERPRINT,
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
  it('renders both panes read-only without mutation controls', async () => {
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

  it('shows the loading status before the response settles', async () => {
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

  it('reports absence when the project owns no non-flat evidence', async () => {
    invoke.mockResolvedValue(null)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('No non-flat layer order is bound to the current pose.')
  })

  const FAILURES: readonly (readonly [string, string])[] = [
    ['stale_authority', 'The pose or project changed, so the view is unavailable.'],
    ['invalid_evidence', 'The layer-order evidence did not satisfy the contract.'],
    ['resource_limit', 'The layer order exceeds the viewer limits.'],
    ['internal_failure', 'The layer order could not be read.'],
  ]

  for (const [category, message] of FAILURES) {
    it(`maps the ${category} category to a closed failure message`, async () => {
      invoke.mockRejectedValue({ version: 1, category })
      render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
      const alert = await screen.findByRole('alert')
      expect(alert.textContent).toBe(message)
    })
  }

  it('never shows a raw native error string', async () => {
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

  const NON_INVOKING: readonly (readonly [string, Partial<Source['appliedPose']>])[] = [
    ['a running pose', { state: 'running' }],
    ['a blocked pose', { state: 'blocked' }],
    ['an indeterminate pose', { state: 'indeterminate' }],
    ['a project ID mismatch', { projectId: uuid(3) }],
    ['a revision mismatch', { revision: 13 }],
    ['a null fixed face', { fixedFaceId: null }],
    ['an empty hinge vector', { hingeAngles: [] }],
    ['a duplicate request hinge', {
      hingeAngles: [
        { edgeId: EDGE_1, angleDegrees: 73.5 },
        { edgeId: EDGE_1, angleDegrees: 12 },
      ],
    }],
    ['a nonfinite request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: Number.NaN }],
    }],
    ['a negative-zero request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: -0 }],
    }],
    ['an out-of-range request hinge', {
      hingeAngles: [{ edgeId: EDGE_1, angleDegrees: 181 }],
    }],
  ]

  for (const [name, overrides] of NON_INVOKING) {
    it(`never invokes the command for ${name}`, async () => {
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

  it('rejects a response whose pose binding differs from the request', async () => {
    const forged = viewResponse()
    forged.pose.hingeAngles = [{ edgeId: EDGE_1, angleDegrees: 73.25 }]
    invoke.mockResolvedValue(forged)
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toBe('The layer-order evidence did not satisfy the contract.')
  })

  it('rejects a response whose project binding differs from the request', async () => {
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

  it('switches locale without refetching or losing the selection', async () => {
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

  it('refetches when the source object changes even with the same values', async () => {
    invoke.mockResolvedValue(viewResponse())
    const { rerender } = render(
      <CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />,
    )
    await screen.findByText('2 faces')
    rerender(<CurrentNonFlatLayerOrderViewer locale="en" source={makeSource()} />)
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  })

  it('refetches when only one hinge angle bit changes', async () => {
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

  it('drops the previous geometry as soon as the source changes', async () => {
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

  it('hides the viewer synchronously when the source becomes unbound', async () => {
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

  it('never lets a late response overwrite a newer request', async () => {
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

  it('survives an A to B to A reissue without reusing the first response', async () => {
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

  it('does not update state after unmount', async () => {
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

  it('does not update state after a rejected unmounted request', async () => {
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

  it('refetches on demand without changing the source', async () => {
    invoke.mockResolvedValue(viewResponse())
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('2 faces')
    fireEvent.click(screen.getByRole('button', { name: 'Reload' }))
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  })
})
