import { useEffect, useRef, useState } from 'react'

import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  stepPaperThicknessFromMillimetres,
  type PaperThicknessStepDirection,
} from '../lib/paperThicknessInput.ts'
import {
  lengthInputSourceToken,
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'
import { PAPER_THICKNESS_TEXT } from '../lib/paperThicknessText.ts'

export type PaperThicknessInputProps = Readonly<{
  id: string
  initialValue: string
  disabled: boolean
  name?: string
  sourceMillimetres?: number
  unit?: ResolvedLengthDisplayUnit
  localeStore?: LocaleStore
}>

export function PaperThicknessInput({
  id,
  initialValue,
  disabled,
  name = 'thickness_mm',
  sourceMillimetres,
  unit = MILLIMETRE_LENGTH_DISPLAY_UNIT,
  localeStore: localeStore_ = localeStore,
}: PaperThicknessInputProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
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
  const unitLabel = unit.mode === 'paper_edge_ratio'
    ? text(PAPER_THICKNESS_TEXT.paperEdgeRatio)
    : unit.label

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
        aria-label={text(PAPER_THICKNESS_TEXT.ariaLabel)}
        aria-describedby={stepDescriptionId}
        title={formatLocalizedText(locale, PAPER_THICKNESS_TEXT.title, {
          unit: unitLabel,
        })}
      />
      <span id={stepDescriptionId} className="visually-hidden">
        {text(PAPER_THICKNESS_TEXT.description)}
      </span>
      <span className="paper-thickness-step-buttons">
        <button
          type="button"
          aria-label={text(PAPER_THICKNESS_TEXT.increase)}
          aria-controls={id}
          disabled={disabled}
          onClick={() => applyStep('up')}
        >
          ▲
        </button>
        <button
          type="button"
          aria-label={text(PAPER_THICKNESS_TEXT.decrease)}
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
