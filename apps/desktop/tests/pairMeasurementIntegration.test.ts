import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
const canvas = readFileSync(new URL('../src/components/CreaseCanvas.tsx', import.meta.url), 'utf8')

test('pair measurement is wired through benchmark display and deterministic reset boundaries', () => {
  assert.match(app, /const displayedLines = benchmarkRun\?\.lines \?\? nativeLines/u)
  assert.match(app, /tool=\{activeTool === 'measure' \? 'measure' : benchmarkRun \? 'select'/u)
  assert.match(app, /if \(activeTool === 'measure'\) return[\s\S]*setMeasurementLineIds\(\[\]\)[\s\S]*setMeasurementVertexIds\(\[\]\)/u)
  assert.match(app, /else if \(activeTool === 'measure'\) \{[\s\S]*setMeasurementLineIds\(\[\]\)[\s\S]*setMeasurementVertexIds\(\[\]\)/u)
  assert.match(app, /retainMeasurementPair\(current, lineIds\)/u)
  assert.match(app, /retainMeasurementPair\(current, vertexIds\)/u)
})

test('measure-mode hit testing keeps zoom-scaled vertex priority over edges', () => {
  assert.match(canvas, /\(tool === 'select' \|\| tool === 'measure'\) && closestVertex/u)
  assert.match(canvas, /VERTEX_HIT_RADIUS_PX \/ transform\.scale/u)
})
