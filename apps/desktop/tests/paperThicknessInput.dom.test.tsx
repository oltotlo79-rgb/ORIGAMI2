import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { PaperThicknessInput } from '../src/components/PaperThicknessInput.tsx'
import {
  readLengthInputMillimetres,
  type ResolvedLengthDisplayUnit,
} from '../src/lib/lengthUnit.ts'
import {
  stepPaperThicknessFromMillimetres,
} from '../src/lib/paperThicknessInput.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

const CENTIMETRES = absoluteUnit('cm', 'cm', 10)
const INCHES = absoluteUnit('inch', 'in', 25.4)
const HORIZONTAL_RATIO = ratioUnit('horizontal-edge', 400, {
  x: 400,
  y: 0,
})
const VERTICAL_RATIO = ratioUnit('vertical-edge', 200, {
  x: 0,
  y: 200,
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
    const descriptionId = input.getAttribute('aria-describedby')
    expect(descriptionId).not.toBeNull()
    expect(document.getElementById(descriptionId ?? '')?.textContent)
      .toContain('物理量0.01 mm')

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
    const firstSourceMillimetres = 0.10000000000000002
    const { rerender } = render(
      <PaperThicknessInput
        id="thickness"
        initialValue="0.01"
        sourceMillimetres={firstSourceMillimetres}
        unit={CENTIMETRES}
        disabled={false}
      />,
    )
    const input = screen.getByRole('spinbutton', {
      name: '紙厚',
    }) as HTMLInputElement

    fireEvent.keyDown(input, { key: 'ArrowUp' })
    const expectedSourceStep = stepPaperThicknessFromMillimetres(
      firstSourceMillimetres,
      'up',
      1,
    )
    expect(expectedSourceStep).not.toBeNull()
    expect(Object.is(
      Number(input.dataset.paperThicknessSteppedMillimetres),
      expectedSourceStep?.millimetres,
    )).toBe(true)

    fireEvent.change(input, { target: { value: '0.123' } })
    rerender(
      <PaperThicknessInput
        id="thickness"
        initialValue="0.01"
        sourceMillimetres={0.2}
        unit={CENTIMETRES}
        disabled={true}
      />,
    )
    expect(input.value).toBe('0.01')
    expect(input.dataset.lengthDirty).toBe('false')
    expect(input.dataset.paperThicknessSteppedMillimetres).toBeUndefined()
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

  it('steps by physical 0.01 mm in a converted unit without rounding direct input', () => {
    let submitted: number | null = null
    render(
      <form
        aria-label="converted thickness"
        onSubmit={(event) => {
          event.preventDefault()
          submitted = readLengthInputMillimetres(
            event.currentTarget,
            'thickness_display',
            0.1,
            CENTIMETRES,
          )
        }}
      >
        <PaperThicknessInput
          id="converted-thickness"
          name="thickness_display"
          initialValue="0.01"
          sourceMillimetres={0.1}
          unit={CENTIMETRES}
          disabled={false}
        />
        <button type="submit">換算して送信</button>
      </form>,
    )
    const input = screen.getByRole('spinbutton', {
      name: '紙厚',
    }) as HTMLInputElement

    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(Number(input.value)).toBeCloseTo(0.011, 15)
    expect(Object.is(
      Number(input.dataset.paperThicknessSteppedMillimetres),
      0.11,
    )).toBe(true)
    fireEvent.change(input, { target: { value: '0.0123456789012345' } })
    expect(input.value).toBe('0.0123456789012345')
    expect(input.dataset.paperThicknessSteppedMillimetres).toBeUndefined()
    fireEvent.click(screen.getByRole('button', { name: '換算して送信' }))
    expect(submitted).toBeCloseTo(0.123456789012345, 15)
  })

  it('returns the exact mm step for cm, inch, and both paper-edge axes', () => {
    const sourceMillimetres = 0.1
    const firstMmStep = stepPaperThicknessFromMillimetres(
      sourceMillimetres,
      'up',
      1,
    )
    expect(firstMmStep).not.toBeNull()
    if (!firstMmStep) throw new Error('millimetre step was unavailable')

    for (const unit of [
      CENTIMETRES,
      INCHES,
      HORIZONTAL_RATIO,
      VERTICAL_RATIO,
    ]) {
      const rendered = render(
        <form aria-label={`paper thickness ${unit.key}`}>
          <PaperThicknessInput
            id={`thickness-${unit.key}`}
            name="thickness_display"
            initialValue={String(
              sourceMillimetres / unit.millimetresPerUnit,
            )}
            sourceMillimetres={sourceMillimetres}
            unit={unit}
            disabled={false}
          />
        </form>,
      )
      const form = screen.getByRole('form', {
        name: `paper thickness ${unit.key}`,
      }) as HTMLFormElement
      const input = screen.getByRole('spinbutton', {
        name: '紙厚',
      }) as HTMLInputElement

      fireEvent.keyDown(input, { key: 'ArrowUp' })
      const exactEvidence = Number(
        input.dataset.paperThicknessSteppedMillimetres,
      )
      expect(Object.is(exactEvidence, firstMmStep.millimetres)).toBe(true)
      expect(input.value).toBe(String(
        firstMmStep.millimetres / unit.millimetresPerUnit,
      ))
      expect(input.dataset.lengthDirty).toBe('true')
      expect(input.dataset.lengthSourceToken).toBeTruthy()
      expect(Object.is(
        readLengthInputMillimetres(
          form,
          'thickness_display',
          sourceMillimetres,
          unit,
        ),
        firstMmStep.millimetres,
      )).toBe(true)

      const validSourceToken = input.dataset.lengthSourceToken
      const forgedMillimetres = firstMmStep.millimetres + 0.01
      input.dataset.lengthSourceToken = 'stale-source-or-unit'
      input.dataset.paperThicknessSteppedMillimetres = String(
        forgedMillimetres,
      )
      expect(Object.is(
        readLengthInputMillimetres(
          form,
          'thickness_display',
          sourceMillimetres,
          unit,
        ),
        forgedMillimetres,
      )).toBe(false)
      if (validSourceToken) {
        input.dataset.lengthSourceToken = validSourceToken
      }

      input.dataset.paperThicknessSteppedMillimetres = String(
        forgedMillimetres,
      )
      expect(Object.is(
        readLengthInputMillimetres(
          form,
          'thickness_display',
          sourceMillimetres,
          unit,
        ),
        forgedMillimetres,
      )).toBe(false)
      input.dataset.paperThicknessSteppedMillimetres = String(
        firstMmStep.millimetres,
      )

      fireEvent.keyDown(input, { key: 'ArrowUp' })
      const secondMmStep = stepPaperThicknessFromMillimetres(
        firstMmStep.millimetres,
        'up',
        1,
      )
      expect(secondMmStep).not.toBeNull()
      expect(Object.is(
        Number(input.dataset.paperThicknessSteppedMillimetres),
        secondMmStep?.millimetres,
      )).toBe(true)
      rendered.unmount()
    }
  })
})

