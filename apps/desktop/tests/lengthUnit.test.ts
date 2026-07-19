import assert from 'node:assert/strict'
import test from 'node:test'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import {
  collectBoundaryLengthReferences,
  formatLength,
  formatLengthInput,
  formatLengthPoint,
  formatLengthValue,
  lengthDisplayUnitLabel,
  lengthDisplayToMillimetres,
  lengthMillimetresToDisplay,
  makePaperEdgeRatioUnit,
  ratioReferenceAxis,
  resolveLengthDisplayUnit,
} from '../src/lib/lengthUnit.ts'

test('absolute display units convert to and from the millimetre model boundary', () => {
  for (const [stored, expectedScale, expectedLabel] of [
    ['mm', 1, 'mm'],
    ['cm', 10, 'cm'],
    ['inch', 25.4, 'in'],
  ] as const) {
    const unit = resolveLengthDisplayUnit(project(stored))
    assert.equal(unit.mode, 'absolute')
    assert.equal(unit.millimetresPerUnit, expectedScale)
    assert.equal(unit.label, expectedLabel)
    assert.equal(
      lengthDisplayToMillimetres(
        lengthMillimetresToDisplay(254, unit),
        unit,
      ),
      254,
    )
  }
})

test('paper-edge ratio resolves only a unique valid cyclic boundary edge', () => {
  const snapshot = project(makePaperEdgeRatioUnit('e-top'))
  const references = collectBoundaryLengthReferences(snapshot)
  assert.deepEqual(
    references.map((reference) => reference.edgeId),
    ['e-top', 'e-right', 'e-bottom', 'e-left'],
  )

  const unit = resolveLengthDisplayUnit(snapshot, references)
  assert.equal(unit.mode, 'paper_edge_ratio')
  assert.equal(unit.millimetresPerUnit, 400)
  assert.equal(unit.reference.edgeId, 'e-top')
  assert.equal(ratioReferenceAxis(unit), 'width')
  assert.equal(formatLengthInput(200, unit), '0.5')
  assert.equal(formatLength(400, unit, 'ja'), '1 紙辺比')
  assert.equal(formatLength(400, unit, 'en'), '1 paper-edge ratio')
  assert.equal(lengthDisplayUnitLabel(unit, 'ja'), '紙辺比')
  assert.equal(lengthDisplayUnitLabel(unit, 'en'), 'paper-edge ratio')
  assert.equal(formatLengthValue(Number.NaN, unit, 'ja'), '計測不可')
  assert.equal(formatLengthValue(Number.NaN, unit, 'en'), 'Unavailable')
  assert.equal(formatLengthPoint(200, null, unit, 'en'), 'Unavailable')

  const vertical = resolveLengthDisplayUnit(
    project(makePaperEdgeRatioUnit('e-right')),
  )
  assert.equal(vertical.mode, 'paper_edge_ratio')
  assert.equal(vertical.millimetresPerUnit, 200)
  assert.equal(ratioReferenceAxis(vertical), 'height')
})

test('paper-edge ratio scale follows the same edge and never silently rebases', () => {
  const moved = project(makePaperEdgeRatioUnit('e-top'))
  moved.crease_pattern.vertices[1].position.x = 500
  const movedUnit = resolveLengthDisplayUnit(moved)
  assert.equal(movedUnit.mode, 'paper_edge_ratio')
  assert.equal(movedUnit.reference.edgeId, 'e-top')
  assert.equal(movedUnit.millimetresPerUnit, 500)

  const missing = project(makePaperEdgeRatioUnit('deleted-edge'))
  const invalid = resolveLengthDisplayUnit(missing)
  assert.equal(invalid.mode, 'invalid_paper_edge_ratio')
  assert.equal(invalid.invalidReferenceEdgeId, 'deleted-edge')
  assert.equal(invalid.effectiveUnit, 'mm')
  assert.equal(invalid.millimetresPerUnit, 1)
  assert.equal(formatLength(12.5, invalid, 'ja'), '12.5 mm')
  assert.equal(formatLength(12.5, invalid, 'en'), '12.5 mm')
})

