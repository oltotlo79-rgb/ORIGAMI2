import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = read('../src/App.tsx')
const clientSource = read('../src/lib/coreClient.ts')
const panelSource = read('../src/components/InstructionTimelinePanel.tsx')
const dialogSource = read('../src/components/InstructionExportDialog.tsx')
const nativeSource = read('../src-tauri/src/lib.rs')
const nativeExportSource = read('../src-tauri/src/instruction_export.rs')
const layoutSource = read('../../../crates/ori-formats/src/instruction_export/layout.rs')

test('the current authored timeline opens one background-blocking instruction export flow', () => {
  assert.match(appSource, /\|\| instructionExportOpen/u)
  assert.match(appSource, /\{instructionExportOpen && \(\s*<InstructionExportDialog/u)
  assert.match(
    appSource,
    /<InstructionTimelinePanel[\s\S]*?exportButtonRef=\{instructionExportButtonRef\}[\s\S]*?onExport=\{beginInstructionExport\}/u,
  )
  assert.match(
    appSource,
    /requestAnimationFrame\(\(\) => instructionExportButtonRef\.current\?\.focus\(\)\)/u,
  )
  assert.match(panelSource, /steps\.some\(\(step\) => step\.stale\)/u)
  assert.match(panelSource, /折り図を書き出す/u)
})

test('the last physical saved step is identified as the completed-form thumbnail', () => {
  assert.match(panelSource, /findLast\(\(step\) => !step\.declarativeOnly\)/u)
  assert.match(panelSource, /Completed-form thumbnail/u)
  assert.match(layoutSource, /rposition\(\|step\| !step\.declarative_only\)/u)
  assert.match(layoutSource, /"Completed-form thumbnail"/u)
})

test('preview save and cancel use opaque identity without exposing bytes or a path', () => {
  assert.match(clientSource, /begin_instruction_export/u)
  assert.match(clientSource, /preview_instruction_export/u)
  assert.match(clientSource, /get_instruction_export_progress/u)
  assert.match(clientSource, /save_instruction_export/u)
  assert.match(clientSource, /cancel_instruction_export/u)
  assert.match(nativeSource, /\.manage\(InstructionExportState::default\(\)\)/u)
  assert.match(
    nativeSource,
    /begin_instruction_export,\s*preview_instruction_export,\s*get_instruction_export_progress,\s*save_instruction_export,\s*cancel_instruction_export/u,
  )
  assert.match(
    clientSource,
    /exportId,\s*expectedProjectId,\s*expectedRevision,\s*format/u,
  )
  assert.match(
    clientSource,
    /exportId,\s*expectedProjectId,\s*expectedRevision,\s*warningsAcknowledged/u,
  )
  const preview = section(
    nativeExportSource,
    'struct InstructionExportPreviewSnapshot',
    'pub(super) struct InstructionExportSaveResponse',
  )
  assert.doesNotMatch(preview, /\bbytes?\b|\bpath\b|content|svg|pdf/iu)
})

test('save remains bound to one project revision and requires warning acknowledgement', () => {
  assert.match(
    appSource,
    /current\.project_id !== preview\.expected_project_id\s*\|\|\s*current\.revision !== preview\.expected_revision/u,
  )
  assert.match(appSource, /saveInstructionExport\(\s*preview\.export_id/u)
  assert.match(appSource, /cancelInstructionExport\(preview\.export_id\)/u)
  assert.match(dialogSource, /preview\.warnings\.length === 0 \|\| warningsAcknowledged/u)
  assert.match(dialogSource, /\(busy && !generationActive\)/u)
  assert.match(dialogSource, /生成を中止/u)
  assert.match(dialogSource, /aria-modal="true"/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function section(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.notEqual(startIndex, -1)
  assert.notEqual(endIndex, -1)
  return source.slice(startIndex, endIndex)
}
