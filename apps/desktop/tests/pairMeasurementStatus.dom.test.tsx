import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { PairMeasurementStatus } from '../src/components/PairMeasurementStatus'

afterEach(cleanup)

describe('PairMeasurementStatus', () => {
  it('announces pending, vertex distance, and unoriented edge angle states bilingually', () => {
    const view = render(<PairMeasurementStatus locale="en" kind="pending" vertexCount={1} lineCount={0} />)
    const status = screen.getByRole('status')
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.dataset.measurementKind).toBe('pending')
    expect(status.textContent).toContain('vertices 1/2')

    view.rerender(<PairMeasurementStatus locale="ja" kind="vertex" formattedValue="5 mm" vertexCount={2} lineCount={0} />)
    expect(status.dataset.measurementKind).toBe('vertex')
    expect(status.textContent).toBe('2頂点間の距離: 5 mm')

    view.rerender(<PairMeasurementStatus locale="en" kind="line" formattedValue="90°" vertexCount={0} lineCount={2} />)
    expect(status.dataset.measurementKind).toBe('line')
    expect(status.textContent).toBe('Unoriented edge angle: 90°')
  })
})
