import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { PaperThicknessInput } from '../src/components/PaperThicknessInput.tsx'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('PaperThicknessInput', () => {
  it('steps finer direct input by exactly 0.01 mm with buttons and keys', () => {
    render(
      <PaperThicknessInput
        id="thickness"
        initialValue="0.075"
        disabled={false}
      />,
    )
    const input = screen.getByRole('spinbutton', {
      name: '紙厚',
    }) as HTMLInputElement

    fireEvent.click(screen.getByRole('button', {
      name: '紙厚を0.01 mm増やす',
    }))
    expect(input.value).toBe('0.085')

    fireEvent.click(screen.getByRole('button', {
      name: '紙厚を0.01 mm減らす',
    }))
    expect(input.value).toBe('0.075')

    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(input.value).toBe('0.065')

    fireEvent.change(input, { target: { value: '0.001' } })
    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(input.value).toBe('0.011')
  })

  it('refreshes for another project and disables every input control', () => {
    const { rerender } = render(
      <PaperThicknessInput
        id="thickness"
        initialValue="0.10"
        disabled={false}
      />,
    )
    const input = screen.getByRole('spinbutton', {
      name: '紙厚',
    }) as HTMLInputElement

    fireEvent.change(input, { target: { value: '0.123' } })
    rerender(
      <PaperThicknessInput
        id="thickness"
        initialValue="0.20"
        disabled={true}
      />,
    )
    expect(input.value).toBe('0.20')
    expect(input.disabled).toBe(true)
    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true)
    }
  })

  it('submits the exact stepped decimal through the native form field', () => {
    let submittedThickness: FormDataEntryValue | null = null
    render(
      <form
        onSubmit={(event) => {
          event.preventDefault()
          submittedThickness = new FormData(event.currentTarget)
            .get('thickness_mm')
        }}
      >
        <PaperThicknessInput
          id="submitted-thickness"
          initialValue="0.075"
          disabled={false}
        />
        <button type="submit">紙設定を送信</button>
      </form>,
    )

    fireEvent.click(screen.getByRole('button', {
      name: '紙厚を0.01 mm増やす',
    }))
    fireEvent.click(screen.getByRole('button', {
      name: '紙設定を送信',
    }))

    expect(submittedThickness).toBe('0.085')
  })
})
