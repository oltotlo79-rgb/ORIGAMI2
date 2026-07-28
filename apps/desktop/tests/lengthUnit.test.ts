import assert from 'node:assert/strict'
import test from 'node:test'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import {
  BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
  BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
} from '../src/lib/boundaryLengthAuthority.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'
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

const INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'
const VERTEX_IDS = [
  '30000000-0000-4000-8000-000000000003',
  '30000000-0000-4000-8000-000000000004',
  '30000000-0000-4000-8000-000000000005',
  '30000000-0000-4000-8000-000000000006',
] as const
const EDGE_IDS = [
  '40000000-0000-4000-8000-000000000003',
  '40000000-0000-4000-8000-000000000004',
  '40000000-0000-4000-8000-000000000005',
  '40000000-0000-4000-8000-000000000006',
] as const

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

test('legacy snapshots keep absolute units but ratio mode fails closed', () => {
  const legacyCentimetres = project('cm')
  delete legacyCentimetres.boundary_length_authority_v1
  const centimetres = resolveLengthDisplayUnit(legacyCentimetres)
  assert.equal(centimetres.mode, 'absolute')
  assert.equal(centimetres.millimetresPerUnit, 10)

  const legacyRatio = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  delete legacyRatio.boundary_length_authority_v1
  const ratio = resolveLengthDisplayUnit(legacyRatio)
  assert.equal(ratio.mode, 'invalid_paper_edge_ratio')
  assert.equal(ratio.millimetresPerUnit, 1)
})

test('paper-edge ratio resolves only a unique valid cyclic boundary edge', () => {
  const snapshot = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  const references = collectBoundaryLengthReferences(snapshot)
  assert.deepEqual(
    references.map((reference) => reference.edgeId),
    EDGE_IDS,
  )

  const unit = resolveLengthDisplayUnit(snapshot)
  assert.equal(unit.mode, 'paper_edge_ratio')
  assert.equal(unit.millimetresPerUnit, 400)
  assert.equal(unit.reference.edgeId, EDGE_IDS[0])
  assert.equal(ratioReferenceAxis(unit), 'width')
  assert.equal(formatLengthInput(200, unit), '0.5')
  assert.equal(formatLength(400, unit, 'ja'), '1 紙辺比')
  assert.equal(formatLength(400, unit, 'en'), '1 paper-edge ratio')
  assert.equal(lengthDisplayUnitLabel(unit, 'ja'), '紙辺比')
  assert.equal(lengthDisplayUnitLabel(unit, 'en'), 'paper-edge ratio')
  assert.equal(formatLengthValue(Number.NaN, unit, 'ja'), '計測不可')
  assert.equal(formatLengthValue(Number.NaN, unit, 'en'), 'Unavailable')
  assert.equal(formatLengthPoint(200, null, unit, 'en'), 'Unavailable')
  assert.equal(
    formatLengthValue(Number.NaN, unit, 'unsupported' as never),
    '計測不可',
  )
  assert.equal(
    lengthDisplayUnitLabel(unit, 'unsupported' as never),
    '紙辺比',
  )

  const vertical = resolveLengthDisplayUnit(
    project(makePaperEdgeRatioUnit(EDGE_IDS[1])),
  )
  assert.equal(vertical.mode, 'paper_edge_ratio')
  assert.equal(vertical.millimetresPerUnit, 200)
  assert.equal(ratioReferenceAxis(vertical), 'height')
})

