import assert from 'node:assert/strict'
import test from 'node:test'
import {
  BUILTIN_PAPER_PATTERNS,
  builtinPaperPatternAsset,
  builtinPaperPatternFromAsset,
  paperPatternCss,
} from '../src/lib/paperPatterns.ts'

test('built-in paper patterns round-trip through stable asset IDs', () => {
  for (const pattern of ['dots', 'grid', 'stripes'] as const) {
    const asset = builtinPaperPatternAsset(pattern)
    assert.equal(asset, BUILTIN_PAPER_PATTERNS[pattern])
    assert.equal(builtinPaperPatternFromAsset(asset), pattern)
    assert.match(paperPatternCss(asset, '#abcdef') ?? '', /gradient/u)
  }
})

test('unknown, empty, and solid selections fail closed to no pattern', () => {
  assert.equal(builtinPaperPatternAsset('none'), null)
  assert.equal(builtinPaperPatternAsset('unknown'), null)
  assert.equal(builtinPaperPatternFromAsset(null), null)
  assert.equal(
    builtinPaperPatternFromAsset('00000000-0000-0000-0000-000000000001'),
    null,
  )
})
