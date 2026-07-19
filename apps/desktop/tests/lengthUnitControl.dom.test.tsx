import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { LengthUnitControl } from '../src/components/LengthUnitControl.tsx'
import type { LengthDisplayUnit } from '../src/lib/coreClient.ts'
import {
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  type BoundaryLengthReference,
  type ResolvedLengthDisplayUnit,
} from '../src/lib/lengthUnit.ts'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

const REFERENCES: readonly BoundaryLengthReference[] = Object.freeze([
  Object.freeze({
    edgeId: 'edge-top',
    startVertexId: 'v0',
    endVertexId: 'v1',
    start: Object.freeze({ x: 0, y: 0 }),
    end: Object.freeze({ x: 400, y: 0 }),
    lengthMm: 400,
    boundaryIndex: 0,
  }),
  Object.freeze({
    edgeId: 'edge-right',
    startVertexId: 'v1',
    endVertexId: 'v2',
    start: Object.freeze({ x: 400, y: 0 }),
    end: Object.freeze({ x: 400, y: 200 }),
    lengthMm: 200,
    boundaryIndex: 1,
  }),
])

describe('LengthUnitControl', () => {
  it('localizes unit labels and accessible names in English', () => {
    render(
      <LengthUnitControl
        unit={MILLIMETRE_LENGTH_DISPLAY_UNIT}
        references={REFERENCES}
        disabled={false}
        onChange={() => {}}
        localeStore={localeFixture('en')}
      />,
    )

    const selector = screen.getByRole('combobox', {
      name: 'Length display unit',
    }) as HTMLSelectElement
    expect([...selector.options].map((option) => option.textContent)).toEqual([
      'Millimetres (mm)',
      'Centimetres (cm)',
      'Inches (in)',
      'Paper-edge ratio',
    ])
  })

  it('offers all four modes and starts ratio mode with a valid boundary edge', () => {
    const onChange = vi.fn<(unit: LengthDisplayUnit) => void>()
    render(
      <LengthUnitControl
        unit={MILLIMETRE_LENGTH_DISPLAY_UNIT}
        references={REFERENCES}
        disabled={false}
        onChange={onChange}
      />,
    )
    const selector = screen.getByRole('combobox', {
      name: '長さの表示単位',
    }) as HTMLSelectElement
    expect([...selector.options].map((option) => option.value)).toEqual([
      'mm',
      'cm',
      'inch',
      'paper_edge_ratio',
    ])
    fireEvent.change(selector, { target: { value: 'paper_edge_ratio' } })
    expect(onChange).toHaveBeenCalledWith({
      paper_edge_ratio: { reference_edge: 'edge-top' },
    })
  })

  it('keeps an invalid stored reference explicit and lets the user repair it', () => {
    const onChange = vi.fn<(unit: LengthDisplayUnit) => void>()
    const invalid: ResolvedLengthDisplayUnit = Object.freeze({
      mode: 'invalid_paper_edge_ratio',
      storedUnit: {
        paper_edge_ratio: { reference_edge: 'deleted-edge' },
      },
      effectiveUnit: 'mm',
      label: 'mm',
      millimetresPerUnit: 1,
      reference: null,
      invalidReferenceEdgeId: 'deleted-edge',
      key: 'invalid_paper_edge_ratio:deleted-edge:mm-repair',
    })
    render(
      <LengthUnitControl
        unit={invalid}
        references={REFERENCES}
        disabled={false}
        onChange={onChange}
      />,
    )

    expect(screen.getByRole('alert').textContent).toContain('mm で表示')
    const reference = screen.getByRole('combobox', {
      name: '紙辺比の基準輪郭辺',
    }) as HTMLSelectElement
    expect(reference.getAttribute('aria-invalid')).toBe('true')
    fireEvent.change(reference, { target: { value: 'edge-right' } })
    expect(onChange).toHaveBeenCalledWith({
      paper_edge_ratio: { reference_edge: 'edge-right' },
    })
  })

  it('disables ratio selection and announces when no valid edge exists', () => {
    const invalid: ResolvedLengthDisplayUnit = Object.freeze({
      mode: 'invalid_paper_edge_ratio',
      storedUnit: { paper_edge_ratio: { reference_edge: 'missing' } },
      effectiveUnit: 'mm',
      label: 'mm',
      millimetresPerUnit: 1,
      reference: null,
      invalidReferenceEdgeId: 'missing',
      key: 'invalid_paper_edge_ratio:missing:mm-repair',
    })
    render(
      <LengthUnitControl
        unit={invalid}
        references={[]}
        disabled={false}
        onChange={() => {}}
      />,
    )
    const selector = screen.getByRole('combobox', {
      name: '長さの表示単位',
    }) as HTMLSelectElement
    expect(selector.options[3].disabled).toBe(true)
    expect(screen.getAllByRole('alert')).toHaveLength(2)
  })
})
