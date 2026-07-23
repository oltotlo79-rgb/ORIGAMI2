import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BulkIntersectionRepairControl } from '../src/components/BulkIntersectionRepairControl'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

describe('BulkIntersectionRepairControl', () => {
  it('hides zero and displays the exact diagnostic count', () => {
    const { rerender } = render(<BulkIntersectionRepairControl count={0} pending={false} disabled={false} locale="ja" onConfirm={() => {}} />)
    expect(screen.queryByTestId('repair-all-unsplit-intersections')).toBeNull()
    rerender(<BulkIntersectionRepairControl count={16} pending={false} disabled={false} locale="ja" onConfirm={() => {}} />)
    expect(screen.getByRole('button').textContent).toContain('16件')
  })

  it('cancels confirmation and prevents pending double execution', () => {
    const action = vi.fn()
    vi.spyOn(window, 'confirm').mockReturnValue(false)
    const { rerender } = render(<BulkIntersectionRepairControl count={8} pending={false} disabled={false} locale="en" onConfirm={action} />)
    fireEvent.click(screen.getByRole('button'))
    expect(action).not.toHaveBeenCalled()
    vi.mocked(window.confirm).mockReturnValue(true)
    fireEvent.click(screen.getByRole('button'))
    expect(action).toHaveBeenCalledOnce()
    rerender(<BulkIntersectionRepairControl count={8} pending disabled={false} locale="en" onConfirm={action} />)
    expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(screen.getByRole('button'))
    expect(action).toHaveBeenCalledOnce()
  })
})
