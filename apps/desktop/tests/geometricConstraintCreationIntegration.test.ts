import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const panelSource = readFileSync(
  new URL('../src/components/GeometricConstraintPanel.tsx', import.meta.url),
  'utf8',
)
const clientSource = readFileSync(new URL('../src/lib/coreClient.ts', import.meta.url), 'utf8')
const nativeSource = readFileSync(
  new URL('../src-tauri/src/lib.rs', import.meta.url),
  'utf8',
)

test('all strict constraint kinds reach the native undoable command', () => {
  for (const kind of [
    'fixed_length',
    'fixed_angle',
    'horizontal',
    'vertical',
    'equal_length',
    'parallel',
    'point_on_line',
    'mirror_symmetry',
    'rotational_symmetry',
    'angle_bisector',
    'length_ratio',
  ]) {
    assert.match(panelSource, new RegExp(kind))
  }
  assert.match(panelSource, /normalizeGeometricConstraintKind\(parsed\)/u)
  assert.match(clientSource, /invoke<ProjectSnapshot>\('add_geometric_constraint'/u)
  assert.match(nativeSource, /fn add_geometric_constraint\(/u)
  assert.match(nativeSource, /Command::AddGeometricConstraint/u)
  assert.match(nativeSource, /id: ConstraintId::new\(\)/u)
})
