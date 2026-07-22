import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
const domain = readFileSync(new URL('../../../crates/ori-domain/src/beginner_design.rs', import.meta.url), 'utf8')
const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')

test('reference consensus is versioned, bounded to four and strictly decoded', () => {
  assert.match(domain, /pub struct BeginnerReferenceConsensusV1/u)
  assert.match(domain, /\(2\.\.=4\)\.contains\(&consensus\.bindings\.len\(\)\)/u)
  assert.match(client, /consensusBindings\.length < 2 \|\| consensusBindings\.length > 4/u)
  assert.match(client, /exactCoreDataRecord\(raw, \['kind', 'asset_id', 'sha256', 'quality'\]/u)
})

test('native computes at most six component extent and branch pairs and gates apply', () => {
  assert.match(native, /fn beginner_reference_consensus_analysis_v1/u)
  assert.match(native, /if pairs\.len\(\) == 6/u)
  assert.match(native, /component_error > 1 \|\| branch_error > 2 \|\| extent_error > 20/u)
  assert.match(native, /let apply_allowed = disagreement_count < 2/u)
  assert.match(native, /reference_consensus_multiple_disagreements/u)
  assert.match(app, /aria-label="Reference consensus"/u)
  assert.match(app, /function excludeBeginnerConsensusAsset/u)
  assert.match(app, /Exclude one outlier/u)
  assert.match(app, /aria-label="Component-aware reference comparisons"/u)
  assert.match(app, /aria-selected=\{selectedConsensusPair === key\}/u)
  assert.match(app, /setSelectedConsensusPair\(null\)/u)
  assert.match(app, /Read-only component highlight/u)
  assert.doesNotMatch(app, /pair_digest_sha256/u)
})

test('apply persists complete consensus bindings exclusion and pair digests', () => {
  assert.match(domain, /pub struct BeginnerReferenceConsensusProvenanceV1/u)
  assert.match(native, /source_revision: expected_revision/u)
  assert.match(native, /bindings: consensus\.bindings\.clone\(\)/u)
  assert.match(native, /pair_digests_sha256: analysis/u)
  assert.match(client, /pair_digests_sha256\.length > 6/u)
})

test('native update revalidates every content-addressed binding without exposing bytes', () => {
  assert.match(native, /fn reference_consensus_is_live_v1/u)
  assert.match(native, /Sha256::digest\(bytes\).*binding\.sha256/u)
  assert.match(native, /reference_consensus_asset_binding_stale/u)
  assert.doesNotMatch(client, /reference_consensus_v1[^}]*\b(?:bytes|path)\b/us)
})
