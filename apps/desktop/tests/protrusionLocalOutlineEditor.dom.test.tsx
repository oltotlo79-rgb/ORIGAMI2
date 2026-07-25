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
  it('switches locale in place without resetting invalid edits or invoking callbacks', () => {
    const change = vi.fn()
    const view = render(<ProtrusionLocalOutlineEditor locale="en" bindingId={5}
      symmetry="bilateral" points={[]} onChange={change} />)
    const editedSource = '-5,-5\n5,-5\n4,5\n-3,5'
    const englishInput = screen.getByLabelText(
      'Local outline points binding 5',
    ) as HTMLTextAreaElement
    fireEvent.change(englishInput, { target: { value: editedSource } })
    fireEvent.click(screen.getByRole('button', { name: 'Apply local outline' }))
    const englishAlert = screen.getByRole('alert')
    expect(englishAlert.textContent).toBe(
      'Enter 3 to 8 bounded points. Bilateral bindings require mirrored points.',
    )
    expect(change).not.toHaveBeenCalled()

    view.rerender(<ProtrusionLocalOutlineEditor locale="ja" bindingId={5}
      symmetry="bilateral" points={[]} onChange={change} />)

    const japaneseInput = screen.getByLabelText(
      '局所輪郭点 binding 5',
    ) as HTMLTextAreaElement
    expect(japaneseInput).toBe(englishInput)
    expect(japaneseInput.value).toBe(editedSource)
    expect(screen.getByRole('alert')).toBe(englishAlert)
    expect(screen.getByRole('alert').textContent).toBe(
      '3〜8点の有界な輪郭を入力してください。左右対称bindingでは鏡像点が必要です。',
    )
    expect(change).not.toHaveBeenCalled()
  })
})
