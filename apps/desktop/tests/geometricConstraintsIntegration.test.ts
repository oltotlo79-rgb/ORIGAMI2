import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  GEOMETRIC_CONSTRAINT_PANEL_TEXT,
} from '../src/lib/geometricConstraintPanelText.ts'

const app = source('../src/App.tsx')
const panel = source('../src/components/GeometricConstraintPanel.tsx')
const panelText = source('../src/lib/geometricConstraintPanelText.ts')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')
const formats = source('../../../crates/ori-formats/src/lib.rs')
const ori2 = source('../../../crates/ori-formats/src/ori2.rs')

test('constraint commands use instance, document, and revision bindings end to end', () => {
  for (const command of [
    'analyze_geometric_constraints',
    'cancel_geometric_constraint_analysis',
    'add_edge_orientation_constraint',
    'remove_geometric_constraint',
  ]) {
    assert.match(client, new RegExp(`'${command}'`, 'u'))
    assert.match(native, new RegExp(`\\n\\s*${command},`, 'u'))
  }
  for (const clientFunction of [
    functionSection(client, 'export function analyzeGeometricConstraints(', 'export function cancelGeometricConstraintAnalysis('),
    functionSection(client, 'export function cancelGeometricConstraintAnalysis(', 'export function previewCreasePatternExport('),
    functionSection(client, 'export function addEdgeOrientationConstraint(', 'export function removeGeometricConstraint('),
    functionSection(client, 'export function removeGeometricConstraint(', 'export function undo('),
  ]) {
    assert.match(clientFunction, /expectedProjectInstanceId/u)
    assert.match(clientFunction, /expectedProjectId/u)
    assert.match(clientFunction, /expectedRevision/u)
  }
  for (const clientFunction of [
    functionSection(client, 'export function analyzeGeometricConstraints(', 'export function cancelGeometricConstraintAnalysis('),
    functionSection(client, 'export function cancelGeometricConstraintAnalysis(', 'export function previewCreasePatternExport('),
  ]) {
    assert.match(clientFunction, /requestGenerationId/u)
  }
  assert.match(
    native,
    /ensure_project_expectation\(\s*project,\s*ProjectExpectation::new\(\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,\s*\),\s*\)\?/u,
  )
  const workerGate = functionSection(
    native,
    'impl GeometricConstraintWorkerGate {',
    'struct GeometricConstraintWorkerPermit {',
  )
  assert.match(
    workerGate,
    /let key = GeometricConstraintWorkerKey \{\s*binding,\s*request_generation_id,\s*\};/u,
  )
  assert.match(
    workerGate,
    /state\.active\.as_ref\(\)\.filter\(\|slot\| slot\.key == key\)[\s\S]*?slot\.cancellation\.store\(true, Ordering::Release\);[\s\S]*?return true;/u,
  )
  assert.match(
    workerGate,
    /state\s*\.pre_cancelled\s*\.iter\(\)\s*\.position\(\|candidate\| \*candidate == key\)\s*\.and_then\(\|index\| state\.pre_cancelled\.remove\(index\)\)\s*\.is_some\(\);[\s\S]*?AtomicBool::new\(pre_cancelled\)/u,
  )
  assert.match(
    workerGate,
    /\.all\(\|candidate\| \*candidate != key\)[\s\S]*?state\.pre_cancelled\.len\(\) >= MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS[\s\S]*?state\.pre_cancelled\.pop_front\(\);[\s\S]*?state\.pre_cancelled\.push_back\(key\);/u,
  )
  assert.match(
    native,
    /const MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS: usize = 64;/u,
  )
  assert.match(
    native,
    /fn cancel_geometric_constraint_analysis\(\s*state: State<'_, AppState>,\s*expected_project_instance_id: ProjectId,\s*expected_project_id: ProjectId,\s*expected_revision: u64,\s*request_generation_id: ProjectId,\s*\) -> bool \{[\s\S]*?state\.cancel_geometric_constraint_worker\(\s*GeometricConstraintAnalysisBinding \{\s*project_instance_id: expected_project_instance_id,\s*project_id: expected_project_id,\s*revision: expected_revision,\s*\},\s*request_generation_id,\s*\)/u,
  )
  assert.match(
    native,
    /fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active\(\)[\s\S]*?!gate\.cancel\(binding, queued_generation\)[\s\S]*?!active\.cancellation\.load\(Ordering::Acquire\)[\s\S]*?try_acquire\(binding, queued_generation\)[\s\S]*?queued\.cancellation\.load\(Ordering::Acquire\)/u,
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
  assert.match(panelText, /安全確認済みとして扱いません/u)
  assert.match(
    panelText,
    /直接矛盾は見つかりません（全制約の充足可能性は未証明）/u,
  )
  assert.doesNotMatch(panelText, /制約を満たしています|安全です/u)
  assert.match(panel, /GEOMETRIC_CONSTRAINT_PANEL_TEXT as TEXT/u)
  assert.doesNotMatch(panel, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(panel, /\blocalized\s*\(/u)
  assert.doesNotMatch(panel, /formatLocalizedText\(locale,\s*\{/u)
  assert.doesNotMatch(
    panel,
    />\s*(?:X \(mm\)|Y \(mm\)|residual:|rank\s|DOF\s|condition\s)/u,
  )
  for (const displayText of collectStrings(
    GEOMETRIC_CONSTRAINT_PANEL_TEXT,
  )) {
    if (displayText === ', ') continue
    assert.equal(
      panel.includes(`'${displayText}'`),
      false,
      `inline constraint-panel display text: ${displayText}`,
    )
  }
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

function collectStrings(value: unknown): string[] {
  if (typeof value === 'string') return [value]
  if (typeof value !== 'object' || value === null) return []
  return Object.values(value).flatMap(collectStrings)
}
