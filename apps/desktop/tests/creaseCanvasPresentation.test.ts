import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  creaseCanvasAngleGuideLabel,
  creaseCanvasGuideDetailLabel,
  creaseCanvasSnapKindLabel,
  creaseCanvasText,
  creaseCanvasTitle,
  localizeCreaseCanvasMeasurementLabel,
} from '../src/lib/creaseCanvasPresentation.ts'
import type { SnapKind } from '../src/lib/snap.ts'

const canvasSource = readFileSync(
  new URL('../src/components/CreaseCanvas.tsx', import.meta.url),
  'utf8',
)

test('crease canvas presents its complete fixed copy in Japanese and English', () => {
  assert.equal(creaseCanvasText('ja', 'ariaLabel'), '展開図編集キャンバス')
  assert.equal(
    creaseCanvasText('en', 'fallback'),
    'Crease pattern. With the select tool, drag a vertex to move it.',
  )
  assert.equal(
    creaseCanvasTitle('en', true),
    'Crease-pattern editing canvas. Editing is currently unavailable.',
  )

  const expected: Readonly<Record<SnapKind, readonly [string, string]>> = {
    vertex: ['頂点', 'Vertex'],
    intersection: ['交点', 'Intersection'],
    midpoint: ['中点', 'Midpoint'],
    horizontal: ['水平', 'Horizontal'],
    vertical: ['垂直', 'Vertical'],
    parallel: ['平行', 'Parallel'],
    angle: ['角度', 'Angle'],
    'circle-intersection': ['円の交点', 'Circle intersection'],
    edge: ['辺', 'Edge'],
    grid: ['グリッド', 'Grid'],
  }
  for (const [kind, [japanese, english]] of Object.entries(expected)) {
    assert.equal(
      creaseCanvasSnapKindLabel('ja', kind as SnapKind),
      japanese,
    )
    assert.equal(
      creaseCanvasSnapKindLabel('en', kind as SnapKind),
      english,
    )
  }
  assert.equal(
    creaseCanvasGuideDetailLabel('en', 'intersection-cluster'),
    'Intersection cluster',
  )
  assert.equal(
    creaseCanvasGuideDetailLabel('en', 'boundary-t-junction'),
    'Boundary T-junction',
  )
})

test('angle guides preserve sign and deterministic precision in both locales', () => {
  assert.equal(
    creaseCanvasAngleGuideLabel('ja', 'counterclockwise', 45),
    '角度 +45°',
  )
  assert.equal(
    creaseCanvasAngleGuideLabel('en', 'clockwise', 0.000125),
    'Angle -1.25e-4°',
  )
  assert.equal(
    creaseCanvasAngleGuideLabel('en', 'counterclockwise', Number.NaN),
    'Angle +—°',
  )
})

test('measurement copy accepts only known numeric units and retranslates stale labels', () => {
  assert.equal(
    localizeCreaseCanvasMeasurementLabel('1,234.5 mm / -45.25°', 'en'),
    '1,234.5 mm / -45.25°',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel('0.5 紙辺比 / 30°', 'en'),
    '0.5 paper-edge ratio / 30°',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel(
      '0.5 paper-edge ratio / 30°',
      'ja',
    ),
    '0.5 紙辺比 / 30°',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel('計測不可', 'en'),
    'Unavailable',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel(
      '<img src=x onerror=alert(1)>',
      'en',
    ),
    'Unavailable',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel(
      '12 mm / 45° ネイティブ例外',
      'en',
    ),
    'Unavailable',
  )
  assert.equal(
    localizeCreaseCanvasMeasurementLabel(
      '1'.repeat(161),
      'ja',
    ),
    '計測不可',
  )
})

test('the component stores semantic guide details and localizes only at paint time', () => {
  assert.match(canvasSource, /detail\?: CreaseCanvasGuideDetail/u)
  assert.match(canvasSource, /detail: target\.kind === 'intersection'/u)
  assert.match(
    canvasSource,
    /creaseCanvasGuideDetailLabel\(locale, guide\.detail\)/u,
  )
  assert.match(
    canvasSource,
    /localizeCreaseCanvasMeasurementLabel\(measurementLabel, locale\)/u,
  )
  assert.doesNotMatch(canvasSource, /[ぁ-んァ-ヶ一-龠]/u)
})
