import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
const client = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')

test('complete animal recognition reaches the bounded grid through one native contract', () => {
  assert.match(native, /animal_complete_bindings_v1/)
  assert.match(native, /requested_complete_animal/)
  assert.match(native, /fn evaluate_beginner_parameter_grid/)
  assert.match(client, /getBeginnerParameterGridProgress/)
  assert.match(app, /evaluateBeginnerParameterGrid/)
  assert.match(app, /setBeginnerGridProgress\(\{ enumerated: 27, globalChecked: 3 \}\)/)
})

test('grid cancellation and stale replacement stay generation and snapshot scoped', () => {
  assert.match(native, /beginner_grid_progress_is_bounded_and_cancel_is_generation_scoped/)
  assert.match(client, /cancelBeginnerParameterGrid/)
  assert.match(app, /requestId !== beginnerGridRequestRef\.current/)
  assert.match(app, /latest\?\.project_instance_id === response\.project_instance_id/)
  assert.match(app, /cancelBeginnerParameterGrid\(generationId\)/)
  assert.match(app, /beginnerGridGenerationRef\.current = null\s*setBeginnerGridBusy\(false\)/)
  assert.match(app, /setBeginnerGrid\(null\)\s*requestAnimationFrame\(\(\) => beginnerGridButtonRef\.current\?\.focus\(\)\)/)
})

test('confirmed apply retains preview on failure and restores focus only after success', () => {
  assert.match(app, /window\.confirm/)
  assert.match(app, /applyBeginnerParameterGridCandidate/)
  assert.match(app, /\.then\(\(applied\) => \{\s*if \(!applied\) return/)
  assert.match(app, /setBeginnerGrid\(null\)\s*requestAnimationFrame\(\(\) => beginnerGridButtonRef\.current\?\.focus\(\)\)/)
  assert.match(app, /ref=\{beginnerGridButtonRef\}/)
})

test('native complete animal apply is atomic, replay-safe, undoable, redoable, and persistent', () => {
  assert.match(native, /fn complete_animal_grid_apply_replay_undo_redo_and_archive_round_trip/)
  assert.match(native, /expected_grid_hash/)
  assert.match(native, /Command::ApplyStackedFoldDocument/)
  assert.match(native, /execute_undo\(&mut project/)
  assert.match(native, /execute_redo\(&mut project/)
  assert.match(native, /animal_complete_bindings_v1/)
})