test('ambiguous IDs, duplicated carrier segments and zero lengths fail closed', () => {
  const duplicateId = project(makePaperEdgeRatioUnit('e-top'))
  duplicateId.crease_pattern.edges.push({
    id: 'e-top',
    start: 'v2',
    end: 'v3',
    kind: 'boundary',
  })
  assert.equal(
    resolveLengthDisplayUnit(duplicateId).mode,
    'invalid_paper_edge_ratio',
  )

  const duplicateCarrier = project(makePaperEdgeRatioUnit('e-top'))
  duplicateCarrier.crease_pattern.edges.push({
    id: 'another-top',
    start: 'v0',
    end: 'v1',
    kind: 'boundary',
  })
  assert.equal(
    resolveLengthDisplayUnit(duplicateCarrier).mode,
    'invalid_paper_edge_ratio',
  )

  const zero = project(makePaperEdgeRatioUnit('e-top'))
  zero.crease_pattern.vertices[1].position = { x: 0, y: 0 }
  assert.equal(
    resolveLengthDisplayUnit(zero).mode,
    'invalid_paper_edge_ratio',
  )
})

test('boundary reference collection reads every edge kind only once', () => {
  const count = 1_024
  const snapshot = project('mm')
  snapshot.paper.boundary_vertices = Array.from(
    { length: count },
    (_, index) => `large-v-${index}`,
  )
  snapshot.crease_pattern.vertices = snapshot.paper.boundary_vertices.map(
    (id, index) => ({
      id,
      position: {
        x: Math.cos((index / count) * Math.PI * 2) * 400,
        y: Math.sin((index / count) * Math.PI * 2) * 400,
      },
    }),
  )
  let kindReads = 0
  snapshot.crease_pattern.edges = snapshot.paper.boundary_vertices.map(
    (start, index) => {
      const edge = {
        id: `large-e-${index}`,
        start,
        end: snapshot.paper.boundary_vertices[(index + 1) % count],
      } as ProjectSnapshot['crease_pattern']['edges'][number]
      Object.defineProperty(edge, 'kind', {
        enumerable: true,
        get() {
          kindReads += 1
          return 'boundary'
        },
      })
      return edge
    },
  )

  const references = collectBoundaryLengthReferences(snapshot)

  assert.equal(references.length, count)
  assert.equal(kindReads, count)
})

function project(
  unit: ProjectSnapshot['paper']['length_display_unit'],
): ProjectSnapshot {
  return {
    project_instance_id: 'instance',
    project_id: 'project',
    name: 'test',
    current_path: null,
    revision: 0,
    saved_revision: 0,
    is_dirty: false,
    crease_pattern: {
      vertices: [
        { id: 'v0', position: { x: 0, y: 0 } },
        { id: 'v1', position: { x: 400, y: 0 } },
        { id: 'v2', position: { x: 400, y: 200 } },
        { id: 'v3', position: { x: 0, y: 200 } },
      ],
      edges: [
        { id: 'e-top', start: 'v0', end: 'v1', kind: 'boundary' },
        { id: 'e-right', start: 'v1', end: 'v2', kind: 'boundary' },
        { id: 'e-bottom', start: 'v2', end: 'v3', kind: 'boundary' },
        { id: 'e-left', start: 'v3', end: 'v0', kind: 'boundary' },
      ],
    },
    paper: {
      boundary_vertices: ['v0', 'v1', 'v2', 'v3'],
      thickness_mm: 0.1,
      length_display_unit: unit,
      cutting_allowed: false,
      front: {
        color: { red: 255, green: 255, blue: 255, alpha: 255 },
        texture_asset: null,
      },
      back: {
        color: { red: 248, green: 248, blue: 245, alpha: 255 },
        texture_asset: null,
      },
    },
    can_undo: false,
    can_redo: false,
    cutting_allowed: false,
    instruction_timeline: { steps: [] },
    fold_model_fingerprint: 'fingerprint',
  }
}
