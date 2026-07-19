import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { LengthValueInput } from '../src/components/LengthValueInput.tsx'
import {
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  readLengthInputMillimetres,
  type ResolvedLengthDisplayUnit,
} from '../src/lib/lengthUnit.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

const CENTIMETRES: ResolvedLengthDisplayUnit = Object.freeze({
  mode: 'absolute',
  storedUnit: 'cm',
  effectiveUnit: 'cm',
  label: 'cm',
  millimetresPerUnit: 10,
  reference: null,
  invalidReferenceEdgeId: null,
  key: 'cm',
})

describe('LengthValueInput', () => {
  it('preserves the exact source number while the converted input is untouched', () => {
    const source = 0.10000000000000002
    let submitted: number | null = null
    render(
      <form
        aria-label="length form"
        onSubmit={(event) => {
          event.preventDefault()
          submitted = readLengthInputMillimetres(
            event.currentTarget,
            'length',
            source,
            CENTIMETRES,
          )
        }}
      >
        <LengthValueInput
          name="length"
          initialMillimetres={source}
          unit={CENTIMETRES}
          ariaLabel="長さ"
        />
        <button type="submit">送信</button>
      </form>,
    )

    const input = screen.getByRole('spinbutton', { name: '長さ' })
    expect(input.getAttribute('data-length-dirty')).toBe('false')
    fireEvent.click(screen.getByRole('button', { name: '送信' }))
    expect(Object.is(submitted, source)).toBe(true)
  })

  it('converts edited display values back to millimetres and resets on unit change', () => {
    const { container, rerender } = render(
      <form>
        <LengthValueInput
          name="length"
          initialMillimetres={254}
          unit={CENTIMETRES}
          ariaLabel="長さ"
        />
      </form>,
    )
    const input = screen.getByRole('spinbutton', {
      name: '長さ',
    }) as HTMLInputElement
    expect(input.value).toBe('25.4')
    fireEvent.change(input, { target: { value: '12.34567' } })
    expect(input.dataset.lengthDirty).toBe('true')

    const form = container.querySelector('form')
    expect(form).not.toBeNull()
    if (!form) throw new Error('form was not rendered')
    expect(readLengthInputMillimetres(form, 'length', 254, CENTIMETRES))
      .toBeCloseTo(123.4567, 12)

    rerender(
      <form>
        <LengthValueInput
          name="length"
          initialMillimetres={254}
          unit={MILLIMETRE_LENGTH_DISPLAY_UNIT}
          ariaLabel="長さ"
        />
      </form>,
    )
    const refreshed = screen.getByRole('spinbutton', {
      name: '長さ',
    }) as HTMLInputElement
    expect(refreshed.value).toBe('254')
    expect(refreshed.dataset.lengthDirty).toBe('false')
  })
})
