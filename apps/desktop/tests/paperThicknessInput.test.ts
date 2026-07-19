import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  formatPaperThicknessInput,
  stepPaperThicknessFromMillimetres,
  stepPaperThicknessInput,
} from '../src/lib/paperThicknessInput.ts'

test('paper thickness display keeps exact input precision and shows hundredths', () => {
  for (const [value, expected] of [
    [0, '0.00'],
    [-0, '0.00'],
    [0.1, '0.10'],
    [1, '1.00'],
    [0.075, '0.075'],
    [3.125, '3.125'],
    [1e-7, '0.0000001'],
  ] as const) {
    const formatted = formatPaperThicknessInput(value)
    assert.equal(formatted, expected)
    assert.equal(Number(formatted), Object.is(value, -0) ? 0 : value)
  }
})

test('invalid stored thicknesses do not become editable defaults', () => {
  for (const value of [null, undefined, -0.01, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.equal(formatPaperThicknessInput(value), '')
  }
})

test('paper thickness steps by an exact decimal 0.01 without snapping finer input', () => {
  for (const [value, direction, expected] of [
    ['0.001', 'up', '0.011'],
    ['0.001', 'down', '0.000'],
    ['0.075', 'up', '0.085'],
    ['0.075', 'down', '0.065'],
    ['0.105', 'up', '0.115'],
    ['0.105', 'down', '0.095'],
    ['1.2300', 'up', '1.2400'],
    ['7.5e-2', 'down', '0.065'],
    ['0', 'up', '0.01'],
    ['0', 'down', '0.00'],
    ['-0.001', 'up', '0.00'],
    ['-1', 'down', '0.00'],
  ] as const) {
    assert.equal(stepPaperThicknessInput(value, direction), expected)
  }
  assert.equal(
    stepPaperThicknessInput(
      stepPaperThicknessInput('0.075', 'up'),
      'down',
    ),
    '0.075',
  )
})

test('paper thickness stepping leaves empty malformed and non-finite input unchanged', () => {
  for (const value of [
    '',
    ' ',
    'NaN',
    'Infinity',
    '-Infinity',
    '1e309',
    '1e-999999',
    'not-a-number',
    '1'.repeat(10_000),
  ]) {
    assert.equal(stepPaperThicknessInput(value, 'up'), value)
    assert.equal(stepPaperThicknessInput(value, 'down'), value)
  }
})

test('every converted step retains the exact same physical binary64 as mm', () => {
  const scales = [
    ['mm', 1],
    ['cm', 10],
    ['inch', 25.4],
    ['horizontal paper-edge ratio', 400],
    ['vertical paper-edge ratio', 200],
  ] as const
  for (const source of [
    0,
    0.075,
    0.1,
    0.10000000000000002,
    0.30000000000000004,
    3,
  ]) {
    for (const direction of ['up', 'down'] as const) {
      const millimetreStep = stepPaperThicknessFromMillimetres(
        source,
        direction,
        1,
      )
      assert.ok(millimetreStep)
      for (const [label, scale] of scales) {
        const convertedStep = stepPaperThicknessFromMillimetres(
          source,
          direction,
          scale,
        )
        assert.ok(convertedStep, `${label} must produce a step`)
        assert.ok(
          Object.is(
            convertedStep.millimetres,
            millimetreStep.millimetres,
          ),
          `${label} must preserve the exact mm result for ${source} ${direction}`,
        )
        assert.equal(
          convertedStep.displayValue,
          String(convertedStep.millimetres / scale),
        )
      }
    }
  }
})

test('both paper thickness controls use exact custom steps and retain direct input', () => {
  const appSource = readFileSync(
    new URL('../src/App.tsx', import.meta.url),
    'utf8',
  )
  const componentSource = readFileSync(
    new URL('../src/components/PaperThicknessInput.tsx', import.meta.url),
    'utf8',
  )
  const cssSource = readFileSync(
    new URL('../src/App.css', import.meta.url),
    'utf8',
  )
  const controls = appSource.match(/<PaperThicknessInput\b/gu) ?? []

  assert.equal(controls.length, 2)
  assert.match(componentSource, /step="any"/u)
  assert.match(componentSource, /aria-label="紙厚"/u)
  assert.match(
    componentSource,
    /useEffect\(\(\) => \{\s*setState\(\{\s*dirty: false,\s*steppedMillimetres: null,\s*value: initialValue,[\s\S]*?\}, \[initialValue, sourceToken\]\)/u,
    'a new source token must refresh display, dirty state, and exact step evidence',
  )
  assert.doesNotMatch(
    componentSource,
    /name="thickness_mm"[\s\S]{0,200}step="0\.01"/u,
  )
  assert.match(appSource, /initialValue="0\.10"/u)
  assert.match(
    appSource,
    /initialValue=\{lengthDisplayUnit\.effectiveUnit === 'mm'[\s\S]*?formatPaperThicknessInput\([\s\S]*?nativeSnapshot\?\.paper\.thickness_mm[\s\S]*?: formatLengthInput\([\s\S]*?lengthDisplayUnit/u,
  )
  assert.match(
    componentSource,
    /event\.key === 'ArrowUp' \? 'up' : 'down'/u,
  )
  assert.match(componentSource, /onClick=\{\(\) => applyStep\('up'\)\}/u)
  assert.match(componentSource, /onClick=\{\(\) => applyStep\('down'\)\}/u)
  assert.match(
    componentSource,
    /data-paper-thickness-stepped-millimetres/u,
  )
  assert.match(componentSource, /aria-describedby=\{stepDescriptionId\}/u)
  assert.match(
    appSource,
    /<form onSubmit=\{submitNewProject\} noValidate>/u,
    'the new-project form must not reject finer directly typed precision as a step mismatch',
  )
  assert.match(
    appSource,
    /thicknessMm === null \|\| thicknessMm < 0/u,
    'the converted paper-properties field must reject empty, non-finite, and negative values',
  )
  assert.match(
    appSource,
    /!thicknessInput \|\| !Number\.isFinite\(thicknessMm\) \|\| thicknessMm < 0/u,
    'the millimetre-only new-project field must reject empty, non-finite, and negative values',
  )
  assert.match(cssSource, /input::-(?:webkit-inner-spin-button|webkit-outer-spin-button)/u)
})
