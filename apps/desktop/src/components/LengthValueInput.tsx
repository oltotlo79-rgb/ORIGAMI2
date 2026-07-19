import { useEffect, useMemo, useState } from 'react'

import {
  formatLengthInput,
  lengthInputSourceToken,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'

export type LengthValueInputProps = Readonly<{
  id?: string
  name: string
  initialMillimetres: number
  unit: ResolvedLengthDisplayUnit
  disabled?: boolean
  readOnly?: boolean
  required?: boolean
  minimumMillimetres?: number
  ariaLabel?: string
  className?: string
}>

export function LengthValueInput({
  id,
  name,
  initialMillimetres,
  unit,
  disabled = false,
  readOnly = false,
  required = false,
  minimumMillimetres,
  ariaLabel,
  className,
}: LengthValueInputProps) {
  const initialValue = useMemo(
    () => formatLengthInput(initialMillimetres, unit),
    [initialMillimetres, unit],
  )
  const sourceToken = useMemo(
    () => lengthInputSourceToken(initialMillimetres, unit),
    [initialMillimetres, unit],
  )
  const [value, setValue] = useState(initialValue)
  const [dirty, setDirty] = useState(false)

  useEffect(() => {
    setValue(initialValue)
    setDirty(false)
  }, [initialValue, sourceToken])

  const minimum = minimumMillimetres === undefined
    ? undefined
    : minimumMillimetres / unit.millimetresPerUnit

  return (
    <input
      id={id}
      name={name}
      type="number"
      min={minimum}
      step="any"
      value={value}
      disabled={disabled}
      readOnly={readOnly}
      required={required}
      aria-label={ariaLabel}
      className={className}
      data-length-dirty={dirty ? 'true' : 'false'}
      data-length-source-token={sourceToken}
      onChange={(event) => {
        setDirty(true)
        setValue(event.currentTarget.value)
      }}
    />
  )
}
