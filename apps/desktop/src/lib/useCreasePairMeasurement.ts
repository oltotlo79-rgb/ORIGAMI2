import { useCallback, useEffect, useMemo, useState } from 'react'

import {
  advanceMeasurementPair,
  measureUnorientedEdgeAngle,
  measureVertexPair,
  retainMeasurementPair,
  type MeasurementEdge,
  type MeasurementPoint,
} from './pairMeasurement.ts'

export function useCreasePairMeasurement(input: Readonly<{
  active: boolean
  lines: readonly MeasurementEdge[]
  vertices: readonly MeasurementPoint[]
}>) {
  const [measurementVertexIds, setMeasurementVertexIds] = useState<string[]>([])
  const [measurementLineIds, setMeasurementLineIds] = useState<string[]>([])

  useEffect(() => {
    const lineIds = new Set(input.lines.map(({ id }) => id))
    const vertexIds = new Set(input.vertices.map(({ id }) => id))
    setMeasurementLineIds((current) => {
      const next = retainMeasurementPair(current, lineIds)
      return next.length === current.length
        && next.every((id, index) => id === current[index]) ? current : next
    })
    setMeasurementVertexIds((current) => {
      const next = retainMeasurementPair(current, vertexIds)
      return next.length === current.length
        && next.every((id, index) => id === current[index]) ? current : next
    })
  }, [input.lines, input.vertices])

  useEffect(() => {
    if (input.active) return
    setMeasurementLineIds([])
    setMeasurementVertexIds([])
  }, [input.active])

  const pairMeasurement = useMemo(() => {
    if (measurementVertexIds.length === 2) {
      const first = input.vertices.find(({ id }) => id === measurementVertexIds[0])
      const second = input.vertices.find(({ id }) => id === measurementVertexIds[1])
      if (first && second) {
        return { kind: 'vertex' as const, value: measureVertexPair(first, second) }
      }
    }
    if (measurementLineIds.length === 2) {
      const first = input.lines.find(({ id }) => id === measurementLineIds[0])
      const second = input.lines.find(({ id }) => id === measurementLineIds[1])
      if (first && second) {
        return { kind: 'line' as const, value: measureUnorientedEdgeAngle(first, second) }
      }
    }
    return null
  }, [input.lines, input.vertices, measurementLineIds, measurementVertexIds])

  const selectMeasurementLine = useCallback((lineId: string | null) => {
    if (lineId) {
      setMeasurementLineIds((current) => advanceMeasurementPair(current, lineId))
      setMeasurementVertexIds([])
    } else {
      setMeasurementLineIds([])
      setMeasurementVertexIds([])
    }
  }, [])

  const selectMeasurementVertex = useCallback((vertexId: string) => {
    setMeasurementVertexIds((current) => advanceMeasurementPair(current, vertexId))
    setMeasurementLineIds([])
  }, [])

  return {
    measurementVertexIds,
    measurementLineIds,
    pairMeasurement,
    selectMeasurementLine,
    selectMeasurementVertex,
  } as const
}
