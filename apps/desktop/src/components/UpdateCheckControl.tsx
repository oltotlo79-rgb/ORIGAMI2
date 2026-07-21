import { getVersion as getTauriVersion } from '@tauri-apps/api/app'
import {
  useEffect,
  useId,
  useRef,
  useState,
  useSyncExternalStore,
  type ChangeEvent,
} from 'react'

import './UpdateCheckControl.css'
import {
  RuntimeUpdaterControl,
  type RuntimeUpdaterUiController,
} from './RuntimeUpdaterControl.tsx'
import { tauriRuntimeUpdaterController } from '../lib/tauriRuntimeUpdaterController.ts'
import {
  compareSemanticVersions,
  createGitHubReleasesFetchTransport,
  createUpdateCheckClient,
  ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX,
  type UpdateCheckClient,
} from '../lib/githubReleaseUpdate.ts'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  updateCheckSettingsStore,
  type UpdateCheckSettingsStore,
} from '../lib/updateCheckSettings.ts'

export type InstalledVersionProvider = Readonly<{
  getVersion: () => unknown
}>

export type UpdateCheckControlProps = Readonly<{
  client?: UpdateCheckClient
  versionProvider?: InstalledVersionProvider
  settingsStore?: UpdateCheckSettingsStore
  localeStore?: LocaleStore
  runtimeUpdaterController?: RuntimeUpdaterUiController
}>

export function UpdateCheckPopover(
  props: UpdateCheckControlProps,
) {
  const localeStore_ = props.localeStore ?? localeStore
  const locale = useLocale(localeStore_)
  const settingsStore_ = props.settingsStore ?? updateCheckSettingsStore
  const settings = useSyncExternalStore(settingsStore_.subscribe, settingsStore_.getSnapshot, settingsStore_.getServerSnapshot)
  return (
    <details className="update-check-popover">
      <summary>
        {selectLocalizedText(locale, UPDATE_CHECK_TEXT.popoverSummary)}
      </summary>
      <UpdateCheckControl {...props} />
      <RuntimeUpdaterControl controller={
        props.runtimeUpdaterController ?? tauriRuntimeUpdaterController
      } enabled={settings.enabled} localeStore={localeStore_} />
    </details>
  )
}

type UpdateCheckViewState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'disabled' }>
  | Readonly<{ kind: 'checking' }>
  | Readonly<{
    kind: 'up_to_date'
    currentVersion: string
    latestVersion: string
  }>
  | Readonly<{
    kind: 'update_available'
    currentVersion: string
    latestVersion: string
    releasePageUrl: string
  }>
  | Readonly<{ kind: 'no_published_release' }>
  | Readonly<{ kind: 'unavailable' }>

const IDLE_STATE: UpdateCheckViewState = Object.freeze({ kind: 'idle' })
const CHECKING_STATE: UpdateCheckViewState =
  Object.freeze({ kind: 'checking' })
const NO_PUBLISHED_RELEASE_STATE: UpdateCheckViewState =
  Object.freeze({ kind: 'no_published_release' })
const UNAVAILABLE_STATE: UpdateCheckViewState =
  Object.freeze({ kind: 'unavailable' })

const defaultUpdateCheckClient = createUpdateCheckClient(
  createGitHubReleasesFetchTransport(),
)

const tauriInstalledVersionProvider: InstalledVersionProvider =
  Object.freeze({
    getVersion: () => getTauriVersion(),
  })

