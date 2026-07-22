import { useEffect, useMemo, useRef, useState, type MouseEvent, type PointerEvent } from 'react'
import {
  DEFAULT_SNAP_SETTINGS,
  createDivisionGrid,
  createSnapSpatialIndex,
  createVisibleGrid,
  prioritizeAdditionSnapTargets,
  prioritizePointSnapTargets,
  resolveCompassIntersectionSnap,
  resolveUniqueSnapAnchor,
  resolveSnapTarget,
  vertexSnapOutranksBlockedIntersection,
  type AdditionSnapTarget,
  type AngleSnapConfig,
  type AngleSnapTarget,
  type ParallelSnapReference,
  type SnapPoint,
  type SnapSpatialIndex,
  type SnapSettings,
} from '../lib/snap'
import { createIntersectionSnapIndex } from '../lib/intersectionSnap'
import { clusterPointLiesOnSegment } from '../lib/intersectionClusterNumerics'
import { createCanvasLineDrawBatches } from '../lib/canvasBatching'
import {
  CREASE_LINE_PRESENTATIONS,
  type CreaseLineKind,
} from '../lib/creaseLinePresentation'
import {
  createVertexPlacement,
  isSupportedIntersectionTarget,
  type VertexPlacement,
} from '../lib/vertexPlacement'
import {
  creaseCanvasAngleGuideLabel,
  creaseCanvasGuideDetailLabel,
  creaseCanvasSnapKindLabel,
  creaseCanvasText,
  creaseCanvasTitle,
  localizeCreaseCanvasMeasurementLabel,
  type CreaseCanvasGuideDetail,
} from '../lib/creaseCanvasPresentation.ts'
import {
  localeStore as defaultLocaleStore,
  useLocale,
  type Locale,
  type LocaleStore,
} from '../lib/i18n.ts'
import type { BuiltinPaperPattern } from '../lib/paperPatterns.ts'

export type CreaseLine = {
  id: string
  startVertexId: string
  endVertexId: string
  x1: number
  y1: number
  x2: number
  y2: number
  kind: CreaseLineKind
  layerId?: string
  layerOrder?: number
  opacity?: number
  locked?: boolean
}

