import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
const domain = readFileSync(new URL('../../../crates/ori-domain/src/beginner_design.rs', import.meta.url), 'utf8')

test('reference consensus is versioned, bounded to four and strictly decoded', () => {
  assert.match(domain, /pub struct BeginnerReferenceConsensusV1/u)
  assert.match(domain, /\(2\.\.=4\)\.contains\(&consensus\.bindings\.len\(\)\)/u)
  assert.match(client, /consensusBindings\.length < 2 \|\| consensusBindings\.length > 4/u)
  assert.match(client, /exactCoreDataRecord\(raw, \['kind', 'asset_id', 'sha256', 'quality'\]/u)
})

test('native update revalidates every content-addressed binding without exposing bytes', () => {
  assert.match(native, /fn reference_consensus_is_live_v1/u)
  assert.match(native, /Sha256::digest\(bytes\).*binding\.sha256/u)
  assert.match(native, /reference_consensus_asset_binding_stale/u)
  assert.doesNotMatch(client, /reference_consensus_v1[^}]*\b(?:bytes|path)\b/us)
})

