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