export type PaperBounds = {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

export type PaperPolygonPoint = {
  id: string
  x: number
  y: number
}

export type CreaseCanvasRenderMetrics = Readonly<{
  requestId: string | number
  lineCount: number
  vertexCount: number
  canvasWidth: number
  canvasHeight: number
  pixelRatio: number
  initialDrawMs: number
  sampleFrameCount: number
  elapsedMs: number
  framesPerSecond: number
  averageDrawMs: number
  p95DrawMs: number
  presentationDelayMs: number
  totalDurationMs: number
}>

export type ValidationVertexHighlight = 'violated' | 'indeterminate'

export type CreaseCanvasFace = Readonly<{
  id: string
  vertexIds: readonly string[]
  edgeIds: readonly string[]
  polygon: readonly Readonly<{ x: number; y: number }>[]
  color?: string
}>

export type CompassConstructionCircle = Readonly<{
  centerX: number
  centerY: number
  radius: number
}>

export type CreaseCanvasAnnotation = Readonly<{
  id: string
  text: string
  x: number
  y: number
  color: string
  opacity: number
  fontSizeMm: number
  bold: boolean
  italic: boolean
}>

export type CreaseCanvasUnderlay = Readonly<{
  id: string
  image: HTMLImageElement
  x: number
  y: number
  scaleX: number
  scaleY: number
  rotationDegrees: number
  opacity: number
}>

type Props = {
  lines: CreaseLine[]
  vertices?: Array<{ id: string; x: number; y: number }>
  faces?: readonly CreaseCanvasFace[]
  paperBounds?: PaperBounds
  paperPolygon?: PaperPolygonPoint[]
  paperColor?: string
  paperPattern?: BuiltinPaperPattern | null
  tool?: string
  selectedVertexId?: string | null
  selectedFaceId?: string | null
  highlightedFaceId?: string | null
  mirrorSelectedVertexIds?: readonly string[]
  mirrorSelectedLineIds?: readonly string[]
  pendingVertexId?: string | null
  selectedLineId: string | null
  measurementLabel?: string
  snapSettings?: SnapSettings
  gridDivisions?: number | null
  gridDiagonals?: boolean
  parallelReference?: ParallelSnapReference | null
  angleConfig?: AngleSnapConfig
  compassCircles?: readonly CompassConstructionCircle[]
  annotations?: readonly CreaseCanvasAnnotation[]
  underlays?: readonly CreaseCanvasUnderlay[]
  validationVertexHighlights?: ReadonlyMap<string, ValidationVertexHighlight>
  lockedVertexIds?: ReadonlySet<string>
  ariaDescribedBy?: string
  onSelectLine: (id: string | null) => void
  onPlaceVertex?: (placement: VertexPlacement) => void
  onPlacementBlocked?: (
    reason: 'intersection-truncated' | 'intersection-blocked'
  ) => void
  onSelectVertex?: (id: string) => void
  onSelectFace?: (id: string | null) => void
  onMoveVertex?: (id: string, x: number, y: number) => void
  cancelInteractionToken?: number
  disabled?: boolean
  renderMetricsRequestId?: string | number | null
  onRenderMetrics?: (metrics: CreaseCanvasRenderMetrics) => void
  localeStore?: LocaleStore
}

type Vertex = { id: string; x: number; y: number }

type ExactVertexBucket = {
  minimum: Vertex
  next: Vertex | null
}

type ExactVertexIndex = Map<number, Map<number, ExactVertexBucket>>

type CanvasSize = { width: number; height: number }

type DragState = {
  pointerId: number
  vertexId: string
  originX: number
  originY: number
  offsetX: number
  offsetY: number
  x: number
  y: number
  parallelReference?: ParallelSnapReference
  angleConfig?: AngleSnapConfig
}

type ViewTransform = {
  bounds: PaperBounds
  left: number
  top: number
  width: number
  height: number
  scale: number
}

type SnapGuide = {
  rawPoint: SnapPoint
  target: AdditionSnapTarget
  detail?: CreaseCanvasGuideDetail
}

type AdditionSnapResolution =
  | Readonly<{
      status: 'ready'
      target: AdditionSnapTarget | null
    }>
  | Readonly<{
      status: 'blocked'
      reason: 'intersection-truncated' | 'intersection-blocked'
    }>

const DEFAULT_PAPER_BOUNDS: PaperBounds = {
  minX: 0,
  minY: 0,
  maxX: 400,
  maxY: 400,
}
const CANVAS_PADDING_X = 36
const CANVAS_PADDING_Y = 28
const VERTEX_HIT_RADIUS_PX = 10
const LINE_HIT_RADIUS_PX = 7
const DESIRED_GRID_INTERVALS = 20
const MAX_GRID_LINES_PER_AXIS = 100
const RENDER_METRICS_SAMPLE_FRAME_COUNT = 30
const EMPTY_VALIDATION_VERTEX_HIGHLIGHTS: ReadonlyMap<string, ValidationVertexHighlight> =
  new Map()
const EMPTY_LOCKED_VERTEX_IDS: ReadonlySet<string> = new Set()

function drawPaperPattern(
  context: CanvasRenderingContext2D,
  pattern: BuiltinPaperPattern,
  left: number,
  top: number,
  width: number,
  height: number,
) {
  context.save()
  context.globalAlpha = 0.22
  context.strokeStyle = '#39434d'
  context.fillStyle = '#39434d'
  context.lineWidth = 1
  const spacing = 14
  if (pattern === 'dots') {
    for (let y = top + spacing / 2; y < top + height; y += spacing) {
      for (let x = left + spacing / 2; x < left + width; x += spacing) {
        context.beginPath()
        context.arc(x, y, 1.4, 0, Math.PI * 2)
        context.fill()
      }
    }
  } else if (pattern === 'grid') {
    context.beginPath()
    for (let x = left; x <= left + width; x += spacing) {
      context.moveTo(x, top)
      context.lineTo(x, top + height)
    }
    for (let y = top; y <= top + height; y += spacing) {
      context.moveTo(left, y)
      context.lineTo(left + width, y)
    }
    context.stroke()
  } else {
    context.beginPath()
    for (let offset = -height; offset < width; offset += spacing) {
      context.moveTo(left + offset, top + height)
      context.lineTo(left + offset + height, top)
    }
    context.stroke()
  }
  context.restore()
}

export function CreaseCanvas({
  lines,
  vertices = [],
  faces = [],
  paperBounds,
  paperPolygon,
  paperColor = '#fffdf9',
  paperPattern = null,
  tool = 'select',
  selectedVertexId = null,
  selectedFaceId = null,
  highlightedFaceId = null,
  mirrorSelectedVertexIds = [],
  mirrorSelectedLineIds = [],
  pendingVertexId = null,
  selectedLineId,
  measurementLabel,
  snapSettings = DEFAULT_SNAP_SETTINGS,
  gridDivisions = null,
  gridDiagonals = false,
  parallelReference = null,
  angleConfig,
  compassCircles = [],
  annotations = [],
  underlays = [],
  validationVertexHighlights = EMPTY_VALIDATION_VERTEX_HIGHLIGHTS,
  lockedVertexIds = EMPTY_LOCKED_VERTEX_IDS,
  ariaDescribedBy,
  onSelectLine,
  onPlaceVertex,
  onPlacementBlocked,
  onSelectVertex,
  onSelectFace,
  onMoveVertex,
  cancelInteractionToken = 0,
  disabled = false,
  renderMetricsRequestId = null,
  onRenderMetrics,
  localeStore: localeStore_ = defaultLocaleStore,
}: Props) {
  const locale = useLocale(localeStore_)
  const resolvedPaperBounds = resolvePaperBounds(paperBounds)
  const drawablePaperPolygon = useMemo(
    () => resolveDrawablePaperPolygon(paperPolygon),
    [paperPolygon],
  )
  const pointTestPaperPolygon = useMemo(
    () => resolvePointTestPaperPolygon(drawablePaperPolygon),
    [drawablePaperPolygon],
  )
  const useLegacyRectangularPaper = paperPolygon === undefined
  const paperBoundaryVertexIds = useMemo(
    () => new Set(paperPolygon?.map((point) => point.id) ?? []),
    [paperPolygon],
  )
  const exactVertexIndex = useMemo(
    () => createExactVertexIndex(vertices),
    [vertices],
  )
  const additionSnapAnchor = useMemo(
    () => resolveUniqueSnapAnchor(vertices, selectedVertexId),
    [selectedVertexId, vertices],
  )
  const snapSpatialIndex = useMemo(
    () => createSnapSpatialIndex(vertices, lines),
    [lines, vertices],
  )
  const intersectionSnapIndex = useMemo(
    () => createIntersectionSnapIndex(
      lines,
      vertices,
    ),
    [lines, vertices],
  )
  const intersectionLinesById = useMemo(() => {
    const index = new Map<string, CreaseLine | null>()
    for (const line of lines) {
      index.set(line.id, index.has(line.id) ? null : line)
    }
    return index
  }, [lines])
  const visibleGrid = useMemo(
    () => gridDivisions === null
      ? createVisibleGrid(
          resolvedPaperBounds,
          DESIRED_GRID_INTERVALS,
          MAX_GRID_LINES_PER_AXIS,
        )
      : createDivisionGrid(
          resolvedPaperBounds,
          gridDivisions,
          MAX_GRID_LINES_PER_AXIS,
          gridDiagonals,
        ),
    [gridDiagonals, gridDivisions, resolvedPaperBounds],
  )
  const lineDrawBatches = useMemo(
    () => createCanvasLineDrawBatches(lines, selectedLineId),
    [lines, selectedLineId],
  )
  const firstVertexById = useMemo(() => {
    const index = new Map<string, Vertex>()
    for (const vertex of vertices) {
      if (!index.has(vertex.id)) index.set(vertex.id, vertex)
    }
    return index
  }, [vertices])
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const dragRef = useRef<DragState | null>(null)
  const suppressClickRef = useRef(false)
  const renderMetricsCallbackRef = useRef(onRenderMetrics)
  const lastReportedRenderMetricsRequestRef = useRef<string | number | null>(null)
  renderMetricsCallbackRef.current = onRenderMetrics
  const [dragPreview, setDragPreview] = useState<DragState | null>(null)
  const [snapGuide, setSnapGuide] = useState<SnapGuide | null>(null)
  const [canvasSize, setCanvasSize] = useState<CanvasSize>({ width: 0, height: 0 })

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    function updateCanvasSize() {
      if (!canvas) return
      const rect = canvas.getBoundingClientRect()
      setCanvasSize((current) => (
        current.width === rect.width && current.height === rect.height
          ? current
          : { width: rect.width, height: rect.height }
      ))
    }

    updateCanvasSize()
    const observer = new ResizeObserver(updateCanvasSize)
    observer.observe(canvas)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    setSnapGuide(null)
    const drag = dragRef.current
    if (!drag) return

    dragRef.current = null
    setDragPreview(null)
    const canvas = canvasRef.current
    if (canvas?.hasPointerCapture(drag.pointerId)) {
      canvas.releasePointerCapture(drag.pointerId)
    }
  }, [cancelInteractionToken])

  useEffect(() => {
    setSnapGuide(null)
  }, [additionSnapAnchor, angleConfig, parallelReference, snapSettings, tool])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const context = canvas.getContext('2d')
    if (!context) return
    const canvasRect = canvas.getBoundingClientRect()
    const pixelRatio = safePixelRatio(window.devicePixelRatio)
    canvas.width = Math.max(1, Math.round(canvasRect.width * pixelRatio))
    canvas.height = Math.max(1, Math.round(canvasRect.height * pixelRatio))
    const transform = createViewTransform(canvasRect, resolvedPaperBounds)
    const mapX = (x: number) => mapPaperX(transform, x)
    const mapY = (y: number) => mapPaperY(transform, y)
    const displayPaperPolygon = drawablePaperPolygon?.map((point) =>
      point.id === dragPreview?.vertexId
        ? { ...point, x: dragPreview.x, y: dragPreview.y }
        : point)

    const drawFrame = () => {
      context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0)
      context.clearRect(0, 0, canvasRect.width, canvasRect.height)

      context.fillStyle = paperColor
      if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
        context.fill('evenodd')
      } else if (useLegacyRectangularPaper) {
        context.fillRect(transform.left, transform.top, transform.width, transform.height)
      }

      context.save()
      let shouldDrawGrid = useLegacyRectangularPaper
      if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
        context.clip('evenodd')
        shouldDrawGrid = true
      }
      if (paperPattern && shouldDrawGrid) {
        drawPaperPattern(
          context,
          paperPattern,
          transform.left,
          transform.top,
          transform.width,
          transform.height,
        )
      }
      if (shouldDrawGrid) {
        context.strokeStyle = '#dbe2ea'
        context.lineWidth = 1
        context.beginPath()
        for (const value of visibleGrid.xValues) {
          const x = mapX(value)
          if (!Number.isFinite(x)) continue
          context.moveTo(x, transform.top)
          context.lineTo(x, transform.top + transform.height)
        }
        for (const value of visibleGrid.yValues) {
          const y = mapY(value)
          if (!Number.isFinite(y)) continue
          context.moveTo(transform.left, y)
          context.lineTo(transform.left + transform.width, y)
        }
        for (const diagonal of visibleGrid.diagonals ?? []) {
          const x1 = mapX(diagonal.x1)
          const y1 = mapY(diagonal.y1)
          const x2 = mapX(diagonal.x2)
          const y2 = mapY(diagonal.y2)
          if (![x1, y1, x2, y2].every(Number.isFinite)) continue
          context.moveTo(x1, y1)
          context.lineTo(x2, y2)
        }
        context.stroke()
      }
      context.restore()

      for (const underlay of underlays) {
        const point = mapPaperPoint(transform, underlay.x, underlay.y)
        const width = underlay.image.naturalWidth
        const height = underlay.image.naturalHeight
        if (!point || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) continue
        context.save()
        context.globalAlpha = Math.max(0, Math.min(1, underlay.opacity))
        context.translate(point.x, point.y)
        context.rotate(underlay.rotationDegrees * Math.PI / 180)
        context.scale(underlay.scaleX * transform.scale, underlay.scaleY * transform.scale)
        context.drawImage(underlay.image, 0, 0)
        context.restore()
      }

      for (const face of faces) {
        if (!face.color || !tracePolygonPath(context, transform, face.polygon)) continue
        context.save()
        context.fillStyle = face.color
        context.globalAlpha = 0.24
        context.fill()
        context.restore()
      }

      const selectedFace = highlightedFaceId || selectedFaceId
        ? faces.find((face) => face.id === (highlightedFaceId ?? selectedFaceId))
        : null
      if (
        selectedFace
        && tracePolygonPath(context, transform, selectedFace.polygon)
      ) {
        context.save()
        context.fillStyle = 'rgba(22, 113, 184, 0.18)'
        context.fill()
        context.strokeStyle = '#1671b8'
        context.lineWidth = 3
        context.setLineDash([7, 4])
        context.stroke()
        context.restore()
      }

      if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
        const presentation = CREASE_LINE_PRESENTATIONS.boundary
        context.strokeStyle = presentation.color
        context.lineWidth = 1.2
        context.lineCap = presentation.lineCap
        context.setLineDash(presentation.canvasDash.slice())
        context.stroke()
      }

      for (const batch of lineDrawBatches) {
        context.beginPath()
        let pathCount = 0
        for (const line of batch.lines) {
          const previewStart = line.startVertexId === dragPreview?.vertexId ? dragPreview : null
          const previewEnd = line.endVertexId === dragPreview?.vertexId ? dragPreview : null
          const start = mapPaperPoint(
            transform,
            previewStart?.x ?? line.x1,
            previewStart?.y ?? line.y1,
          )
          const end = mapPaperPoint(
            transform,
            previewEnd?.x ?? line.x2,
            previewEnd?.y ?? line.y2,
          )
          if (!start || !end) continue
          context.moveTo(start.x, start.y)
          context.lineTo(end.x, end.y)
          pathCount += 1
        }
        if (pathCount > 0) {
          const presentation = CREASE_LINE_PRESENTATIONS[batch.kind]
          context.globalAlpha = batch.opacity
          context.strokeStyle = presentation.color
          context.lineWidth = batch.selected ? 4 : presentation.lineWidth
          context.lineCap = presentation.lineCap
          context.setLineDash(presentation.canvasDash.slice())
          context.stroke()
          context.globalAlpha = 1
        }
      }
      context.setLineDash([])
      context.lineCap = 'butt'

      const mirrorLineIds = new Set(mirrorSelectedLineIds)
      if (mirrorLineIds.size > 0) {
        context.save()
        context.beginPath()
        for (const line of lines) {
          if (!mirrorLineIds.has(line.id)) continue
          const start = mapPaperPoint(transform, line.x1, line.y1)
          const end = mapPaperPoint(transform, line.x2, line.y2)
          if (!start || !end) continue
          context.moveTo(start.x, start.y)
          context.lineTo(end.x, end.y)
        }
        context.strokeStyle = '#b45309'
        context.lineWidth = 5
        context.setLineDash([3, 4])
        context.stroke()
        context.restore()
      }

      if (parallelReference) {
        const start = mapPaperPoint(transform, parallelReference.x1, parallelReference.y1)
        const end = mapPaperPoint(transform, parallelReference.x2, parallelReference.y2)
        if (start && end) {
          context.save()
          context.beginPath()
          context.moveTo(start.x, start.y)
          context.lineTo(end.x, end.y)
          context.strokeStyle = '#8b4fb3'
          context.lineWidth = 4.5
          context.setLineDash([2, 5])
          context.stroke()
          context.restore()
        }
      }

      for (const circle of compassCircles) {
        const center = mapPaperPoint(transform, circle.centerX, circle.centerY)
        const radius = circle.radius * transform.scale
        if (!center || !Number.isFinite(radius) || radius <= 0) continue
        context.save()
        context.beginPath()
        context.arc(center.x, center.y, radius, 0, Math.PI * 2)
        context.strokeStyle = '#8b4fb3'
        context.lineWidth = 1.5
        context.setLineDash([7, 4])
        context.stroke()
        context.restore()
      }

      const mappedVertices: Array<{
        id: string
        x: number
        y: number
      }> = []
      for (const vertex of vertices) {
        const preview = vertex.id === dragPreview?.vertexId ? dragPreview : null
        const x = preview?.x ?? vertex.x
        const y = preview?.y ?? vertex.y
        const point = mapPaperPoint(transform, x, y)
        if (!point) continue
        mappedVertices.push({ id: vertex.id, ...point })
      }
      const mirrorVertexIds = new Set(mirrorSelectedVertexIds)
      if (mirrorVertexIds.size > 0) {
        context.save()
        context.strokeStyle = '#b45309'
        context.lineWidth = 3
        context.setLineDash([3, 3])
        for (const vertex of mappedVertices) {
          if (!mirrorVertexIds.has(vertex.id)) continue
          context.beginPath()
          context.arc(vertex.x, vertex.y, 8, 0, Math.PI * 2)
          context.stroke()
        }
        context.restore()
      }

      drawValidationVertexHaloBatch(
        context,
        mappedVertices,
        validationVertexHighlights,
        'indeterminate',
      )
      drawValidationVertexHaloBatch(
        context,
        mappedVertices,
        validationVertexHighlights,
        'violated',
      )
      drawVertexHaloBatch(
        context,
        mappedVertices,
        selectedVertexId,
        pendingVertexId,
        false,
      )
      drawVertexHaloBatch(
        context,
        mappedVertices,
        selectedVertexId,
        pendingVertexId,
        true,
      )
      if (mappedVertices.length > 0) {
        context.beginPath()
        for (const point of mappedVertices) traceCircle(context, point.x, point.y, 5)
        context.fillStyle = '#176b87'
        context.fill()
        context.strokeStyle = '#ffffff'
        context.lineWidth = 2
        context.stroke()
      }

      for (const annotation of annotations) {
        const point = mapPaperPoint(transform, annotation.x, annotation.y)
        if (!point || !annotation.text) continue
        context.save()
        context.globalAlpha = Math.max(0, Math.min(1, annotation.opacity))
        context.fillStyle = annotation.color
        context.textBaseline = 'alphabetic'
        context.font = `${annotation.italic ? 'italic ' : ''}${annotation.bold ? '700 ' : ''}${Math.max(8, annotation.fontSizeMm * transform.scale)}px sans-serif`
        for (const [index, line] of annotation.text.split(/\r?\n/u).entries()) {
          context.fillText(line, point.x, point.y + index * Math.max(10, annotation.fontSizeMm * transform.scale * 1.2))
        }
        context.restore()
      }

      if (tool === 'measure' && selectedLineId) {
        const selectedLine = lines.find((line) => line.id === selectedLineId)
        if (selectedLine) {
          const start = mapPaperPoint(transform, selectedLine.x1, selectedLine.y1)
          const end = mapPaperPoint(transform, selectedLine.x2, selectedLine.y2)
          const labelX = start && end
            ? (start.x + end.x) / 2
            : transform.left + transform.width / 2
          const labelY = start && end
            ? (start.y + end.y) / 2 - 18
            : transform.top + 20
          drawMeasurementLabel(
            context,
            localizeCreaseCanvasMeasurementLabel(measurementLabel, locale),
            labelX,
            labelY,
            canvasRect.width,
            canvasRect.height,
          )
        }
      }

      if (snapGuide) {
        drawSnapGuide(
          context,
          transform,
          snapGuide,
          canvasRect.width,
          canvasRect.height,
          locale,
        )
      }
    }

    const shouldMeasure = renderMetricsRequestId !== null &&
      !Object.is(lastReportedRenderMetricsRequestRef.current, renderMetricsRequestId)
    const initialDrawStartedAt = shouldMeasure ? performance.now() : 0
    drawFrame()
    if (!shouldMeasure) return

    const initialDrawFinishedAt = performance.now()
    const initialDrawMs = Math.max(0, initialDrawFinishedAt - initialDrawStartedAt)
    const drawDurations: number[] = []
    let cancelled = false
    let animationFrameId = 0
    let firstSampleFrameAt = 0
    let lastDrawFinishedAt = initialDrawFinishedAt

    const finishMeasurement = (presentedAt: number) => {
      if (cancelled || renderMetricsRequestId === null) return
      const elapsedMs = Math.max(0, presentedAt - firstSampleFrameAt)
      const totalDurationMs = Math.max(0, presentedAt - initialDrawStartedAt)
      lastReportedRenderMetricsRequestRef.current = renderMetricsRequestId
      renderMetricsCallbackRef.current?.({
        requestId: renderMetricsRequestId,
        lineCount: lines.length,
        vertexCount: vertices.length,
        canvasWidth: canvas.width,
        canvasHeight: canvas.height,
        pixelRatio,
        initialDrawMs,
        sampleFrameCount: drawDurations.length,
        elapsedMs,
        framesPerSecond: elapsedMs > 0 ? drawDurations.length * 1_000 / elapsedMs : 0,
        averageDrawMs: average(drawDurations),
        p95DrawMs: percentile95(drawDurations),
        presentationDelayMs: Math.max(0, presentedAt - lastDrawFinishedAt),
        totalDurationMs,
      })
    }

    const sampleFrame = (frameStartedAt: number) => {
      if (cancelled) return
      if (drawDurations.length === 0) firstSampleFrameAt = frameStartedAt
      const drawStartedAt = performance.now()
      drawFrame()
      lastDrawFinishedAt = performance.now()
      drawDurations.push(Math.max(0, lastDrawFinishedAt - drawStartedAt))
      animationFrameId = window.requestAnimationFrame(
        drawDurations.length < RENDER_METRICS_SAMPLE_FRAME_COUNT
          ? sampleFrame
          : finishMeasurement,
      )
    }

    animationFrameId = window.requestAnimationFrame(sampleFrame)
    return () => {
      cancelled = true
      window.cancelAnimationFrame(animationFrameId)
    }
  }, [
    canvasSize.height,
    canvasSize.width,
    annotations,
    compassCircles,
    dragPreview,
    faces,
    lineDrawBatches,
    lines,
    locale,
    measurementLabel,
    mirrorSelectedLineIds,
    mirrorSelectedVertexIds,
    paperColor,
    paperPattern,
    parallelReference,
    drawablePaperPolygon,
    pendingVertexId,
    resolvedPaperBounds,
    renderMetricsRequestId,
    selectedLineId,
    selectedFaceId,
    highlightedFaceId,
    selectedVertexId,
    snapGuide,
    tool,
    underlays,
    useLegacyRectangularPaper,
    validationVertexHighlights,
    vertices,
    visibleGrid,
  ])

  function resolveAdditionSnap(
    point: SnapPoint,
    transform: ViewTransform,
  ): AdditionSnapResolution {
    const accept = (target: { point: SnapPoint }) => isInsidePaper(
      target.point.x,
      target.point.y,
      transform,
      pointTestPaperPolygon,
      useLegacyRectangularPaper,
    )
    const ordinaryTarget = resolveSnapTarget({
      point,
      scale: transform.scale,
      settings: snapSettings,
      vertices,
      segments: lines,
      grid: visibleGrid,
      anchor: additionSnapAnchor,
      parallelReference: parallelReference ?? undefined,
      angleConfig,
      accept,
      spatialIndex: snapSpatialIndex,
    })
    const compassTarget = snapSettings.intersection
      ? resolveCompassIntersectionSnap({
          point,
          scale: transform.scale,
          circles: compassCircles,
          segments: lines,
          accept,
        })
      : null
    const pointTarget = prioritizePointSnapTargets(ordinaryTarget, compassTarget)
    if (!snapSettings.intersection) return { status: 'ready', target: pointTarget }

    const intersectionResult = intersectionSnapIndex.query({
      point,
      scale: transform.scale,
      accept,
    })
    if (intersectionResult.truncated) {
      return { status: 'blocked', reason: 'intersection-truncated' }
    }
    if (intersectionResult.blocked) {
      if (vertexSnapOutranksBlockedIntersection(
        pointTarget,
        intersectionResult.blockedDistancePx,
      )) return { status: 'ready', target: pointTarget }
      return { status: 'blocked', reason: 'intersection-blocked' }
    }
    const intersectionTarget = intersectionResult.target
    if (intersectionTarget) {
      const sourceLines: CreaseLine[] = []
      for (const source of intersectionTarget.sourceEdges) {
        const line = intersectionLinesById.get(source.id)
        if (!line) return { status: 'blocked', reason: 'intersection-blocked' }
        sourceLines.push(line)
      }
      if (!isSupportedIntersectionTarget(intersectionTarget, sourceLines)) {
        return { status: 'blocked', reason: 'intersection-blocked' }
      }
    }
    return {
      status: 'ready',
      target: prioritizeAdditionSnapTargets(pointTarget, intersectionTarget),
    }
  }

  function resolveDraggedPosition(
    drag: DragState,
    pointer: { x: number; y: number; transform: ViewTransform },
  ) {
    const boundaryDrag = paperBoundaryVertexIds.has(drag.vertexId)
    const rawPoint = {
      x: resolveDragCoordinate(
        pointer.x + drag.offsetX,
        pointer.transform.bounds.minX,
        pointer.transform.bounds.maxX,
        boundaryDrag,
        drag.x,
      ),
      y: resolveDragCoordinate(
        pointer.y + drag.offsetY,
        pointer.transform.bounds.minY,
        pointer.transform.bounds.maxY,
        boundaryDrag,
        drag.y,
      ),
    }
    const target = resolveSnapTarget({
      point: rawPoint,
      scale: pointer.transform.scale,
      settings: snapSettings,
      vertices,
      segments: lines,
      grid: visibleGrid,
      anchor: {
        id: drag.vertexId,
        x: drag.originX,
        y: drag.originY,
      },
      parallelReference: drag.parallelReference,
      angleConfig: drag.angleConfig,
      excludedVertexId: drag.vertexId,
      spatialIndex: snapSpatialIndex,
      accept: (candidate) => {
        if (
          candidate.kind === 'edge' ||
          candidate.kind === 'midpoint' ||
          lines.some((line) => clusterPointLiesOnSegment(candidate.point, line)) ||
          !isFiniteSnapPoint(candidate.point) ||
          lookupExactVertex(exactVertexIndex, candidate.point, drag.vertexId)
        ) return false
        return boundaryDrag || isInsidePaper(
          candidate.point.x,
          candidate.point.y,
          pointer.transform,
          pointTestPaperPolygon,
          useLegacyRectangularPaper,
        )
      },
    })
    const overlapsOtherVertex = !target &&
      lookupExactVertex(exactVertexIndex, rawPoint, drag.vertexId) !== null
    return {
      point: target?.point ?? (overlapsOtherVertex ? { x: drag.x, y: drag.y } : rawPoint),
      rawPoint,
      target,
    }
  }

  function handleClick(event: MouseEvent<HTMLCanvasElement>) {
    if (disabled) return
    if (suppressClickRef.current) {
      suppressClickRef.current = false
      return
    }
    setSnapGuide(null)
    const canvas = canvasRef.current
    if (!canvas) return
    const pointer = eventToPaperPosition(canvas, event, resolvedPaperBounds)
    const { x, y } = pointer
    const closestVertex = findClosestVertex(
      snapSpatialIndex.vertices,
      x,
      y,
      pointer.transform,
    )
    if (tool === 'vertex' && onPlaceVertex) {
      const resolvedSnap = resolveAdditionSnap({ x, y }, pointer.transform)
      if (resolvedSnap.status === 'blocked') {
        onPlacementBlocked?.(resolvedSnap.reason)
        return
      }
      const { target } = resolvedSnap
      const point = target?.point ?? { x, y }
      const existingVertex = lookupExactVertex(exactVertexIndex, point)
      if (existingVertex) {
        if (
          target?.kind === 'intersection'
          && (
            target.classification === 't-junction'
            || target.classification === 'cluster'
          )
          && target.junctionVertexId === existingVertex.id
          && lookupExactVertex(exactVertexIndex, point, existingVertex.id) === null
          && isInsidePaper(
            point.x,
            point.y,
            pointer.transform,
            pointTestPaperPolygon,
            useLegacyRectangularPaper,
          )
        ) {
          const placement = createVertexPlacement(point, target, lines, vertices)
          if (
            placement?.operation === 'connect-t-junction'
            || placement?.operation === 'connect-intersection-cluster'
          ) {
            setSnapGuide(null)
            onPlaceVertex(placement)
            return
          }
        }
        setSnapGuide(null)
        onSelectVertex?.(existingVertex.id)
        onSelectLine(null)
        return
      }
      if (isInsidePaper(
        point.x,
        point.y,
        pointer.transform,
        pointTestPaperPolygon,
        useLegacyRectangularPaper,
      )) {
        setSnapGuide(null)
        const placement = createVertexPlacement(point, target, lines, vertices)
        if (placement) onPlaceVertex(placement)
        return
      }
    }
    if (
      (tool === 'mountain' || tool === 'valley' || tool === 'auxiliary' || tool === 'cut')
      && onSelectVertex
    ) {
      if (closestVertex) {
        onSelectVertex(closestVertex.id)
        return
      }
    }
    if (tool === 'select' && closestVertex && onSelectVertex) {
      onSelectVertex(closestVertex.id)
      onSelectLine(null)
      return
    }
    const best = snapSpatialIndex.segments.nearest({
      point: { x, y },
      radius: LINE_HIT_RADIUS_PX / pointer.transform.scale,
      boundary: 'exclusive',
    })
    if (best) {
      onSelectLine(best.value.id)
      onSelectFace?.(null)
      return
    }
    onSelectLine(null)
    const face = faces.find((candidate) =>
      pointInPolygonInclusive(x, y, candidate.polygon))
    onSelectFace?.(face?.id ?? null)
  }

  function handlePointerDown(event: PointerEvent<HTMLCanvasElement>) {
    if (
      disabled ||
      dragRef.current ||
      tool !== 'select' ||
      event.button !== 0 ||
      !onSelectVertex ||
      !onMoveVertex
    ) return

    const canvas = event.currentTarget
    const pointer = eventToPaperPosition(canvas, event, resolvedPaperBounds)
    const closestVertex = findClosestVertex(
      snapSpatialIndex.vertices,
      pointer.x,
      pointer.y,
      pointer.transform,
    )
    if (!closestVertex) return

    const vertex = firstVertexById.get(closestVertex.id)
    if (!vertex) return
    if (lockedVertexIds.has(vertex.id)) return

    event.preventDefault()
    onSelectVertex(vertex.id)
    onSelectLine(null)
    setSnapGuide(null)
    const drag: DragState = {
      pointerId: event.pointerId,
      vertexId: vertex.id,
      originX: vertex.x,
      originY: vertex.y,
      offsetX: vertex.x - pointer.x,
      offsetY: vertex.y - pointer.y,
      x: vertex.x,
      y: vertex.y,
      parallelReference: parallelReference ?? undefined,
      angleConfig,
    }
    dragRef.current = drag
    setDragPreview(drag)
    canvas.setPointerCapture(event.pointerId)
  }

  function handlePointerMove(event: PointerEvent<HTMLCanvasElement>) {
    if (disabled) return
    const drag = dragRef.current
    if (!drag) {
      if (tool !== 'vertex') {
        setSnapGuide(null)
        return
      }
      const pointer = eventToPaperPosition(event.currentTarget, event, resolvedPaperBounds)
      const resolvedSnap = resolveAdditionSnap(
        { x: pointer.x, y: pointer.y },
        pointer.transform,
      )
      if (resolvedSnap.status === 'blocked') {
        setSnapGuide(null)
        return
      }
      const { target } = resolvedSnap
      setSnapGuide(target
        ? {
            rawPoint: { x: pointer.x, y: pointer.y },
            target,
            detail: target.kind === 'intersection'
              && target.classification === 'cluster'
              ? 'intersection-cluster'
              : target.kind === 'intersection'
                && target.classification === 't-junction'
                && target.sourceEdges.some(
                  ({ id }) => intersectionLinesById.get(id)?.kind === 'boundary',
                )
                ? 'boundary-t-junction'
                : undefined,
          }
        : null)
      return
    }
    if (drag.pointerId !== event.pointerId) return

    event.preventDefault()
    const pointer = eventToPaperPosition(event.currentTarget, event, resolvedPaperBounds)
    const resolved = resolveDraggedPosition(drag, pointer)
    setSnapGuide(resolved.target
      ? { rawPoint: resolved.rawPoint, target: resolved.target }
      : null)
    updateDragPreview({
      ...drag,
      x: resolved.point.x,
      y: resolved.point.y,
    })
  }

  function handlePointerUp(event: PointerEvent<HTMLCanvasElement>) {
    if (disabled) return
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return

    event.preventDefault()
    const pointer = eventToPaperPosition(event.currentTarget, event, resolvedPaperBounds)
    const resolved = resolveDraggedPosition(drag, pointer)
    const { x, y } = resolved.point
    const hasMoved = x !== drag.originX || y !== drag.originY
    dragRef.current = null
    setDragPreview(null)
    setSnapGuide(null)
    suppressClickRef.current = hasMoved
    if (hasMoved) {
      window.setTimeout(() => {
        suppressClickRef.current = false
      }, 0)
    }
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    if (hasMoved) onMoveVertex?.(drag.vertexId, x, y)
  }

  function handlePointerCancel(event: PointerEvent<HTMLCanvasElement>) {
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return

    dragRef.current = null
    setDragPreview(null)
    setSnapGuide(null)
    suppressClickRef.current = false
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function handleLostPointerCapture(event: PointerEvent<HTMLCanvasElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) return
    dragRef.current = null
    setDragPreview(null)
    setSnapGuide(null)
  }

  function handlePointerLeave() {
    if (!dragRef.current) setSnapGuide(null)
  }

  function updateDragPreview(drag: DragState) {
    dragRef.current = drag
    setDragPreview(drag)
  }

  return (
    <canvas
      ref={canvasRef}
      className={`crease-canvas tool-${tool}${dragPreview ? ' is-dragging' : ''}${disabled ? ' is-disabled' : ''}`}
      aria-label={creaseCanvasText(locale, 'ariaLabel')}
      aria-describedby={ariaDescribedBy}
      aria-disabled={disabled}
      title={creaseCanvasTitle(locale, disabled)}
      onClick={handleClick}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerCancel}
      onLostPointerCapture={handleLostPointerCapture}
      onPointerLeave={handlePointerLeave}
    >
      {creaseCanvasText(locale, 'fallback')}
    </canvas>
  )
}

