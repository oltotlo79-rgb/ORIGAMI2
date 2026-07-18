import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  formatPaperThicknessInput,
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
    /useEffect\(\(\) => \{\s*setValue\(initialValue\)\s*\}, \[initialValue\]\)/u,
    'an opened project with a different thickness must refresh the control',
  )
  assert.doesNotMatch(
    componentSource,
    /name="thickness_mm"[\s\S]{0,200}step="0\.01"/u,
  )
  assert.match(appSource, /initialValue="0\.10"/u)
  assert.match(
    appSource,
    /initialValue=\{formatPaperThicknessInput\([\s\S]*?nativeSnapshot\?\.paper\.thickness_mm,[\s\S]*?\)\}/u,
  )
  assert.match(
    componentSource,
    /event\.key === 'ArrowUp' \? 'up' : 'down'/u,
  )
  assert.match(componentSource, /onClick=\{\(\) => applyStep\('up'\)\}/u)
  assert.match(componentSource, /onClick=\{\(\) => applyStep\('down'\)\}/u)
  assert.match(
    appSource,
    /<form onSubmit=\{submitNewProject\} noValidate>/u,
    'the new-project form must not reject finer directly typed precision as a step mismatch',
  )
  assert.equal(
    (appSource.match(
      /!thicknessInput \|\| !Number\.isFinite\(thicknessMm\) \|\| thicknessMm < 0/gu,
    ) ?? []).length,
    2,
    'both forms must reject an empty, non-finite, or negative thickness',
  )
  assert.match(cssSource, /input::-(?:webkit-inner-spin-button|webkit-outer-spin-button)/u)
})