function absoluteUnit(
  storedUnit: 'cm' | 'inch',
  label: string,
  millimetresPerUnit: number,
): ResolvedLengthDisplayUnit {
  return Object.freeze({
    mode: 'absolute',
    storedUnit,
    effectiveUnit: storedUnit,
    label,
    millimetresPerUnit,
    reference: null,
    invalidReferenceEdgeId: null,
    key: storedUnit,
  })
}

function ratioUnit(
  edgeId: string,
  millimetresPerUnit: number,
  end: Readonly<{ x: number; y: number }>,
): ResolvedLengthDisplayUnit {
  return Object.freeze({
    mode: 'paper_edge_ratio',
    storedUnit: {
      paper_edge_ratio: { reference_edge: edgeId },
    },
    effectiveUnit: 'paper_edge_ratio',
    label: '紙辺比',
    millimetresPerUnit,
    reference: Object.freeze({
      edgeId,
      startVertexId: 'start',
      endVertexId: 'end',
      start: Object.freeze({ x: 0, y: 0 }),
      end: Object.freeze(end),
      lengthMm: millimetresPerUnit,
      boundaryIndex: 0,
    }),
    invalidReferenceEdgeId: null,
    key: `paper_edge_ratio:${edgeId}:${millimetresPerUnit}`,
  })
}
