import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  CreaseCanvas,
  type CreaseLine,
} from '../src/components/CreaseCanvas.tsx'
import {
  DEFAULT_SNAP_SETTINGS,
  type SnapSettings,
} from '../src/lib/snap.ts'
import {
  isConstructedVertexPlacement,
} from '../src/lib/vertexPlacement.ts'
import { localeFixture } from './localeTestFixture.ts'

const CANVAS_RECT = {
  x: 0,
  y: 0,
  top: 0,
  right: 500,
  bottom: 500,
  left: 0,
  width: 500,
  height: 500,
  toJSON: () => ({}),
} as DOMRect

const NO_SNAP_SETTINGS: SnapSettings = Object.freeze({
  ...DEFAULT_SNAP_SETTINGS,
  vertex: false,
  intersection: false,
  midpoint: false,
  horizontal: false,
  vertical: false,
  parallel: false,
  angle: false,
  edge: false,
  grid: false,
})

let paintedText: string[] = []
let paintedStrokeAlphas: number[] = []

beforeEach(() => {
  paintedText = []
  paintedStrokeAlphas = []
  const context = createCanvasContext(paintedText, paintedStrokeAlphas)
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context)
  vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect')
    .mockReturnValue(CANVAS_RECT)
  Object.defineProperties(HTMLCanvasElement.prototype, {
    setPointerCapture: { configurable: true, value: vi.fn() },
    hasPointerCapture: { configurable: true, value: vi.fn(() => true) },
    releasePointerCapture: { configurable: true, value: vi.fn() },
  })
  vi.stubGlobal('ResizeObserver', MockResizeObserver)
})

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
  vi.unstubAllGlobals()
})

