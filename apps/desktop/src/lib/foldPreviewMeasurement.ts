export type WorldPoint3 = Readonly<{ x: number; y: number; z: number }>
export type MidsurfaceIncidence = Readonly<{
  x: number
  z: number
  offsetX: number
  offsetZ: number
  matrix: readonly number[]
}>

export function resolveMidsurfaceVertexSample(
  incidences: readonly MidsurfaceIncidence[],
  worldScale: number,
): WorldPoint3 | null {
  if (!Number.isFinite(worldScale) || incidences.length === 0) return null
  const points: WorldPoint3[] = []
  for (const incidence of incidences) {
    const { matrix } = incidence
    if (matrix.length !== 16 || !matrix.every(Number.isFinite)
      || ![incidence.x, incidence.z, incidence.offsetX, incidence.offsetZ].every(Number.isFinite)) return null
    const x = incidence.x + incidence.offsetX
    const z = incidence.z + incidence.offsetZ
    const w = matrix[3]! * x + matrix[11]! * z + matrix[15]!
    if (!Number.isFinite(w) || w === 0) return null
    points.push({
      x: (matrix[0]! * x + matrix[8]! * z + matrix[12]!) / w,
      y: (matrix[1]! * x + matrix[9]! * z + matrix[13]!) / w,
      z: (matrix[2]! * x + matrix[10]! * z + matrix[14]!) / w,
    })
  }
  if (points.some((point) => ![point.x, point.y, point.z].every(Number.isFinite))) return null
  const tolerance = Math.max(1e-9, Math.max(1, Math.abs(worldScale)) * 1e-7)
  const first = points[0]!
  if (points.some((point) => Math.hypot(
    point.x - first.x, point.y - first.y, point.z - first.z,
  ) > tolerance)) return null
  const count = points.length
  const result = {
    x: first.x + points.reduce((sum, point) => sum + (point.x - first.x), 0) / count,
    y: first.y + points.reduce((sum, point) => sum + (point.y - first.y), 0) / count,
    z: first.z + points.reduce((sum, point) => sum + (point.z - first.z), 0) / count,
  }
  return [result.x, result.y, result.z].every(Number.isFinite)
    ? Object.freeze(result)
    : null
}

export function measureWorldVertexDistanceMm(
  first: WorldPoint3,
  second: WorldPoint3,
  worldUnitsPerMillimetre: number,
) {
  if (![first.x, first.y, first.z, second.x, second.y, second.z,
    worldUnitsPerMillimetre].every(Number.isFinite)
    || worldUnitsPerMillimetre <= 0) return null
  const distance = Math.hypot(
    second.x - first.x,
    second.y - first.y,
    second.z - first.z,
  ) / worldUnitsPerMillimetre
  return Number.isFinite(distance) ? distance : null
}

export function measureWorldFaceNormalAngleDegrees(
  first: WorldPoint3,
  second: WorldPoint3,
) {
  if (![first.x, first.y, first.z, second.x, second.y, second.z].every(Number.isFinite)) return null
  const firstLength = Math.hypot(first.x, first.y, first.z)
  const secondLength = Math.hypot(second.x, second.y, second.z)
  if (!Number.isFinite(firstLength) || !Number.isFinite(secondLength)
    || firstLength <= 0 || secondLength <= 0) return null
  const dot = (first.x / firstLength) * (second.x / secondLength)
    + (first.y / firstLength) * (second.y / secondLength)
    + (first.z / firstLength) * (second.z / secondLength)
  const degrees = Math.acos(Math.min(1, Math.max(-1, dot))) * 180 / Math.PI
  return Number.isFinite(degrees) ? degrees : null
}

export function advanceFoldPreviewMeasurementIds(current: readonly string[], id: string) {
  if (current.includes(id)) return current.filter((item) => item !== id)
  return current.length < 2 ? [...current, id] : [id]
}
