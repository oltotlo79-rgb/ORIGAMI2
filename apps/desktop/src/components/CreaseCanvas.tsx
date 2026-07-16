import { useEffect, useMemo, useRef, useState, type MouseEvent, type PointerEvent } from 'react'

export type CreaseLine = {
  id: string
  startVertexId: string
  endVertexId: string
  x1: number
  y1: number
  x2: number
  y2: number
  kind: 'mountain' | 'valley' | 'boundary' | 'cut'
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

type Props = {
  lines: CreaseLine[]
  vertices?: Array<{ id: string; x: number; y: number }>
  paperBounds?: PaperBounds
  paperPolygon?: PaperPolygonPoint[]
  paperColor?: string
  tool?: string
  selectedVertexId?: string | null
  pendingVertexId?: string | null
  selectedLineId: string | null
  measurementLabel?: string
  onSelectLine: (id: string | null) => void
  onAddVertex?: (x: number, y: number) => void
  onSelectVertex?: (id: string) => void
  onMoveVertex?: (id: string, x: number, y: number) => void
  cancelInteractionToken?: number
  disabled?: boolean
}

type Vertex = { id: string; x: number; y: number }

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
}

type ViewTransform = {
  bounds: PaperBounds
  left: number
  top: number
  width: number
  height: number
  scale: number
}

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

const COLORS: Record<CreaseLine['kind'], string> = {
  mountain: '#d95252',
  valley: '#3678d4',
  boundary: '#23303f',
  cut: '#e59b35',
}

