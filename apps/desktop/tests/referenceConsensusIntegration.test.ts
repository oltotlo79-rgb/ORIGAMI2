import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/beginner_design_commands.rs', import.meta.url), 'utf8')
const domain = readFileSync(new URL('../../../crates/ori-domain/src/beginner_design.rs', import.meta.url), 'utf8')
const app = [
  readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/appText.ts', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerCandidateControls.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerCandidateResults.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/useBeginnerCandidateWorkflow.ts', import.meta.url), 'utf8'),
].join('\n')

function sourceSection(start: string, end: string) {
  const startIndex = native.indexOf(start)
  const endIndex = native.indexOf(end, startIndex + start.length)
  assert.notEqual(startIndex, -1, start)
  assert.notEqual(endIndex, -1, end)
  return native.slice(startIndex, endIndex)
}

test('reference consensus is versioned, bounded to four and strictly decoded', () => {
  assert.match(domain, /pub struct BeginnerReferenceConsensusV1/u)
  assert.match(domain, /\(2\.\.=4\)\.contains\(&consensus\.bindings\.len\(\)\)/u)
  assert.match(
    client,
    /snapshotCoreDataArray\(consensus\.bindings, 4\)/u,
  )
  assert.match(client, /consensusBindingInputs\.length < 2/u)
  assert.match(client, /exactCoreDataRecord\(raw, \['kind', 'asset_id', 'sha256', 'quality'\]/u)
})

test('native computes at most six component extent and branch pairs and gates apply', () => {
  assert.match(native, /fn beginner_reference_consensus_analysis_v1/u)
  assert.match(native, /if pairs\.len\(\) == 6/u)
  assert.match(native, /component_error > 1 \|\| branch_error > 2 \|\| extent_error > 20/u)
  assert.match(native, /let apply_allowed = disagreement_count < 2/u)
  assert.match(native, /reference_consensus_multiple_disagreements/u)
  assert.match(app, /referenceConsensus: localized\('参照資料の合意', 'Reference consensus'\)/u)
  assert.match(app, /function excludeBeginnerConsensusAsset/u)
  assert.match(app, /Exclude one outlier/u)
  assert.match(
    app,
    /componentAwareReferenceComparisons: localized\(\s*'部品別の参照資料比較',\s*'Component-aware reference comparisons'/u,
  )
  assert.match(app, /aria-selected=\{selectedConsensusPair === key\}/u)
  assert.match(app, /setSelectedConsensusPair\(null\)/u)
  assert.match(app, /Read-only component highlight/u)
  assert.doesNotMatch(app, /pair_digest_sha256/u)
})

test('apply persists complete consensus bindings exclusion and pair digests', () => {
  const builder = sourceSection(
    'fn build_beginner_reference_consensus_provenance_v1(',
    'fn build_beginner_generic_tree_provenance_v1(',
  )
  const directApply = sourceSection(
    'pub(super) fn apply_beginner_generated_plan_document(',
    'pub(super) fn apply_grid_plan_document(',
  )
  const gridApply = sourceSection(
    'pub(super) fn apply_grid_plan_document(',
    '#[tauri::command]\npub(super) fn apply_beginner_parameter_grid_candidate(',
  )
  assert.match(domain, /pub struct BeginnerReferenceConsensusProvenanceV1/u)
  assert.match(builder, /profile\s*\.reference_consensus_v1/u)
  assert.match(
    builder,
    /beginner_reference_consensus_analysis_v1\(project, None\)/u,
  )
  assert.match(builder, /source_revision: expected_revision/u)
  assert.match(builder, /bindings: consensus\.bindings\.clone\(\)/u)
  assert.match(
    builder,
    /excluded_asset_id: consensus\.excluded_asset_id/u,
  )
  assert.match(builder, /pair_digests_sha256: analysis/u)
  assert.match(
    builder,
    /source_count: consensus\.bindings\.len\(\) as u8/u,
  )
  assert.match(
    builder,
    /excluded_count: u8::from\(consensus\.excluded_asset_id\.is_some\(\)\)/u,
  )
  assert.match(
    directApply,
    /build_beginner_reference_consensus_provenance_v1\(\s*&project,\s*&beginner_design_profile,\s*expected_revision,\s*\)\?/u,
  )
  assert.match(
    gridApply,
    /build_beginner_reference_consensus_provenance_v1\(\s*project,\s*&beginner_design_profile,\s*expected_revision,\s*\)\?/u,
  )
  for (const apply of [directApply, gridApply]) {
    assert.match(
      apply,
      /reference_consensus_summary: reference_consensus_provenance\s*\.as_ref\(\)\s*\.map\(\|value\| value\.summary\.clone\(\)\),/u,
    )
    assert.match(
      apply,
      /reference_consensus: reference_consensus_provenance,/u,
    )
    assert.match(
      apply,
      /beginner_design_profile: Box::new\(beginner_design_profile\)/u,
    )
  }
  assert.match(
    client,
    /snapshotCoreDataArray\(provenanceConsensus\.pair_digests_sha256, 6\)/u,
  )
  assert.match(
    client,
    /provenanceConsensusPairDigestInputs\.length < 1/u,
  )
})

test('native update revalidates every content-addressed binding without exposing bytes', () => {
  assert.match(native, /fn reference_consensus_is_live_v1/u)
  assert.match(native, /Sha256::digest\(bytes\).*binding\.sha256/u)
  assert.match(native, /reference_consensus_asset_binding_stale/u)
  assert.doesNotMatch(client, /reference_consensus_v1[^}]*\b(?:bytes|path)\b/us)
})
