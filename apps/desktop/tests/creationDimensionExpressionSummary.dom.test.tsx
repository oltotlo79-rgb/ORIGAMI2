import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { CreationDimensionExpressionSummary } from '../src/components/CreationDimensionExpressionSummary.tsx'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('CreationDimensionExpressionSummary', () => {
  it('switches between the persisted source and its native adopted values', () => {
    render(
      <CreationDimensionExpressionSummary
        binding={{
          schema_version: 1,
          width_source: '200 * sqrt(2)',
          height_source: '400 / 3',
          adopted_width_mm: 282.842712474619,
          adopted_height_mm: 133.33333333333334,
        }}
      />,
    )

    expect(screen.getByText(/200 \* sqrt\(2\) × 400 \/ 3 mm/u)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '評価値を表示' }))
    expect(screen.getByText(/282\.842712474619 × 133\.333333333333 mm/u)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '式を表示' }))
    expect(screen.getByText(/200 \* sqrt\(2\) × 400 \/ 3 mm/u)).toBeTruthy()
  })

  it('fails closed for absent or non-finite persisted metadata', () => {
    const { container, rerender } = render(
      <CreationDimensionExpressionSummary binding={undefined} />,
    )
    expect(container.childElementCount).toBe(0)

    rerender(
      <CreationDimensionExpressionSummary
        binding={{
          schema_version: 1,
          width_source: '400',
          height_source: '400',
          adopted_width_mm: Number.NaN,
          adopted_height_mm: 400,
        }}
      />,
    )
    expect(container.childElementCount).toBe(0)
  })
})
