import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
const canvasSource = readFileSync(
  new URL('../src/components/CreaseCanvas.tsx', import.meta.url),
  'utf8',
)

test('compass construction uses the selected vertex, display unit, and bounded guides', () => {
  assert.match(appSource, /name="compass_radius_display"/u)
  assert.match(appSource, /selectedVertex\.position\.x/u)
  assert.match(appSource, /selectedVertex\.position\.y/u)
  assert.match(appSource, /lengthDisplayUnit\.millimetresPerUnit/u)
  assert.match(appSource, /\.slice\(-64\)/u)
  assert.match(appSource, /if \(forceReplacement\) setCompassCircles\(\[\]\)/u)
})

test('crease canvas renders every valid compass circle as a dashed full circle', () => {
  assert.match(canvasSource, /for \(const circle of compassCircles\)/u)
  assert.match(canvasSource, /circle\.radius \* transform\.scale/u)
  assert.match(canvasSource, /context\.arc\(center\.x, center\.y, radius, 0, Math\.PI \* 2\)/u)
  assert.match(canvasSource, /context\.setLineDash\(\[7, 4\]\)/u)
})
