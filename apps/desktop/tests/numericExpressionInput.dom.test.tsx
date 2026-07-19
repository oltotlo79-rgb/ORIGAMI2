import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { NumericExpressionInput } from '../src/components/NumericExpressionInput.tsx'
import {
  MAX_NUMERIC_EXPRESSION_SOURCE_BYTES,
  NUMERIC_EXPRESSION_SCHEMA,
  NumericExpressionNativeError,
  type NumericExpressionEvaluation,
  type NumericExpressionNativeTransport,
} from '../src/lib/numericExpressionNative.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('NumericExpressionInput', () => {
  it('evaluates on blur, preserves the source in form data, and toggles the result view', async () => {
    const transport = fixedTransport()
    render(
      <form aria-label="paper">
        <NumericExpressionInput
          id="width"
          name="width_expression"
          defaultSource="400"
          ariaLabel="paper width expression"
          transport={transport}
        />
      </form>,
    )
    const input = screen.getByRole('textbox', {
      name: 'paper width expression',
    }) as HTMLInputElement
    fireEvent.change(input, { target: { value: '200 * 2' } })
    fireEvent.blur(input)

    await screen.findByText(/評価値: 400/u)
    const form = screen.getByRole('form', { name: 'paper' }) as HTMLFormElement
    expect(new FormData(form).get('width_expression')).toBe('200 * 2')

    fireEvent.click(screen.getByRole('button', { name: '式を表示' }))
    expect(screen.getByText('式: 200 * 2')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '評価値を表示' }))
    expect(screen.getByText(/評価値: 400/u)).toBeTruthy()
  })

  it('keeps the latest edit authoritative when an older evaluation finishes late', async () => {
    const pending: Array<{
      source: string
      resolve: (value: NumericExpressionEvaluation) => void
    }> = []
    const transport: NumericExpressionNativeTransport = {
      evaluate(source, precisionBits) {
        return new Promise((resolve) => {
          pending.push({
            source,
            resolve: (value) => resolve(value),
          })
        }).then((value) => {
          expect(precisionBits).toBe(192)
          return value
        })
      },
    }
    render(
      <NumericExpressionInput
        id="height"
        name="height_expression"
        defaultSource="400"
        ariaLabel="paper height expression"
        transport={transport}
      />,
    )
    const input = screen.getByRole('textbox', {
      name: 'paper height expression',
    }) as HTMLInputElement
    fireEvent.change(input, { target: { value: '100 * 2' } })
    fireEvent.blur(input)
    await waitFor(() => expect(pending).toHaveLength(1))

    fireEvent.change(input, { target: { value: '100 * 3' } })
    fireEvent.blur(input)
    pending[0]?.resolve(evaluation('100 * 2', 200))
    await waitFor(() => expect(pending).toHaveLength(2))
    expect(screen.queryByText(/200 mm/u)).toBeNull()

    pending[1]?.resolve(evaluation('100 * 3', 300))
    await screen.findByText(/評価値: 300/u)
    expect(input.value).toBe('100 * 3')
  })

  it('handles Enter, Escape, IME, and fixed-category failures without submitting', async () => {
    let submitted = 0
    let calls = 0
    const transport: NumericExpressionNativeTransport = {
      async evaluate(source) {
        calls += 1
        if (source === 'bad') {
          throw new NumericExpressionNativeError('invalid_expression')
        }
        return evaluation(source, 400)
      },
    }
    render(
      <form
        onSubmit={(event) => {
          event.preventDefault()
          submitted += 1
        }}
      >
        <NumericExpressionInput
          id="width"
          name="width_expression"
          defaultSource="400"
          ariaLabel="paper width expression"
          transport={transport}
        />
      </form>,
    )
    const input = screen.getByRole('textbox', {
      name: 'paper width expression',
    }) as HTMLInputElement

    fireEvent.keyDown(input, { key: 'Enter' })
    await screen.findByText(/評価値: 400/u)
    expect(submitted).toBe(0)

    fireEvent.change(input, { target: { value: 'bad' } })
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true })
    await Promise.resolve()
    expect(calls).toBe(1)
    fireEvent.keyDown(input, { key: 'Enter' })
    await screen.findByText(/式を解釈できません/u)
    expect(input.getAttribute('aria-invalid')).toBe('true')

    fireEvent.keyDown(input, { key: 'Escape' })
    expect(input.value).toBe('400')
    expect(input.getAttribute('aria-invalid')).toBe('false')
  })

  it('explains the browser-only limitation without exposing a raw transport error', async () => {
    render(
      <NumericExpressionInput
        id="browser-width"
        name="width_expression"
        defaultSource="400"
        ariaLabel="browser paper width expression"
      />,
    )
    const input = screen.getByRole('textbox', {
      name: 'browser paper width expression',
    })
    fireEvent.blur(input)
    await screen.findByText('式の評価はデスクトップ版で利用できます。')
    expect(screen.queryByText(/native_unavailable/iu)).toBeNull()
    expect(input.getAttribute('aria-invalid')).toBe('true')
  })

  it('accepts a delayed result after the StrictMode effect cleanup and re-setup cycle', async () => {
    render(
      <StrictMode>
        <NumericExpressionInput
          id="strict-width"
          name="width_expression"
          defaultSource="400"
          ariaLabel="strict paper width expression"
          transport={fixedTransport()}
        />
      </StrictMode>,
    )
    const input = screen.getByRole('textbox', {
      name: 'strict paper width expression',
    })
    fireEvent.blur(input)
    await screen.findByText(/評価値: 400/u)
    expect(input.getAttribute('aria-invalid')).toBe('false')
  })

  it('contains a hostile rejection while rendering one fixed error', async () => {
    const hostile = new Proxy({}, {
      getPrototypeOf() {
        throw new Error('C:\\private\\numeric-expression.txt')
      },
    })
    const transport: NumericExpressionNativeTransport = {
      async evaluate() {
        throw hostile
      },
    }
    render(
      <NumericExpressionInput
        id="hostile-width"
        name="width_expression"
        defaultSource="400"
        ariaLabel="hostile paper width expression"
        transport={transport}
      />,
    )
    fireEvent.blur(screen.getByRole('textbox', {
      name: 'hostile paper width expression',
    }))
    await screen.findByText('式の評価結果を採用できませんでした。')
    expect(document.body.textContent).not.toContain('numeric-expression.txt')
  })

  it('caps interactive input and rejects a hostile oversized paste before transport', async () => {
    let calls = 0
    const transport: NumericExpressionNativeTransport = {
      async evaluate(source) {
        calls += 1
        return evaluation(source, 400)
      },
    }
    render(
      <NumericExpressionInput
        id="bounded-width"
        name="width_expression"
        defaultSource="400"
        ariaLabel="bounded paper width expression"
        transport={transport}
      />,
    )
    const input = screen.getByRole('textbox', {
      name: 'bounded paper width expression',
    }) as HTMLInputElement
    expect(input.maxLength).toBe(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES)

    fireEvent.change(input, {
      target: {
        value: 'x'.repeat(MAX_NUMERIC_EXPRESSION_SOURCE_BYTES + 1),
      },
    })
    fireEvent.blur(input)

    await screen.findByText('式が空か、入力上限を超えています。')
    expect(calls).toBe(0)
    expect(input.getAttribute('aria-invalid')).toBe('true')
  })
})

function fixedTransport(): NumericExpressionNativeTransport {
  return {
    async evaluate(source, precisionBits) {
      expect(precisionBits).toBe(192)
      return evaluation(source, 400)
    },
  }
}

function evaluation(source: string, value: number): NumericExpressionEvaluation {
  return Object.freeze({
    schema: NUMERIC_EXPRESSION_SCHEMA,
    source,
    requestedPrecisionBits: 192,
    exact: true,
    operations: 1,
    lowerBound: value,
    upperBound: value,
    lowerDisplay: value.toExponential(17).replace('e+', 'e'),
    upperDisplay: value.toExponential(17).replace('e+', 'e'),
  })
}
