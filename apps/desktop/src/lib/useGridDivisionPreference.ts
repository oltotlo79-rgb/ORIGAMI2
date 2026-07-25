import { useEffect, useState } from 'react'

import {
  loadGridDivisionPreferenceFromHost,
  saveGridDivisionPreferenceToHost,
} from './gridPreference.ts'

export function useGridDivisionPreference() {
  const [initialPreference] = useState(() => typeof window === 'undefined'
    ? null
    : loadGridDivisionPreferenceFromHost(window))
  const [gridDivisionsInput, setGridDivisionsInput] = useState(
    initialPreference?.divisions === null || !initialPreference
      ? ''
      : String(initialPreference.divisions),
  )
  const [gridDiagonals, setGridDiagonals] = useState(
    initialPreference?.diagonals ?? false,
  )
  const parsedGridDivisions = Number(gridDivisionsInput)
  const gridDivisions = gridDivisionsInput === ''
    ? null
    : parsedGridDivisions
  const gridDivisionsValid = gridDivisions === null
    || Number.isSafeInteger(gridDivisions)
      && gridDivisions >= 2
      && gridDivisions <= 63

  useEffect(() => {
    if (!gridDivisionsValid || typeof window === 'undefined') return
    saveGridDivisionPreferenceToHost(window, {
      divisions: gridDivisions,
      diagonals: gridDiagonals,
    })
  }, [gridDiagonals, gridDivisions, gridDivisionsValid])

  return {
    gridDivisionsInput,
    setGridDivisionsInput,
    gridDiagonals,
    setGridDiagonals,
    gridDivisions,
    gridDivisionsValid,
  } as const
}