export function CreaseCanvas({
  lines,
  vertices = [],
  paperBounds,
  paperPolygon,
  paperColor = '#fffdf9',
  tool = 'select',
  selectedVertexId = null,
  pendingVertexId = null,
  selectedLineId,
  measurementLabel,
  onSelectLine,
  onAddVertex,
  onSelectVertex,
  onMoveVertex,
  cancelInteractionToken = 0,
  disabled = false,
}: Props) {
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
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const dragRef = useRef<DragState | null>(null)
  const suppressClickRef = useRef(false)
  const [dragPreview, setDragPreview] = useState<DragState | null>(null)
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
    const canvas = canvasRef.current
    if (!canvas) return
    const context = canvas.getContext('2d')
    if (!context) return
    const canvasRect = canvas.getBoundingClientRect()
    const pixelRatio = safePixelRatio(window.devicePixelRatio)
    canvas.width = Math.max(1, Math.round(canvasRect.width * pixelRatio))
    canvas.height = Math.max(1, Math.round(canvasRect.height * pixelRatio))
    context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0)
    context.clearRect(0, 0, canvasRect.width, canvasRect.height)

    const transform = createViewTransform(canvasRect, resolvedPaperBounds)
    const mapX = (x: number) => mapPaperX(transform, x)
    const mapY = (y: number) => mapPaperY(transform, y)
    const displayPaperPolygon = drawablePaperPolygon?.map((point) =>
      point.id === dragPreview?.vertexId
        ? { ...point, x: dragPreview.x, y: dragPreview.y }
        : point)

    context.fillStyle = paperColor
    if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
      context.fill('evenodd')
    } else if (useLegacyRectangularPaper) {
      context.fillRect(transform.left, transform.top, transform.width, transform.height)
    }

    const gridStep = niceGridStep(
      Math.max(
        transform.bounds.maxX - transform.bounds.minX,
        transform.bounds.maxY - transform.bounds.minY,
      ) / DESIRED_GRID_INTERVALS,
    )
    context.save()
    let shouldDrawGrid = useLegacyRectangularPaper
    if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
      context.clip('evenodd')
      shouldDrawGrid = true
    }
    if (shouldDrawGrid) {
      context.strokeStyle = '#dbe2ea'
      context.lineWidth = 1
      forEachGridValue(
        transform.bounds.minX,
        transform.bounds.maxX,
        gridStep,
        (value) => {
          const x = mapX(value)
          if (!Number.isFinite(x)) return
          context.beginPath()
          context.moveTo(x, transform.top)
          context.lineTo(x, transform.top + transform.height)
          context.stroke()
        },
      )
      forEachGridValue(
        transform.bounds.minY,
        transform.bounds.maxY,
        gridStep,
        (value) => {
          const y = mapY(value)
          if (!Number.isFinite(y)) return
          context.beginPath()
          context.moveTo(transform.left, y)
          context.lineTo(transform.left + transform.width, y)
          context.stroke()
        },
      )
    }
    context.restore()

    if (displayPaperPolygon && tracePolygonPath(context, transform, displayPaperPolygon)) {
      context.strokeStyle = COLORS.boundary
      context.lineWidth = 1.2
      context.setLineDash([])
      context.stroke()
    }

    for (const line of lines) {
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
      context.beginPath()
      context.moveTo(start.x, start.y)
      context.lineTo(end.x, end.y)
      context.strokeStyle = COLORS[line.kind]
      context.lineWidth = line.id === selectedLineId ? 4 : line.kind === 'boundary' ? 2.5 : 1.8
      context.setLineDash(line.kind === 'valley' ? [7, 5] : line.kind === 'cut' ? [12, 4, 2, 4] : [])
      context.stroke()
    }
    context.setLineDash([])

    for (const vertex of vertices) {
      const preview = vertex.id === dragPreview?.vertexId ? dragPreview : null
      const x = preview?.x ?? vertex.x
      const y = preview?.y ?? vertex.y
      const point = mapPaperPoint(transform, x, y)
      if (!point) continue
      if (vertex.id === selectedVertexId || vertex.id === pendingVertexId) {
        context.beginPath()
        context.arc(point.x, point.y, 9, 0, Math.PI * 2)
        context.fillStyle = vertex.id === pendingVertexId
          ? 'rgba(229, 155, 53, 0.28)'
          : 'rgba(23, 107, 135, 0.2)'
        context.fill()
      }
      context.beginPath()
      context.arc(point.x, point.y, 5, 0, Math.PI * 2)
      context.fillStyle = '#176b87'
      context.fill()
      context.strokeStyle = '#ffffff'
      context.lineWidth = 2
      context.stroke()
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
          measurementLabel ?? '計測不可',
          labelX,
          labelY,
          canvasRect.width,
          canvasRect.height,
        )
      }
    }
  }, [
    canvasSize.height,
    canvasSize.width,
    dragPreview,
    lines,
    measurementLabel,
    paperColor,
    drawablePaperPolygon,
    pendingVertexId,
    resolvedPaperBounds,
    selectedLineId,
    selectedVertexId,
    tool,
    useLegacyRectangularPaper,
    vertices,
  ])

  function handleClick(event: MouseEvent<HTMLCanvasElement>) {
    if (disabled) return
    if (suppressClickRef.current) {
      suppressClickRef.current = false
      return
    }
    const canvas = canvasRef.current
    if (!canvas) return
    const pointer = eventToPaperPosition(canvas, event, resolvedPaperBounds)
    const { x, y } = pointer
    const closestVertex = findClosestVertex(
      vertices,
      pointer.canvasX,
      pointer.canvasY,
      pointer.transform,
    )
    if (
      tool === 'vertex' &&
      onAddVertex &&
      isInsidePaper(
        pointer.x,
        pointer.y,
        pointer.transform,
        pointTestPaperPolygon,
        useLegacyRectangularPaper,
      )
    ) {
      onAddVertex(x, y)
      return
    }
    if ((tool === 'mountain' || tool === 'valley' || tool === 'cut') && onSelectVertex) {
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
    let best: { id: string; distance: number } | null = null
    for (const line of lines) {
      const start = mapPaperPoint(pointer.transform, line.x1, line.y1)
      const end = mapPaperPoint(pointer.transform, line.x2, line.y2)
      if (!start || !end) continue
      const distance = pointSegmentDistance(
        pointer.canvasX,
        pointer.canvasY,
        start.x,
        start.y,
        end.x,
        end.y,
      )
      if (
        distance < LINE_HIT_RADIUS_PX &&
        (!best || distance < best.distance)
      ) best = { id: line.id, distance }
    }
    onSelectLine(best?.id ?? null)
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
      vertices,
      pointer.canvasX,
      pointer.canvasY,
      pointer.transform,
    )
    if (!closestVertex) return

    const vertex = vertices.find((candidate) => candidate.id === closestVertex.id)
    if (!vertex) return

    event.preventDefault()
    onSelectVertex(vertex.id)
    onSelectLine(null)
    const drag: DragState = {
      pointerId: event.pointerId,
      vertexId: vertex.id,
      originX: vertex.x,
      originY: vertex.y,
      offsetX: vertex.x - pointer.x,
      offsetY: vertex.y - pointer.y,
      x: vertex.x,
      y: vertex.y,
    }
    dragRef.current = drag
    setDragPreview(drag)
    canvas.setPointerCapture(event.pointerId)
  }

  function handlePointerMove(event: PointerEvent<HTMLCanvasElement>) {
    if (disabled) return
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return

    event.preventDefault()
    const pointer = eventToPaperPosition(event.currentTarget, event, resolvedPaperBounds)
    const boundaryDrag = paperBoundaryVertexIds.has(drag.vertexId)
    updateDragPreview({
      ...drag,
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
    })
  }

  function handlePointerUp(event: PointerEvent<HTMLCanvasElement>) {
    if (disabled) return
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return

    event.preventDefault()
    const pointer = eventToPaperPosition(event.currentTarget, event, resolvedPaperBounds)
    const boundaryDrag = paperBoundaryVertexIds.has(drag.vertexId)
    const x = resolveDragCoordinate(
      pointer.x + drag.offsetX,
      pointer.transform.bounds.minX,
      pointer.transform.bounds.maxX,
      boundaryDrag,
      drag.x,
    )
    const y = resolveDragCoordinate(
      pointer.y + drag.offsetY,
      pointer.transform.bounds.minY,
      pointer.transform.bounds.maxY,
      boundaryDrag,
      drag.y,
    )
    const hasMoved = x !== drag.originX || y !== drag.originY
    dragRef.current = null
    setDragPreview(null)
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
    suppressClickRef.current = false
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function handleLostPointerCapture(event: PointerEvent<HTMLCanvasElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) return
    dragRef.current = null
    setDragPreview(null)
  }

  function updateDragPreview(drag: DragState) {
    dragRef.current = drag
    setDragPreview(drag)
  }

  return (
    <canvas
      ref={canvasRef}
      className={`crease-canvas tool-${tool}${dragPreview ? ' is-dragging' : ''}${disabled ? ' is-disabled' : ''}`}
      aria-label="展開図編集キャンバス"
      aria-disabled={disabled}
      onClick={handleClick}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerCancel}
      onLostPointerCapture={handleLostPointerCapture}
    >
      展開図。選択ツールでは頂点をドラッグして移動できます。
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
  polygon: PaperPolygonPoint[],
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