function eventToPaperPosition(
  canvas: HTMLCanvasElement,
  event: Pick<
    MouseEvent<HTMLCanvasElement> | PointerEvent<HTMLCanvasElement>,
    'clientX' | 'clientY'
  >,
  paperBounds: PaperBounds,
) {
  const canvasRect = canvas.getBoundingClientRect()
  const transform = createViewTransform(canvasRect, paperBounds)
  const canvasX = event.clientX - canvasRect.left
  const canvasY = event.clientY - canvasRect.top
  return {
    x: unmapCanvasCoordinate(
      canvasX,
      transform.left,
      transform.bounds.minX,
      transform.scale,
    ),
    y: unmapCanvasCoordinate(
      canvasY,
      transform.top,
      transform.bounds.minY,
      transform.scale,
    ),
    canvasX,
    canvasY,
    transform,
  }
}

function resolvePaperBounds(bounds?: PaperBounds): PaperBounds {
  if (!bounds) return DEFAULT_PAPER_BOUNDS
  if (
    !Number.isFinite(bounds.minX) ||
    !Number.isFinite(bounds.minY) ||
    !Number.isFinite(bounds.maxX) ||
    !Number.isFinite(bounds.maxY) ||
    bounds.minX >= bounds.maxX ||
    bounds.minY >= bounds.maxY
  ) return DEFAULT_PAPER_BOUNDS

  const width = bounds.maxX - bounds.minX
  const height = bounds.maxY - bounds.minY
  if (
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) return DEFAULT_PAPER_BOUNDS

  return bounds
}

