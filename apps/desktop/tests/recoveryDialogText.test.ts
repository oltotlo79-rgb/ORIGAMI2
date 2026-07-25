import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  formatRecoveryTimestamp,
  RECOVERY_DIALOG_TEXT,
} from '../src/lib/recoveryDialogText.ts'

test('recovery dialog catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(RECOVERY_DIALOG_TEXT), [
    'eyebrow', 'availableTitle', 'invalidTitle', 'availableDescription',
    'lastUpdated', 'caution', 'invalidDescription', 'actionError',
    'restoring', 'restore', 'checking', 'retry', 'discarding',
    'discard', 'noTimestamp', 'unavailable',
  ])
  assert.equal(Object.isFrozen(RECOVERY_DIALOG_TEXT), true)
  for (const text of Object.values(RECOVERY_DIALOG_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(RECOVERY_DIALOG_TEXT.availableTitle, {
    ja: '未保存の編集内容を復元しますか？',
    en: 'Restore unsaved edits?',
  })
})

test('recovery timestamps use catalog-owned locale formatting and safe fallbacks', () => {
  const timestamp = Date.UTC(2026, 6, 26, 1, 2, 3)
  for (const [locale, numberLocale] of [
    ['ja', 'ja-JP'],
    ['en', 'en-US'],
  ] as const) {
    assert.equal(
      formatRecoveryTimestamp(timestamp, locale),
      new Intl.DateTimeFormat(numberLocale, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(timestamp)),
    )
    assert.equal(
      formatRecoveryTimestamp(null, locale),
      RECOVERY_DIALOG_TEXT.noTimestamp[locale],
    )
    assert.equal(
      formatRecoveryTimestamp(Number.NaN, locale),
      RECOVERY_DIALOG_TEXT.unavailable[locale],
    )
  }
})

test('recovery dialog delegates locale-sensitive timestamps to its catalog module', () => {
  const source = readFileSync(
    new URL('../src/components/RecoveryDialog.tsx', import.meta.url),
    'utf8',
  )
  assert.match(source, /formatRecoveryTimestamp\(/u)
  assert.doesNotMatch(source, /locale\s*===|locale\s*!==/u)
  assert.doesNotMatch(source, /Intl\.DateTimeFormat/u)
})
