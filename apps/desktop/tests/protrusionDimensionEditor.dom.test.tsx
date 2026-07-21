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
  })
  it('renders bilateral semantics in Japanese and removes explicitly', () => {
    const remove = vi.fn()
    render(<ul><ProtrusionDimensionEditor locale="ja" target={{ ...target, count: 2, symmetry: 'bilateral' }} onChange={() => {}} onRemove={remove} /></ul>)
    expect(screen.getByText('binding 1・左右対称・数 2')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '削除' }))
    expect(remove).toHaveBeenCalledOnce()
  })
})
