import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const panel = read('../src/components/InstructionTimelinePanel.tsx')
const client = read('../src/lib/coreClient.ts')
const native = read('../src-tauri/src/lib.rs')
const editor = read('../../../crates/ori-core/src/editor.rs')
const history = read('../../../crates/ori-core/src/editor/history_persistence.rs')
const formats = read('../../../crates/ori-formats/src/lib.rs')
const ori2 = read('../../../crates/ori-formats/src/ori2.rs')
const folder = read('../../../crates/ori-formats/src/project_folder.rs')

test('INS-005 and INS-006 production UI reaches every bound timeline edit command', () => {
  for (const operation of [
    'addInstructionStep', 'updateInstructionStepMetadata', 'replaceInstructionStepPose',
    'removeInstructionStep', 'moveInstructionStep',
  ]) assert.match(panel, new RegExp(`${operation}\\(`, 'u'))
  for (const command of [
    'add_instruction_step', 'update_instruction_step_metadata',
    'replace_instruction_step_pose', 'remove_instruction_step', 'move_instruction_step',
  ]) {
    assert.match(client, new RegExp(`'${command}'`, 'u'))
    assert.match(native, new RegExp(`fn ${command}\\(`, 'u'))
  }
  assert.match(native, /expected_project_instance_id/u)
  assert.match(native, /expected_revision/u)
})

test('INS-007 every edit has inverse history and survives all project stores', () => {
  for (const inverse of [
    'RemoveAddedInstructionStep', 'RestoreInstructionStepMetadata',
    'RestoreInstructionStepPose', 'RestoreRemovedInstructionStep',
    'RestoreInstructionStepOrder', 'RemoveAppendedInstructionSteps',
  ]) {
    assert.match(editor, new RegExp(`Inverse::${inverse}`, 'u'))
    assert.match(history, new RegExp(inverse, 'u'))
  }
  assert.match(formats, /pub instruction_timeline: InstructionTimeline/u)
  assert.match(ori2, /instruction_timeline/u)
  assert.match(folder, /instruction_timeline/u)
  assert.match(native, /recovery[\s\S]*instruction_timeline/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
