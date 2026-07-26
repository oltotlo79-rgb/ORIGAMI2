import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = [
  readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/SelectedLineInspector.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/SelectedVertexInspector.tsx', import.meta.url), 'utf8'),
].join('\n')

test('the 10,000-edge benchmark exposes basic vertex and edge editing', () => {
  assert.match(appSource, /generateBenchmarkPattern\(10_000\)/u)
  assert.match(appSource, /function moveBenchmarkVertex/u)
  assert.match(appSource, /line\.startVertexId === vertexId/u)
  assert.match(appSource, /line\.endVertexId === vertexId/u)
  assert.match(appSource, /function deleteBenchmarkLine/u)
  assert.match(appSource, /lines: current\.lines\.filter/u)
  assert.match(appSource, /\? moveBenchmarkVertex/u)
  assert.match(appSource, /onDeleteBenchmarkLine\(line\.id\)/u)
})
