import type { LocalizedText } from './i18n.ts'

export const RUNTIME_UPDATER_CONTROL_TEXT: Readonly<Record<
  | 'title' | 'privacyDescription' | 'detailsAriaLabel' | 'version'
  | 'platform' | 'size' | 'releaseNotes' | 'sizeMegabytes'
  | 'checkForUpdates' | 'downloadAndVerify' | 'restartAndApply' | 'cancel'
  | 'statusDisabled' | 'statusRecovering' | 'statusIdle' | 'statusChecking'
  | 'statusDownloading' | 'statusVerified' | 'statusApplying'
  | 'statusApplied' | 'statusCancelled' | 'statusAvailable' | 'statusError',
  LocalizedText
>> = Object.freeze({
  title: Object.freeze({ ja: 'アプリ更新', en: 'App update' }),
  privacyDescription: Object.freeze({
    ja: '確認ではproject dataを送信しません。payloadは明示操作後にのみ取得し、署名とchecksumを検証します。',
    en: 'Checks never send project data. Payloads are fetched only after an explicit action and are verified by signature and checksum.',
  }),
  detailsAriaLabel: Object.freeze({ ja: '更新内容', en: 'Update details' }),
  version: Object.freeze({ ja: 'バージョン', en: 'Version' }),
  platform: Object.freeze({ ja: 'プラットフォーム', en: 'Platform' }),
  size: Object.freeze({ ja: 'サイズ', en: 'Size' }),
  releaseNotes: Object.freeze({ ja: 'リリースノート', en: 'Release notes' }),
  sizeMegabytes: Object.freeze({ ja: '{size} MB', en: '{size} MB' }),
  checkForUpdates: Object.freeze({ ja: '更新を確認', en: 'Check for updates' }),
  downloadAndVerify: Object.freeze({
    ja: 'ダウンロードして検証',
    en: 'Download and verify',
  }),
  restartAndApply: Object.freeze({
    ja: '再起動して適用',
    en: 'Restart and apply',
  }),
  cancel: Object.freeze({ ja: 'キャンセル', en: 'Cancel' }),
  statusDisabled: Object.freeze({
    ja: '更新確認は無効です',
    en: 'Update checks are disabled',
  }),
  statusRecovering: Object.freeze({
    ja: '保留中の更新を確認しています',
    en: 'Checking pending update',
  }),
  statusIdle: Object.freeze({
    ja: '更新を手動で確認できます',
    en: 'Check for updates manually',
  }),
  statusChecking: Object.freeze({
    ja: '更新を確認しています',
    en: 'Checking for updates',
  }),
  statusDownloading: Object.freeze({
    ja: 'ダウンロードして署名とchecksumを検証しています',
    en: 'Downloading and verifying signature and checksum',
  }),
  statusVerified: Object.freeze({
    ja: '検証済みです。明示的に再起動して適用できます',
    en: 'Verified. Restart explicitly to apply',
  }),
  statusApplying: Object.freeze({
    ja: '再起動と適用を準備しています',
    en: 'Preparing restart and apply',
  }),
  statusApplied: Object.freeze({
    ja: '更新の適用を確認しました',
    en: 'Update application confirmed',
  }),
  statusCancelled: Object.freeze({
    ja: '操作をキャンセルしました',
    en: 'Operation cancelled',
  }),
  statusAvailable: Object.freeze({
    ja: '更新を利用できます。内容を確認してダウンロードしてください',
    en: 'An update is available. Review it before downloading.',
  }),
  statusError: Object.freeze({
    ja: '更新を安全に停止しました: {error}',
    en: 'Update stopped safely: {error}',
  }),
})
