import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const app = readFileSync('src/App.tsx', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')
const browser = readFileSync('scripts/generic-target-browser-e2e.mjs', 'utf8')
const browserHarness = readFileSync('scripts/generic-target-browser-harness.tsx', 'utf8')

test('generic feature bindings cross the exact frontend DTO boundary', () => {
  assert.match(client, /generic_feature_bindings: ReadonlyArray/u)
  assert.match(client, /'skeleton_segment_id', 'skeleton_endpoint', 'mount_distance_squared_tenths_mm'/u)
  assert.match(client, /featureBindings\.length < 2/u)
  assert.match(client, /!\[1, 2, 4\]\.includes\(Number\(binding\.endpoint_count\)\)/u)
  assert.match(client, /Number\(binding\.crease_start\) \+ Number\(binding\.endpoint_count\)/u)
  assert.match(client, /normalizedPlans\.generated_plans\[index\]\.kind === 'composite_generic_target_base'/u)
})

test('generic feature topology is visible and browser-covered through persistence', () => {
  assert.match(app, /汎用部位topology証明/u)
  assert.match(app, /Generic feature topology witness/u)
  assert.match(app, /→skeleton/u)
  assert.match(native, /Shape generated \{topology_label\}/u)
  assert.match(browser, /Try tampered generic feature binding/u)
  assert.match(browser, /Generated generic feature instruction steps/u)
  assert.match(browser, /Undo generic target/u)
  assert.match(browser, /Redo generic target/u)
  assert.match(browser, /Save and reopen generic target/u)
})

test('3D generalization stays bounded to confirmed semantic parts', () => {
  assert.match(client, /protrusions\.length < 1 \|\| protrusions\.length > 8/u)
  assert.match(client, /semantic[\s\S]*remain the user's current target_parts/u)
  assert.match(app, /geometry evidence only; part meanings come from the parts you confirmed/u)
  assert.match(native, /four explicit generic features remain a bounded candidate/u)
  assert.match(native, /reference_model_suggestion_confirmation_required/u)
  assert.match(native, /reference_model_suggestion_matches_live_v1/u)
})

test('confirmed image and 3D generic candidates retain vertical proof coverage', () => {
  assert.match(browser, /Image meanings unconfirmed: generic topology candidate blocked/u)
  assert.match(browser, /Confirm explicit image part meanings/u)
  assert.match(browser, /Exclude unconfirmed image noise/u)
  assert.match(browser, /Restore excluded image candidate/u)
  assert.match(browser, /retained unique ID and outline evidence/u)
  assert.match(browser, /meaning remains unconfirmed/u)
  assert.match(browser, /Applied image outline evidence \+ 2 explicitly confirmed part meanings/u)
  assert.match(browserHarness, /Global flat-foldability proven/u)
  assert.match(browserHarness, /Native foldability admission: global proof \+ bounded fold path certificate/u)
  assert.match(browserHarness, /Generated generic feature instruction steps/u)
  assert.match(browser, /Undo generic target/u)
  assert.match(browser, /Redo generic target/u)
  assert.match(browser, /Save and reopen generic target/u)
})
