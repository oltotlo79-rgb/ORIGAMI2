import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  formatLength,
  formatLengthPoint,
  lengthDisplayUnitLabel,
  type ResolvedLengthDisplayUnit,
} from '../src/lib/lengthUnit.ts'
import { useLocale, type LocaleStore } from '../src/lib/i18n.ts'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('localized length presentation', () => {
  it('retranslates paper-edge ratio and unavailable measurements live', () => {
    const store = localeFixture('ja')
    render(<LengthPresentation localeStore={store} />)

    expect(screen.getByTestId('unit').textContent).toBe('紙辺比')
    expect(screen.getByTestId('length').textContent).toBe('1 紙辺比')
    expect(screen.getByTestId('point').textContent).toBe('計測不可')

    act(() => {
      store.setLocale('en')
    })

    expect(screen.getByTestId('unit').textContent).toBe('paper-edge ratio')
    expect(screen.getByTestId('length').textContent).toBe(
      '1 paper-edge ratio',
    )
    expect(screen.getByTestId('point').textContent).toBe('Unavailable')
    expect(document.body.textContent).not.toMatch(/[ぁ-んァ-ン一-龯]/u)
  })
})

function LengthPresentation({
  localeStore,
}: Readonly<{ localeStore: LocaleStore }>) {
  const locale = useLocale(localeStore)
  return (
    <>
      <output data-testid="unit">
        {lengthDisplayUnitLabel(PAPER_EDGE_RATIO_UNIT, locale)}
      </output>
      <output data-testid="length">
        {formatLength(400, PAPER_EDGE_RATIO_UNIT, locale)}
      </output>
      <output data-testid="point">
        {formatLengthPoint(200, null, PAPER_EDGE_RATIO_UNIT, locale)}
      </output>
    </>
  )
}

const PAPER_EDGE_RATIO_UNIT: ResolvedLengthDisplayUnit = Object.freeze({
  mode: 'paper_edge_ratio',
  storedUnit: Object.freeze({
    paper_edge_ratio: Object.freeze({ reference_edge: 'edge-top' }),
  }),
  effectiveUnit: 'paper_edge_ratio',
  label: '紙辺比',
  millimetresPerUnit: 400,
  reference: Object.freeze({
    edgeId: 'edge-top',
    startVertexId: 'v0',
    endVertexId: 'v1',
    start: Object.freeze({ x: 0, y: 0 }),
    end: Object.freeze({ x: 400, y: 0 }),
    lengthMm: 400,
    boundaryIndex: 0,
  }),
  invalidReferenceEdgeId: null,
  key: 'paper_edge_ratio:edge-top:test',
})
