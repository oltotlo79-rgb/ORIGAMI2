import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it } from 'vitest'

import { useGridDivisionPreference } from '../src/lib/useGridDivisionPreference.ts'

const originalStorage = Object.getOwnPropertyDescriptor(window, 'localStorage')
let values: Map<string, string>

beforeEach(() => {
  values = new Map()
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => { values.set(key, value) },
    },
  })
})

afterEach(() => {
  cleanup()
  if (originalStorage) Object.defineProperty(window, 'localStorage', originalStorage)
  else Reflect.deleteProperty(window, 'localStorage')
})

it('loads, validates, and persists the existing bounded grid preference', async () => {
  window.localStorage.setItem('origami2.grid-division-preference.v1', JSON.stringify({
    version: 1,
    divisions: 8,
    diagonals: true,
  }))
  const { result } = renderHook(() => useGridDivisionPreference())
  expect(result.current.gridDivisionsInput).toBe('8')
  expect(result.current.gridDivisions).toBe(8)
  expect(result.current.gridDiagonals).toBe(true)
  expect(result.current.gridDivisionsValid).toBe(true)

  act(() => {
    result.current.setGridDivisionsInput('3')
    result.current.setGridDiagonals(false)
  })
  await waitFor(() => expect(JSON.parse(
    window.localStorage.getItem('origami2.grid-division-preference.v1')!,
  )).toEqual({ version: 1, divisions: 3, diagonals: false }))
})

it('keeps invalid input visible without replacing the last valid preference', async () => {
  window.localStorage.setItem('origami2.grid-division-preference.v1', JSON.stringify({
    version: 1,
    divisions: 4,
    diagonals: false,
  }))
  const { result } = renderHook(() => useGridDivisionPreference())
  act(() => result.current.setGridDivisionsInput('64'))
  expect(result.current.gridDivisionsValid).toBe(false)
  await Promise.resolve()
  expect(JSON.parse(
    window.localStorage.getItem('origami2.grid-division-preference.v1')!,
  )).toEqual({ version: 1, divisions: 4, diagonals: false })
})
