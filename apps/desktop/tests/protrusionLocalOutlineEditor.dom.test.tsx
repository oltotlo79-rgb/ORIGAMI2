import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ProtrusionLocalOutlineEditor } from '../src/components/ProtrusionLocalOutlineEditor'
afterEach(cleanup)
describe('ProtrusionLocalOutlineEditor', () => {
  it('canonicalizes a general local triangle CCW', () => { const change = vi.fn()
    render(<ProtrusionLocalOutlineEditor locale="en" bindingId={3} symmetry="none" points={[]} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Local outline points binding 3'), { target: { value: '5,-4\n-6,-3\n1,7' } })
    fireEvent.click(screen.getByRole('button', { name: 'Apply local outline' }))
    expect(change).toHaveBeenCalledWith([[-60, -30], [50, -40], [10, 70]]) })
  it('rejects a bilateral outline without mirror points', () => { const change = vi.fn()
    render(<ProtrusionLocalOutlineEditor locale="en" bindingId={2} symmetry="bilateral" points={[]} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Local outline points binding 2'), { target: { value: '-5,-5\n5,-5\n4,5\n-3,5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Apply local outline' }))
    expect(screen.getByRole('alert')).toBeTruthy(); expect(change).not.toHaveBeenCalled() })
  it('clears optional geometry explicitly in Japanese', () => { const change = vi.fn()
    render(<ProtrusionLocalOutlineEditor locale="ja" bindingId={1} symmetry="none"
      points={[[0, 0], [10, 0], [0, 10]]} onChange={change} />)
    fireEvent.click(screen.getByRole('button', { name: '局所輪郭を解除' }))
    expect(change).toHaveBeenCalledWith(undefined) })
})
