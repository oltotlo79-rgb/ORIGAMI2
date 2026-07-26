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

function viewResponse(cells = 1) {
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
          droppedWorldAxis: 'z', planeAxes: ['x', 'y'],
          sourceToPlaneProjectionExact: affine(),
        },
      },
      {
        faceId: FACE_B,
        faceKeySha256: 'b'.repeat(64),
        worldOuterBoundaryXyzMm: [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
        projection: {
          droppedWorldAxis: 'z', planeAxes: ['x', 'y'],
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
          droppedWorldAxis: 'z', planeAxes: ['x', 'y'],
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

const source = {
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
  },
} as const

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
    expect(screen.getByText('2 faces')).toBeTruthy()
    expect(screen.getByText('1 overlap cells')).toBeTruthy()
    for (const forbidden of ['Apply', 'Commit', 'Adopt']) {
      expect(screen.queryByRole('button', { name: forbidden })).toBeNull()
    }
  })

  it('shows the zero-cell warning without claiming a collision-free proof', async () => {
    invoke.mockResolvedValue(viewResponse(0))
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

  it('maps a native category to a closed failure message', async () => {
    invoke.mockRejectedValue({ version: 1, category: 'stale_authority' })
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={source} />)
    await screen.findByText('The pose or project changed, so the view is unavailable.')
  })

  it('hides itself when the applied pose is not stable', () => {
    render(<CurrentNonFlatLayerOrderViewer locale="en" source={null} />)
    expect(screen.queryByRole('region')).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
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
  })
})
