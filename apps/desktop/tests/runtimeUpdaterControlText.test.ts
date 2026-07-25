import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { RUNTIME_UPDATER_CONTROL_TEXT } from '../src/lib/runtimeUpdaterControlText.ts'

test('runtime updater control catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(RUNTIME_UPDATER_CONTROL_TEXT), [
    'title',
    'privacyDescription',
    'detailsAriaLabel',
    'version',
    'platform',
    'size',
    'releaseNotes',
    'sizeMegabytes',
    'checkForUpdates',
    'downloadAndVerify',
    'restartAndApply',
    'cancel',
    'statusDisabled',
    'statusRecovering',
    'statusIdle',
    'statusChecking',
    'statusDownloading',
    'statusVerified',
    'statusApplying',
    'statusApplied',
    'statusCancelled',
    'statusAvailable',
    'statusError',
  ])
  assert.deepEqual(RUNTIME_UPDATER_CONTROL_TEXT, {
    title: { ja: 'アプリ更新', en: 'App update' },
    privacyDescription: {
      ja: '確認ではproject dataを送信しません。payloadは明示操作後にのみ取得し、署名とchecksumを検証します。',
      en: 'Checks never send project data. Payloads are fetched only after an explicit action and are verified by signature and checksum.',
    },
    detailsAriaLabel: { ja: '更新内容', en: 'Update details' },
    version: { ja: 'バージョン', en: 'Version' },
    platform: { ja: 'プラットフォーム', en: 'Platform' },
    size: { ja: 'サイズ', en: 'Size' },
    releaseNotes: { ja: 'リリースノート', en: 'Release notes' },
    sizeMegabytes: { ja: '{size} MB', en: '{size} MB' },
    checkForUpdates: { ja: '更新を確認', en: 'Check for updates' },
    downloadAndVerify: {
      ja: 'ダウンロードして検証',
      en: 'Download and verify',
    },
    restartAndApply: {
      ja: '再起動して適用',
      en: 'Restart and apply',
    },
    cancel: { ja: 'キャンセル', en: 'Cancel' },
    statusDisabled: {
      ja: '更新確認は無効です',
      en: 'Update checks are disabled',
    },
    statusRecovering: {
      ja: '保留中の更新を確認しています',
      en: 'Checking pending update',
    },
    statusIdle: {
      ja: '更新を手動で確認できます',
      en: 'Check for updates manually',
    },
    statusChecking: {
      ja: '更新を確認しています',
      en: 'Checking for updates',
    },
    statusDownloading: {
      ja: 'ダウンロードして署名とchecksumを検証しています',
      en: 'Downloading and verifying signature and checksum',
    },
    statusVerified: {
      ja: '検証済みです。明示的に再起動して適用できます',
      en: 'Verified. Restart explicitly to apply',
    },
    statusApplying: {
      ja: '再起動と適用を準備しています',
      en: 'Preparing restart and apply',
    },
    statusApplied: {
      ja: '更新の適用を確認しました',
      en: 'Update application confirmed',
    },
    statusCancelled: {
      ja: '操作をキャンセルしました',
      en: 'Operation cancelled',
    },
    statusAvailable: {
      ja: '更新を利用できます。内容を確認してダウンロードしてください',
      en: 'An update is available. Review it before downloading.',
    },
    statusError: {
      ja: '更新を安全に停止しました: {error}',
      en: 'Update stopped safely: {error}',
    },
  })
  assert.equal(Object.isFrozen(RUNTIME_UPDATER_CONTROL_TEXT), true)
  for (const text of Object.values(RUNTIME_UPDATER_CONTROL_TEXT)) {
    assert.deepEqual(Object.keys(text), ['ja', 'en'])
    assert.equal(Object.isFrozen(text), true)
  }
})

test('runtime updater placeholders are closed and preserve rendered output', () => {
  const placeholders = Object.fromEntries(
    Object.entries(RUNTIME_UPDATER_CONTROL_TEXT).flatMap(([key, text]) => {
      const ja = [...text.ja.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
        .map((match) => match[1])
      const en = [...text.en.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
        .map((match) => match[1])
      return ja.length === 0 && en.length === 0
        ? []
        : [[key, { ja, en }]]
    }),
  )
  assert.deepEqual(placeholders, {
    sizeMegabytes: { ja: ['size'], en: ['size'] },
    statusError: { ja: ['error'], en: ['error'] },
  })
  assert.equal(
    formatLocalizedText(
      'ja',
      RUNTIME_UPDATER_CONTROL_TEXT.sizeMegabytes,
      { size: '25.0' },
    ),
    '25.0 MB',
  )
  assert.equal(
    formatLocalizedText(
      'en',
      RUNTIME_UPDATER_CONTROL_TEXT.statusError,
      { error: 'signature' },
    ),
    'Update stopped safely: signature',
  )
  assert.equal(
    formatLocalizedText(
      'ja',
      RUNTIME_UPDATER_CONTROL_TEXT.statusError,
      { error: 'malformed' },
    ),
    '更新を安全に停止しました: malformed',
  )
})