describe('CreaseCanvas localization', () => {
  it('retranslates the mounted canvas accessibility copy immediately', () => {
    const store = localeFixture('ja')
    renderCanvas({ localeStore: store })

    const japaneseCanvas = screen.getByLabelText(
      '展開図編集キャンバス',
    )
    expect(japaneseCanvas.getAttribute('title')).toContain(
      '頂点をドラッグ',
    )
    expect(japaneseCanvas.textContent).toContain(
      '選択ツールでは頂点をドラッグ',
    )

    act(() => {
      store.setLocale('en')
    })

    const englishCanvas = screen.getByLabelText(
      'Crease-pattern editing canvas',
    )
    expect(englishCanvas).toBe(japaneseCanvas)
    expect(englishCanvas.getAttribute('title')).toContain(
      'drag a vertex',
    )
    expect(englishCanvas.textContent).toContain(
      'With the select tool',
    )
    expect(englishCanvas.textContent).not.toContain('展開図')
  })

  it('repaints an existing snap guide in the newly selected language', async () => {
    const store = localeFixture('ja')
    const gridOnly: SnapSettings = {
      ...DEFAULT_SNAP_SETTINGS,
      vertex: false,
      intersection: false,
      midpoint: false,
      horizontal: false,
      vertical: false,
      parallel: false,
      angle: false,
      edge: false,
      grid: true,
    }
    renderCanvas({
      localeStore: store,
      tool: 'vertex',
      snapSettings: gridOnly,
    })
    const canvas = screen.getByLabelText(
      '展開図編集キャンバス',
    )

    fireEvent.pointerMove(canvas, {
      clientX: 250,
      clientY: 250,
      pointerId: 1,
    })
    await waitFor(() => {
      expect(paintedText).toContain('グリッド')
    })

    paintedText.length = 0
    act(() => {
      store.setLocale('en')
    })
    await waitFor(() => {
      expect(paintedText).toContain('Grid')
    })
    expect(paintedText).not.toContain('グリッド')
  })

  it('translates known measurement units and hides untrusted raw copy', async () => {
    const store = localeFixture('en')
    const selectedLine: CreaseLine = {
      id: 'crease-1',
      startVertexId: 'a',
      endVertexId: 'b',
      x1: 0,
      y1: 0,
      x2: 400,
      y2: 400,
      kind: 'mountain',
    }
    const rendered = renderCanvas({
      localeStore: store,
      tool: 'measure',
      lines: [selectedLine],
      selectedLineId: selectedLine.id,
      measurementLabel: '0.5 紙辺比 / 30°',
    })
    await waitFor(() => {
      expect(paintedText).toContain('0.5 paper-edge ratio / 30°')
    })

    paintedText.length = 0
    rendered.rerender(
      <CreaseCanvas
        localeStore={store}
        tool="measure"
        lines={[selectedLine]}
        selectedLineId={selectedLine.id}
        measurementLabel="<img src=x onerror=alert(1)>"
        onSelectLine={() => undefined}
      />,
    )
    await waitFor(() => {
      expect(paintedText).toContain('Unavailable')
    })
    expect(paintedText.join(' ')).not.toContain('onerror')
  })

  it('localizes the unavailable title while retaining disabled semantics', () => {
    const store = localeFixture('en')
    renderCanvas({ localeStore: store, disabled: true })
    const canvas = screen.getByLabelText(
      'Crease-pattern editing canvas',
    )
    expect(canvas.getAttribute('aria-disabled')).toBe('true')
    expect(canvas.getAttribute('title')).toContain(
      'Editing is currently unavailable',
    )
  })

  it('selects a locked vertex before an overlapping edge in measure mode without moving it', () => {
    const onSelectVertex = vi.fn()
    const onSelectLine = vi.fn()
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'measure',
      paperBounds: { minX: 0, minY: 0, maxX: 100, maxY: 100 },
      vertices: [{ id: 'locked', x: 50, y: 50 }],
      lockedVertexIds: new Set(['locked']),
      lines: [{
        id: 'edge', startVertexId: 'a', endVertexId: 'b',
        x1: 0, y1: 50, x2: 100, y2: 50, kind: 'mountain',
      }],
      onSelectVertex,
      onSelectLine,
      onMoveVertex,
    })

    fireEvent.click(screen.getByLabelText('Crease-pattern editing canvas'), {
      clientX: 250,
      clientY: 250,
    })

    expect(onSelectVertex).toHaveBeenCalledWith('locked')
    expect(onSelectLine).toHaveBeenCalledWith(null)
    expect(onMoveVertex).not.toHaveBeenCalled()
  })

  it('applies each admitted layer opacity to its crease stroke', async () => {
    renderCanvas({
      localeStore: localeFixture('en'),
      lines: [{
        id: 'translucent',
        startVertexId: 'a',
        endVertexId: 'b',
        x1: 0,
        y1: 0,
        x2: 400,
        y2: 400,
        kind: 'mountain',
        layerOrder: 2,
        opacity: 0.35,
      }],
    })

    await waitFor(() => {
      expect(paintedStrokeAlphas).toContain(0.35)
    })
  })

  it('selects a topology face when empty paper is clicked', () => {
    const onSelectFace = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      faces: [{
        id: 'face-a',
        polygon: [
          { x: 0, y: 0 },
          { x: 400, y: 0 },
          { x: 400, y: 400 },
          { x: 0, y: 400 },
        ],
      }],
      onSelectFace,
    })

    fireEvent.click(screen.getByLabelText('Crease-pattern editing canvas'), {
      clientX: 250,
      clientY: 250,
    })
    expect(onSelectFace).toHaveBeenCalledWith('face-a')
  })
})

