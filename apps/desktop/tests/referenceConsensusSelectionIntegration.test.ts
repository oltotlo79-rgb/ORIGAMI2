import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const native = readFileSync(new URL('../src-tauri/src/beginner_design_commands.rs', import.meta.url), 'utf8')
const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const app = [
  readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerCandidateControls.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerCandidateResults.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerReferenceAssetPanel.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/appText.ts', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/useBeginnerCandidateWorkflow.ts', import.meta.url), 'utf8'),
].join('\n')

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
  assert.match(app, /type="checkbox"\s*checked=\{checked\}/u)
  assert.match(app, /disabled=\{!checked && consensusSelectionDraft\.length >= 4\}/u)
  assert.match(app, /setConsensusSelectionDraft\(\s*\(snapshotRef\.current\?/u)
  assert.match(app, /Save consensus references/u)
  assert.doesNotMatch(app, /References for consensus[\s\S]{0,1400}sha256/u)
})

test('consensus analysis is generation-scoped cancellable and progress bounded', () => {
  assert.match(native, /struct ReferenceConsensusWorkV1/u)
  assert.match(native, /reference-consensus-progress-v1/u)
  assert.match(native, /if work\.cancelled\.load\(Ordering::Acquire\)/u)
  assert.match(native, /fn cancel_reference_consensus/u)
  assert.match(client, /export function cancelReferenceConsensus/u)
  assert.match(app, /consensusGenerationRef/u)
  assert.match(app, /Cancel consensus analysis/u)
  assert.match(app, /payload\.request_generation_id\s*!== consensusGenerationRef\.current/u)
  assert.match(app, /\.catch\(\(\) => undefined\)/u)
})
