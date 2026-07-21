import { useEffect, useRef, useState } from 'react'
import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]
type Point = readonly [number, number]

function canonical(points: Point[], clockwise: boolean): Array<[number, number]> {
  const centre = points.reduce(([x, y], point) => [x + point[0], y + point[1]], [0, 0])
  const sorted = [...points].sort((left, right) => {
    const difference = Math.atan2(left[1] * points.length - centre[1], left[0] * points.length - centre[0])
      - Math.atan2(right[1] * points.length - centre[1], right[0] * points.length - centre[0])
    return clockwise ? -difference : difference
  })
  const start = sorted.reduce((best, point, index) => point[0] < sorted[best]![0]
    || (point[0] === sorted[best]![0] && point[1] < sorted[best]![1]) ? index : best, 0)
  return [...sorted.slice(start), ...sorted.slice(0, start)].map(([x, y]) => [x, y])
}

export function BeginnerShapeCanvasPreview({ locale, bodySize, bodyOutline, bodyMode, protrusions,
  onBodyOutlineChange, onProtrusionChange }: {
  locale: 'ja' | 'en'
  bodySize?: readonly [number, number]
  bodyOutline: readonly Point[]
  bodyMode: 'symmetric' | 'general'
  protrusions: readonly Protrusion[]
  onBodyOutlineChange: (points: Array<[number, number]>) => void
  onProtrusionChange: (target: Protrusion) => void
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [selection, setSelection] = useState('body')
  const [controlPoint, setControlPoint] = useState(0)
  const selected = selection === 'body' ? null
    : protrusions.find((target) => String(target.id) === selection) ?? null
  useEffect(() => {
    if (selection !== 'body' && !selected) setSelection('body')
  }, [selection, selected])
  const editable = (selected?.local_outline_tenths_mm ?? bodyOutline) as readonly Point[]
  const drawingPoints = selected ? editable.map(([x, y]) => [
    x + selected.position_tenths_mm[0], y + selected.position_tenths_mm[1],
  ] as Point) : editable
  const maximum = Math.max(1, ...drawingPoints.flatMap(([x, y]) => [Math.abs(x), Math.abs(y)]))
  const scale = Math.min(100 / maximum, 1_000)
  const commitPoint = (index: number, x: number, y: number) => {
    if (editable.length === 0 || index < 0 || index >= editable.length) return
    const limit = selected ? 10_000 : 100_000
    const next = editable.map((point) => [...point] as [number, number])
    const old = next[index]!
    next[index] = [Math.max(-limit, Math.min(limit, Math.round(x))),
      Math.max(-limit, Math.min(limit, Math.round(y)))]
    const symmetric = selected ? selected.symmetry === 'bilateral' : bodyMode === 'symmetric'
    if (symmetric) {
      const mirror = next.findIndex((point, mirrorIndex) => mirrorIndex !== index
        && point[0] === -old[0] && point[1] === old[1])
      if (mirror >= 0) next[mirror] = [-next[index]![0], next[index]![1]]
    }
    const normalized = canonical(next, !selected && bodyMode === 'symmetric')
    if (selected) onProtrusionChange({ ...selected, local_outline_tenths_mm: normalized })
    else onBodyOutlineChange(normalized)
  }
  useEffect(() => {
    const context = canvasRef.current?.getContext('2d')
    if (!context) return
    const width = 240
    const height = 180
    context.clearRect(0, 0, width, height)
    context.save()
    context.translate(width / 2, height / 2)
    context.strokeStyle = '#2563eb'
    context.lineWidth = 2
    const points = drawingPoints.length > 0 ? drawingPoints : bodySize
      ? [[-bodySize[0] / 2, -bodySize[1] / 2], [bodySize[0] / 2, -bodySize[1] / 2],
          [bodySize[0] / 2, bodySize[1] / 2], [-bodySize[0] / 2, bodySize[1] / 2]] as Point[]
      : []
    if (points.length > 0) {
      const maximum = Math.max(1, ...points.flatMap(([x, y]) => [Math.abs(x), Math.abs(y)]))
      const scale = Math.min(100 / maximum, 1_000)
      context.beginPath()
      points.forEach(([x, y], index) => {
        if (index === 0) context.moveTo(x * scale, y * scale)
        else context.lineTo(x * scale, y * scale)
      })
      context.closePath()
      context.stroke()
      points.forEach(([x, y], index) => {
        context.beginPath()
        context.arc(x * scale, y * scale, index === controlPoint ? 4 : 3, 0, Math.PI * 2)
        context.fill()
      })
    }
    context.restore()
  }, [bodySize, drawingPoints, scale, controlPoint])
  return <section aria-labelledby="beginner-shape-preview-heading">
    <h3 id="beginner-shape-preview-heading">{locale === 'ja' ? '目標形状2Dプレビュー' : '2D target-shape preview'}</h3>
    <label>{locale === 'ja' ? '表示する輪郭' : 'Outline to preview'}
      <select value={selection} onChange={(event) => setSelection(event.currentTarget.value)}>
        <option value="body">{locale === 'ja' ? '胴体' : 'Body'}</option>
        {protrusions.map((target) => <option key={target.id} value={target.id}>
          {locale === 'ja' ? `binding ${target.id}` : `Binding ${target.id}`}
        </option>)}
      </select>
    </label>
    <canvas ref={canvasRef} width={240} height={180} role="img" tabIndex={0}
      onPointerDown={(event) => {
        if (drawingPoints.length === 0) return
        const bounds = event.currentTarget.getBoundingClientRect()
        const x = event.clientX - bounds.left - 120
        const y = event.clientY - bounds.top - 90
        const nearest = drawingPoints.reduce((best, point, index) => {
          const distance = (point[0] * scale - x) ** 2 + (point[1] * scale - y) ** 2
          const bestDistance = (drawingPoints[best]![0] * scale - x) ** 2
            + (drawingPoints[best]![1] * scale - y) ** 2
          return distance < bestDistance ? index : best
        }, 0)
        setControlPoint(nearest)
        commitPoint(nearest, x / scale - (selected?.position_tenths_mm[0] ?? 0),
          y / scale - (selected?.position_tenths_mm[1] ?? 0))
      }}
      onKeyDown={(event) => {
        const delta = event.shiftKey ? 10 : 1
        const point = editable[controlPoint]
        if (!point || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
        event.preventDefault()
        commitPoint(controlPoint, point[0] + (event.key === 'ArrowLeft' ? -delta : event.key === 'ArrowRight' ? delta : 0),
          point[1] + (event.key === 'ArrowUp' ? -delta : event.key === 'ArrowDown' ? delta : 0))
      }}
      aria-describedby="beginner-shape-canvas-help"
      aria-label={locale === 'ja'
        ? `${selection === 'body' ? '胴体' : `binding ${selection}`}の輪郭プレビュー`
        : `${selection === 'body' ? 'Body' : `Binding ${selection}`} outline preview`} />
    <p id="beginner-shape-canvas-help">{locale === 'ja'
      ? 'control pointをpointerで移動できます。矢印キーは0.1 mm、Shift+矢印は1 mm移動します。'
      : 'Move a control point with the pointer. Arrow keys move 0.1 mm; Shift+Arrow moves 1 mm.'}</p>
    {selected && !selected.local_outline_tenths_mm && <p role="status">{locale === 'ja'
      ? 'このbindingには局所輪郭がありません。' : 'This binding has no local outline.'}</p>}
  </section>
}