describe('CreaseCanvas vertex dragging', () => {
  it('does not silently snap a moved vertex onto an unsplit edge midpoint', () => {
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [
        { id: 'moving', x: 100, y: 100 },
        { id: 'left', x: 0, y: 200 },
        { id: 'right', x: 400, y: 200 },
      ],
      lines: [{
        id: 'target-edge',
        startVertexId: 'left',
        endVertexId: 'right',
        x1: 0,
        y1: 200,
        x2: 400,
        y2: 200,
        kind: 'mountain',
      }],
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 7, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 246, clientY: 246, pointerId: 7 })
    fireEvent.pointerUp(canvas, { clientX: 246, clientY: 246, pointerId: 7 })

    expect(onMoveVertex).toHaveBeenCalledOnce()
    expect(onMoveVertex).not.toHaveBeenCalledWith('moving', 200, 200)
  })

  it('rejects a raw drag along an unconnected edge through an unsplit intersection', () => {
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [
        { id: 'moving', x: 100, y: 100 },
        { id: 'a', x: 0, y: 0 },
        { id: 'b', x: 400, y: 400 },
        { id: 'c', x: 0, y: 400 },
        { id: 'd', x: 400, y: 0 },
      ],
      lines: [
        {
          id: 'ascending', startVertexId: 'a', endVertexId: 'b',
          x1: 0, y1: 0, x2: 400, y2: 400, kind: 'mountain',
        },
        {
          id: 'descending', startVertexId: 'c', endVertexId: 'd',
          x1: 0, y1: 400, x2: 400, y2: 0, kind: 'valley',
        },
      ],
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 8, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 246, clientY: 246, pointerId: 8 })
    fireEvent.pointerUp(canvas, { clientX: 246, clientY: 246, pointerId: 8 })

    expect(onMoveVertex).not.toHaveBeenCalled()
  })

  it('rejects an exact raw drag onto an unconnected edge when edge snapping is disabled', () => {
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [
        { id: 'moving', x: 100, y: 100 },
        { id: 'left', x: 0, y: 200 },
        { id: 'right', x: 400, y: 200 },
      ],
      lines: [{
        id: 'target-edge',
        startVertexId: 'left',
        endVertexId: 'right',
        x1: 0,
        y1: 200,
        x2: 400,
        y2: 200,
        kind: 'mountain',
      }],
      snapSettings: NO_SNAP_SETTINGS,
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 12, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 245, clientY: 245, pointerId: 12 })
    fireEvent.pointerUp(canvas, { clientX: 245, clientY: 245, pointerId: 12 })

    expect(onMoveVertex).not.toHaveBeenCalled()
  })

  it('allows an endpoint to move along its own incident edge when snapping is disabled', () => {
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [
        { id: 'moving', x: 100, y: 100 },
        { id: 'fixed', x: 300, y: 300 },
      ],
      lines: [{
        id: 'incident-edge',
        startVertexId: 'moving',
        endVertexId: 'fixed',
        x1: 100,
        y1: 100,
        x2: 300,
        y2: 300,
        kind: 'mountain',
      }],
      snapSettings: NO_SNAP_SETTINGS,
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 13, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 245, clientY: 245, pointerId: 13 })
    fireEvent.pointerUp(canvas, { clientX: 245, clientY: 245, pointerId: 13 })

    expect(onMoveVertex).toHaveBeenCalledWith('moving', 200, 200)
  })

  it('keeps grid snapping available for a moved vertex away from topology', () => {
    const onMoveVertex = vi.fn()
    const gridOnly: SnapSettings = {
      ...DEFAULT_SNAP_SETTINGS,
      vertex: false,
      intersection: false,
      midpoint: false,
      horizontal: false,
      vertical: false,
      parallel: false,
      angle: false,
      edge: false,
      grid: true,
    }
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [{ id: 'moving', x: 100, y: 100 }],
      gridDivisions: 4,
      snapSettings: gridOnly,
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 9, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 246, clientY: 246, pointerId: 9 })
    fireEvent.pointerUp(canvas, { clientX: 246, clientY: 246, pointerId: 9 })

    expect(onMoveVertex).toHaveBeenCalledWith('moving', 200, 200)
  })

  it('carries source-only native authority for an angle-snapped move', () => {
    const onMoveVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [{ id: 'moving', x: 100, y: 100 }],
      angleConfig: {
        angleDegrees: 45,
        referenceKind: 'global-horizontal',
      },
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 10, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 246, clientY: 246, pointerId: 10 })
    fireEvent.pointerUp(canvas, { clientX: 246, clientY: 246, pointerId: 10 })

    expect(onMoveVertex).toHaveBeenCalledOnce()
    const [vertexId, x, y, construction] = onMoveVertex.mock.calls[0]
    expect(vertexId).toBe('moving')
    expect(x).toBeCloseTo(y, 12)
    expect(construction).toMatchObject({
      schemaVersion: 1,
      constructionModelId: 'ori_canvas_constructed_vertex_binary64_native_v1',
      source: {
        kind: 'angle',
        anchorId: 'moving',
        rawX: expect.any(Number),
        rawY: expect.any(Number),
        angleDegrees: 45,
        angleSide: 'counterclockwise',
        referenceKind: 'global-horizontal',
      },
    })
  })

  it('does not attach angle authority to an outside-paper boundary drag', () => {
    const onMoveVertex = vi.fn()
    const angleOnly: SnapSettings = {
      ...DEFAULT_SNAP_SETTINGS,
      vertex: false,
      intersection: false,
      midpoint: false,
      horizontal: false,
      vertical: false,
      parallel: false,
      angle: true,
      edge: false,
      grid: false,
    }
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'select',
      vertices: [{ id: 'moving', x: 100, y: 100 }],
      paperPolygon: [
        { id: 'moving', x: 100, y: 100 },
        { id: 'right', x: 150, y: 100 },
        { id: 'corner', x: 150, y: 150 },
        { id: 'bottom', x: 100, y: 150 },
      ],
      angleConfig: {
        angleDegrees: 45,
        referenceKind: 'global-horizontal',
      },
      snapSettings: angleOnly,
      selectedVertexId: 'moving',
      onSelectVertex: () => undefined,
      onMoveVertex,
    })
    const canvas = screen.getByLabelText('Crease-pattern editing canvas')
    fireEvent.pointerDown(canvas, { clientX: 138, clientY: 138, pointerId: 11, button: 0 })
    fireEvent.pointerMove(canvas, { clientX: 246, clientY: 246, pointerId: 11 })
    fireEvent.pointerUp(canvas, { clientX: 246, clientY: 246, pointerId: 11 })

    expect(onMoveVertex).toHaveBeenCalledOnce()
    expect(onMoveVertex.mock.calls[0]).toHaveLength(3)
    expect(onMoveVertex.mock.calls[0][1]).toBeGreaterThan(150)
    expect(onMoveVertex.mock.calls[0][1]).toBeCloseTo(
      onMoveVertex.mock.calls[0][2],
      12,
    )
  })
})

