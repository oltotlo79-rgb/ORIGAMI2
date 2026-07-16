import { useEffect, useRef } from 'react'

export type CreaseLine = {
  id: string
  x1: number
  y1: number
  x2: number
  y2: number
  kind: 'mountain' | 'valley' | 'boundary' | 'cut'
}

type Props = {
  lines: CreaseLine[]
  selectedLineId: string | null
  onSelectLine: (id: string | null) => void
}

const COLORS: Record<CreaseLine['kind'], string> = {
  mountain: '#d95252',
  valley: '#3678d4',
  boundary: '#23303f',
  cut: '#e59b35',
}

export function CreaseCanvas({ lines, selectedLineId, onSelectLine }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

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

    const mapX = (x: number) => 36 + x * ((bounds.width - 72) / 400)
    const mapY = (y: number) => 28 + y * ((bounds.height - 56) / 400)

    context.strokeStyle = '#dbe2ea'
    context.lineWidth = 1
    for (let value = 0; value <= 400; value += 20) {
      context.beginPath()
      context.moveTo(mapX(value), mapY(0))
      context.lineTo(mapX(value), mapY(400))
      context.stroke()
      context.beginPath()
      context.moveTo(mapX(0), mapY(value))
      context.lineTo(mapX(400), mapY(value))
      context.stroke()
    }

    context.fillStyle = '#fffdf9'
    context.fillRect(mapX(0), mapY(0), mapX(400) - mapX(0), mapY(400) - mapY(0))

    for (const line of lines) {
      context.beginPath()
      context.moveTo(mapX(line.x1), mapY(line.y1))
      context.lineTo(mapX(line.x2), mapY(line.y2))
      context.strokeStyle = COLORS[line.kind]
      context.lineWidth = line.id === selectedLineId ? 4 : line.kind === 'boundary' ? 2.5 : 1.8
      context.setLineDash(line.kind === 'valley' ? [7, 5] : line.kind === 'cut' ? [12, 4, 2, 4] : [])
      context.stroke()
    }
    context.setLineDash([])
  }, [lines, selectedLineId])

  function handleClick(event: React.MouseEvent<HTMLCanvasElement>) {
    const canvas = canvasRef.current
    if (!canvas) return
    const bounds = canvas.getBoundingClientRect()
    const x = (event.clientX - bounds.left - 36) / ((bounds.width - 72) / 400)
    const y = (event.clientY - bounds.top - 28) / ((bounds.height - 56) / 400)
    let best: { id: string; distance: number } | null = null
    for (const line of lines) {
      const distance = pointSegmentDistance(x, y, line)
      if (distance < 7 && (!best || distance < best.distance)) best = { id: line.id, distance }
    }
    onSelectLine(best?.id ?? null)
  }

  return <canvas ref={canvasRef} className="crease-canvas" onClick={handleClick} />
}

function pointSegmentDistance(px: number, py: number, line: CreaseLine) {
  const dx = line.x2 - line.x1
  const dy = line.y2 - line.y1
  const lengthSquared = dx * dx + dy * dy
  const t = lengthSquared === 0 ? 0 : Math.max(0, Math.min(1, ((px - line.x1) * dx + (py - line.y1) * dy) / lengthSquared))
  return Math.hypot(px - (line.x1 + t * dx), py - (line.y1 + t * dy))
}
