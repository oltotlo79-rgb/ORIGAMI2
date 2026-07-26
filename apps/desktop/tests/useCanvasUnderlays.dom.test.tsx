import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import {
  useCanvasUnderlays,
  type CanvasUnderlayImageLoader,
} from '../src/lib/useCanvasUnderlays.ts'

const PROJECT_ID = '10000000-0000-4000-8000-000000000001'
const INSTANCE_ID = '20000000-0000-4000-8000-000000000001'
const LAYER_ID = '30000000-0000-4000-8000-000000000001'
const ASSET_A = '40000000-0000-4000-8000-000000000001'
const ASSET_B = '40000000-0000-4000-8000-000000000002'

afterEach(cleanup)

describe('useCanvasUnderlays', () => {
  it('loads the bound assets and creates only visible underlay canvas records', async () => {
    const imageA = new Image()
    const imageB = new Image()
    const imageLoader = vi.fn<CanvasUnderlayImageLoader>(
      async (_projectId, _revision, _instanceId, asset) =>
        asset === ASSET_A ? imageA : imageB,
    )
    const snapshot = projectSnapshot([
      underlay('50000000-0000-4000-8000-000000000001', ASSET_A, LAYER_ID),
      underlay(
        '50000000-0000-4000-8000-000000000002',
        ASSET_B,
        '30000000-0000-4000-8000-000000000002',
      ),
    ], [
      layer(LAYER_ID, true, 0.5),
      layer('30000000-0000-4000-8000-000000000002', false, 1),
    ])

    render(<Harness snapshot={snapshot} imageLoader={imageLoader} />)

    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).toBe(
        '50000000-0000-4000-8000-000000000001:0.4',
      )
    })
    expect(imageLoader.mock.calls).toEqual([
      [PROJECT_ID, 7, INSTANCE_ID, ASSET_A],
      [PROJECT_ID, 7, INSTANCE_ID, ASSET_B],
    ])
  })

  it('fails closed for the whole generation when any asset cannot load', async () => {
    const imageLoader = vi.fn<CanvasUnderlayImageLoader>(
      async (_projectId, _revision, _instanceId, asset) => {
        if (asset === ASSET_B) throw new Error('unavailable')
        return new Image()
      },
    )
    const view = render(<Harness
      snapshot={projectSnapshot([
        underlay('50000000-0000-4000-8000-000000000001', ASSET_A, LAYER_ID),
      ])}
      imageLoader={imageLoader}
    />)
    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).toContain(
        '50000000-0000-4000-8000-000000000001',
      )
    })

    view.rerender(<Harness
      snapshot={projectSnapshot([
        underlay('50000000-0000-4000-8000-000000000001', ASSET_A, LAYER_ID),
        underlay('50000000-0000-4000-8000-000000000002', ASSET_B, LAYER_ID),
      ], undefined, 8)}
      imageLoader={imageLoader}
    />)

    await waitFor(() => {
      expect(imageLoader).toHaveBeenCalledTimes(3)
      expect(screen.getByTestId('underlays').textContent).toBe('')
    })
  })

  it('does not reuse an image from another snapshot with the same asset id', async () => {
    const pending: Array<(image: HTMLImageElement) => void> = []
    const imageLoader = vi.fn<CanvasUnderlayImageLoader>(
      () => new Promise((resolve) => pending.push(resolve)),
    )
    const view = render(<Harness
      snapshot={projectSnapshot([
        underlay('50000000-0000-4000-8000-000000000001', ASSET_A, LAYER_ID),
      ])}
      imageLoader={imageLoader}
    />)
    await waitFor(() => expect(pending).toHaveLength(1))
    pending[0](new Image())
    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).not.toBe('')
    })

    view.rerender(<Harness
      snapshot={projectSnapshot([
        underlay('50000000-0000-4000-8000-000000000002', ASSET_A, LAYER_ID),
      ], undefined, 8)}
      imageLoader={imageLoader}
    />)

    expect(screen.getByTestId('underlays').textContent).toBe('')
    await waitFor(() => expect(pending).toHaveLength(2))
    pending[1](new Image())
    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).toContain(
        '50000000-0000-4000-8000-000000000002',
      )
    })
  })

  it('ignores stale completions across an A-B-A snapshot sequence', async () => {
    const pending: Array<{
      asset: string
      resolve: (image: HTMLImageElement) => void
    }> = []
    const imageLoader = vi.fn<CanvasUnderlayImageLoader>(
      (_projectId, _revision, _instanceId, asset) =>
        new Promise((resolve) => pending.push({ asset, resolve })),
    )
    const snapshotA = projectSnapshot([
      underlay('50000000-0000-4000-8000-000000000001', ASSET_A, LAYER_ID),
    ])
    const snapshotB = projectSnapshot([
      underlay('50000000-0000-4000-8000-000000000002', ASSET_B, LAYER_ID),
    ], undefined, 8)
    const view = render(
      <Harness snapshot={snapshotA} imageLoader={imageLoader} />,
    )
    await waitFor(() => expect(pending).toHaveLength(1))

    view.rerender(
      <Harness snapshot={snapshotB} imageLoader={imageLoader} />,
    )
    await waitFor(() => expect(pending).toHaveLength(2))
    pending[1].resolve(new Image())
    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).toContain(
        '50000000-0000-4000-8000-000000000002',
      )
    })

    view.rerender(
      <Harness snapshot={snapshotA} imageLoader={imageLoader} />,
    )
    await waitFor(() => expect(pending).toHaveLength(3))
    expect(screen.getByTestId('underlays').textContent).toBe('')
    pending[0].resolve(new Image())
    await Promise.resolve()
    expect(screen.getByTestId('underlays').textContent).toBe('')

    pending[2].resolve(new Image())
    await waitFor(() => {
      expect(screen.getByTestId('underlays').textContent).toContain(
        '50000000-0000-4000-8000-000000000001',
      )
    })
  })
})

function Harness({
  snapshot,
  imageLoader,
}: Readonly<{
  snapshot: ProjectSnapshot | null
  imageLoader: CanvasUnderlayImageLoader
}>) {
  const underlays = useCanvasUnderlays(snapshot, imageLoader)
  return <output data-testid="underlays">{
    underlays.map(({ id, opacity }) => `${id}:${opacity}`).join(',')
  }</output>
}

function projectSnapshot(
  underlays: NonNullable<ProjectSnapshot['underlays']>['underlays'],
  layers = [layer(LAYER_ID, true, 0.5)],
  revision = 7,
): ProjectSnapshot {
  return {
    project_id: PROJECT_ID,
    project_instance_id: INSTANCE_ID,
    revision,
    underlays: { underlays },
    project_layers: { version: 1, layers },
  } as ProjectSnapshot
}

function underlay(
  id: string,
  asset: string,
  layerId: string,
): NonNullable<ProjectSnapshot['underlays']>['underlays'][number] {
  return {
    id,
    asset,
    transform: {
      position: { x: 1, y: 2 },
      scale_x: 0.1,
      scale_y: 0.2,
      rotation_degrees: 5,
    },
    opacity: 0.8,
    layer: layerId,
  }
}

function layer(id: string, visible: boolean, opacity: number) {
  return {
    id,
    name: id,
    content_kind: 'underlay' as const,
    visible,
    locked: false,
    opacity,
  }
}
