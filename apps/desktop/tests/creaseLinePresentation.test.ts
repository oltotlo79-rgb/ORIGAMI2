import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  CREASE_LINE_KINDS,
  CREASE_LINE_PRESENTATIONS,
  type CreaseLineKind,
} from '../src/lib/creaseLinePresentation.ts'

const cssSource = readFileSync(
  new URL('../src/App.css', import.meta.url),
  'utf8',
)
const canvasSource = readFileSync(
  new URL('../src/components/CreaseCanvas.tsx', import.meta.url),
  'utf8',
)

test('all five crease kinds remain unique after every screen colour is removed', () => {
  assert.deepEqual(
    CREASE_LINE_KINDS,
    ['boundary', 'mountain', 'valley', 'auxiliary', 'cut'],
  )

  const colours = new Set(
    CREASE_LINE_KINDS.map((kind) => CREASE_LINE_PRESENTATIONS[kind].color),
  )
  assert.equal(colours.size, CREASE_LINE_KINDS.length)

  const monochromePatterns = new Set(
    CREASE_LINE_KINDS.map((kind) => monochromeSignature(kind)),
  )
  assert.equal(monochromePatterns.size, CREASE_LINE_KINDS.length)
})

test('every 2D Canvas line colour keeps non-text contrast against the paper', () => {
  for (const kind of CREASE_LINE_KINDS) {
    const contrast = contrastRatio(
      CREASE_LINE_PRESENTATIONS[kind].color,
      '#fffdf9',
    )
    assert.ok(contrast >= 3, `${kind} contrast was only ${contrast.toFixed(3)}:1`)
  }
})

test('the fixed monochrome vocabulary is solid, dash-dot, dash, dot, dash-dot-dot', () => {
  assert.deepEqual(
    CREASE_LINE_KINDS.map((kind) => ({
      kind,
      pattern: CREASE_LINE_PRESENTATIONS[kind].pattern,
      lineCap: CREASE_LINE_PRESENTATIONS[kind].lineCap,
    })),
    [
      { kind: 'boundary', pattern: 'solid', lineCap: 'butt' },
      { kind: 'mountain', pattern: 'dash-dot', lineCap: 'butt' },
      { kind: 'valley', pattern: 'dash', lineCap: 'butt' },
      { kind: 'auxiliary', pattern: 'dot', lineCap: 'round' },
      { kind: 'cut', pattern: 'dash-dot-dot', lineCap: 'butt' },
    ],
  )
  assert.deepEqual(CREASE_LINE_PRESENTATIONS.boundary.canvasDash, [])
  for (const kind of CREASE_LINE_KINDS.filter((kind) => kind !== 'boundary')) {
    assert.notDeepEqual(CREASE_LINE_PRESENTATIONS[kind].canvasDash, [])
  }
})

test('the production 2D Canvas paints every batch from the fixed presentation table', () => {
  assert.match(
    canvasSource,
    /const presentation = CREASE_LINE_PRESENTATIONS\[batch\.kind\]/u,
  )
  assert.match(canvasSource, /context\.strokeStyle = presentation\.color/u)
  assert.match(canvasSource, /context\.lineCap = presentation\.lineCap/u)
  assert.match(
    canvasSource,
    /context\.setLineDash\(presentation\.canvasDash\.slice\(\)\)/u,
  )
  assert.doesNotMatch(canvasSource, /const (?:COLORS|LINE_DASHES)\b/u)
})

