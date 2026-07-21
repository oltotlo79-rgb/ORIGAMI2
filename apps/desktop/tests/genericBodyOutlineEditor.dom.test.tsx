import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { GenericBodyOutlineEditor } from '../src/components/GenericBodyOutlineEditor'

afterEach(cleanup)

describe('GenericBodyOutlineEditor', () => {
  it('canonicalizes a bounded symmetric outline in English', () => {
    const change = vi.fn()
    render(<GenericBodyOutlineEditor locale="en" points={[]} mode="symmetric"
      onModeChange={() => {}} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Body outline points'), {
      target: { value: '10,-5\n-10,5\n10,5\n-10,-5' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Apply outline' }))
    expect(change).toHaveBeenCalledWith([[-100, -50], [-100, 50], [100, 50], [100, -50]])
  })

  it('rejects asymmetric and out-of-paper input without changing state', () => {
    const change = vi.fn()
    render(<GenericBodyOutlineEditor locale="en" points={[]} mode="symmetric"
      onModeChange={() => {}} onChange={change} />)
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
      points={[[-10, -10], [-10, 10], [10, 10], [10, -10]]} mode="symmetric"
      onModeChange={() => {}} onChange={change} />)
    expect(screen.getByLabelText('胴体輪郭点')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '輪郭指定を解除' }))
    expect(change).toHaveBeenCalledWith([])
  })
  it('explicitly switches to general mode and canonicalizes an asymmetric CCW polygon', () => {
    const change = vi.fn()
    const modeChange = vi.fn()
    render(<GenericBodyOutlineEditor locale="en" points={[]} mode="general"
      onModeChange={modeChange} onChange={change} />)
    fireEvent.change(screen.getByLabelText('Body outline points'), {
      target: { value: '8,-6\n-12,-4\n-7,8\n10,5' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Apply outline' }))
    expect(change).toHaveBeenCalledWith([[-120, -40], [80, -60], [100, 50], [-70, 80]])
    fireEvent.change(screen.getByLabelText('Body outline mode'), { target: { value: 'symmetric' } })
    expect(modeChange).toHaveBeenCalledWith('symmetric')
  })
})
