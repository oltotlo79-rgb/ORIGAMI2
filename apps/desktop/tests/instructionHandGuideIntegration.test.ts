import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const previewSource = readFileSync(
  new URL('../src/components/FoldPreview.tsx', import.meta.url),
  'utf8',
)
const panelSource = readFileSync(
  new URL('../src/components/InstructionTimelinePanel.tsx', import.meta.url),
  'utf8',
)

test('instruction hand guides are authored and rendered as a touch point plus direction', () => {
  assert.match(panelSource, /hand_guides.*pinch\/hold\/push\/regrip/u)
  assert.match(previewSource, /for \(const guide of visual\.hand_guides\)/u)
  assert.match(previewSource, /new THREE\.TorusGeometry/u)
  assert.match(previewSource, /new THREE\.ArrowHelper/u)
  assert.match(previewSource, /guide\.kind === 'pinch'/u)
  assert.match(previewSource, /guide\.kind === 'hold'/u)
})
