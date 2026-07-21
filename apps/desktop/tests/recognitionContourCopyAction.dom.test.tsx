import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { RecognitionContourCopyAction } from '../src/components/RecognitionContourCopyAction'
afterEach(() => { cleanup(); vi.restoreAllMocks() })
describe('RecognitionContourCopyAction', () => {
  it('copies only after explicit English confirmation', () => { const copy = vi.fn()
    vi.spyOn(window, 'confirm').mockReturnValue(false)
    render(<RecognitionContourCopyAction locale="en" bodyPointCount={4} localContourCount={2} onCopy={copy} />)
    fireEvent.click(screen.getByRole('button', { name: 'Review and copy contours to editor' }))
    expect(copy).not.toHaveBeenCalled()
    vi.mocked(window.confirm).mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'Review and copy contours to editor' }))
    expect(copy).toHaveBeenCalledOnce() })
  it('renders Japanese counts and confirmation', () => { vi.spyOn(window, 'confirm').mockReturnValue(true)
    render(<RecognitionContourCopyAction locale="ja" bodyPointCount={6} localContourCount={1} onCopy={() => {}} />)
    expect(screen.getByText('編集可能な胴体輪郭 6 点・局所輪郭 1 件')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '確認して輪郭を編集欄へコピー' }))
    expect(window.confirm).toHaveBeenCalled() })
  it('hides when the proposal has no contour', () => {
    const { container } = render(<RecognitionContourCopyAction locale="en"
      bodyPointCount={0} localContourCount={0} onCopy={() => {}} />)
    expect(container.textContent).toBe('') })
})
