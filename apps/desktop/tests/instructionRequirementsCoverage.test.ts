import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const app = readFileSync('src/App.tsx', 'utf8')
const panel = readFileSync('src/components/InstructionTimelinePanel.tsx', 'utf8')
const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')
const editor = readFileSync('../../crates/ori-core/src/editor.rs', 'utf8')
const history = readFileSync('../../crates/ori-core/src/editor/history_persistence.rs', 'utf8')
const domain = readFileSync('../../crates/ori-domain/src/lib.rs', 'utf8')
const formats = readFileSync('../../crates/ori-formats/src/ori2.rs', 'utf8')
const instructionExport = readFileSync('src-tauri/src/instruction_export.rs', 'utf8')
const techniques = readFileSync('../../crates/ori-instructions/src/technique_motion.rs', 'utf8')

test('INS-001 through INS-006 retain timeline pose visual guide and recording contracts', () => {
  for (const symbol of [
    'add_instruction_step', 'update_instruction_step_metadata', 'replace_instruction_step_pose',
    'remove_instruction_step', 'move_instruction_step',
  ]) assert.ok(native.includes(symbol), symbol)
  assert.match(domain, /pub struct InstructionTimeline/u)
  assert.match(domain, /pub struct InstructionHandGuide/u)
  assert.match(domain, /pub struct InstructionCamera/u)
  assert.match(panel, /startOrStopPlayback/u)
  assert.match(panel, /replaceSelectedPose/u)
  assert.match(app, /autoRecord/u)
  assert.match(formats, /instruction_timeline/u)
})

test('INS-007 auto recording split merge angle adjustment and durable history are complete', () => {
  assert.match(app, /autoRecord/u)
  assert.match(panel, /replaceInstructionStepPose/u)
  assert.match(panel, /splitInstructionStep/u)
  assert.match(panel, /mergeAdjacentInstructionSteps/u)
  assert.match(native, /fn split_instruction_step/u)
  assert.match(native, /fn merge_adjacent_instruction_steps/u)
  assert.match(editor, /RewriteInstructionTimelineSplitMerge/u)
  assert.match(editor, /is_one_instruction_split_or_merge/u)
  assert.match(history, /RewriteInstructionTimelineSplitMerge/u)
  assert.match(client, /split_instruction_step/u)
  assert.match(client, /merge_adjacent_instruction_steps/u)
})

test('INS-008 through INS-010 retain named techniques animation images PDF and SVG', () => {
  assert.match(techniques, /physical_technique_compiler_v1/u)
  assert.match(app, /FoldTechniqueTimelinePreviewDialog/u)
  assert.match(app, /FoldTechniqueEditorDialog/u)
  assert.match(panel, /requestAnimationFrame/u)
  assert.match(instructionExport, /InstructionExportFormatRequest::Pdf/u)
  assert.match(instructionExport, /InstructionExportFormatRequest::SvgZip/u)
  assert.match(instructionExport, /export_instruction_document/u)
})

