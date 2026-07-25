import type { LocalizedText } from './i18n.ts'

export const UPDATE_CHECK_TEXT: Readonly<Record<
  | 'popoverSummary' | 'eyebrow' | 'title' | 'enabled' | 'manualOnly'
  | 'privacy' | 'checkButton' | 'checkingButton' | 'openRelease' | 'idle'
  | 'disabled' | 'checking' | 'upToDate' | 'updateAvailable'
  | 'noPublishedRelease' | 'unavailable' | 'persistenceFailed',
  LocalizedText
>> = Object.freeze({
  popoverSummary: Object.freeze({
    ja: '更新',
    en: 'Updates',
  }),
  eyebrow: Object.freeze({
    ja: 'GitHub Releases',
    en: 'GitHub Releases',
  }),
  title: Object.freeze({
    ja: 'ソフトウェア更新',
    en: 'Software updates',
  }),
  enabled: Object.freeze({
    ja: '更新確認を有効にする',
    en: 'Enable update checks',
  }),
  manualOnly: Object.freeze({
    ja: '起動時には確認しません。「今すぐ確認」を押したときだけGitHubへ接続します。',
    en: 'No check runs at startup. GitHub is contacted only when you choose “Check now”.',
  }),
  privacy: Object.freeze({
    ja: '確認時に送信されるのは標準的な接続メタデータだけです。作品データ、利用状況、インストール済みバージョンは送信しません。自動ダウンロードや自動インストールも行いません。',
    en: 'Only standard connection metadata is sent during a check. Project data, usage data, and the installed version are not sent. Nothing is downloaded or installed automatically.',
  }),
  checkButton: Object.freeze({
    ja: '今すぐ確認',
    en: 'Check now',
  }),
  checkingButton: Object.freeze({
    ja: '確認中…',
    en: 'Checking…',
  }),
  openRelease: Object.freeze({
    ja: 'GitHubで {version} のリリースを開く',
    en: 'Open release {version} on GitHub',
  }),
  idle: Object.freeze({
    ja: 'この起動中はまだ更新を確認していません。',
    en: 'Updates have not been checked during this session.',
  }),
  disabled: Object.freeze({
    ja: '更新確認は無効です。',
    en: 'Update checks are disabled.',
  }),
  checking: Object.freeze({
    ja: 'GitHub Releasesを確認しています。',
    en: 'Checking GitHub Releases.',
  }),
  upToDate: Object.freeze({
    ja: '最新版です。現在 {currentVersion}、公開版 {latestVersion}。',
    en: 'Up to date. Installed {currentVersion}; latest release {latestVersion}.',
  }),
  updateAvailable: Object.freeze({
    ja: '更新があります。現在 {currentVersion}、公開版 {latestVersion}。',
    en: 'An update is available. Installed {currentVersion}; latest release {latestVersion}.',
  }),
  noPublishedRelease: Object.freeze({
    ja: '公開済みの更新はありません。',
    en: 'No published release is available.',
  }),
  unavailable: Object.freeze({
    ja: '更新情報を確認できませんでした。時間をおいてもう一度お試しください。',
    en: 'Update information could not be checked. Please try again later.',
  }),
  persistenceFailed: Object.freeze({
    ja: '更新確認の設定をこのPCに保存できませんでした。この起動中だけ適用されます。',
    en: 'The update-check setting could not be saved on this PC. It applies only for this session.',
  }),
})
