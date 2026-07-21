import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BeginnerShapeCanvasPreview } from '../src/components/BeginnerShapeCanvasPreview'

const context = { clearRect: vi.fn(), save: vi.fn(), restore: vi.fn(), translate: vi.fn(),
  beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), closePath: vi.fn(), stroke: vi.fn(),
  strokeStyle: '', lineWidth: 0 }
beforeEach(() => { Object.values(context).forEach((value) => typeof value === 'function' && value.mockClear())
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context as never) })
afterEach(() => { cleanup(); vi.restoreAllMocks() })

const target = { id: 2, count: 1, length_tenths_mm: 100, thickness_tenths_mm: 10,
  position_tenths_mm: [10, 20, 0] as [number, number, number], direction_milli: [1000, 0, 0] as [number, number, number],
  symmetry: 'none' as const, curvature_degrees: 0, joint: 'fixed' as const,
  motion_degrees: [0, 0] as [number, number], side: 'either' as const, priority: 50,
  local_outline_tenths_mm: [[-10, -10], [10, -10], [0, 10]] as Array<[number, number]> }

describe('BeginnerShapeCanvasPreview', () => {
  it('draws the body and lets the user select a binding outline', () => {
    render(<BeginnerShapeCanvasPreview locale="en" bodySize={[200, 100]} bodyOutline={[]} protrusions={[target]} />)
    expect(screen.getByLabelText('Body outline preview')).toBeTruthy()
    expect(context.stroke).toHaveBeenCalledOnce()
    fireEvent.change(screen.getByLabelText('Outline to preview'), { target: { value: '2' } })
    expect(screen.getByLabelText('Binding 2 outline preview')).toBeTruthy()
    expect(context.clearRect).toHaveBeenCalledTimes(2)
  })
  it('redraws immediately when outline dimensions change', () => {
    const view = render(<BeginnerShapeCanvasPreview locale="en" bodySize={undefined}
      bodyOutline={[[-20, -10], [20, -10], [20, 10], [-20, 10]]} protrusions={[]} />)
    view.rerender(<BeginnerShapeCanvasPreview locale="en" bodySize={undefined}
      bodyOutline={[[-30, -10], [30, -10], [30, 10], [-30, 10]]} protrusions={[]} />)
    expect(context.clearRect).toHaveBeenCalledTimes(2)
    expect(context.lineTo).toHaveBeenCalled()
  })
  it('exposes Japanese accessible names and missing-local status', () => {
    render(<BeginnerShapeCanvasPreview locale="ja" bodySize={[100, 100]} bodyOutline={[]}
      protrusions={[{ ...target, local_outline_tenths_mm: undefined }]} />)
    fireEvent.change(screen.getByLabelText('表示する輪郭'), { target: { value: '2' } })
    expect(screen.getByLabelText('binding 2の輪郭プレビュー')).toBeTruthy()
    expect(screen.getByRole('status').textContent).toContain('局所輪郭がありません')
  })
})
