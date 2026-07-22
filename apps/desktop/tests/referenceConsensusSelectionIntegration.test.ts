import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')

test('consensus selection accepts only two to four native-resolved project assets', () => {
  assert.match(native, /fn update_beginner_reference_consensus/u)
  assert.match(native, /if !\(2\.\.=4\)\.contains\(&selections\.len\(\)\)/u)
  assert.match(native, /canonical\.sort_by_key\(\|selection\| selection\.asset_id\.canonical_bytes\(\)\)/u)
  assert.match(native, /Sha256::digest\(bytes\)/u)
  assert.match(native, /Command::UpdateBeginnerDesignProfile/u)
  assert.doesNotMatch(client, /updateBeginnerReferenceConsensus[\s\S]{0,700}sha256/u)
})

test('selection UI is bounded, keyboard native and stale-reset by snapshot', () => {
  assert.match(app, /<fieldset aria-describedby="reference-consensus-selection-help">/u)
  assert.match(app, /type="checkbox" checked=\{checked\}/u)
  assert.match(app, /disabled=\{!checked && consensusSelectionDraft\.length >= 4\}/u)
  assert.match(app, /setConsensusSelectionDraft\(\(nativeSnapshot\?/u)
  assert.match(app, /Save consensus references/u)
  assert.doesNotMatch(app, /References for consensus[\s\S]{0,1400}sha256/u)
})

