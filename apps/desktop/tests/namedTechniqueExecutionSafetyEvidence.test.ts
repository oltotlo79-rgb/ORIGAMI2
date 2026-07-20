import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const technique = read('../../../crates/ori-instructions/src/fold_technique_file.rs')
const native = read('../src-tauri/src/lib.rs')
const dialog = read('../src/components/FoldTechniqueTimelinePreviewDialog.tsx')

test('INS-008 keeps every unbound physical technique inert at the project boundary', () => {
  assert.match(technique, /pub const fn grants_project_mutation_authority\(&self\) -> bool \{\s*false/u)
  assert.match(technique, /StraightLineStackedFold/u)
  assert.match(technique, /FoldTechniqueExecutionSupportV1::DeclarativeOnly/u)
  assert.match(technique, /UnsupportedPhysicalOperation/u)
  assert.doesNotMatch(technique, /SupportedPhysicalOperation/u)
  assert.match(native, /InstructionPoseModel::DeclarativeOnlyV1/u)
  assert.match(native, /hinge_angles: Vec::new\(\)/u)
  assert.match(native, /fixed_face: None/u)
})

test('the UI requires confirmation and discloses that no 3D motion is executed', () => {
  assert.match(dialog, /confirm/u)
  assert.match(dialog, /3D/u)
  assert.match(dialog, /no physical command[\s\S]*is executed/u)
  assert.match(dialog, /stale/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
