import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const canvas = source('../src/components/CreaseCanvas.tsx')
const preview = source('../src/components/FoldPreview.tsx')
const picking = source('../src/lib/foldPreviewPicking.ts')

test('App shares line vertex and face selections across both views', () => {
  assert.match(app, /const \[selectedFaceId, setSelectedFaceId\]/u)
  assert.match(
    app,
    /<CreaseCanvas[\s\S]*?selectedVertexId=\{selectedVertexId\}[\s\S]*?selectedFaceId=\{selectedFaceId\}[\s\S]*?selectedLineId=\{selectedLineId\}/u,
  )
  assert.match(
    app,
    /<FoldPreview[\s\S]*?selectedHingeId=\{selectedPreviewHingeId\}[\s\S]*?selectedFaceId=\{selectedFaceId\}[\s\S]*?selectedVertexId=\{selectedVertexId\}/u,
  )
  assert.match(app, /onSelectFace=\{benchmarkRun[\s\S]*?setSelectedFaceId/u)
  assert.match(app, /onSelectVertex=\{benchmarkRun[\s\S]*?setSelectedVertexId/u)
})

test('2D topology faces and 3D element markers are selectable and highlighted', () => {
  assert.match(canvas, /pointInPolygonInclusive\(x, y, candidate\.polygon\)/u)
  assert.match(canvas, /rgba\(22, 113, 184, 0\.18\)/u)
  assert.match(preview, /new THREE\.SphereGeometry/u)
  assert.match(preview, /createdSelectedVertexMaterial/u)
  assert.match(preview, /faceId === selectedFaceIdRef\.current/u)
  assert.match(preview, /target\?\.kind === 'vertex'/u)
  assert.match(picking, /\{ kind: 'vertex', vertexId \}/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
