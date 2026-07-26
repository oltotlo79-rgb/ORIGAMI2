import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const css = readFileSync(
  new URL('../src/components/proofProgress.css', import.meta.url),
  'utf8',
)

test('proof UI styles inherit theme tokens and keep non-colour distinctions', () => {
  for (const token of [
    '--panel',
    '--panel-strong',
    '--border',
    '--border-dark',
    '--text',
    '--muted',
    '--accent',
    '--accent-soft',
  ]) {
    assert.match(css, new RegExp(`var\\(${token}\\)`, 'u'))
  }
  assert.doesNotMatch(css, /#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/iu)
  assert.match(css, /\.proof-badge--unproven\s*\{[^}]*border-style:\s*dashed/su)
  assert.match(
    css,
    /\.proof-progress-panel\[data-proof-trust='unproven'\]\s*\{[^}]*border-inline-start-style:\s*dashed/su,
  )
})

test('proof UI styles preserve focus, disabled, narrow, and motion preferences', () => {
  assert.match(css, /:focus-visible/u)
  assert.match(css, /:disabled/u)
  assert.match(css, /@media \(max-width:\s*520px\)/u)
  assert.match(css, /@media \(prefers-reduced-motion:\s*reduce\)/u)
  assert.match(css, /transition:\s*none/u)
  assert.match(css, /@media \(forced-colors:\s*active\)/u)
})

test('both proof UI entry components load the scoped stylesheet', () => {
  for (const component of [
    'ProofProgressPanel.tsx',
    'SpeculativeStackedFoldApplyControl.tsx',
  ]) {
    const source = readFileSync(
      new URL(`../src/components/${component}`, import.meta.url),
      'utf8',
    )
    assert.match(source, /import '\.\/proofProgress\.css'/u)
  }
})
