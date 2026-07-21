import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { BeginnerGridProgressStatus } from '../src/components/BeginnerGridProgressStatus'

describe('BeginnerGridProgressStatus', () => {
  it('bounds progress, cancels, and retranslates without publishing stale values', () => {
    const cancel = vi.fn()
    const { rerender } = render(<BeginnerGridProgressStatus locale="ja" busy
      enumerated={99} checked={99} onCancel={cancel} />)
    expect(screen.getByRole('status').textContent).toBe('列挙 27/27・大域検証 3/3')
    fireEvent.click(screen.getByRole('button', { name: '27案の評価をキャンセル' }))
    expect(cancel).toHaveBeenCalledTimes(1)
    rerender(<BeginnerGridProgressStatus locale="en" busy
      enumerated={Number.NaN} checked={-1} onCancel={cancel} />)
    expect(screen.getByRole('status').textContent).toBe('Enumerated 0/27 · globally checked 0/3')
    expect(screen.getByRole('group', { name: 'Progress of the 27-design search including complete animals' })).toBeTruthy()
  })

  it('publishes nothing after completion or replacement', () => {
    const { container, rerender } = render(<BeginnerGridProgressStatus locale="en" busy
      enumerated={12} checked={1} onCancel={() => {}} />)
    rerender(<BeginnerGridProgressStatus locale="en" busy={false}
      enumerated={27} checked={3} onCancel={() => {}} />)
    expect(container.innerHTML).toBe('')
  })
})
