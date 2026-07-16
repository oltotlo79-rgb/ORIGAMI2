import assert from 'node:assert/strict'
import test from 'node:test'

import { createCanvasLineDrawBatches } from '../src/lib/canvasBatching.ts'

type TestLine = {
  id: string
  kind: 'mountain' | 'valley' | 'auxiliary' | 'boundary' | 'cut'
}

test('canvas line batching collapses ten thousand alternating strokes by style', () => {
  const lines: TestLine[] = Array.from({ length: 10_000 }, (_, index) => ({
    id: `line-${index}`,
    kind: index % 2 === 0 ? 'mountain' : 'valley',
  }))

  const batches = createCanvasLineDrawBatches(lines, null)

  assert.equal(batches.length, 2)
  assert.deepEqual(
    batches.map(({ kind, selected, lines: batchLines }) => ({
      kind,
      selected,
      count: batchLines.length,
    })),
    [
      { kind: 'mountain', selected: false, count: 5_000 },
      { kind: 'valley', selected: false, count: 5_000 },
    ],
  )
  assert.equal(batches[0]?.lines[0]?.id, 'line-0')
  assert.equal(batches[0]?.lines.at(-1)?.id, 'line-9998')
})

test('selected canvas strokes are separated and emitted after ordinary strokes', () => {
  const lines: TestLine[] = [
    { id: 'selected', kind: 'valley' },
    { id: 'ordinary-cut', kind: 'cut' },
    { id: 'selected', kind: 'mountain' },
    { id: 'ordinary-valley', kind: 'valley' },
  ]

  const batches = createCanvasLineDrawBatches(lines, 'selected')

  assert.deepEqual(
    batches.map(({ kind, selected, lines: batchLines }) => ({
      kind,
      selected,
      ids: batchLines.map(({ id }) => id),
    })),
    [
      { kind: 'valley', selected: false, ids: ['ordinary-valley'] },
      { kind: 'cut', selected: false, ids: ['ordinary-cut'] },
      { kind: 'mountain', selected: true, ids: ['selected'] },
      { kind: 'valley', selected: true, ids: ['selected'] },
    ],
  )
})

test('canvas line batching handles an empty pattern', () => {
  assert.deepEqual(createCanvasLineDrawBatches([], null), [])
})