test('paper-edge ratio uses only refreshed revision-bound native lengths', () => {
  const moved = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  moved.crease_pattern.vertices[1].position.x = 500
  const unchangedAuthority = resolveLengthDisplayUnit(moved)
  assert.equal(unchangedAuthority.mode, 'paper_edge_ratio')
  assert.equal(unchangedAuthority.reference.edgeId, EDGE_IDS[0])
  assert.equal(unchangedAuthority.millimetresPerUnit, 400)

  moved.revision = 1
  assert.equal(
    resolveLengthDisplayUnit(moved).mode,
    'invalid_paper_edge_ratio',
  )
  refreshAuthorityLength(moved, 0, 500)
  const refreshed = resolveLengthDisplayUnit(moved)
  assert.equal(refreshed.mode, 'paper_edge_ratio')
  assert.equal(refreshed.millimetresPerUnit, 500)

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
  const duplicateId = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  duplicateId.crease_pattern.edges.push({
    id: EDGE_IDS[0],
    start: VERTEX_IDS[2],
    end: VERTEX_IDS[3],
    kind: 'boundary',
  })
  assert.equal(
    resolveLengthDisplayUnit(duplicateId).mode,
    'invalid_paper_edge_ratio',
  )

  const duplicateCarrier = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  duplicateCarrier.crease_pattern.edges.push({
    id: '40000000-0000-4000-8000-000000000007',
    start: VERTEX_IDS[0],
    end: VERTEX_IDS[1],
    kind: 'boundary',
  })
  assert.equal(
    resolveLengthDisplayUnit(duplicateCarrier).mode,
    'invalid_paper_edge_ratio',
  )

  const zero = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  refreshAuthorityLength(zero, 0, 0)
  assert.equal(
    resolveLengthDisplayUnit(zero).mode,
    'invalid_paper_edge_ratio',
  )
})

test('boundary authority parser rejects accessors without invoking them', () => {
  const snapshot = project(makePaperEdgeRatioUnit(EDGE_IDS[0]))
  let kindReads = 0
  Object.defineProperty(snapshot.crease_pattern.edges[0], 'kind', {
    enumerable: true,
    get() {
      kindReads += 1
      return 'boundary'
    },
  })

  const references = collectBoundaryLengthReferences(snapshot)

  assert.equal(references.length, 0)
  assert.equal(kindReads, 0)
})

function project(
  unit: ProjectSnapshot['paper']['length_display_unit'],
): ProjectSnapshot {
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    name: 'test',
    current_path: null,
    revision: 0,
    saved_revision: 0,
    is_dirty: false,
    crease_pattern: {
      vertices: [
        { id: VERTEX_IDS[0], position: { x: 0, y: 0 } },
        { id: VERTEX_IDS[1], position: { x: 400, y: 0 } },
        { id: VERTEX_IDS[2], position: { x: 400, y: 200 } },
        { id: VERTEX_IDS[3], position: { x: 0, y: 200 } },
      ],
      edges: [
        { id: EDGE_IDS[0], start: VERTEX_IDS[0], end: VERTEX_IDS[1], kind: 'boundary' },
        { id: EDGE_IDS[1], start: VERTEX_IDS[1], end: VERTEX_IDS[2], kind: 'boundary' },
        { id: EDGE_IDS[2], start: VERTEX_IDS[2], end: VERTEX_IDS[3], kind: 'boundary' },
        { id: EDGE_IDS[3], start: VERTEX_IDS[3], end: VERTEX_IDS[0], kind: 'boundary' },
      ],
    },
    paper: {
      boundary_vertices: [...VERTEX_IDS],
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
    boundary_length_authority_v1: boundaryLengthAuthority(0),
  }
}

function boundaryLengthAuthority(revision: number) {
  const lengths = [400, 200, 400, 200]
  return {
    schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
    model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision,
    status: 'available',
    entries: VERTEX_IDS.map((start, index) => ({
      boundary_index: index,
      edge_id: EDGE_IDS[index],
      start_vertex_id: start,
      end_vertex_id: VERTEX_IDS[(index + 1) % VERTEX_IDS.length],
      length_mm: lengths[index],
      length_bits_be: float64Bytes(lengths[index]),
    })),
  }
}

function refreshAuthorityLength(
  snapshot: ProjectSnapshot,
  index: number,
  lengthMm: number,
) {
  const authority = snapshot.boundary_length_authority_v1 as ReturnType<
    typeof boundaryLengthAuthority
  >
  authority.revision = snapshot.revision
  authority.entries[index].length_mm = lengthMm
  authority.entries[index].length_bits_be = float64Bytes(lengthMm)
}

function float64Bytes(value: number): number[] {
  const buffer = new ArrayBuffer(8)
  new DataView(buffer).setFloat64(0, value, false)
  return [...new Uint8Array(buffer)]
}
