import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { CreationDimensionExpressionSummary } from '../src/components/CreationDimensionExpressionSummary.tsx'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('CreationDimensionExpressionSummary', () => {
  it('localizes labels and keeps expression variables as inert text', () => {
    render(
      <CreationDimensionExpressionSummary
        localeStore={localeFixture('en')}
        binding={{
          schema_version: 1,
          width_source: '<img src=x onerror=alert(1)>',
          height_source: '{height}',
          adopted_width_mm: 200,
          adopted_height_mm: 300,
        }}
      />,
    )

    expect(screen.getByText(/Creation size:/u).textContent).toContain(
      '<img src=x onerror=alert(1)> × {height} mm',
    )
    expect(document.querySelector('img')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Show values' }))
    expect(screen.getByText(/200 × 300 mm/u)).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Show expressions',
    })).toBeTruthy()
  })

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
