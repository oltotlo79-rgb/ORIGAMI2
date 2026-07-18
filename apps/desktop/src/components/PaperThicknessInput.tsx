import { useEffect, useRef, useState } from 'react'

import {
  stepPaperThicknessInput,
  type PaperThicknessStepDirection,
} from '../lib/paperThicknessInput.ts'

export type PaperThicknessInputProps = Readonly<{
  id: string
  initialValue: string
  disabled: boolean
}>

export function PaperThicknessInput({
  id,
  initialValue,
  disabled,
}: PaperThicknessInputProps) {
  const [value, setValue] = useState(initialValue)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    setValue(initialValue)
  }, [initialValue])

  function applyStep(direction: PaperThicknessStepDirection) {
    setValue((current) => stepPaperThicknessInput(current, direction))
    inputRef.current?.focus()
  }

  return (
    <span className="paper-thickness-input">
      <input
        ref={inputRef}
        id={id}
        name="thickness_mm"
        type="number"
        min="0"
        step="any"
        value={value}
        onChange={(event) => setValue(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
          event.preventDefault()
          applyStep(event.key === 'ArrowUp' ? 'up' : 'down')
        }}
        required
        disabled={disabled}
        aria-label="紙厚"
        title="上下ボタンと矢印キーは0.01 mm刻み。値は直接入力できます"
      />
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