function resolveDrawablePaperPolygon(
  polygon?: PaperPolygonPoint[],
): PaperPolygonPoint[] | null {
  if (!polygon || polygon.length < 3) return null
  if (new Set(polygon.map((point) => point.id)).size !== polygon.length) return null
  if (!polygon.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y))) {
    return null
  }
  return polygon
}

function resolvePointTestPaperPolygon(
  polygon: PaperPolygonPoint[] | null,
): PaperPolygonPoint[] | null {
  if (!polygon) return null
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (let index = 0; index < polygon.length; index += 1) {
    const point = polygon[index]
    minX = Math.min(minX, point.x)
    minY = Math.min(minY, point.y)
    maxX = Math.max(maxX, point.x)
    maxY = Math.max(maxY, point.y)
    for (let previous = 0; previous < index; previous += 1) {
      if (point.x === polygon[previous].x && point.y === polygon[previous].y) return null
    }
  }

  const width = maxX - minX
  const height = maxY - minY
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return null
  }

  let twiceArea = 0
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const areaTerm = current.x * next.y - next.x * current.y
    if (!Number.isFinite(areaTerm)) return null
    twiceArea += areaTerm
    if (!Number.isFinite(twiceArea)) return null
  }
  if (twiceArea === 0 || polygonSelfIntersects(polygon)) return null
  return polygon
}