describe('CreaseCanvas compass intersection placement', () => {
  it('routes a circle-line intersection through the existing edge-split operation', () => {
    const onPlaceVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'vertex',
      vertices: [
        { id: 'left', x: 0, y: 200 },
        { id: 'right', x: 400, y: 200 },
        { id: 'center', x: 200, y: 200 },
      ],
      lines: [{
        id: 'crease',
        startVertexId: 'left',
        endVertexId: 'right',
        x1: 0,
        y1: 200,
        x2: 400,
        y2: 200,
        kind: 'mountain',
      }],
      compassCircles: [{
        centerVertexId: 'center',
        centerX: 200,
        centerY: 200,
        radius: 100,
      }],
      onPlaceVertex,
    })

    fireEvent.click(screen.getByLabelText('Crease-pattern editing canvas'), {
      clientX: 357,
      clientY: 250,
    })

    expect(onPlaceVertex).toHaveBeenCalledOnce()
    const placement = onPlaceVertex.mock.calls[0][0]
    expect(placement).toEqual(expect.objectContaining({
      operation: 'split-edge',
      edgeId: 'crease',
      fraction: 0.75,
    }))
    expect(isConstructedVertexPlacement(placement)).toBe(true)
    expect(placement.nativeConstruction.source).toEqual({
      kind: 'circle-line',
      centerVertexId: 'center',
      radius: 100,
      edgeId: 'crease',
      rootSide: 1,
    })
  })

  it('routes a circle-circle intersection through the existing vertex-add operation', () => {
    const onPlaceVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'vertex',
      vertices: [
        { id: 'first-center', x: 140, y: 200 },
        { id: 'second-center', x: 260, y: 200 },
      ],
      compassCircles: [
        { centerVertexId: 'first-center', centerX: 140, centerY: 200, radius: 100 },
        { centerVertexId: 'second-center', centerX: 260, centerY: 200, radius: 100 },
      ],
      onPlaceVertex,
    })

    fireEvent.click(screen.getByLabelText('Crease-pattern editing canvas'), {
      clientX: 250,
      clientY: 336,
    })

    expect(onPlaceVertex).toHaveBeenCalledOnce()
    const placement = onPlaceVertex.mock.calls[0][0]
    expect(placement).toEqual(expect.objectContaining({
      operation: 'add',
      x: 200,
      y: 280,
    }))
    expect(isConstructedVertexPlacement(placement)).toBe(true)
    expect(placement.nativeConstruction.source).toEqual({
      kind: 'circle-circle',
      firstCenterVertexId: 'first-center',
      firstRadius: 100,
      secondCenterVertexId: 'second-center',
      secondRadius: 100,
      intersectionSide: 0,
    })
  })

  it('does not place a circle intersection outside a non-rectangular paper boundary', () => {
    const onPlaceVertex = vi.fn()
    renderCanvas({
      localeStore: localeFixture('en'),
      tool: 'vertex',
      vertices: [
        { id: 'first-center', x: 140, y: 200 },
        { id: 'second-center', x: 260, y: 200 },
      ],
      paperPolygon: [
        { x: 0, y: 0 },
        { x: 400, y: 0 },
        { x: 200, y: 200 },
      ],
      compassCircles: [
        { centerVertexId: 'first-center', centerX: 140, centerY: 200, radius: 100 },
        { centerVertexId: 'second-center', centerX: 260, centerY: 200, radius: 100 },
      ],
      onPlaceVertex,
    })

    fireEvent.click(screen.getByLabelText('Crease-pattern editing canvas'), {
      clientX: 250,
      clientY: 336,
    })

    expect(onPlaceVertex).not.toHaveBeenCalled()
  })
})

