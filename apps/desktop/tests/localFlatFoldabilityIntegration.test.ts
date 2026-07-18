import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const canvasSource = readSource('../src/components/CreaseCanvas.tsx')
const clientSource = readSource('../src/lib/coreClient.ts')
const presentationSource = readSource('../src/lib/localFlatFoldabilityPresentation.ts')
const cssSource = readSource('../src/App.css')

test('the native validation snapshot keeps geometry and local flat-foldability separate', () => {
  assert.match(
    clientSource,
    /local_flat_foldability: LocalFlatFoldabilityReport/u,
  )
  assert.match(appSource, /validation\.is_valid/u)
  assert.match(
    appSource,
    /validation\.local_flat_foldability/u,
  )
  assert.match(
    appSource,
    /createLocalFlatFoldabilityPresentation\(\s*result\.local_flat_foldability/u,
  )
  assert.match(
    appSource,
    /result\.project_id !== latest\.project_id[\s\S]*result\.revision !== latest\.revision/u,
  )
})

test('snapshot replacement, validation errors, and benchmark mode cannot retain local halos', () => {
  const applySnapshot = sourceSection(
    appSource,
    'const applySnapshot = useCallback',
    'const nativeLines = useMemo',
  )
  assert.match(applySnapshot, /setValidation\(null\)/u)
  assert.match(
    appSource,
    /const canvasLocalFlatFoldabilityHighlights = !benchmarkRun/u,
  )
  assert.match(
    appSource,
    /disabled=\{coreBusy \|\| benchmarkLoading \|\| Boolean\(benchmarkRun\) \|\| !nativeSnapshot\}/u,
  )
  assert.match(appSource, /setValidation\(null\)[\s\S]*setCoreStatus\(`検証エラー/u)
})

test('the inspector exposes bounded, selectable, non-color-only local results', () => {
  assert.match(appSource, />局所平坦折り条件</u)
  assert.match(appSource, /role="status"/u)
  assert.match(appSource, /aria-live="polite"/u)
  assert.match(appSource, /aria-atomic="true"/u)
  assert.match(appSource, /aria-pressed=\{selectedVertexId === item\.vertexId\}/u)
  assert.match(appSource, /川崎条件/u)
  assert.match(appSource, /前川条件/u)
  assert.match(appSource, /確認が必要な頂点/u)
  assert.match(appSource, /展開図全体が平坦に折り畳めることや、実際の折り経路は保証しません/u)
  assert.match(
    presentationSource,
    /LOCAL_FLAT_FOLDABILITY_VISIBLE_ITEM_LIMIT = 20/u,
  )
  assert.match(cssSource, /\.local-verdict\.is-violated/u)
  assert.match(cssSource, /\.local-verdict\.is-indeterminate/u)
  assert.match(cssSource, /\.local-flat-foldability-items button:focus-visible/u)
})

test('canvas renders exactly two O(V) validation halo batches and links its description', () => {
  const mappedVertexRendering = sourceSection(
    canvasSource,
    'const mappedVertices:',
    'drawVertexHaloBatch(',
  )
  assert.equal(
    mappedVertexRendering.match(/drawValidationVertexHaloBatch\(/gu)?.length,
    2,
  )
  assert.match(mappedVertexRendering, /'indeterminate'/u)
  assert.match(mappedVertexRendering, /'violated'/u)

  const validationHalo = sourceSection(
    canvasSource,
    'function drawValidationVertexHaloBatch(',
    'function traceCircle(',
  )
  assert.match(validationHalo, /for \(const vertex of vertices\)/u)
  assert.match(validationHalo, /highlights\.get\(vertex\.id\) !== severity/u)
  assert.equal(validationHalo.match(/context\.stroke\(\)/gu)?.length, 1)
  assert.match(validationHalo, /severity === 'violated' \? \[\] : \[4, 3\]/u)
  assert.match(canvasSource, /aria-describedby=\{ariaDescribedBy\}/u)
  assert.match(appSource, /ariaDescribedBy=\{localFlatFoldabilitySummaryId\}/u)
})

test('the presentation boundary is exact, fail-closed, and linear by construction', () => {
  assert.match(
    presentationSource,
    /Object\.getOwnPropertyDescriptors\(value\)/u,
  )
  assert.match(presentationSource, /verticesById\.has\(vertexId\)/u)
  assert.match(presentationSource, /verticesById\.size !== projectVertexIds\.ids\.length/u)
  assert.match(presentationSource, /foldDegree !== assignmentCount/u)
  assert.match(presentationSource, /input\.foldDegree > input\.maxExactFoldDegree/u)
  assert.doesNotMatch(
    sourceSection(
      presentationSource,
      'for (const rawVertex of rawVertices)',
      'const visibleItems:',
      presentationSource.indexOf('for (const rawVertex of rawVertices)'),
    ),
    /currentProjectVertexIds\.(find|some|filter)\(/u,
  )
})

function sourceSection(
  source: string,
  start: string,
  end: string,
  fromIndex = 0,
) {
  const startIndex = source.indexOf(start, fromIndex)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
