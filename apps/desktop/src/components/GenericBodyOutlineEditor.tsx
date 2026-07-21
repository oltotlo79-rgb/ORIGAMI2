import { useEffect, useState } from 'react'

type Point = [number, number]

function canonicalize(points: Point[]): Point[] | null {
  if (points.length < 4 || points.length > 16
    || points.some(([x, y]) => !Number.isFinite(x) || !Number.isFinite(y)
      || Math.abs(x) > 10_000 || Math.abs(y) > 10_000)) return null
  const tenths = points.map(([x, y]) => [Math.round(x * 10), Math.round(y * 10)] as Point)
  const keys = new Set(tenths.map(([x, y]) => `${x},${y}`))
  if (keys.size !== tenths.length
    || tenths.some(([x, y]) => !keys.has(`${-x},${y}`))) return null
  const centre = tenths.reduce(([x, y], point) => [x + point[0], y + point[1]], [0, 0] as Point)
  tenths.sort((left, right) => Math.atan2(right[1] * tenths.length - centre[1], right[0] * tenths.length - centre[0])
    - Math.atan2(left[1] * tenths.length - centre[1], left[0] * tenths.length - centre[0]))
  const start = tenths.reduce((best, point, index) =>
    point[0] < tenths[best]![0] || (point[0] === tenths[best]![0] && point[1] < tenths[best]![1])
      ? index : best, 0)
  return [...tenths.slice(start), ...tenths.slice(0, start)]
}

export function GenericBodyOutlineEditor({ locale, points, onChange }: {
  locale: 'ja' | 'en'
  points: readonly Point[]
  onChange: (points: Point[]) => void
}) {
  const [source, setSource] = useState('')
  const [invalid, setInvalid] = useState(false)
  useEffect(() => setSource(points.map(([x, y]) => `${x / 10}, ${y / 10}`).join('\n')), [points])
  const apply = () => {
    const parsed = source.split(/\r?\n/u).filter((line) => line.trim() !== '').map((line) => {
      const values = line.split(',').map((value) => Number(value.trim()))
      return values.length === 2 ? values as Point : null
    })
    const canonical = parsed.every((point): point is Point => point !== null)
      ? canonicalize(parsed) : null
    setInvalid(canonical === null)
    if (canonical) onChange(canonical)
  }
  return <fieldset>
    <legend>{locale === 'ja' ? '左右対称の胴体輪郭' : 'Symmetric body outline'}</legend>
    <label>{locale === 'ja' ? '輪郭点（1行に X, Y mm）' : 'Outline points (X, Y mm per line)'}
      <textarea aria-label={locale === 'ja' ? '胴体輪郭点' : 'Body outline points'}
        value={source} onChange={(event) => setSource(event.currentTarget.value)} />
    </label>
    <button type="button" onClick={apply}>{locale === 'ja' ? '輪郭を反映' : 'Apply outline'}</button>
    <button type="button" onClick={() => { setSource(''); setInvalid(false); onChange([]) }}>
      {locale === 'ja' ? '輪郭指定を解除' : 'Clear outline'}
    </button>
    {invalid && <p role="alert">{locale === 'ja'
      ? '4〜16点の左右対称な有限座標を入力してください。'
      : 'Enter 4 to 16 finite, left-right symmetric points.'}</p>}
  </fieldset>
}