export function UpdateCheckControl({
  client = defaultUpdateCheckClient,
  versionProvider = tauriInstalledVersionProvider,
  settingsStore = updateCheckSettingsStore,
  localeStore: localeStore_ = localeStore,
}: UpdateCheckControlProps) {
  const locale = useLocale(localeStore_)
  const settings = useSyncExternalStore(
    settingsStore.subscribe,
    settingsStore.getSnapshot,
    settingsStore.getServerSnapshot,
  )
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const [viewState, setViewState] =
    useState<UpdateCheckViewState>(IDLE_STATE)
  const [persistenceFailed, setPersistenceFailed] = useState(false)
  const mountedRef = useRef(false)
  const checkingRef = useRef(false)
  const operationRef = useRef(0)
  const abortRef = useRef<AbortController | null>(null)
  const enabledRef = useRef(settings.enabled)
  const authorityRef = useRef({ client, versionProvider, settingsStore })
  const titleId = useId()
  const manualDescriptionId = useId()
  const privacyDescriptionId = useId()

  enabledRef.current = settings.enabled
  if (
    authorityRef.current.client !== client
    || authorityRef.current.versionProvider !== versionProvider
    || authorityRef.current.settingsStore !== settingsStore
  ) {
    authorityRef.current = { client, versionProvider, settingsStore }
    checkingRef.current = false
    operationRef.current += 1
  }

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      abortRef.current?.abort()
      abortRef.current = null
      checkingRef.current = false
      operationRef.current += 1
    }
  }, [])

  useEffect(() => {
    checkingRef.current = false
    abortRef.current?.abort()
    abortRef.current = null
    operationRef.current += 1
    setViewState(IDLE_STATE)
  }, [client, settings.enabled, settingsStore, versionProvider])

  const changeEnabled = (event: ChangeEvent<HTMLInputElement>) => {
    const enabled = event.currentTarget.checked
    checkingRef.current = false
    abortRef.current?.abort()
    abortRef.current = null
    operationRef.current += 1
    setViewState(IDLE_STATE)

    try {
      const result = settingsStore.setEnabled(enabled)
      setPersistenceFailed(!result.ok || !result.persisted)
    } catch {
      setPersistenceFailed(true)
    }
  }

  const checkNow = async () => {
    if (checkingRef.current || !enabledRef.current) return

    checkingRef.current = true
    abortRef.current?.abort()
    const controller = new AbortController()
    abortRef.current = controller
    const operation = ++operationRef.current
    const requestClient = client
    const requestVersionProvider = versionProvider
    const requestSettings = Object.freeze({ enabled: true })
    setViewState(CHECKING_STATE)

    const isCurrent = () => (
      mountedRef.current
      && enabledRef.current
      && operation === operationRef.current
      && requestClient === authorityRef.current.client
      && requestVersionProvider === authorityRef.current.versionProvider
    )

    try {
      const installedVersion = await requestVersionProvider.getVersion()
      if (!isCurrent()) return

      const result = await requestClient.checkNow(
        installedVersion,
        requestSettings,
        controller.signal,
      )
      if (!isCurrent()) return
      setViewState(toViewState(result, installedVersion))
    } catch {
      if (isCurrent()) setViewState(UNAVAILABLE_STATE)
    } finally {
      if (abortRef.current === controller) abortRef.current = null
      if (isCurrent()) checkingRef.current = false
    }
  }

  const effectiveState = settings.enabled
    ? viewState
    : DISABLED_VIEW_STATE
  const status = viewStateText(effectiveState, locale)
  const checking = effectiveState.kind === 'checking'
  const release = effectiveState.kind === 'update_available'
    ? effectiveState
    : null

  return (
    <section
      className="update-check-control"
      aria-labelledby={titleId}
      aria-busy={checking}
      data-update-state={effectiveState.kind}
      data-testid="update-check-control"
    >
      <div className="update-check-control-heading">
        <div>
          <span className="update-check-control-eyebrow">
            {text(UPDATE_CHECK_TEXT.eyebrow)}
          </span>
          <h3 id={titleId}>{text(UPDATE_CHECK_TEXT.title)}</h3>
        </div>
        <label className="update-check-toggle">
          <input
            type="checkbox"
            role="switch"
            checked={settings.enabled}
            aria-describedby={
              `${manualDescriptionId} ${privacyDescriptionId}`
            }
            onChange={changeEnabled}
          />
          <span>{text(UPDATE_CHECK_TEXT.enabled)}</span>
        </label>
      </div>

      <p id={manualDescriptionId} className="update-check-manual-note">
        {text(UPDATE_CHECK_TEXT.manualOnly)}
      </p>
      <p id={privacyDescriptionId} className="update-check-privacy-note">
        {text(UPDATE_CHECK_TEXT.privacy)}
      </p>

      <div className="update-check-actions">
        <button
          type="button"
          disabled={!settings.enabled || checking}
          aria-describedby={privacyDescriptionId}
          onClick={() => void checkNow()}
        >
          {checking
            ? text(UPDATE_CHECK_TEXT.checkingButton)
            : text(UPDATE_CHECK_TEXT.checkButton)}
        </button>
        {release && (
          <a
            className="update-check-release-link"
            href={release.releasePageUrl}
            target="_blank"
            rel="noopener noreferrer"
          >
            {formatLocalizedText(
              locale,
              UPDATE_CHECK_TEXT.openRelease,
              { version: release.latestVersion },
            )}
          </a>
        )}
      </div>

      <p
        className={`update-check-status update-check-status-${effectiveState.kind}`}
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {status}
      </p>

      {persistenceFailed && (
        <p
          className="update-check-persistence-error"
          role="alert"
          aria-live="assertive"
        >
          {text(UPDATE_CHECK_TEXT.persistenceFailed)}
        </p>
      )}
    </section>
  )
}

