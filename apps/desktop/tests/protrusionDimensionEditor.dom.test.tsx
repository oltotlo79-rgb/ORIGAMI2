import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ProtrusionDimensionEditor } from '../src/components/ProtrusionDimensionEditor'

const target = { id: 1, count: 1, symmetry: 'none' as const, length_tenths_mm: 200,
  thickness_tenths_mm: 20, position_tenths_mm: [0, 0, 0] as [number, number, number],
  direction_milli: [1000, 0, 0] as [number, number, number], curvature_degrees: 0,
  joint: 'fixed' as const, motion_degrees: [0, 0] as [number, number],
  side: 'either' as const, priority: 50 }
afterEach(cleanup)

describe('ProtrusionDimensionEditor', () => {
  it('shows asymmetric-none semantics and emits bounded tenths-mm edits', () => {
    const change = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="en" target={target} onChange={change} onRemove={() => {}} /></ul>)
    expect(screen.getByText('Binding 1 · Asymmetric single · count 1')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Length binding 1 (mm)'), { target: { value: '25.5' } })
    expect(change).toHaveBeenCalledWith(expect.objectContaining({ length_tenths_mm: 255 }))
    fireEvent.change(screen.getByLabelText('Thickness binding 1 (mm)'), { target: { value: '0' } })
    expect(change).toHaveBeenCalledTimes(1)
    fireEvent.change(screen.getByLabelText('Symmetry binding 1'), { target: { value: 'bilateral' } })
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ symmetry: 'bilateral', count: 2 }))
  })
  it('renders bilateral semantics in Japanese and removes explicitly', () => {
    const remove = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="ja" target={{ ...target, count: 2, symmetry: 'bilateral' }} onChange={() => {}} onRemove={remove} /></ul>)
    expect(screen.getByText('binding 1・左右対称・数 2')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '削除' }))
    expect(remove).toHaveBeenCalledOnce()
  })
  it('exposes bounded reorder controls', () => {
    const moveUp = vi.fn()
    const moveDown = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="en" target={target} onChange={() => {}}
      onRemove={() => {}} onMoveUp={moveUp} onMoveDown={moveDown}
      canMoveUp={false} canMoveDown /></ul>)
    const up = screen.getByRole('button', { name: 'Move up' })
    const down = screen.getByRole('button', { name: 'Move down' })
    expect((up as HTMLButtonElement).disabled).toBe(true)
    expect((down as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(up)
    fireEvent.click(down)
    expect(moveUp).not.toHaveBeenCalled()
    expect(moveDown).toHaveBeenCalledOnce()
  })
  it('edits the bound part kind without changing dimensions', () => {
    const kindChange = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="en" target={target} kind="tail"
      onKindChange={kindChange} onChange={() => {}} onRemove={() => {}} /></ul>)
    fireEvent.change(screen.getByLabelText('Part kind binding 1'), { target: { value: 'wing' } })
    expect(kindChange).toHaveBeenCalledWith('wing')
  })
  it('bounds mount position and direction edits and labels bilateral spacing', () => {
    const change = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="en"
      target={{ ...target, count: 2, symmetry: 'bilateral' }} onChange={change} onRemove={() => {}} /></ul>)
    expect(screen.getByLabelText('Bilateral spacing binding 1 (mm)')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Mount vertical binding 1 (mm)'), { target: { value: '12.5' } })
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ position_tenths_mm: [0, 125, 0] }))
    fireEvent.change(screen.getByLabelText('Mount fore-aft binding 1 (mm)'), { target: { value: '10001' } })
    expect(change).toHaveBeenCalledTimes(1)
    fireEvent.change(screen.getByLabelText('Direction horizontal binding 1'), { target: { value: '0.5' } })
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ direction_milli: [500, 0, 0] }))
  })
  it('edits optional root and tip widths and clears them without inventing fallback values', () => {
    const change = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="en"
      target={{ ...target, root_width_tenths_mm: 40, tip_width_tenths_mm: 10 }}
      onChange={change} onRemove={() => {}} /></ul>)
    fireEvent.change(screen.getByLabelText('Root width binding 1 (mm)'), { target: { value: '5.5' } })
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ root_width_tenths_mm: 55 }))
    fireEvent.change(screen.getByLabelText('Tip width binding 1 (mm)'), { target: { value: '' } })
    expect(change.mock.calls.at(-1)?.[0]).not.toHaveProperty('tip_width_tenths_mm')
    fireEvent.change(screen.getByLabelText('Root width binding 1 (mm)'), { target: { value: '1001' } })
    expect(change).toHaveBeenCalledTimes(2)
  })
  it('exposes the optional taper controls in Japanese', () => {
    render(<ul><ProtrusionDimensionEditor locale="ja" target={target}
      onChange={() => {}} onRemove={() => {}} /></ul>)
    expect(screen.getByLabelText('根元幅 binding 1 (mm)')).toBeTruthy()
    expect(screen.getByLabelText('先端幅 binding 1 (mm)')).toBeTruthy()
  })
})