test('FOLD and SVG import previews use the same five monochrome meanings', () => {
  const foldSelectors: Record<CreaseLineKind, string> = {
    boundary: '.fold-preview-edge.assignment-b',
    mountain: '.fold-preview-edge.assignment-m',
    valley: '.fold-preview-edge.assignment-v',
    auxiliary: '.fold-preview-edge.assignment-f',
    cut: '.fold-preview-edge.assignment-c',
  }
  const svgSelectors: Record<CreaseLineKind, string> = {
    boundary: '.svg-preview-edge.target-boundary',
    mountain: '.svg-preview-edge.target-mountain',
    valley: '.svg-preview-edge.target-valley',
    auxiliary: '.svg-preview-edge.target-auxiliary',
    cut: '.svg-preview-edge.target-cut',
  }

  for (const {
    selectors,
    baseSelector,
    auxiliaryAliases,
  } of [
    {
      selectors: foldSelectors,
      baseSelector: '.fold-preview-edge',
      auxiliaryAliases: [
        '.fold-preview-edge.assignment-u',
        '.fold-preview-edge.assignment-j',
      ],
    },
    {
      selectors: svgSelectors,
      baseSelector: '.svg-preview-edge',
      auxiliaryAliases: ['.svg-preview-edge.target-ignore'],
    },
  ]) {
    const patterns = new Set(
      CREASE_LINE_KINDS.map((kind) => cssDashSignature(selectors[kind])),
    )
    assert.equal(patterns.size, CREASE_LINE_KINDS.length)
    assert.equal(cssDashSignature(selectors.boundary), 'solid')
    assert.equal(cssDashSignature(selectors.mountain), '10 3 2 3')
    assert.equal(cssDashSignature(selectors.valley), '5 3')
    assert.equal(cssDashSignature(selectors.auxiliary), '1 3')
    assert.equal(cssDashSignature(selectors.cut), '10 3 2 3 2 3')

    assert.deepEqual(
      CREASE_LINE_KINDS.map((kind) => resolvedCssLineCap(
        selectors[kind],
        baseSelector,
      )),
      ['butt', 'butt', 'butt', 'round', 'butt'],
    )
    for (const alias of auxiliaryAliases) {
      assert.equal(
        cssDashSignature(alias),
        cssDashSignature(selectors.auxiliary),
      )
      assert.equal(resolvedCssLineCap(alias, baseSelector), 'round')
    }
  }
})

test('import previews keep a light paper surface in both OS colour schemes', () => {
  const foldSurface = cssRuleBody('.fold-import-preview')
  const svgSurface = cssRuleBody('.svg-import-preview')
  assert.equal(foldSurface, svgSurface)
  assert.match(foldSurface, /#ffffff/u)
  assert.doesNotMatch(foldSurface, /var\(--panel\)/u)
})

function monochromeSignature(kind: CreaseLineKind) {
  const presentation = CREASE_LINE_PRESENTATIONS[kind]
  return `${presentation.lineCap}:${presentation.canvasDash.join(',') || 'solid'}`
}

function cssDashSignature(selector: string) {
  const dash = cssRuleBody(selector).match(/stroke-dasharray:\s*([^;]+);/u)
  return dash?.[1]?.trim() ?? 'solid'
}

function resolvedCssLineCap(selector: string, baseSelector: string) {
  const own = cssRuleBody(selector).match(/stroke-linecap:\s*([^;]+);/u)
  const base = cssRuleBody(baseSelector).match(/stroke-linecap:\s*([^;]+);/u)
  return own?.[1]?.trim() ?? base?.[1]?.trim() ?? 'butt'
}

function cssRuleBody(selector: string) {
  for (const match of cssSource.matchAll(/([^{}]+)\{([^{}]*)\}/gu)) {
    const selectors = match[1]?.split(',').map((value) => value.trim())
    if (selectors?.includes(selector)) return match[2] ?? ''
  }
  assert.fail(`missing CSS rule for ${selector}`)
}

function contrastRatio(left: string, right: string) {
  const leftLuminance = relativeLuminance(left)
  const rightLuminance = relativeLuminance(right)
  return (
    (Math.max(leftLuminance, rightLuminance) + 0.05)
    / (Math.min(leftLuminance, rightLuminance) + 0.05)
  )
}

function relativeLuminance(hex: string) {
  assert.match(hex, /^#[0-9a-f]{6}$/iu)
  const channels = [1, 3, 5].map((start) =>
    Number.parseInt(hex.slice(start, start + 2), 16) / 255)
  const linear = channels.map((channel) =>
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4)
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!
}
