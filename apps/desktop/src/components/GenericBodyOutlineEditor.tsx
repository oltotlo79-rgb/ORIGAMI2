import { useEffect, useState } from 'react'
import {
  GENERIC_BODY_OUTLINE_EDITOR_TEXT as TEXT,
} from '../lib/genericBodyOutlineEditorText.ts'
import {
  selectLocalizedText,
  type LocalizedText,
} from '../lib/i18n.ts'
import { canonicalizeIntegerPolarPolygonV1 } from '../lib/integerPolarPolygon.ts'

type Point = [number, number]

function canonicalize(points: Point[], mode: 'symmetric' | 'general'): Point[] | null {
  if (points.length < 4 || points.length > 16
    || points.some(([x, y]) => !Number.isFinite(x) || !Number.isFinite(y)
      || Math.abs(x) > 10_000 || Math.abs(y) > 10_000)) return null
  const tenths = points.map(([x, y]) => [Math.round(x * 10), Math.round(y * 10)] as Point)
  const keys = new Set(tenths.map(([x, y]) => `${x},${y}`))
  if (keys.size !== tenths.length
    || (mode === 'symmetric' && tenths.some(([x, y]) => !keys.has(`${-x},${y}`)))) return null
  return canonicalizeIntegerPolarPolygonV1(tenths, mode === 'symmetric')
}

export function GenericBodyOutlineEditor({ locale, points, mode, onChange, onModeChange }: {
  locale: 'ja' | 'en'
  points: readonly Point[]
  mode: 'symmetric' | 'general'
  onChange: (points: Point[]) => void
  onModeChange: (mode: 'symmetric' | 'general') => void
}) {
  const [source, setSource] = useState('')
  const [invalid, setInvalid] = useState(false)
  const t = (value: LocalizedText) => selectLocalizedText(locale, value)
  useEffect(() => setSource(points.map(([x, y]) => `${x / 10}, ${y / 10}`).join('\n')), [points])
  const apply = () => {
    const parsed = source.split(/\r?\n/u).filter((line) => line.trim() !== '').map((line) => {
      const values = line.split(',').map((value) => Number(value.trim()))
      return values.length === 2 ? values as Point : null
    })
    const canonical = parsed.every((point): point is Point => point !== null)
      ? canonicalize(parsed, mode) : null
    setInvalid(canonical === null)
    if (canonical) onChange(canonical)
  }
  return <fieldset>
    <legend>{t(TEXT.legend)}</legend>
    <label>{t(TEXT.outlineMode)}
      <select aria-label={t(TEXT.outlineModeAria)} value={mode}
        onChange={(event) => onModeChange(event.currentTarget.value as 'symmetric' | 'general')}>
        <option value="symmetric">{t(TEXT.symmetricOption)}</option>
        <option value="general">{t(TEXT.generalOption)}</option>
      </select>
    </label>
    <label>{t(TEXT.outlinePoints)}
      <textarea aria-label={t(TEXT.outlinePointsAria)}
        value={source} onChange={(event) => setSource(event.currentTarget.value)} />
    </label>
    <button type="button" onClick={apply}>{t(TEXT.applyOutline)}</button>
    <button type="button" onClick={() => { setSource(''); setInvalid(false); onChange([]) }}>
      {t(TEXT.clearOutline)}
    </button>
    {invalid && <p role="alert">{t(
      mode === 'symmetric'
        ? TEXT.invalidSymmetricOutline
        : TEXT.invalidGeneralOutline,
    )}</p>}
  </fieldset>
}