function polygonSelfIntersects(polygon: PaperPolygonPoint[]) {
  for (let first = 0; first < polygon.length; first += 1) {
    const firstNext = (first + 1) % polygon.length
    for (let second = first + 1; second < polygon.length; second += 1) {
      const secondNext = (second + 1) % polygon.length
      if (first === second || firstNext === second || secondNext === first) continue
      if (segmentsIntersectOrAreInvalid(
        polygon[first],
        polygon[firstNext],
        polygon[second],
        polygon[secondNext],
      )) return true
    }
  }
  return false
}

function segmentsIntersectOrAreInvalid(
  firstStart: PaperPolygonPoint,
  firstEnd: PaperPolygonPoint,
  secondStart: PaperPolygonPoint,
  secondEnd: PaperPolygonPoint,
) {
  const firstSideStart = crossProduct(firstStart, firstEnd, secondStart)
  const firstSideEnd = crossProduct(firstStart, firstEnd, secondEnd)
  const secondSideStart = crossProduct(secondStart, secondEnd, firstStart)
  const secondSideEnd = crossProduct(secondStart, secondEnd, firstEnd)
  if (
    firstSideStart === null ||
    firstSideEnd === null ||
    secondSideStart === null ||
    secondSideEnd === null
  ) return true
  if (firstSideStart === 0 && pointIsOnSegment(secondStart, firstStart, firstEnd)) return true
  if (firstSideEnd === 0 && pointIsOnSegment(secondEnd, firstStart, firstEnd)) return true
  if (secondSideStart === 0 && pointIsOnSegment(firstStart, secondStart, secondEnd)) return true
  if (secondSideEnd === 0 && pointIsOnSegment(firstEnd, secondStart, secondEnd)) return true
  return (firstSideStart > 0) !== (firstSideEnd > 0) &&
    (secondSideStart > 0) !== (secondSideEnd > 0)
}

