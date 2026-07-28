import { useEffect, useState } from 'react'
import {
  formatLocalizedText,
  selectLocalizedText,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  PROTRUSION_LOCAL_OUTLINE_EDITOR_TEXT as TEXT,
} from '../lib/protrusionLocalOutlineEditorText.ts'
import { canonicalizeIntegerPolarPolygonV1 } from '../lib/integerPolarPolygon.ts'

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
  return canonicalizeIntegerPolarPolygonV1(points, false)
}

export function ProtrusionLocalOutlineEditor({ locale, bindingId, symmetry, points, onChange }: {
  locale: 'ja' | 'en'; bindingId: number; symmetry: 'none' | 'bilateral' | 'radial'
  points: readonly Point[]; onChange: (points: Point[] | undefined) => void
}) {
  const [source, setSource] = useState('')
  const [invalid, setInvalid] = useState(false)
  const t = (value: LocalizedText) => selectLocalizedText(locale, value)
  const savedSource = points.map(([x, y]) => `${x / 10}, ${y / 10}`).join('\n')
  useEffect(() => setSource(savedSource), [savedSource])
  return <fieldset><legend>{t(TEXT.legend)}</legend>
    <label>{t(TEXT.outlinePoints)}
      <textarea aria-label={formatLocalizedText(locale, TEXT.outlinePointsAria, { bindingId })}
        value={source} onChange={(event) => setSource(event.currentTarget.value)} /></label>
    <button type="button" onClick={() => { const result = canonicalize(source, symmetry === 'bilateral')
      setInvalid(result === null); if (result) onChange(result) }}>
      {t(TEXT.applyOutline)}</button>
    <button type="button" onClick={() => { setSource(''); setInvalid(false); onChange(undefined) }}>
      {t(TEXT.clearOutline)}</button>
    {invalid && <p role="alert">{t(TEXT.invalidOutline)}</p>}
  </fieldset>
}
