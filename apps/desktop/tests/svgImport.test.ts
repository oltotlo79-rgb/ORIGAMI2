import assert from 'node:assert/strict'
import test from 'node:test'
import {
  initialSvgImportMapping,
  isValidSvgImportName,
  isSvgImportTarget,
  isSvgImportLineCap,
  parseSvgImportScale,
  safeSvgStrokeColor,
  svgImportBoundaryIsValid,
  svgImportPreviewBounds,
  svgImportStyleLabel,
  svgImportTargetOptions,
  unresolvedSvgImportGroups,
  type SvgImportPreview,
  type SvgImportStyleGroup,
} from '../src/lib/svgImport.ts'

const groups: readonly SvgImportStyleGroup[] = [
  {
    group_id: 0,
    element_count: 1,
    segment_count: 4,
    stroke: '#000000',
    stroke_color: '#000000',
    dash_array: null,
    line_cap: 'butt',
    classes: ['paper'],
    layer: '外周',
    representative_id: 'paper-outline',
    semantic_hint: 'boundary',
  },
  {
    group_id: 1,
    element_count: 2,
    segment_count: 2,
    stroke: '#ff0000',
    stroke_color: '#ff0000',
    dash_array: '4 2',
    line_cap: 'round',
    classes: ['fold'],
    layer: null,
    representative_id: null,
    semantic_hint: 'mountain',
  },
  {
    group_id: 2,
    element_count: 1,
    segment_count: 1,
    stroke: 'currentColor',
    stroke_color: null,
    dash_array: null,
    line_cap: 'square',
    classes: [],
    layer: null,
    representative_id: null,
    semantic_hint: null,
  },
]

test('SVG import preselects only explicit semantic hints', () => {
  assert.deepEqual(initialSvgImportMapping(groups), {
    0: 'boundary',
    1: 'mountain',
  })
  assert.deepEqual(
    unresolvedSvgImportGroups(groups, initialSvgImportMapping(groups))
      .map(({ group_id }) => group_id),
    [2],
  )
  assert.equal(isSvgImportTarget('cut'), true)
  assert.equal(isSvgImportTarget('unknown'), false)
  assert.equal(isSvgImportLineCap('butt'), true)
  assert.equal(isSvgImportLineCap('round'), true)
  assert.equal(isSvgImportLineCap('square'), true)
  assert.equal(isSvgImportLineCap(undefined), false)
  assert.equal(isSvgImportLineCap('triangle'), false)
})

test('SVG import requires exactly one boundary selection mechanism', () => {
  const preview = makePreview()
  const mapped = { 0: 'boundary', 1: 'mountain', 2: 'ignore' } as const
  const candidate = { 0: 'ignore', 1: 'mountain', 2: 'ignore' } as const

  assert.equal(svgImportBoundaryIsValid(preview, null, mapped), true)
  assert.equal(svgImportBoundaryIsValid(preview, 7, candidate), true)
  assert.equal(svgImportBoundaryIsValid(preview, 8, candidate), false)
  assert.equal(svgImportBoundaryIsValid(preview, 7, mapped), false)
  assert.equal(svgImportBoundaryIsValid(preview, null, candidate), false)
})

test('candidate boundaries remove the conflicting boundary mapping option', () => {
  assert.equal(
    svgImportTargetOptions(null).some(({ value }) => value === 'boundary'),
    true,
  )
  assert.equal(
    svgImportTargetOptions(7).some(({ value }) => value === 'boundary'),
    false,
  )
})

test('SVG style labels remain textual and color previews accept only canonical hex', () => {
  assert.equal(
    svgImportStyleLabel(groups[0]),
    'レイヤー: 外周 / class: paper / 代表ID: paper-outline / 属性: data-origami-kind=boundary / 色: #000000 / 線端: butt',
  )
  assert.equal(safeSvgStrokeColor('#A0b1C2'), '#a0b1c2')
  assert.equal(safeSvgStrokeColor('#a0b1c280'), '#a0b1c280')
  for (const unsafe of ['red', 'rgb(1 2 3)', 'url(https://example.test/a)', '#abcd']) {
    assert.equal(safeSvgStrokeColor(unsafe), null)
  }
})

test('SVG import rejects old or malformed line-cap wire values instead of merging them', () => {
  const mapping = initialSvgImportMapping(groups)
  const oldWireGroup = { ...groups[0], line_cap: undefined as never }
  const unknownWireGroup = { ...groups[1], line_cap: 'triangle' as never }

  assert.deepEqual(
    unresolvedSvgImportGroups([oldWireGroup, unknownWireGroup], mapping)
      .map(({ group_id }) => group_id),
    [0, 1],
  )
})

test('SVG name, scale, and preview bounds share the bounded import contract', () => {
  assert.equal(isValidSvgImportName('作品'), true)
  assert.equal(isValidSvgImportName(''), false)
  assert.equal(parseSvgImportScale('0.2645833333'), 0.2645833333)
  assert.equal(parseSvgImportScale('0'), null)
  assert.equal(parseSvgImportScale('1e309'), null)
  assert.deepEqual(
    svgImportPreviewBounds([{ x: 0, y: 0 }, { x: 210, y: 297 }]),
    { minX: 0, minY: 0, width: 210, height: 297 },
  )
  assert.equal(
    svgImportPreviewBounds([
      { x: -Number.MAX_VALUE, y: 0 },
      { x: Number.MAX_VALUE, y: 1 },
    ]),
    null,
  )
})

function makePreview(): SvgImportPreview {
  return {
    import_id: 'preview',
    file_name: '選択したSVGファイル',
    suggested_name: 'SVG取込',
    default_mm_per_unit: null,
    root_view_box: { x: 0, y: 0, width: 10, height: 10 },
    root_physical_size: {
      width_millimetres: null,
      height_millimetres: null,
      width_unit: null,
      height_unit: null,
    },
    source_segment_count: 7,
    style_groups: groups,
    boundary_candidates: [{
      candidate_id: 7,
      kind: 'closed_shape',
      segment_count: 4,
      width: 10,
      height: 10,
      vertices: [
        { x: 0, y: 0 },
        { x: 10, y: 0 },
        { x: 10, y: 10 },
        { x: 0, y: 10 },
      ],
    }],
    preview_vertices: [],
    preview_edges: [],
    preview_truncated: false,
    warnings: [],
  }
}
