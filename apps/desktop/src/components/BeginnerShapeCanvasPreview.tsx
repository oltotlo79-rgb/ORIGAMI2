import { useEffect, useRef, useState } from 'react'
import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]
type Point = readonly [number, number]

export function BeginnerShapeCanvasPreview({ locale, bodySize, bodyOutline, protrusions }: {
  locale: 'ja' | 'en'
  bodySize?: readonly [number, number]
  bodyOutline: readonly Point[]
  protrusions: readonly Protrusion[]
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [selection, setSelection] = useState('body')
  const selected = selection === 'body' ? null
    : protrusions.find((target) => String(target.id) === selection) ?? null
  useEffect(() => {
    if (selection !== 'body' && !selected) setSelection('body')
  }, [selection, selected])
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
    const points = selected?.local_outline_tenths_mm?.map(([x, y]) => [
      x + selected.position_tenths_mm[0], y + selected.position_tenths_mm[1],
    ] as Point) ?? (bodyOutline.length > 0 ? bodyOutline : bodySize
      ? [[-bodySize[0] / 2, -bodySize[1] / 2], [bodySize[0] / 2, -bodySize[1] / 2],
          [bodySize[0] / 2, bodySize[1] / 2], [-bodySize[0] / 2, bodySize[1] / 2]] as Point[]
      : [])
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
    }
    context.restore()
  }, [bodySize, bodyOutline, selected])
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
    <canvas ref={canvasRef} width={240} height={180} role="img"
      aria-label={locale === 'ja'
        ? `${selection === 'body' ? '胴体' : `binding ${selection}`}の輪郭プレビュー`
        : `${selection === 'body' ? 'Body' : `Binding ${selection}`} outline preview`} />
    {selected && !selected.local_outline_tenths_mm && <p role="status">{locale === 'ja'
      ? 'このbindingには局所輪郭がありません。' : 'This binding has no local outline.'}</p>}
  </section>
}
