import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const native = source('../src-tauri/src/lib.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')

test('mirror preview and apply share one immutable identity-bound request', () => {
  assert.match(app, /binding: string[\s\S]*?request: MirrorSelectionRequest/u)
  assert.match(
    app,
    /project_instance_id,[\s\S]*?project_id,[\s\S]*?revision,[\s\S]*?join\(':'\)/u,
  )
  assert.match(app, /latest !== current/u)
  assert.match(app, /binding !== preview\.binding/u)
  assert.match(app, /preview\.request/u)
  assert.match(app, /mirrorOperationRef\.current/u)
  assert.match(app, /mirrorRequestSequenceRef\.current \+= 1/u)
})

test('native preflight executes the same dedicated core command as apply', () => {
  const commands = native.match(/Command::MirrorSelection \{/gu) ?? []
  assert.ok(commands.length >= 2)
  assert.doesNotMatch(native, /fn build_mirrored_selection/u)
  assert.match(native, /let mut probe = project\.editor\.clone\(\)/u)
  assert.match(native, /probe[\s\S]*?\.execute\(/u)
})

test('core mirror admission closes every mutation and identity escape', () => {
  assert.match(editor, /!canonical\(vertices\)/u)
  assert.match(editor, /!canonical\(new_vertices\)/u)
  assert.match(editor, /paper\.boundary_vertices\.contains\(id\)/u)
  assert.match(editor, /CommandError::LayerLocked\(layer\)/u)
  assert.match(editor, /CommandError::VertexAlreadyExists/u)
  assert.match(editor, /CommandError::EdgeAlreadyExists/u)
  assert.match(editor, /validate_crease_pattern\(&target\)/u)
  assert.match(editor, /Inverse::RestoreMirrorSelection/u)
})

test('mirror failures are translated from a closed vocabulary', () => {
  assert.match(app, /function mirrorPreflightIssueText/u)
  assert.match(app, /case 'invalid_axis'/u)
  assert.match(app, /case 'core_rejected'/u)
  assert.doesNotMatch(
    app,
    /Cannot apply: \{issue\}/u,
  )
  assert.match(app, /aria-labelledby="mirror-selection-heading"/u)
  assert.match(app, /role="status"/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
