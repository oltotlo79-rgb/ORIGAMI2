import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')

test('the empty inspector exposes direct coordinate vertex creation in both locales', () => {
  assert.match(appSource, /name="direct_x_display"/)
  assert.match(appSource, /name="direct_y_display"/)
  assert.match(appSource, /Add vertex by coordinates/)
  assert.match(appSource, /座標から頂点を追加/)
  assert.match(appSource, /readLengthInputMillimetres\([\s\S]*?'direct_x_display'/)
  assert.match(appSource, /readLengthInputMillimetres\([\s\S]*?'direct_y_display'/)
})

test('direct coordinate creation uses the guarded native edit and selects only its new vertex', () => {
  assert.match(
    appSource,
    /async function submitDirectVertex[\s\S]*?await runNativeEdit[\s\S]*?await addVertex/,
  )
  assert.match(
    appSource,
    /previousVertexIds[\s\S]*?!previousVertexIds\.has\(id\)[\s\S]*?setSelectedVertexId/,
  )
  assert.match(
    appSource,
    /benchmarkRun \|\| nativeLayerView\.defaultLayerLocked/,
  )
})

test('a selected vertex can create an endpoint from an explicit length and angle', () => {
  assert.match(appSource, /name="polar_length_display"/)
  assert.match(appSource, /name="polar_angle_degrees"/)
  assert.match(appSource, /name="polar_edge_kind"/)
  assert.match(appSource, /value="polar_endpoint"/)
  assert.match(appSource, /Draw line by length and angle/)
  assert.match(
    appSource,
    /form\.get\('vertex_action'\) === 'polar_endpoint'[\s\S]*?Math\.cos[\s\S]*?Math\.sin/,
  )
  assert.match(
    appSource,
    /polar_endpoint[\s\S]*?await runNativeEdit[\s\S]*?await addConnectedVertex/,
  )
})
