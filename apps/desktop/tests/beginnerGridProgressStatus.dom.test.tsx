import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { BeginnerGridProgressStatus } from '../src/components/BeginnerGridProgressStatus'

describe('BeginnerGridProgressStatus', () => {
  it('bounds refinement progress, cancels, and retranslates without stale values', () => {
    const cancel = vi.fn()
    const { rerender } = render(<BeginnerGridProgressStatus locale="ja" busy
      enumerated={99} checked={99} refined={99} onCancel={cancel} />)
    expect(screen.getByRole('status').textContent).toBe('列挙 27/27・局所改善 24/24・大域検証 3/3')
    fireEvent.click(screen.getByRole('button', { name: '候補生成をキャンセル' }))
    expect(cancel).toHaveBeenCalledTimes(1)
    rerender(<BeginnerGridProgressStatus locale="en" busy
      enumerated={Number.NaN} checked={-1} refined={Number.NaN} onCancel={cancel} />)
    expect(screen.getByRole('status').textContent).toBe('Enumerated 0/27 · refined 0/24 · globally checked 0/3')
    expect(screen.getByRole('group', { name: 'Candidate generation and local refinement progress' })).toBeTruthy()
  })

  it('publishes nothing after completion or replacement', () => {
    const { container, rerender } = render(<BeginnerGridProgressStatus locale="en" busy
      enumerated={12} checked={1} refined={4} onCancel={() => {}} />)
    rerender(<BeginnerGridProgressStatus locale="en" busy={false}
      enumerated={27} checked={3} refined={24} onCancel={() => {}} />)
    expect(container.innerHTML).toBe('')
  })
})
