import { useEffect, useState } from 'react'
type Point = [number, number]

function canonicalize(source: string, bilateral: boolean): Point[] | null {
  const parsed = source.split(/\r?\n/u).filter((line) => line.trim()).map((line) => {
    const values = line.split(',').map((value) => Number(value.trim()))
    return values.length === 2 ? values as Point : null
  })
  if (!parsed.every((point): point is Point => point !== null) || parsed.length < 3 || parsed.length > 8
    || parsed.some(([x, y]) => !Number.isFinite(x) || !Number.isFinite(y)
      || Math.abs(x) > 1_000 || Math.abs(y) > 1_000)) return null
  const points = parsed.map(([x, y]) => [Math.round(x * 10), Math.round(y * 10)] as Point)
  const keys = new Set(points.map(([x, y]) => `${x},${y}`))
  if (keys.size !== points.length || (bilateral
    && points.some(([x, y]) => !keys.has(`${-x},${y}`)))) return null
  const centre = points.reduce(([x, y], point) => [x + point[0], y + point[1]], [0, 0] as Point)
  points.sort((left, right) => Math.atan2(left[1] * points.length - centre[1], left[0] * points.length - centre[0])
    - Math.atan2(right[1] * points.length - centre[1], right[0] * points.length - centre[0]))
  const start = points.reduce((best, point, index) => point[0] < points[best]![0]
    || (point[0] === points[best]![0] && point[1] < points[best]![1]) ? index : best, 0)
  return [...points.slice(start), ...points.slice(0, start)]
}

export function ProtrusionLocalOutlineEditor({ locale, bindingId, symmetry, points, onChange }: {
  locale: 'ja' | 'en'; bindingId: number; symmetry: 'none' | 'bilateral' | 'radial'
  points: readonly Point[]; onChange: (points: Point[] | undefined) => void
}) {
  const [source, setSource] = useState('')
  const [invalid, setInvalid] = useState(false)
  useEffect(() => setSource(points.map(([x, y]) => `${x / 10}, ${y / 10}`).join('\n')), [points])
  return <fieldset><legend>{locale === 'ja' ? '局所輪郭（任意）' : 'Local outline (optional)'}</legend>
    <label>{locale === 'ja' ? '局所輪郭点（X, Y mm）' : 'Local outline points (X, Y mm)'}
      <textarea aria-label={`${locale === 'ja' ? '局所輪郭点' : 'Local outline points'} binding ${bindingId}`}
        value={source} onChange={(event) => setSource(event.currentTarget.value)} /></label>
    <button type="button" onClick={() => { const result = canonicalize(source, symmetry === 'bilateral')
      setInvalid(result === null); if (result) onChange(result) }}>
      {locale === 'ja' ? '局所輪郭を反映' : 'Apply local outline'}</button>
    <button type="button" onClick={() => { setSource(''); setInvalid(false); onChange(undefined) }}>
      {locale === 'ja' ? '局所輪郭を解除' : 'Clear local outline'}</button>
    {invalid && <p role="alert">{locale === 'ja'
      ? '3〜8点の有界な輪郭を入力してください。左右対称bindingでは鏡像点が必要です。'
      : 'Enter 3 to 8 bounded points. Bilateral bindings require mirrored points.'}</p>}
  </fieldset>
}
