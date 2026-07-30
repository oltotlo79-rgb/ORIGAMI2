import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { readDesktopRustUnitTestSources } from './testRustSource.ts'

const app = [
  readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/components/BeginnerCandidateControls.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/useBeginnerParameterGridWorkflow.ts', import.meta.url), 'utf8'),
].join('\n')
const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/beginner_design_commands.rs', import.meta.url), 'utf8')
const nativeTests = readDesktopRustUnitTestSources()
const recognition = readFileSync(new URL('../src-tauri/src/beginner_recognition.rs', import.meta.url), 'utf8')
const workflow = readFileSync(new URL('../src/lib/beginnerGridWorkflow.ts', import.meta.url), 'utf8')

test('complete animal recognition reaches the bounded grid through one native contract', () => {
  assert.match(native, /animal_complete_bindings_v1/)
  assert.match(native, /requested_complete_animal/)
  assert.match(native, /fn evaluate_beginner_parameter_grid/)
  assert.match(client, /getBeginnerParameterGridProgress/)
  assert.match(app, /evaluateBeginnerParameterGrid/)
  assert.match(
    app,
    /setBeginnerGridProgress\(\{\s*enumerated: 27,\s*globalChecked: 3,\s*refined: response\.refinement_iterations,\s*\}\)/u,
  )
})

test('optional wing pair stays the strict fifth binding across image, GLB, and wire', () => {
  assert.match(recognition, /matches!\(wing_candidate_ids\.len\(\), 0 \| 2\)/)
  assert.match(recognition, /wing_target\.id = 5/)
  assert.match(recognition, /candidate_pair_is_symmetric/)
  assert.match(native, /requested_animal_wings/)
  assert.match(native, /wings\.id = 5/)
  assert.match(native, /animal_complete_winged_bindings_v1/)
  assert.match(client, /completeAnimalHasWings/)
  assert.match(client, /composite_complete_winged_animal_base/)
})

test('grid cancellation and stale replacement stay generation and snapshot scoped', () => {
  assert.match(nativeTests, /beginner_grid_progress_is_bounded_and_cancel_is_generation_scoped/)
  assert.match(client, /cancelBeginnerParameterGrid/)
  assert.match(app, /requestId !== requestRef\.current/)
  assert.match(
    app,
    /matchesBeginnerProjectBinding\(\s*response,\s*input\.getCurrentSnapshot\(\),?\s*\)/u,
  )
  assert.match(app, /transport\.cancel\(generationId\)/)
  assert.match(app, /generationRef\.current = null[\s\S]*setBeginnerGridBusy\(false\)/u)
  assert.match(app, /finishBeginnerGridCancellation/)
  assert.match(workflow, /clearPreview\(\)\s*restoreFocus\(\)/)
})

test('confirmed apply retains preview on failure and restores focus only after success', () => {
  assert.match(app, /window\.confirm/)
  assert.match(app, /applyBeginnerParameterGridCandidate/)
  assert.match(app, /runBeginnerGridApplyWorkflow/)
  assert.match(workflow, /if \(!confirm\(\)\) return false/)
  assert.match(workflow, /if \(!await apply\(\)\) return false/)
  assert.match(app, /function restoreFocus\(\)[\s\S]*buttonRef\.current\?\.focus\(\)/u)
  assert.match(app, /ref=\{beginnerGridButtonRef\}/)
})

test('native complete animal apply is atomic, replay-safe, undoable, redoable, and persistent', () => {
  assert.match(nativeTests, /fn complete_animal_grid_apply_replay_undo_redo_and_archive_round_trip/)
  assert.match(nativeTests, /fn complete_winged_animal_grid_apply_and_archive_round_trip/)
  assert.match(native, /expected_grid_hash/)
  assert.match(native, /Command::ApplyBeginnerGeneratedDocument/)
  assert.match(nativeTests, /execute_undo\(&mut project/)
  assert.match(nativeTests, /execute_redo\(&mut project/)
  assert.match(native, /animal_complete_bindings_v1/)
})
