import { useEffect, useRef, useState, type MouseEvent, type PointerEvent } from 'react'

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

type Props = {
  lines: CreaseLine[]
  vertices?: Array<{ id: string; x: number; y: number }>
  tool?: string
  selectedVertexId?: string | null
  pendingVertexId?: string | null
  selectedLineId: string | null
  onSelectLine: (id: string | null) => void
  onAddVertex?: (x: number, y: number) => void
  onSelectVertex?: (id: string) => void
  onMoveVertex?: (id: string, x: number, y: number) => void
  cancelInteractionToken?: number
  disabled?: boolean
}

type Vertex = { id: string; x: number; y: number }

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

const PAPER_SIZE = 400
const CANVAS_PADDING_X = 36
const CANVAS_PADDING_Y = 28

const COLORS: Record<CreaseLine['kind'], string> = {
  mountain: '#d95252',
  valley: '#3678d4',
  boundary: '#23303f',
  cut: '#e59b35',
}

export function CreaseCanvas({
  lines,
  vertices = [],
  tool = 'select',
  selectedVertexId = null,
  pendingVertexId = null,
  selectedLineId,
  onSelectLine,
  onAddVertex,
  onSelectVertex,
  onMoveVertex,
  cancelInteractionToken = 0,
  disabled = false,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const dragRef = useRef<DragState | null>(null)
  const suppressClickRef = useRef(false)
  const [dragPreview, setDragPreview] = useState<DragState | null>(null)

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
    const bounds = canvas.getBoundingClientRect()
    const scale = window.devicePixelRatio || 1
    canvas.width = Math.round(bounds.width * scale)
    canvas.height = Math.round(bounds.height * scale)
    context.scale(scale, scale)
    context.clearRect(0, 0, bounds.width, bounds.height)

    const mapX = (x: number) =>
      CANVAS_PADDING_X + x * ((bounds.width - CANVAS_PADDING_X * 2) / PAPER_SIZE)
    const mapY = (y: number) =>
      CANVAS_PADDING_Y + y * ((bounds.height - CANVAS_PADDING_Y * 2) / PAPER_SIZE)

    context.strokeStyle = '#dbe2ea'
    context.lineWidth = 1
    for (let value = 0; value <= PAPER_SIZE; value += 20) {
      context.beginPath()
      context.moveTo(mapX(value), mapY(0))
      context.lineTo(mapX(value), mapY(PAPER_SIZE))
      context.stroke()
      context.beginPath()
      context.moveTo(mapX(0), mapY(value))
      context.lineTo(mapX(PAPER_SIZE), mapY(value))
      context.stroke()
    }

    context.fillStyle = '#fffdf9'
    context.fillRect(
      mapX(0),
      mapY(0),
      mapX(PAPER_SIZE) - mapX(0),
      mapY(PAPER_SIZE) - mapY(0),
    )

    for (const line of lines) {
      const previewStart = line.startVertexId === dragPreview?.vertexId ? dragPreview : null
      const previewEnd = line.endVertexId === dragPreview?.vertexId ? dragPreview : null
      context.beginPath()
      context.moveTo(mapX(previewStart?.x ?? line.x1), mapY(previewStart?.y ?? line.y1))
      context.lineTo(mapX(previewEnd?.x ?? line.x2), mapY(previewEnd?.y ?? line.y2))
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
      if (vertex.id === selectedVertexId || vertex.id === pendingVertexId) {
        context.beginPath()
        context.arc(mapX(x), mapY(y), 9, 0, Math.PI * 2)
        context.fillStyle = vertex.id === pendingVertexId
          ? 'rgba(229, 155, 53, 0.28)'
          : 'rgba(23, 107, 135, 0.2)'
        context.fill()
      }
      context.beginPath()
      context.arc(mapX(x), mapY(y), 5, 0, Math.PI * 2)
      context.fillStyle = '#176b87'
      context.fill()
      context.strokeStyle = '#ffffff'
      context.lineWidth = 2
      context.stroke()
    }
  }, [dragPreview, lines, pendingVertexId, selectedLineId, selectedVertexId, vertices])

  function handleClick(event: MouseEvent<HTMLCanvasElement>) {
    if (disabled) return
    if (suppressClickRef.current) {
      suppressClickRef.current = false
      return
    }
    const canvas = canvasRef.current
    if (!canvas) return
    const { x, y } = eventToPaperPosition(canvas, event)
    const closestVertex = findClosestVertex(vertices, x, y)
    if (
      tool === 'vertex' &&
      onAddVertex &&
      x >= 0 &&
      x <= PAPER_SIZE &&
      y >= 0 &&
      y <= PAPER_SIZE
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
      const distance = pointSegmentDistance(x, y, line)
      if (distance < 7 && (!best || distance < best.distance)) best = { id: line.id, distance }
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
    const pointer = eventToPaperPosition(canvas, event)
    const closestVertex = findClosestVertex(vertices, pointer.x, pointer.y)
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
    const pointer = eventToPaperPosition(event.currentTarget, event)
    updateDragPreview({
      ...drag,
      x: clampToPaper(pointer.x + drag.offsetX),
      y: clampToPaper(pointer.y + drag.offsetY),
    })
  }

  function handlePointerUp(event: PointerEvent<HTMLCanvasElement>) {
    if (disabled) return
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return

    event.preventDefault()
    const pointer = eventToPaperPosition(event.currentTarget, event)
    const x = clampToPaper(pointer.x + drag.offsetX)
    const y = clampToPaper(pointer.y + drag.offsetY)
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
) {
  const bounds = canvas.getBoundingClientRect()
  return {
    x: (event.clientX - bounds.left - CANVAS_PADDING_X) /
      ((bounds.width - CANVAS_PADDING_X * 2) / PAPER_SIZE),
    y: (event.clientY - bounds.top - CANVAS_PADDING_Y) /
      ((bounds.height - CANVAS_PADDING_Y * 2) / PAPER_SIZE),
  }
}

function clampToPaper(value: number) {
  return Math.max(0, Math.min(PAPER_SIZE, value))
}

function findClosestVertex(
  vertices: Vertex[],
  x: number,
  y: number,
) {
  let closest: { id: string; distance: number } | null = null
  for (const vertex of vertices) {
    const distance = Math.hypot(x - vertex.x, y - vertex.y)
    if (distance < 10 && (!closest || distance < closest.distance)) {
      closest = { id: vertex.id, distance }
    }
  }
  return closest
}

function pointSegmentDistance(px: number, py: number, line: CreaseLine) {
  const dx = line.x2 - line.x1
  const dy = line.y2 - line.y1
  const lengthSquared = dx * dx + dy * dy
  const t = lengthSquared === 0 ? 0 : Math.max(0, Math.min(1, ((px - line.x1) * dx + (py - line.y1) * dy) / lengthSquared))
  return Math.hypot(px - (line.x1 + t * dx), py - (line.y1 + t * dy))
}