function crossProduct(
  start: PaperPolygonPoint,
  end: PaperPolygonPoint,
  point: PaperPolygonPoint,
) {
  const value = (end.x - start.x) * (point.y - start.y) -
    (end.y - start.y) * (point.x - start.x)
  return Number.isFinite(value) ? value : null
}

function pointIsOnSegment(
  point: PaperPolygonPoint,
  start: PaperPolygonPoint,
  end: PaperPolygonPoint,
) {
  return point.x >= Math.min(start.x, end.x) &&
    point.x <= Math.max(start.x, end.x) &&
    point.y >= Math.min(start.y, end.y) &&
    point.y <= Math.max(start.y, end.y)
}

function tracePolygonPath(
  context: CanvasRenderingContext2D,
  transform: ViewTransform,
  polygon: readonly Readonly<{ x: number; y: number }>[],
) {
  const first = mapPaperPoint(transform, polygon[0].x, polygon[0].y)
  if (!first) return false
  context.beginPath()
  context.moveTo(first.x, first.y)
  for (let index = 1; index < polygon.length; index += 1) {
    const point = mapPaperPoint(transform, polygon[index].x, polygon[index].y)
    if (!point) {
      context.beginPath()
      return false
    }
    context.lineTo(point.x, point.y)
  }
  context.closePath()
  return true
}

function createViewTransform(
  canvasRect: Pick<DOMRect, 'width' | 'height'>,
  requestedBounds: PaperBounds,
): ViewTransform {
  const bounds = resolvePaperBounds(requestedBounds)
  return createTransform(canvasRect, bounds) ??
    createTransform(canvasRect, DEFAULT_PAPER_BOUNDS) ?? {
      bounds: DEFAULT_PAPER_BOUNDS,
      left: 0,
      top: 0,
      width: 1,
      height: 1,
      scale: 1 / 400,
    }
}

function createTransform(
  canvasRect: Pick<DOMRect, 'width' | 'height'>,
  bounds: PaperBounds,
): ViewTransform | null {
  const canvasWidth = Number.isFinite(canvasRect.width) && canvasRect.width > 0
    ? canvasRect.width
    : 1
  const canvasHeight = Number.isFinite(canvasRect.height) && canvasRect.height > 0
    ? canvasRect.height
    : 1
  const paddingX = Math.min(CANVAS_PADDING_X, Math.max(0, (canvasWidth - 1) / 2))
  const paddingY = Math.min(CANVAS_PADDING_Y, Math.max(0, (canvasHeight - 1) / 2))
  const availableWidth = Math.max(1, canvasWidth - paddingX * 2)
  const availableHeight = Math.max(1, canvasHeight - paddingY * 2)
  const paperWidth = bounds.maxX - bounds.minX
  const paperHeight = bounds.maxY - bounds.minY
  const scale = Math.min(availableWidth / paperWidth, availableHeight / paperHeight)
  const width = paperWidth * scale
  const height = paperHeight * scale
  if (
    !Number.isFinite(scale) ||
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    scale <= 0 ||
    width <= 0 ||
    height <= 0
  ) return null

  const left = paddingX + (availableWidth - width) / 2
  const top = paddingY + (availableHeight - height) / 2
  if (!Number.isFinite(left) || !Number.isFinite(top)) return null

  return { bounds, left, top, width, height, scale }
}

