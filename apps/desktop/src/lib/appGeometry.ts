import type {
  CreaseLine,
  PaperBounds,
  PaperPolygonPoint,
} from '../components/CreaseCanvas'
import type { ProjectSnapshot } from './coreClient'
import { APP_TEXT } from './appText.ts'
import { selectLocalizedText, type Locale } from './i18n'
import {
  formatLength,
  resolveLengthDisplayUnit,
} from './lengthUnit'

export function resolvePaperBounds(
  snapshot: ProjectSnapshot | null,
): PaperBounds | undefined {
  if (!snapshot) return undefined
  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points = snapshot.paper.boundary_vertices.flatMap((id) => {
    const point = positions.get(id)
    return point ? [point] : []
  })
  if (points.length < 2) return undefined

  const bounds = points.reduce<PaperBounds>((current, point) => ({
    minX: Math.min(current.minX, point.x),
    minY: Math.min(current.minY, point.y),
    maxX: Math.max(current.maxX, point.x),
    maxY: Math.max(current.maxY, point.y),
  }), {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
  })
  if (
    !Object.values(bounds).every(Number.isFinite)
    || bounds.maxX <= bounds.minX
    || bounds.maxY <= bounds.minY
  ) return undefined
  return bounds
}

export function resolvePaperPolygon(
  snapshot: ProjectSnapshot | null,
): PaperPolygonPoint[] {
  if (!snapshot) return []
  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points: PaperPolygonPoint[] = []
  for (const id of snapshot.paper.boundary_vertices) {
    const position = positions.get(id)
    if (!position) return []
    points.push({ id, x: position.x, y: position.y })
  }
  return points
}

export type RectangularPaperSize = {
  width: number
  height: number
}

export function resolveRectangularPaperSize(
  snapshot: ProjectSnapshot | null,
): RectangularPaperSize | null {
  if (!snapshot) return null
  const boundaryIds = snapshot.paper.boundary_vertices
  if (boundaryIds.length !== 4 || new Set(boundaryIds).size !== 4) return null

  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points: Array<{ x: number; y: number }> = []
  for (const id of boundaryIds) {
    const point = positions.get(id)
    if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
    points.push(point)
  }

  const minX = Math.min(...points.map((point) => point.x))
  const minY = Math.min(...points.map((point) => point.y))
  const maxX = Math.max(...points.map((point) => point.x))
  const maxY = Math.max(...points.map((point) => point.y))
  const width = maxX - minX
  const height = maxY - minY
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return null
  }

  const corners = new Set<string>()
  for (const point of points) {
    const horizontalSide = point.x === minX ? 'left' : point.x === maxX ? 'right' : null
    const verticalSide = point.y === minY ? 'top' : point.y === maxY ? 'bottom' : null
    if (!horizontalSide || !verticalSide) return null
    corners.add(`${horizontalSide}:${verticalSide}`)
  }
  if (corners.size !== 4) return null

  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    const sharesX = current.x === next.x
    const sharesY = current.y === next.y
    if (sharesX === sharesY) return null
  }

  return { width, height }
}

export type LineMeasurement = {
  deltaX: number
  deltaY: number
  length: number
  angleDegrees: number
}

export function resolveUniqueParallelReference(
  lines: readonly CreaseLine[],
  referenceEdgeId: string | null,
) {
  if (!referenceEdgeId) return null
  let reference: CreaseLine | null = null
  for (const line of lines) {
    if (line.id !== referenceEdgeId) continue
    if (reference) return null
    reference = line
  }
  if (
    !reference
    || ![reference.x1, reference.y1, reference.x2, reference.y2].every(Number.isFinite)
    || (reference.x1 === reference.x2 && reference.y1 === reference.y2)
  ) return null
  return reference
}

export function measureCreaseLine(
  line: Pick<CreaseLine, 'x1' | 'y1' | 'x2' | 'y2'>,
): LineMeasurement | null {
  if (![line.x1, line.y1, line.x2, line.y2].every(Number.isFinite)) return null
  const rawDeltaX = line.x2 - line.x1
  const rawDeltaY = line.y2 - line.y1
  if (!Number.isFinite(rawDeltaX) || !Number.isFinite(rawDeltaY)) return null
  const deltaX = Object.is(rawDeltaX, -0) ? 0 : rawDeltaX
  const deltaY = Object.is(rawDeltaY, -0) ? 0 : rawDeltaY
  const length = Math.hypot(deltaX, deltaY)
  if (!Number.isFinite(length) || length <= 0) return null
  const rawAngle = Math.atan2(deltaY, deltaX) * 180 / Math.PI
  if (!Number.isFinite(rawAngle)) return null
  const angleDegrees = Object.is(rawAngle, -0) ? 0 : rawAngle
  return { deltaX, deltaY, length, angleDegrees }
}

export function formatMeasurementValue(
  value: number | null | undefined,
  unit: string,
  maximumFractionDigits = 3,
  locale: Locale = 'ja',
) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return selectLocalizedText(locale, APP_TEXT.unavailable2)
  }
  const normalized = Object.is(value, -0) ? 0 : value
  return `${normalized.toLocaleString(
    locale === 'ja' ? 'ja-JP' : 'en-US',
    { maximumFractionDigits },
  )}${unit}`
}

export function formatAngleDegrees(value: number) {
  if (!Number.isFinite(value)) return '—'
  if (value !== 0 && Math.abs(value) < 0.000001) return value.toExponential(3)
  return String(Number(value.toFixed(6)))
}

export function formatLineMeasurementLabel(
  measurement: LineMeasurement | null,
  unit: ReturnType<typeof resolveLengthDisplayUnit>,
  locale: Locale,
) {
  if (!measurement) {
    return selectLocalizedText(locale, APP_TEXT.unavailable2)
  }
  return `${formatLength(measurement.length, unit, locale)} / ${
    formatMeasurementValue(measurement.angleDegrees, '°', 2, locale)
  }`
}
