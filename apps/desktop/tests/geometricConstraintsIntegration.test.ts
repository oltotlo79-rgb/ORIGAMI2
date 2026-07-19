import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const panel = source('../src/components/GeometricConstraintPanel.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')
const formats = source('../../../crates/ori-formats/src/lib.rs')
const ori2 = source('../../../crates/ori-formats/src/ori2.rs')

test('constraint commands use instance, document, and revision bindings end to end', () => {
  for (const command of [
    'analyze_geometric_constraints',
    'add_edge_orientation_constraint',
    'remove_geometric_constraint',
  ]) {
    assert.match(client, new RegExp(`'${command}'`, 'u'))
    assert.match(native, new RegExp(`\\n\\s*${command},`, 'u'))
  }
  for (const clientFunction of [
    functionSection(client, 'export function analyzeGeometricConstraints(', 'export function openProject('),
    functionSection(client, 'export function addEdgeOrientationConstraint(', 'export function removeGeometricConstraint('),
    functionSection(client, 'export function removeGeometricConstraint(', 'export function undo('),
  ]) {
    assert.match(clientFunction, /expectedProjectInstanceId/u)
    assert.match(clientFunction, /expectedProjectId/u)
    assert.match(clientFunction, /expectedRevision/u)
  }
  assert.match(
    native,
    /ensure_expected_project\(\s*&project,\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,/u,
  )
  assert.match(app, /current\.project_instance_id[\s\S]*?response\.project_instance_id/u)
  assert.match(
    app,
    /latestSnapshotRef\.current !== current[\s\S]*?!isExpectedNativeEditSnapshot\(/u,
  )
  assert.match(
    app,
    /snapshot\.geometric_constraints === undefined[\s\S]*?: snapshot\.geometric_constraints/u,
  )
  assert.doesNotMatch(app, /snapshot\.geometric_constraints \?\?/u)
})

test('constraints are editor-owned, dirty-tracked, snapshotted, and persisted', () => {
  assert.match(editor, /geometric_constraints:\s*GeometricConstraintDocumentV1/u)
  assert.match(editor, /AddGeometricConstraint/u)
  assert.match(editor, /RemoveGeometricConstraint/u)
  assert.match(native, /geometric_constraints:\s*self\.editor\.geometric_constraints\(\)\.clone\(\)/u)
  assert.match(native, /saved\.geometric_constraints\s*!=\s*\*self\.editor\.geometric_constraints\(\)/u)
  assert.match(
    native,
    /geometric_constraints:\s*project\.editor\.geometric_constraints\(\)\.clone\(\)/u,
  )
  assert.match(formats, /pub geometric_constraints:\s*GeometricConstraintDocumentV1/u)
  assert.match(ori2, /ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1/u)
  assert.match(
    ori2,
    /!document\.geometric_constraints\.is_empty\(\)[\s\S]*?required_features\.push\(ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1/u,
  )
})

test('the visible panel never upgrades unknown or direct conflict to a safe result', () => {
  assert.match(app, /<GeometricConstraintPanel/u)
  assert.match(panel, /preflight\?\.status === 'direct_conflict'/u)
  assert.match(panel, /preflight\?\.status === 'unknown'/u)
  assert.match(panel, /className = 'is-blocking'/u)
  assert.match(panel, /安全確認済みとして扱いません/u)
  assert.match(
    panel,
    /直接矛盾は見つかりません（全制約の充足可能性は未証明）/u,
  )
  assert.doesNotMatch(panel, /制約を満たしています|安全です/u)
})

function functionSection(text: string, start: string, end: string) {
  const startIndex = text.indexOf(start)
  const endIndex = text.indexOf(end, startIndex + start.length)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `${start} section`)
  return text.slice(startIndex, endIndex)
}

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
