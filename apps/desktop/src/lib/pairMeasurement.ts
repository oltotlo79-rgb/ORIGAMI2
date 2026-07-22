export type MeasurementPoint = Readonly<{ id: string; x: number; y: number }>
export type MeasurementEdge = Readonly<{
  id: string; x1: number; y1: number; x2: number; y2: number
}>

export function measureVertexPair(first: MeasurementPoint, second: MeasurementPoint) {
  if (first.id === second.id || ![first.x, first.y, second.x, second.y].every(Number.isFinite)) return null
  const distance = Math.hypot(second.x - first.x, second.y - first.y)
  return Number.isFinite(distance) ? distance : null
}

export function measureUnorientedEdgeAngle(first: MeasurementEdge, second: MeasurementEdge) {
  if (first.id === second.id) return null
  const ax = first.x2 - first.x1; const ay = first.y2 - first.y1
  const bx = second.x2 - second.x1; const by = second.y2 - second.y1
  const firstLength = Math.hypot(ax, ay)
  const secondLength = Math.hypot(bx, by)
  if (![ax, ay, bx, by, firstLength, secondLength].every(Number.isFinite)
    || firstLength <= 0 || secondLength <= 0) return null
  const normalizedDot = (ax / firstLength) * (bx / secondLength)
    + (ay / firstLength) * (by / secondLength)
  const cosine = Math.min(1, Math.max(0, Math.abs(normalizedDot)))
  const degrees = Math.acos(cosine) * 180 / Math.PI
  return Number.isFinite(degrees) ? degrees : null
}

export function advanceMeasurementPair(current: readonly string[], id: string) {
  if (current.includes(id)) return current.filter((item) => item !== id)
  return current.length < 2 ? [...current, id] : [id]
}

export function retainMeasurementPair(
  current: readonly string[],
  validIds: ReadonlySet<string>,
) : string[] {
  return current
    .filter((id, index) => validIds.has(id) && current.indexOf(id) === index)
    .slice(0, 2)
}