function pointInPolygonInclusive(
  x: number,
  y: number,
  polygon: PaperPolygonPoint[],
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
  vertices: Vertex[],
  canvasX: number,
  canvasY: number,
  transform: ViewTransform,
) {
  let closest: { id: string; distance: number } | null = null
  for (const vertex of vertices) {
    const point = mapPaperPoint(transform, vertex.x, vertex.y)
    if (!point) continue
    const distance = Math.hypot(canvasX - point.x, canvasY - point.y)
    if (
      distance < VERTEX_HIT_RADIUS_PX &&
      (!closest || distance < closest.distance)
    ) {
      closest = { id: vertex.id, distance }
    }
  }
  return closest
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

function niceGridStep(rawStep: number) {
  if (!Number.isFinite(rawStep) || rawStep <= 0) return 1
  const exponent = Math.floor(Math.log10(rawStep))
  const magnitude = 10 ** exponent
  if (!Number.isFinite(magnitude) || magnitude <= 0) return rawStep
  const fraction = rawStep / magnitude
  const niceFraction = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10
  const step = niceFraction * magnitude
  return Number.isFinite(step) && step > 0 ? step : rawStep
}

function forEachGridValue(
  minimum: number,
  maximum: number,
  requestedStep: number,
  callback: (value: number) => void,
) {
  const span = maximum - minimum
  if (!Number.isFinite(span) || span <= 0) return
  const step = Number.isFinite(requestedStep) && requestedStep > 0
    ? requestedStep
    : span
  const requestedIntervals = Math.ceil(span / step)
  const intervalCount = Number.isFinite(requestedIntervals)
    ? Math.max(1, Math.min(MAX_GRID_LINES_PER_AXIS - 1, requestedIntervals))
    : MAX_GRID_LINES_PER_AXIS - 1
  const effectiveStep = requestedIntervals > intervalCount
    ? span / intervalCount
    : step

  for (let index = 0; index <= intervalCount; index += 1) {
    const value = index === intervalCount
      ? maximum
      : minimum + Math.min(span, index * effectiveStep)
    if (Number.isFinite(value)) callback(value)
  }
}

function safePixelRatio(pixelRatio: number) {
  if (!Number.isFinite(pixelRatio) || pixelRatio <= 0) return 1
  return Math.min(pixelRatio, 4)
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
