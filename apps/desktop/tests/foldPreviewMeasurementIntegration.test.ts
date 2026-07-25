import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  FOLD_PREVIEW_COMPONENT_TEXT as TEXT,
} from '../src/lib/foldPreviewComponentText.ts'

const source = readFileSync(new URL('../src/components/FoldPreview.tsx', import.meta.url), 'utf8')
const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')

test('measurement mode is read-only and returns before ordinary selection callbacks', () => {
  assert.match(source, /if \(measurementModeRef\.current\) \{[\s\S]*updateMeasurement[\s\S]*return[\s\S]*onSelectHingeRef\.current/u)
  assert.match(source, /measurementModeRef\.current = measurementMode && !disabled/u)
  assert.match(app, /disabled=\{coreBusy \|\| recoveryBlocking \|\| Boolean\(benchmarkRun\)\}/u)
})

test('every topology kind registers real face groups and samples source midsurfaces', () => {
  assert.match(source, /measurementFaceGroups\.set\(face\.id, \{ group, offsetX: 0, offsetZ: 0 \}\)/u)
  assert.match(source, /measurementFaceGroups\.set\(singleAnchor\.movingFace\.id/u)
  assert.match(source, /x: source\.x,[\s\S]*z: source\.z,[\s\S]*matrix: registered\.group\.matrixWorld\.elements\.slice\(\)/u)
  assert.match(source, /throw new Error\('duplicate face ID'\)/u)
})

test('pose identity and disabled changes clear state with bilingual live status', () => {
  assert.match(source, /setMeasurementSelection\(null\)[\s\S]*if \(disabled\) setMeasurementMode\(false\)[\s\S]*\[disabled, model, renderError, renderedAppliedPose\]/u)
  assert.match(source, /role="status" aria-live="polite" data-measurement-kind/u)
  assert.deepEqual(TEXT.measurementMode, {
    ja: '3D計測モード',
    en: '3D measurement mode',
  })
  assert.deepEqual(TEXT.vertexDistance, {
    ja: '2頂点間の距離',
    en: 'Vertex distance',
  })
  assert.deepEqual(TEXT.faceNormalAngle, {
    ja: '2面の法線角',
    en: 'Face-normal angle',
  })
})
