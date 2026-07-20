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
  assert.match(client, /record\.target_approximation_score/)
  assert.match(client, /admitted\[index - 1\]\.total_score < candidate\.total_score/)
  assert.match(app, /candidate\.shape_score/)
  assert.match(app, /candidate\.target_approximation_score/)
  assert.match(app, /Target-shape approximation/)
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

test('AUT-101 exposes bounded generated crease patterns and instructions as read-only previews', () => {
  assert.match(native, /generate_beginner_plans_v1/)
  assert.match(client, /response\.generated_plans\.length > 3/)
  assert.match(client, /pattern\.vertices\.length > 5/)
  assert.match(client, /pattern\.edges\.length > 4/)
  assert.match(client, /vertexIds\.has\(edge\.start\)/)
  assert.match(client, /new Set\(admittedEdges\.map\(\(edge\) => edge\.id\)\)/)
  assert.match(app, /Candidate crease-pattern preview/)
  assert.match(app, /Candidate folding instructions/)
  assert.match(app, /read-only candidate/)
  assert.match(app, /plan\.crease_pattern\.edges\.map/)
  assert.match(app, /cancelBeginnerCandidates/)
  assert.match(app, /beginnerCandidateRequestRef\.current !== requestId/)
})

test('AUT-101 admits only explicit symmetric animal and insect templates', () => {
  assert.match(client, /'symmetric_four_leg_base'/)
  assert.match(client, /'symmetric_wing_base'/)
  assert.match(native, /UnsupportedAnimalTemplate/)
  assert.match(native, /UnsupportedInsectTemplate/)
  assert.match(app, /bilateral four-part protrusion target/)
  assert.match(app, /bilateral two-part protrusion target/)
  assert.match(app, /Create the symmetric four-leg base/)
  assert.match(app, /Create the bilateral wing base/)
})

test('AUT-106 presents one recommendation first and adds bounded candidates on demand', () => {
  assert.match(native, /requested_candidate_count: u8/)
  assert.match(native, /candidates\.truncate\(usize::from\(requested_candidate_count\)\)/)
  assert.match(client, /response\.candidates\.length !== requestedCandidateCount/)
  assert.match(client, /requestedCandidateCount > 3/)
  assert.match(app, /requestBeginnerCandidates\(1\)/)
  assert.match(app, /requested_candidate_count \+ 1/)
  assert.match(app, /Generate and compare another candidate/)
  assert.match(app, /追加候補を生成して比較/)
})

test('AUT-101 apply rebinds candidate authority natively and requires confirmation', () => {
  assert.match(native, /fn apply_beginner_generated_plan/)
  assert.match(native, /expected_profile: ori_domain::BeginnerDesignProfileV1/)
  assert.match(native, /expected_candidate_edge_id: EdgeId/)
  assert.match(native, /generated candidate identity changed before apply/)
  assert.match(native, /Command::ApplyStackedFoldDocument/)
  assert.match(native, /SymmetricFourLegBase/)
  assert.match(native, /SymmetricWingBase/)
  assert.match(client, /invoke<ProjectSnapshot>\('apply_beginner_generated_plan'/)
  assert.match(client, /isCanonicalNonNilUuid\(expectedCandidateEdgeId\)/)
  assert.match(app, /window\.confirm/)
  assert.match(app, /Review and apply this candidate/)
  assert.match(app, /対角折り候補を確認して適用/)
})
