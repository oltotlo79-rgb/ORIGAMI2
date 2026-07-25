import assert from 'node:assert/strict'
import test from 'node:test'

import { DIAGNOSTICS_COPY } from '../src/lib/diagnosticsDialogText.ts'

test('diagnostics dialog catalog is locale-complete and deeply frozen', () => {
  assert.deepEqual(Object.keys(DIAGNOSTICS_COPY), ['ja', 'en'])
  assertLocaleShape(DIAGNOSTICS_COPY.ja, DIAGNOSTICS_COPY.en)
  assertDeeplyFrozen(DIAGNOSTICS_COPY)
})

test('diagnostics dialog catalog preserves reviewed privacy and action copy', () => {
  assert.equal(DIAGNOSTICS_COPY.ja.eyebrow, '問題報告の準備')
  assert.equal(DIAGNOSTICS_COPY.en.title, 'Review diagnostics')
  assert.deepEqual(DIAGNOSTICS_COPY.ja.disclosure, [
    '作品名、作品形状、ファイル内容、ローカルパス、ID、座標、時刻、アプリ版、OS、CPU、GPU情報は含みません。',
    'この情報は自動送信されません。下に表示されたJSONと保存されるJSONは同一です。',
    '保存後、内容を確認したうえで利用者自身がGitHub Issuesへ添付してください。',
  ])
  assert.equal(
    DIAGNOSTICS_COPY.en.proofScopeDisclosure,
    'Contains only certificate models, versions, counts, and allowlisted reasons. It excludes coordinates, project IDs, UUIDs, and timestamps.',
  )
  assert.deepEqual(DIAGNOSTICS_COPY.en.notices, {
    selected: 'All contents are selected. Press Ctrl/Cmd+C to copy.',
    save_canceled: 'Save was canceled.',
    saved: 'Diagnostics JSON was saved.',
    save_failed:
      'Diagnostics JSON could not be saved. Check the destination and try again.',
  })
})

function assertLocaleShape(
  left: Readonly<Record<string, unknown>>,
  right: Readonly<Record<string, unknown>>,
) {
  assert.deepEqual(Object.keys(left), Object.keys(right))
  for (const key of Object.keys(left)) {
    const leftValue = left[key]
    const rightValue = right[key]
    if (Array.isArray(leftValue) || Array.isArray(rightValue)) {
      assert.equal(Array.isArray(leftValue), Array.isArray(rightValue), key)
      assert.equal(
        (leftValue as readonly unknown[]).length,
        (rightValue as readonly unknown[]).length,
        key,
      )
      continue
    }
    assert.equal(typeof leftValue, typeof rightValue, key)
    if (
      typeof leftValue === 'object'
      && leftValue !== null
      && typeof rightValue === 'object'
      && rightValue !== null
    ) {
      assertLocaleShape(
        leftValue as Readonly<Record<string, unknown>>,
        rightValue as Readonly<Record<string, unknown>>,
      )
    }
  }
}

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) {
    assertDeeplyFrozen(child)
  }
}