function renderCanvas(
  overrides: Partial<React.ComponentProps<typeof CreaseCanvas>> = {},
) {
  return render(
    <CreaseCanvas
      lines={[]}
      selectedLineId={null}
      onSelectLine={() => undefined}
      {...overrides}
    />,
  )
}

class MockResizeObserver {
  private readonly callback: ResizeObserverCallback

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback
  }

  observe() {
    this.callback([], this as unknown as ResizeObserver)
  }

  unobserve() {}

  disconnect() {}
}

function createCanvasContext(
  text: string[],
  strokeAlphas: number[],
): CanvasRenderingContext2D {
  const context = {
    arc: vi.fn(),
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    clip: vi.fn(),
    closePath: vi.fn(),
    fill: vi.fn(),
    fillRect: vi.fn(),
    fillText: vi.fn((value: string) => {
      text.push(value)
    }),
    lineTo: vi.fn(),
    measureText: vi.fn((value: string) => ({
      width: value.length * 6,
    })),
    moveTo: vi.fn(),
    restore: vi.fn(),
    save: vi.fn(),
    setLineDash: vi.fn(),
    setTransform: vi.fn(),
    stroke: vi.fn(() => {
      strokeAlphas.push(context.globalAlpha)
    }),
    strokeRect: vi.fn(),
    globalAlpha: 1,
  } as unknown as CanvasRenderingContext2D
  return context
}