function mapPaperX(transform: ViewTransform, x: number) {
  if (!Number.isFinite(x)) return Number.NaN
  const mapped = transform.left + (x - transform.bounds.minX) * transform.scale
  return Number.isFinite(mapped) ? mapped : Number.NaN
}

function mapPaperY(transform: ViewTransform, y: number) {
  if (!Number.isFinite(y)) return Number.NaN
  const mapped = transform.top + (y - transform.bounds.minY) * transform.scale
  return Number.isFinite(mapped) ? mapped : Number.NaN
}

function mapPaperPoint(transform: ViewTransform, x: number, y: number) {
  const mappedX = mapPaperX(transform, x)
  const mappedY = mapPaperY(transform, y)
  if (!Number.isFinite(mappedX) || !Number.isFinite(mappedY)) return null
  return { x: mappedX, y: mappedY }
}

function unmapCanvasCoordinate(
  value: number,
  origin: number,
  minimum: number,
  scale: number,
) {
  if (!Number.isFinite(value)) return minimum
  return minimum + (value - origin) / scale
}

function clampToRange(value: number, minimum: number, maximum: number) {
  if (Number.isNaN(value)) return minimum
  return Math.max(minimum, Math.min(maximum, value))
}

function resolveDragCoordinate(
  value: number,
  minimum: number,
  maximum: number,
  allowOutside: boolean,
  fallback: number,
) {
  if (!Number.isFinite(value)) return fallback
  return allowOutside ? value : clampToRange(value, minimum, maximum)
}

function isInsidePaper(
  x: number,
  y: number,
  transform: ViewTransform,
  polygon: PaperPolygonPoint[] | null,
  useLegacyRectangle: boolean,
) {
  if (polygon) return pointInPolygonInclusive(x, y, polygon)
  if (!useLegacyRectangle) return false
  return Number.isFinite(x) &&
    Number.isFinite(y) &&
    x >= transform.bounds.minX &&
    x <= transform.bounds.maxX &&
    y >= transform.bounds.minY &&
    y <= transform.bounds.maxY
}

function isFiniteSnapPoint(point: SnapPoint) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}

function createExactVertexIndex(
  vertices: readonly Vertex[],
) {
  const index: ExactVertexIndex = new Map()
  for (const vertex of vertices) {
    if (!Number.isFinite(vertex.x) || !Number.isFinite(vertex.y)) continue

    let byY = index.get(vertex.x)
    if (!byY) {
      byY = new Map()
      index.set(vertex.x, byY)
    }
    const bucket = byY.get(vertex.y)
    if (!bucket) {
      byY.set(vertex.y, { minimum: vertex, next: null })
      continue
    }
    if (vertex.id === bucket.minimum.id || vertex.id === bucket.next?.id) continue
    if (vertex.id < bucket.minimum.id) {
      bucket.next = bucket.minimum
      bucket.minimum = vertex
    } else if (!bucket.next || vertex.id < bucket.next.id) {
      bucket.next = vertex
    }
  }
  return index
}

function lookupExactVertex(
  index: ExactVertexIndex,
  point: SnapPoint,
  excludedVertexId?: string,
) {
  if (!isFiniteSnapPoint(point)) return null
  const bucket = index.get(point.x)?.get(point.y)
  if (!bucket) return null
  return bucket.minimum.id === excludedVertexId ? bucket.next : bucket.minimum
}

function pointInPolygonInclusive(
  x: number,
  y: number,
  polygon: readonly Readonly<{ x: number; y: number }>[],
) {
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const point of polygon) {
    minX = Math.min(minX, point.x)
    minY = Math.min(minY, point.y)
    maxX = Math.max(maxX, point.x)
    maxY = Math.max(maxY, point.y)
  }
  const scale = Math.max(1, maxX - minX, maxY - minY)
  const boundaryTolerance = Number.isFinite(scale) ? scale * 1e-9 : 1e-9

  let inside = false
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    if (pointSegmentDistance(x, y, current.x, current.y, next.x, next.y) <= boundaryTolerance) {
      return true
    }
    if ((current.y > y) === (next.y > y)) continue
    const intersectionX = current.x +
      (next.x - current.x) * (y - current.y) / (next.y - current.y)
    if (!Number.isFinite(intersectionX)) return false
    if (x < intersectionX) inside = !inside
  }
  return inside
}

function findClosestVertex(
  index: SnapSpatialIndex['vertices'],
  x: number,
  y: number,
  transform: ViewTransform,
) {
  const closest = index.nearest({
    point: { x, y },
    radius: VERTEX_HIT_RADIUS_PX / transform.scale,
    boundary: 'exclusive',
  })
  return closest
    ? { id: closest.value.id, distance: closest.distance * transform.scale }
    : null
}

function pointSegmentDistance(
  px: number,
  py: number,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
) {
  const dx = x2 - x1
  const dy = y2 - y1
  const lengthSquared = dx * dx + dy * dy
  if (!Number.isFinite(lengthSquared)) return Number.POSITIVE_INFINITY
  const t = lengthSquared === 0
    ? 0
    : Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / lengthSquared))
  return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy))
}

function safePixelRatio(pixelRatio: number) {
  if (!Number.isFinite(pixelRatio) || pixelRatio <= 0) return 1
  return Math.min(pixelRatio, 4)
}

function drawVertexHaloBatch(
  context: CanvasRenderingContext2D,
  vertices: readonly Vertex[],
  selectedVertexId: string | null,
  pendingVertexId: string | null,
  pending: boolean,
) {
  const targetId = pending ? pendingVertexId : selectedVertexId
  if (targetId === null || (!pending && targetId === pendingVertexId)) return
  context.beginPath()
  let pathCount = 0
  for (const vertex of vertices) {
    if (vertex.id !== targetId) continue
    traceCircle(context, vertex.x, vertex.y, 9)
    pathCount += 1
  }
  if (pathCount === 0) return
  context.fillStyle = pending
    ? 'rgba(229, 155, 53, 0.28)'
    : 'rgba(23, 107, 135, 0.2)'
  context.fill()
}

function drawValidationVertexHaloBatch(
  context: CanvasRenderingContext2D,
  vertices: readonly Vertex[],
  highlights: ReadonlyMap<string, ValidationVertexHighlight>,
  severity: ValidationVertexHighlight,
) {
  context.beginPath()
  let pathCount = 0
  for (const vertex of vertices) {
    if (highlights.get(vertex.id) !== severity) continue
    traceCircle(context, vertex.x, vertex.y, severity === 'violated' ? 13 : 12)
    pathCount += 1
  }
  if (pathCount === 0) return

  context.save()
  context.strokeStyle = severity === 'violated' ? '#b4232f' : '#9a6700'
  context.lineWidth = 3
  context.setLineDash(severity === 'violated' ? [] : [4, 3])
  context.stroke()
  context.restore()
}

function traceCircle(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  radius: number,
) {
  context.moveTo(x + radius, y)
  context.arc(x, y, radius, 0, Math.PI * 2)
}

function average(values: readonly number[]) {
  if (values.length === 0) return 0
  let sum = 0
  for (const value of values) sum += value
  return Number.isFinite(sum) ? sum / values.length : 0
}

function percentile95(values: readonly number[]) {
  if (values.length === 0) return 0
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)] ?? 0
}

