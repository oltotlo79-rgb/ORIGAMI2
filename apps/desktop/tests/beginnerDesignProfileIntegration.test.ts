import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const domain = source('../../../crates/ori-domain/src/beginner_design.rs')
const generation = source('../../../crates/ori-domain/src/beginner_generation.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')
const formats = source('../../../crates/ori-formats/src/lib.rs')

test('AUT-103 and AUT-104 expose three bounded on-device scoring presets', () => {
  assert.match(domain, /BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1: u32 = 1/u)
  assert.match(domain, /Balanced[\s\S]*ShapePriority[\s\S]*FoldabilityPriority/u)
  assert.match(domain, /== 100/u)
  assert.match(app, /Beginner design priorities/u)
  assert.match(app, /Shape fidelity priority/u)
  assert.match(app, /Foldability priority/u)
  assert.match(app, /It does not change the current crease pattern/u)
})

test('the profile crosses strict IPC and one native history command', () => {
  assert.match(client, /normalizeBeginnerDesignProfile/u)
  assert.match(client, /exactCoreDataRecord\(value, \[[\s\S]*?'schema_version'/u)
  assert.match(client, /update_beginner_design_profile/u)
  assert.match(native, /fn update_beginner_design_profile/u)
  assert.match(native, /expected_project_instance_id/u)
  assert.match(editor, /Command::UpdateBeginnerDesignProfile/u)
  assert.match(editor, /Inverse::RestoreBeginnerDesignProfile/u)
})

test('the versioned profile is project-saved and recovery-visible', () => {
  assert.match(formats, /pub beginner_design_profile: ori_domain::BeginnerDesignProfileV1/u)
  assert.match(native, /beginner_design_profile: self\.editor\.beginner_design_profile\(\)\.clone\(\)/u)
  assert.match(native, /saved\.beginner_design_profile != \*self\.editor\.beginner_design_profile\(\)/u)
  assert.match(app, /aria-describedby="beginner-design-weights"/u)
})

test('AUT-105 generation constraints share the profile history and strict project boundary', () => {
  assert.match(generation, /pub maximum_steps: u16/u)
  assert.match(generation, /pub detail_level: BeginnerDetailLevelV1/u)
  assert.match(generation, /pub allowed_techniques: Vec<BeginnerFoldTechniqueV1>/u)
  assert.match(client, /'maximum_steps',/u)
  assert.match(client, /record\.allowed_techniques\.length > 8/u)
  assert.match(app, /name="maximum_steps"/u)
  assert.match(app, /name="detail_level"/u)
  assert.match(app, /name="allowed_techniques"/u)
  assert.match(app, /利用可能な折り技法/u)
  assert.match(app, /Allowed fold techniques/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
