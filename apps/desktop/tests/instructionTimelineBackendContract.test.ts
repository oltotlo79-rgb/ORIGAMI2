import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const clientSource = readSource('../src/lib/coreClient.ts')
const nativeSource = readSource('../src-tauri/src/lib.rs')

const commands = [
  ['addInstructionStep', 'add_instruction_step'],
  ['updateInstructionStepMetadata', 'update_instruction_step_metadata'],
  ['replaceInstructionStepPose', 'replace_instruction_step_pose'],
  ['removeInstructionStep', 'remove_instruction_step'],
  ['moveInstructionStep', 'move_instruction_step'],
] as const

test('all instruction commands keep camel-case invoke arguments and registered native handlers', () => {
  for (const [clientName, nativeName] of commands) {
    const clientCommand = exportedFunction(clientName)
    assert.match(clientCommand, new RegExp(`invoke<ProjectSnapshot>\\('${nativeName}'`, 'u'))
    assert.match(nativeSource, new RegExp(`\\n\\s*${nativeName},`, 'u'))
  }

  const add = exportedFunction('addInstructionStep')
  for (const argument of [
    'expectedProjectInstanceId',
    'expectedProjectId',
    'expectedRevision',
    'title',
    'description',
    'caution',
    'durationMs',
    'fixedFace',
    'hingeAngles',
  ]) {
    assert.match(add, new RegExp(`\\b${argument},`, 'u'), argument)
  }
  assert.doesNotMatch(add, /fingerprint|sourceModel/u)

  const replace = exportedFunction('replaceInstructionStepPose')
  assert.match(
    replace,
    /\{\s*expectedProjectInstanceId,\s*expectedProjectId,\s*expectedRevision,\s*stepId,\s*fixedFace,\s*hingeAngles,\s*\}/u,
  )
  assert.doesNotMatch(replace, /fingerprint|sourceModel/u)
})

test('the wire DTO remains snake-case and the native snapshot carries timeline provenance', () => {
  const dtoSection = sourceSection(
    clientSource,
    'export type InstructionHingeAngle',
    'export type NewProjectSettings',
  )
  for (const field of [
    'angle_degrees',
    'source_model_fingerprint',
    'fixed_face',
    'hinge_angles',
    'duration_ms',
  ]) {
    assert.match(dtoSection, new RegExp(`\\b${field}\\b`, 'u'), field)
  }

  const snapshot = sourceSection(
    nativeSource,
    'struct ProjectSnapshot',
    'struct ProjectFileResponse',
  )
  assert.match(snapshot, /instruction_timeline: InstructionTimeline/u)
  assert.match(snapshot, /fold_model_fingerprint: String/u)

  const snapshotBuilder = sourceSection(
    nativeSource,
    'fn snapshot(project: &ProjectState)',
    'fn canceled_file_response',
  )
  assert.match(
    snapshotBuilder,
    /instruction_timeline: project\.editor\.instruction_timeline\(\)\.clone\(\)/u,
  )
  assert.match(
    snapshotBuilder,
    /fold_model_fingerprint: project\.editor\.fold_model_fingerprint_v1\(\)/u,
  )
})

test('only Rust derives pose provenance and persistence includes instruction changes', () => {
  const add = sourceSection(
    nativeSource,
    'async fn add_instruction_step(',
    'fn update_instruction_step_metadata(',
  )
  const replace = sourceSection(
    nativeSource,
    'async fn replace_instruction_step_pose(',
    'fn remove_instruction_step(',
  )
  for (const command of [add, replace]) {
    assert.match(command, /analyze_instruction_pose\(/u)
    assert.match(command, /finish_instruction_pose\(/u)
    assert.doesNotMatch(command, /source_model_fingerprint\s*:/u)
  }

  const finishPose = sourceSection(
    nativeSource,
    'fn finish_instruction_pose(',
    'fn instruction_pose_from_topology(',
  )
  assert.match(finishPose, /is_current_for\(project\.project_id, &project\.editor\)/u)
  assert.match(finishPose, /project\.editor\.fold_model_fingerprint_v1\(\)/u)

  const document = sourceSection(
    nativeSource,
    'fn document(&self)',
    'fn is_dirty(&self)',
  )
  assert.match(
    document,
    /instruction_timeline: self\.editor\.instruction_timeline\(\)\.clone\(\)/u,
  )
  const dirty = sourceSection(
    nativeSource,
    'fn is_dirty(&self)',
    '\n}\n\nfn initial_project_state',
  )
  assert.match(
    dirty,
    /saved\.instruction_timeline != \*self\.editor\.instruction_timeline\(\)/u,
  )
})

function exportedFunction(name: string) {
  const start = `export function ${name}(`
  const startIndex = clientSource.indexOf(start)
  assert.ok(startIndex >= 0, `missing exported function: ${name}`)
  const nextIndex = clientSource.indexOf('\nexport function ', startIndex + start.length)
  return clientSource.slice(startIndex, nextIndex < 0 ? clientSource.length : nextIndex)
}

function sourceSection(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