function toViewState(
  result: unknown,
  installedVersion: unknown,
): UpdateCheckViewState {
  try {
    const kind = ownDataValue(result, 'kind')
    if (kind === 'disabled') {
      return exactDataRecord(result, ['kind']) ? IDLE_STATE : UNAVAILABLE_STATE
    }
    if (kind === 'unavailable') {
      const unavailable = exactDataRecord(result, ['kind', 'reason'])
      if (!unavailable) return UNAVAILABLE_STATE
      return unavailable.reason === 'no_published_release'
        ? NO_PUBLISHED_RELEASE_STATE
        : UNAVAILABLE_STATE
    }

    if (kind !== 'up_to_date' && kind !== 'update_available') {
      return UNAVAILABLE_STATE
    }
    const record = exactDataRecord(
      result,
      kind === 'up_to_date'
        ? ['kind', 'currentVersion', 'latestVersion']
        : [
          'kind',
          'currentVersion',
          'latestVersion',
          'releasePageUrl',
        ],
    )
    if (
      !record
      || typeof record.currentVersion !== 'string'
      || typeof record.latestVersion !== 'string'
    ) return UNAVAILABLE_STATE
    if (
      compareSemanticVersions(
        installedVersion,
        record.currentVersion,
      ) !== 0
    ) return UNAVAILABLE_STATE
    const comparison = compareSemanticVersions(
      record.currentVersion,
      record.latestVersion,
    )
    if (kind === 'up_to_date') {
      if (comparison === null || comparison < 0) return UNAVAILABLE_STATE
      return Object.freeze({
        kind: 'up_to_date',
        currentVersion: record.currentVersion,
        latestVersion: record.latestVersion,
      })
    }

    if (comparison !== -1) return UNAVAILABLE_STATE
    const releasePageUrl = trustedReleasePageUrl(
      record.releasePageUrl,
      record.latestVersion,
    )
    if (!releasePageUrl) return UNAVAILABLE_STATE
    return Object.freeze({
      kind: 'update_available',
      currentVersion: record.currentVersion,
      latestVersion: record.latestVersion,
      releasePageUrl,
    })
  } catch {
    return UNAVAILABLE_STATE
  }
}

function ownDataValue(value: unknown, key: string): unknown {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return undefined
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return undefined
  const descriptor = Object.getOwnPropertyDescriptor(value, key)
  return descriptor && 'value' in descriptor && descriptor.enumerable
    ? descriptor.value
    : undefined
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const actualKeys = Reflect.ownKeys(descriptors)
  if (
    actualKeys.length !== keys.length
    || actualKeys.some((key) => typeof key !== 'string')
    || keys.some((key) => !Object.hasOwn(descriptors, key))
  ) return null

  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}

function trustedReleasePageUrl(
  value: unknown,
  latestVersion: string,
): string | null {
  if (typeof value !== 'string') return null
  for (const tag of [latestVersion, `v${latestVersion}`]) {
    const expected =
      `${ORIGAMI2_GITHUB_RELEASE_PAGE_PREFIX}${encodeURIComponent(tag)}`
    if (value === expected) return expected
  }
  return null
}

function viewStateText(
  state: UpdateCheckViewState,
  locale: Locale,
): string {
  switch (state.kind) {
    case 'idle':
      return selectLocalizedText(locale, UPDATE_CHECK_TEXT.idle)
    case 'disabled':
      return selectLocalizedText(locale, UPDATE_CHECK_TEXT.disabled)
    case 'checking':
      return selectLocalizedText(locale, UPDATE_CHECK_TEXT.checking)
    case 'up_to_date':
      return formatLocalizedText(locale, UPDATE_CHECK_TEXT.upToDate, {
        currentVersion: state.currentVersion,
        latestVersion: state.latestVersion,
      })
    case 'update_available':
      return formatLocalizedText(locale, UPDATE_CHECK_TEXT.updateAvailable, {
        currentVersion: state.currentVersion,
        latestVersion: state.latestVersion,
      })
    case 'no_published_release':
      return selectLocalizedText(
        locale,
        UPDATE_CHECK_TEXT.noPublishedRelease,
      )
    case 'unavailable':
      return selectLocalizedText(locale, UPDATE_CHECK_TEXT.unavailable)
  }
}

const DISABLED_VIEW_STATE = Object.freeze({
  kind: 'disabled',
} as const)

const UPDATE_CHECK_TEXT = Object.freeze({
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
