import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { GenericBodyOutlineEditor } from '../src/components/GenericBodyOutlineEditor'

afterEach(cleanup)

describe('GenericBodyOutlineEditor', () => {
  it('canonicalizes a bounded symmetric outline in English', () => {
    const change = vi.fn()
    render(<GenericBodyOutlineEditor locale="en" points={[]} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Body outline points'), {
      target: { value: '10,-5\n-10,5\n10,5\n-10,-5' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Apply outline' }))
    expect(change).toHaveBeenCalledWith([[-100, -50], [-100, 50], [100, 50], [100, -50]])
  })

  it('rejects asymmetric and out-of-paper input without changing state', () => {
    const change = vi.fn()
    render(<GenericBodyOutlineEditor locale="en" points={[]} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Body outline points'), {
      target: { value: '-1,-1\n-1,1\n2,1\n2,-1' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Apply outline' }))
    expect(screen.getByRole('alert')).toBeTruthy()
    expect(change).not.toHaveBeenCalled()
  })

  it('provides Japanese apply and explicit clear controls', () => {
    const change = vi.fn()
    render(<GenericBodyOutlineEditor locale="ja"
      points={[[-10, -10], [-10, 10], [10, 10], [10, -10]]} onChange={change} />)
    expect(screen.getByLabelText('胴体輪郭点')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '輪郭指定を解除' }))
    expect(change).toHaveBeenCalledWith([])
  })
})
