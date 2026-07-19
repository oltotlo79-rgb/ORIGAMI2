import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = read('../src/App.tsx')
const preview = read('../src/components/FoldPreview.tsx')
const thickness = read('../src/components/PaperThicknessInput.tsx')

test('App applies one snapshot display unit to every authoring length surface', () => {
  assert.match(app, /<LengthUnitControl/u)
  assert.match(
    app,
    /setLengthDisplayUnit\(projectId, revision, projectInstanceId, unit\)/u,
  )
  assert.match(
    app,
    /paperSizeLabel[\s\S]*?formatLengthValue\([\s\S]*?lengthDisplayUnit,[\s\S]*?locale,[\s\S]*?formatLength\([\s\S]*?lengthDisplayUnit,[\s\S]*?locale/u,
  )
  assert.match(app, /measurementLabel=\{formatLineMeasurementLabel\([\s\S]*?displayedLengthUnit/u)
  assert.match(app, /formatLengthPoint\([\s\S]*?displayedLengthUnit,[\s\S]*?locale/u)
  assert.match(app, /name="x_display"[\s\S]*?unit=\{lengthDisplayUnit\}/u)
  assert.match(app, /name="y_display"[\s\S]*?unit=\{lengthDisplayUnit\}/u)
  assert.match(app, /name="thickness_display"[\s\S]*?unit=\{lengthDisplayUnit\}/u)
  assert.match(app, /name="width_display"[\s\S]*?unit=\{lengthDisplayUnit\}/u)
  assert.match(app, /name="height_display"[\s\S]*?unit=\{lengthDisplayUnit\}/u)
  assert.match(app, /lengthDisplayUnit=\{lengthDisplayUnit\}/u)
  assert.match(
    preview,
    /formatLength\(safeThicknessMm, lengthDisplayUnit, locale\)/u,
  )
  assert.match(
    app,
    /lengthDisplayUnitLabel\(\s*lengthDisplayUnit,\s*locale,\s*\)/u,
  )
  assert.doesNotMatch(app, /lengthDisplayUnit\.label/u)
})

test('converted editing crosses the native boundary only as millimetres', () => {
  assert.match(
    app,
    /readLengthInputMillimetres\([\s\S]*?'x_display'[\s\S]*?moveVertex\([\s\S]*?x, y/u,
  )
  assert.match(
    app,
    /readLengthInputMillimetres\([\s\S]*?'thickness_display'[\s\S]*?thicknessMm/u,
  )
  assert.match(
    app,
    /readLengthInputMillimetres\([\s\S]*?'width_display'[\s\S]*?resizeRectangularPaper\([\s\S]*?widthMm, heightMm/u,
  )
  assert.doesNotMatch(app, /resizeRectangularPaper\([\s\S]{0,160}width_display/u)
})

test('paper thickness stepping stays a physical 0.01 mm operation', () => {
  assert.match(thickness, /unit\.millimetresPerUnit/u)
  assert.match(thickness, /物理量0\.01 mm刻み/u)
  assert.match(thickness, /data-length-dirty/u)
  assert.match(thickness, /data-length-source-token/u)
})

test('paper-edge ratio resize keeps the reference-parallel physical dimension', () => {
  assert.match(
    app,
    /referenceAxis === 'width'\s*\?\s*currentSize\.width/u,
  )
  assert.match(
    app,
    /referenceAxis === 'height'\s*\?\s*currentSize\.height/u,
  )
  assert.match(
    app,
    /readOnly=\{rectangularRatioReferenceAxis === 'width'\}/u,
  )
  assert.match(
    app,
    /readOnly=\{rectangularRatioReferenceAxis === 'height'\}/u,
  )
  assert.match(app, /基準辺の物理長は維持します/u)
})

test('benchmark, expression-backed new-project, and import contracts remain explicitly millimetres', () => {
  assert.match(
    app,
    /const displayedLengthUnit = benchmarkRun\s*\?\s*MILLIMETRE_LENGTH_DISPLAY_UNIT/u,
  )
  assert.equal((app.match(/name="width_expression"/gu) ?? []).length, 1)
  assert.equal((app.match(/name="height_expression"/gu) ?? []).length, 1)
  assert.match(
    app,
    /ariaLabel=\{text\(\{\s*ja: '用紙の幅の式 \(mm\)',\s*en: 'Paper width expression \(mm\)',\s*\}\)\}/u,
  )
  assert.match(
    app,
    /ariaLabel=\{text\(\{\s*ja: '用紙の高さの式 \(mm\)',\s*en: 'Paper height expression \(mm\)',\s*\}\)\}/u,
  )
  assert.match(
    app,
    /ja: formatLocalizedText\('ja',[\s\S]*?validation\.width_mm\.toLocaleString\('ja'\)[\s\S]*?validation\.height_mm\.toLocaleString\('ja'\)[\s\S]*?en: formatLocalizedText\('en',[\s\S]*?validation\.width_mm\.toLocaleString\('en'\)[\s\S]*?validation\.height_mm\.toLocaleString\('en'\)/u,
  )
})

function read(path: string) {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}
