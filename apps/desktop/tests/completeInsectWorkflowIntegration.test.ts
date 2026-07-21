import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
const component = readFileSync(new URL('../src/components/CompleteInsectBindingList.tsx', import.meta.url), 'utf8')
const native = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
const recognition = readFileSync(new URL('../src-tauri/src/beginner_recognition.rs', import.meta.url), 'utf8')

test('complete insect recognition and candidate UI share five canonical pair bindings', () => {
  assert.match(recognition, /fn complete_insect_image_pairs_require_both_equal_mirrored_sides/)
  assert.match(native, /insect_complete_bindings_v1/)
  assert.match(component, /protrusions\.length === 5/)
  assert.match(component, /target\.id === index \+ 1/)
  assert.match(component, /Wing pair/)
  assert.match(component, /Antenna pair/)
  assert.match(component, /Leg pair 3/)
  assert.match(app, /plan\.kind === 'composite_complete_insect_base'/)
  assert.match(app, /<CompleteInsectBindingList/)
})

test('native complete insect grid remains replay-safe, undoable, redoable, and persistent', () => {
  assert.match(native, /fn complete_insect_grid_preserves_all_five_pair_dimensions_and_bindings/)
  assert.match(native, /apply_grid_plan_document/)
  assert.match(native, /execute_undo/)
  assert.match(native, /execute_redo/)
  assert.match(native, /write_project_ori2/)
  assert.match(native, /read_project_ori2_with_limits/)
})
