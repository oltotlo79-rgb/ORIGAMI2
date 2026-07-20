import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')

test('AUT-106 candidate reads bind instance, project, and revision without mutation', () => {
  assert.match(native, /fn evaluate_beginner_candidates/)
  assert.match(native, /ensure_expected_project\([\s\S]*expected_project_instance_id[\s\S]*expected_project_id[\s\S]*expected_revision/)
  assert.match(client, /invoke<unknown>\('evaluate_beginner_candidates'/)
  assert.match(client, /response\.candidates\.length > 3/)
})

test('candidate admission requires ordered bounded explainable scores', () => {
  assert.match(client, /record\.rank !== index \+ 1/)
  assert.match(client, /Number\(score\) > 100/)
  assert.match(client, /admitted\[index - 1\]\.total_score < candidate\.total_score/)
  assert.match(app, /candidate\.shape_score/)
  assert.match(app, /candidate\.paper_efficiency_score/)
})

test('candidate UI is bilingual, accessible, single-flight, and rejects stale ABA results', () => {
  assert.match(app, /設計候補の比較/)
  assert.match(app, /Compare design candidates/)
  assert.match(app, /aria-describedby="beginner-candidate-description"/)
  assert.match(app, /if \(beginnerCandidateBusy\) return/)
  assert.match(app, /latestSnapshotRef\.current !== current/)
})

test('AUT-107 fixes the initial bulge and elasticity policy in native, IPC, and UI', () => {
  assert.match(native, /TargetShapeApproximation/)
  assert.match(native, /BeginnerElasticityModelV1::NotComputed/)
  assert.match(client, /response\.bulge_treatment !== 'target_shape_approximation'/)
  assert.match(client, /response\.elasticity_model !== 'not_computed'/)
  assert.match(app, /膨らみを目標形状への近似として扱い/)
  assert.match(app, /does not compute paper elasticity/)
  assert.match(app, /role="note"/)
})
