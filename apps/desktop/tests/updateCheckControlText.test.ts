import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { UPDATE_CHECK_TEXT } from '../src/lib/updateCheckControlText.ts'

test('update check control catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(UPDATE_CHECK_TEXT), [
    'popoverSummary',
    'eyebrow',
    'title',
    'enabled',
    'manualOnly',
    'privacy',
    'checkButton',
    'checkingButton',
    'openRelease',
    'idle',
    'disabled',
    'checking',
    'upToDate',
    'updateAvailable',
    'noPublishedRelease',
    'unavailable',
    'persistenceFailed',
  ])
  assert.deepEqual(UPDATE_CHECK_TEXT, {
    popoverSummary: {
      ja: '更新',
      en: 'Updates',
    },
    eyebrow: {
      ja: 'GitHub Releases',
      en: 'GitHub Releases',
    },
    title: {
      ja: 'ソフトウェア更新',
      en: 'Software updates',
    },
    enabled: {
      ja: '更新確認を有効にする',
      en: 'Enable update checks',
    },
    manualOnly: {
      ja: '起動時には確認しません。「今すぐ確認」を押したときだけGitHubへ接続します。',
      en: 'No check runs at startup. GitHub is contacted only when you choose “Check now”.',
    },
    privacy: {
      ja: '確認時に送信されるのは標準的な接続メタデータだけです。作品データ、利用状況、インストール済みバージョンは送信しません。自動ダウンロードや自動インストールも行いません。',
      en: 'Only standard connection metadata is sent during a check. Project data, usage data, and the installed version are not sent. Nothing is downloaded or installed automatically.',
    },
    checkButton: {
      ja: '今すぐ確認',
      en: 'Check now',
    },
    checkingButton: {
      ja: '確認中…',
      en: 'Checking…',
    },
    openRelease: {
      ja: 'GitHubで {version} のリリースを開く',
      en: 'Open release {version} on GitHub',
    },
    idle: {
      ja: 'この起動中はまだ更新を確認していません。',
      en: 'Updates have not been checked during this session.',
    },
    disabled: {
      ja: '更新確認は無効です。',
      en: 'Update checks are disabled.',
    },
    checking: {
      ja: 'GitHub Releasesを確認しています。',
      en: 'Checking GitHub Releases.',
    },
    upToDate: {
      ja: '最新版です。現在 {currentVersion}、公開版 {latestVersion}。',
      en: 'Up to date. Installed {currentVersion}; latest release {latestVersion}.',
    },
    updateAvailable: {
      ja: '更新があります。現在 {currentVersion}、公開版 {latestVersion}。',
      en: 'An update is available. Installed {currentVersion}; latest release {latestVersion}.',
    },
    noPublishedRelease: {
      ja: '公開済みの更新はありません。',
      en: 'No published release is available.',
    },
    unavailable: {
      ja: '更新情報を確認できませんでした。時間をおいてもう一度お試しください。',
      en: 'Update information could not be checked. Please try again later.',
    },
    persistenceFailed: {
      ja: '更新確認の設定をこのPCに保存できませんでした。この起動中だけ適用されます。',
      en: 'The update-check setting could not be saved on this PC. It applies only for this session.',
    },
  })
  assert.equal(Object.isFrozen(UPDATE_CHECK_TEXT), true)
  for (const text of Object.values(UPDATE_CHECK_TEXT)) {
    assert.deepEqual(Object.keys(text), ['ja', 'en'])
    assert.equal(Object.isFrozen(text), true)
  }
})

test('update check placeholders are closed and preserve rendered output', () => {
  const placeholders = Object.fromEntries(
    Object.entries(UPDATE_CHECK_TEXT).flatMap(([key, text]) => {
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
    openRelease: {
      ja: ['version'],
      en: ['version'],
    },
    upToDate: {
      ja: ['currentVersion', 'latestVersion'],
      en: ['currentVersion', 'latestVersion'],
    },
    updateAvailable: {
      ja: ['currentVersion', 'latestVersion'],
      en: ['currentVersion', 'latestVersion'],
    },
  })
  assert.equal(
    formatLocalizedText('ja', UPDATE_CHECK_TEXT.openRelease, {
      version: '1.2.3',
    }),
    'GitHubで 1.2.3 のリリースを開く',
  )
  assert.equal(
    formatLocalizedText('en', UPDATE_CHECK_TEXT.upToDate, {
      currentVersion: '1.0.0',
      latestVersion: '1.2.3',
    }),
    'Up to date. Installed 1.0.0; latest release 1.2.3.',
  )
  assert.equal(
    formatLocalizedText('ja', UPDATE_CHECK_TEXT.updateAvailable, {
      currentVersion: '1.0.0',
      latestVersion: '1.2.3',
    }),
    '更新があります。現在 1.0.0、公開版 1.2.3。',
  )
})
