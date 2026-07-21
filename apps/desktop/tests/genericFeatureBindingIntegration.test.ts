import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const app = readFileSync('src/App.tsx', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')
const browser = readFileSync('scripts/generic-target-browser-e2e.mjs', 'utf8')

test('generic feature bindings cross the exact frontend DTO boundary', () => {
  assert.match(client, /generic_feature_bindings: ReadonlyArray/u)
  assert.match(client, /'protrusion_id', 'generated_feature_id', 'endpoint_count', 'crease_start'/u)
  assert.match(client, /featureBindings\.length < 2/u)
  assert.match(client, /!\[1, 2, 4\]\.includes\(Number\(binding\.endpoint_count\)\)/u)
  assert.match(client, /Number\(binding\.crease_start\) \+ Number\(binding\.endpoint_count\)/u)
  assert.match(client, /normalizedPlans\.generated_plans\[index\]\.kind === 'composite_generic_target_base'/u)
})

test('generic feature topology is visible and browser-covered through persistence', () => {
  assert.match(app, /汎用部位topology証明/u)
  assert.match(app, /Generic feature topology witness/u)
  assert.match(native, /Shape generated \{topology_label\}/u)
  assert.match(browser, /Try tampered generic feature binding/u)
  assert.match(browser, /Generated generic feature instruction steps/u)
  assert.match(browser, /Undo generic target/u)
  assert.match(browser, /Redo generic target/u)
  assert.match(browser, /Save and reopen generic target/u)
})
