import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'

import { useCreasePairMeasurement } from '../src/lib/useCreasePairMeasurement.ts'

afterEach(cleanup)

const lines = [
  { id: 'x', x1: 0, y1: 0, x2: 1, y2: 0 },
  { id: 'y', x1: 0, y1: 0, x2: 0, y2: 1 },
]
const vertices = [
  { id: 'a', x: 0, y: 0 },
  { id: 'b', x: 3, y: 4 },
]

it('preserves the exact 2D selection transitions and displayed geometry values', () => {
  const { result } = renderHook(() => useCreasePairMeasurement({
    active: true,
    lines,
    vertices,
  }))
  act(() => {
    result.current.selectMeasurementVertex('a')
    result.current.selectMeasurementVertex('b')
  })
  expect(result.current.measurementVertexIds).toEqual(['a', 'b'])
  expect(result.current.measurementLineIds).toEqual([])
  expect(result.current.pairMeasurement).toEqual({ kind: 'vertex', value: 5 })

  act(() => {
    result.current.selectMeasurementLine('x')
    result.current.selectMeasurementLine('y')
  })
  expect(result.current.measurementVertexIds).toEqual([])
  expect(result.current.measurementLineIds).toEqual(['x', 'y'])
  expect(result.current.pairMeasurement).toEqual({ kind: 'line', value: 90 })
  act(() => result.current.selectMeasurementLine(null))
  expect(result.current.measurementLineIds).toEqual([])
})

it('retains surviving IDs, recomputes moved geometry, and clears outside measure mode', async () => {
  const { result, rerender } = renderHook(
    ({ active, currentVertices }) => useCreasePairMeasurement({
      active,
      lines,
      vertices: currentVertices,
    }),
    { initialProps: { active: true, currentVertices: vertices } },
  )
  act(() => {
    result.current.selectMeasurementVertex('a')
    result.current.selectMeasurementVertex('b')
  })
  rerender({
    active: true,
    currentVertices: [{ id: 'a', x: 0, y: 0 }, { id: 'b', x: 6, y: 8 }],
  })
  expect(result.current.pairMeasurement).toEqual({ kind: 'vertex', value: 10 })
  rerender({ active: true, currentVertices: [{ id: 'a', x: 0, y: 0 }] })
  await waitFor(() => expect(result.current.measurementVertexIds).toEqual(['a']))
  expect(result.current.pairMeasurement).toBeNull()
  rerender({ active: false, currentVertices: [{ id: 'a', x: 0, y: 0 }] })
  await waitFor(() => expect(result.current.measurementVertexIds).toEqual([]))
})