function drawSnapGuide(
  context: CanvasRenderingContext2D,
  transform: ViewTransform,
  guide: SnapGuide,
  canvasWidth: number,
  canvasHeight: number,
  locale: Locale,
) {
  if (!Number.isFinite(canvasWidth) || !Number.isFinite(canvasHeight)) return
  const target = mapPaperPoint(transform, guide.target.point.x, guide.target.point.y)
  if (
    !target ||
    target.x < -20 ||
    target.y < -20 ||
    target.x > canvasWidth + 20 ||
    target.y > canvasHeight + 20
  ) return
  const raw = mapPaperPoint(transform, guide.rawPoint.x, guide.rawPoint.y)
  const directionAnchor = (
    guide.target.kind === 'horizontal'
    || guide.target.kind === 'vertical'
    || guide.target.kind === 'parallel'
    || guide.target.kind === 'angle'
  )
    ? mapPaperPoint(
        transform,
        guide.target.anchorPoint.x,
        guide.target.anchorPoint.y,
      )
    : null

  context.save()
  context.strokeStyle = '#b14c83'
  context.fillStyle = '#b14c83'
  context.lineWidth = 1.5
  if (directionAnchor && guide.target.kind === 'angle') {
    drawAngleReferenceGuide(context, directionAnchor, target, guide.target)
  }
  if (directionAnchor) {
    context.setLineDash([7, 4])
    context.beginPath()
    context.moveTo(directionAnchor.x, directionAnchor.y)
    context.lineTo(target.x, target.y)
    context.stroke()
    context.setLineDash([])
    context.beginPath()
    context.arc(directionAnchor.x, directionAnchor.y, 3.5, 0, Math.PI * 2)
    context.fill()
  }
  if (raw && Math.hypot(raw.x - target.x, raw.y - target.y) > 1) {
    context.setLineDash([3, 3])
    context.beginPath()
    context.moveTo(raw.x, raw.y)
    context.lineTo(target.x, target.y)
    context.stroke()
  }
  context.setLineDash([])
  context.beginPath()
  context.arc(target.x, target.y, 7, 0, Math.PI * 2)
  context.fillStyle = 'rgba(255, 255, 255, 0.9)'
  context.fill()
  context.stroke()
  context.beginPath()
  context.moveTo(target.x - 10, target.y)
  context.lineTo(target.x + 10, target.y)
  context.moveTo(target.x, target.y - 10)
  context.lineTo(target.x, target.y + 10)
  context.stroke()

  const label = guide.detail
    ? creaseCanvasGuideDetailLabel(locale, guide.detail)
    : guide.target.kind === 'angle'
      ? creaseCanvasAngleGuideLabel(
          locale,
          guide.target.angleSide,
          guide.target.angleDegrees,
        )
      : creaseCanvasSnapKindLabel(locale, guide.target.kind)
  context.font = '600 10px system-ui, sans-serif'
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  const labelWidth = Math.max(32, context.measureText(label).width + 10)
  const labelHeight = 18
  const labelX = clampToRange(
    target.x + 14 + labelWidth / 2,
    labelWidth / 2 + 3,
    Math.max(labelWidth / 2 + 3, canvasWidth - labelWidth / 2 - 3),
  )
  const labelY = clampToRange(
    target.y - 13,
    labelHeight / 2 + 3,
    Math.max(labelHeight / 2 + 3, canvasHeight - labelHeight / 2 - 3),
  )
  context.fillStyle = 'rgba(38, 49, 59, 0.92)'
  context.fillRect(
    labelX - labelWidth / 2,
    labelY - labelHeight / 2,
    labelWidth,
    labelHeight,
  )
  context.fillStyle = '#ffffff'
  context.fillText(label, labelX, labelY)
  context.restore()
}

function drawAngleReferenceGuide(
  context: CanvasRenderingContext2D,
  anchor: { x: number; y: number },
  snappedPoint: { x: number; y: number },
  angleTarget: AngleSnapTarget,
) {
  const baseDirection = angleTarget.referenceKind === 'global-horizontal'
    ? { x: 1, y: 0 }
    : normalizedGuideDirection(
        angleTarget.referenceStartPoint.x,
        angleTarget.referenceStartPoint.y,
        angleTarget.referenceEndPoint.x,
        angleTarget.referenceEndPoint.y,
      )
  if (!baseDirection) return
  const screenX = baseDirection.x
  const screenY = baseDirection.y
  const screenLength = Math.hypot(screenX, screenY)
  if (!Number.isFinite(screenLength) || screenLength <= 0) return
  const unitX = screenX / screenLength
  const unitY = screenY / screenLength
  const halfLength = 22
  context.save()
  context.strokeStyle = 'rgba(139, 79, 179, 0.8)'
  context.lineWidth = 1.25
  context.setLineDash([3, 3])
  context.beginPath()
  context.moveTo(anchor.x - unitX * halfLength, anchor.y - unitY * halfLength)
  context.lineTo(anchor.x + unitX * halfLength, anchor.y + unitY * halfLength)
  context.stroke()

  let startAngle = Math.atan2(unitY, unitX)
  const delta = angleTarget.angleDegrees * Math.PI / 180
    * (angleTarget.angleSide === 'counterclockwise' ? 1 : -1)
  const targetRayAngle = startAngle + delta
  const targetOffsetX = snappedPoint.x - anchor.x
  const targetOffsetY = snappedPoint.y - anchor.y
  const targetRayDot = targetOffsetX * Math.cos(targetRayAngle)
    + targetOffsetY * Math.sin(targetRayAngle)
  if (Number.isFinite(targetRayDot) && targetRayDot < 0) startAngle += Math.PI
  if (Number.isFinite(startAngle) && Number.isFinite(delta)) {
    context.setLineDash([])
    context.beginPath()
    context.arc(anchor.x, anchor.y, 15, startAngle, startAngle + delta, delta < 0)
    context.stroke()
  }
  context.restore()
}

function normalizedGuideDirection(x1: number, y1: number, x2: number, y2: number) {
  let x = x2 - x1
  let y = y2 - y1
  if (!Number.isFinite(x) || !Number.isFinite(y)) {
    const coordinateScale = Math.max(Math.abs(x1), Math.abs(y1), Math.abs(x2), Math.abs(y2))
    if (!Number.isFinite(coordinateScale) || coordinateScale <= 0) return null
    x = x2 / coordinateScale - x1 / coordinateScale
    y = y2 / coordinateScale - y1 / coordinateScale
  }
  const maximumComponent = Math.max(Math.abs(x), Math.abs(y))
  if (!Number.isFinite(maximumComponent) || maximumComponent <= 0) return null
  const normalizedX = x / maximumComponent
  const normalizedY = y / maximumComponent
  return Number.isFinite(normalizedX) && Number.isFinite(normalizedY)
    ? { x: normalizedX, y: normalizedY }
    : null
}

function drawMeasurementLabel(
  context: CanvasRenderingContext2D,
  text: string,
  requestedX: number,
  requestedY: number,
  canvasWidth: number,
  canvasHeight: number,
) {
  if (![requestedX, requestedY, canvasWidth, canvasHeight].every(Number.isFinite)) return
  context.save()
  context.font = '600 12px system-ui, sans-serif'
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  const horizontalPadding = 8
  const labelHeight = 24
  const labelWidth = Math.max(52, context.measureText(text).width + horizontalPadding * 2)
  const halfWidth = labelWidth / 2
  const halfHeight = labelHeight / 2
  const x = clampToRange(requestedX, halfWidth + 4, Math.max(halfWidth + 4, canvasWidth - halfWidth - 4))
  const y = clampToRange(requestedY, halfHeight + 4, Math.max(halfHeight + 4, canvasHeight - halfHeight - 4))
  context.fillStyle = 'rgba(255, 255, 255, 0.94)'
  context.strokeStyle = 'rgba(64, 78, 90, 0.45)'
  context.lineWidth = 1
  context.fillRect(x - halfWidth, y - halfHeight, labelWidth, labelHeight)
  context.strokeRect(x - halfWidth, y - halfHeight, labelWidth, labelHeight)
  context.fillStyle = '#26313b'
  context.fillText(text, x, y)
  context.restore()
}
