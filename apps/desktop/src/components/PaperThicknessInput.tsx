import { useEffect, useRef, useState } from 'react'

import {
  stepPaperThicknessFromMillimetres,
  type PaperThicknessStepDirection,
} from '../lib/paperThicknessInput.ts'
import {
  lengthInputSourceToken,
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'

export type PaperThicknessInputProps = Readonly<{
  id: string
  initialValue: string
  disabled: boolean
  name?: string
  sourceMillimetres?: number
  unit?: ResolvedLengthDisplayUnit
}>

export function PaperThicknessInput({
  id,
  initialValue,
  disabled,
  name = 'thickness_mm',
  sourceMillimetres,
  unit = MILLIMETRE_LENGTH_DISPLAY_UNIT,
}: PaperThicknessInputProps) {
  const [state, setState] = useState(() => ({
    dirty: false,
    steppedMillimetres: null as number | null,
    value: initialValue,
  }))
  const inputRef = useRef<HTMLInputElement>(null)
  const stepDescriptionId = `${id}-physical-step-description`
  const sourceToken = sourceMillimetres === undefined
    ? undefined
    : lengthInputSourceToken(sourceMillimetres, unit)

  useEffect(() => {
    setState({
      dirty: false,
      steppedMillimetres: null,
      value: initialValue,
    })
  }, [initialValue, sourceToken])

  function applyStep(direction: PaperThicknessStepDirection) {
    setState((current) => {
      const exactSourceMillimetres =
        typeof sourceMillimetres === 'number'
        && Number.isFinite(sourceMillimetres)
          ? sourceMillimetres
          : null
      const displayed = Number(current.value)
      const baseMillimetres = current.steppedMillimetres
        ?? (!current.dirty && exactSourceMillimetres !== null
          ? exactSourceMillimetres
          : displayed * unit.millimetresPerUnit)
      const stepped = stepPaperThicknessFromMillimetres(
        baseMillimetres,
        direction,
        unit.millimetresPerUnit,
      )
      if (!stepped) return current
      return {
        dirty: true,
        steppedMillimetres: stepped.millimetres,
        value: stepped.displayValue,
      }
    })
    inputRef.current?.focus()
  }

  return (
    <span className="paper-thickness-input">
      <input
        ref={inputRef}
        id={id}
        name={name}
        type="number"
        min="0"
        step="any"
        value={state.value}
        data-length-dirty={state.dirty ? 'true' : 'false'}
        data-length-source-token={sourceToken}
        data-paper-thickness-stepped-millimetres={
          state.steppedMillimetres === null
            ? undefined
            : String(state.steppedMillimetres)
        }
        onChange={(event) => {
          setState({
            dirty: true,
            steppedMillimetres: null,
            value: event.currentTarget.value,
          })
        }}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
          event.preventDefault()
          applyStep(event.key === 'ArrowUp' ? 'up' : 'down')
        }}
        required
        disabled={disabled}
        aria-label="紙厚"
        aria-describedby={stepDescriptionId}
        title={`上下ボタンと矢印キーは物理量0.01 mm刻み。値は${unit.label}で直接入力できます`}
      />
      <span id={stepDescriptionId} className="visually-hidden">
        上下ボタンと矢印キーは表示単位に関係なく、
        紙厚を物理量0.01 mmずつ増減します。値は直接入力できます。
      </span>
      <span className="paper-thickness-step-buttons">
        <button
          type="button"
          aria-label="紙厚を0.01 mm増やす"
          aria-controls={id}
          disabled={disabled}
          onClick={() => applyStep('up')}
        >
          ▲
        </button>
        <button
          type="button"
          aria-label="紙厚を0.01 mm減らす"
          aria-controls={id}
          disabled={disabled}
          onClick={() => applyStep('down')}
        >
          ▼
        </button>
      </span>
    </span>
  )
}
