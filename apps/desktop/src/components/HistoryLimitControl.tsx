import {
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
} from 'react'

import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  HISTORY_LIMIT_SCHEMA_VERSION,
  historyLimitClient,
  historyLimitSettingsMatchExpectedBinding,
  isHistoryEntryLimit,
  MAX_HISTORY_ENTRY_LIMIT,
  MIN_HISTORY_ENTRY_LIMIT,
  parseHistoryLimitSettings,
  type HistoryLimitClient,
  type HistoryLimitExpectedProjectBinding,
  type HistoryLimitSettings,
  type SetHistoryEntryLimitRequest,
} from '../lib/historyLimitClient.ts'

export type HistoryLimitControlProps = Readonly<{
  settings: HistoryLimitSettings
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  client?: HistoryLimitClient
  disabled?: boolean
  onApplied: (
    settings: HistoryLimitSettings,
  ) => void | Promise<void>
  localeStore?: LocaleStore
}>

type ControlAuthority = Readonly<{
  settings: HistoryLimitSettings
  expected: HistoryLimitExpectedProjectBinding
  key: string
}>

export function HistoryLimitControl({
  settings,
  expectedProjectInstanceId,
  expectedProjectId,
  expectedRevision,
  client = historyLimitClient,
  disabled = false,
  onApplied,
  localeStore: localeStore_ = localeStore,
}: HistoryLimitControlProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const authority = prepareControlAuthority(
    settings,
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  )
  const authorityKey = authority?.key ?? null
  const authoritativeDraft = authority
    ? String(authority.settings.historyEntryLimit)
    : ''
  const [draft, setDraft] = useState(
    () => authoritativeDraft,
  )
  const [pending, setPending] = useState(false)
  const [operationError, setOperationError] = useState(authority === null)
  const mountedRef = useRef(false)
  const submittingRef = useRef(false)
  const operationSequenceRef = useRef(0)
  const authorityKeyRef = useRef(authorityKey)
  const clientRef = useRef(client)
  const disabledRef = useRef(disabled)
  const onAppliedRef = useRef(onApplied)

  disabledRef.current = disabled
  onAppliedRef.current = onApplied

  if (
    authorityKeyRef.current !== authorityKey
    || clientRef.current !== client
  ) {
    authorityKeyRef.current = authorityKey
    clientRef.current = client
    submittingRef.current = false
    operationSequenceRef.current += 1
  }

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      submittingRef.current = false
      operationSequenceRef.current += 1
    }
  }, [])

  useEffect(() => {
    operationSequenceRef.current += 1
    submittingRef.current = false
    setPending(false)
    setDraft(authoritativeDraft)
    setOperationError(authorityKey === null)
  }, [authorityKey, authoritativeDraft, client])

  const candidate = parseDraftHistoryEntryLimit(draft)
  const draftInvalid = candidate === null
  const unchanged = authority !== null
    && candidate === authority.settings.historyEntryLimit
  const effectiveDisabled = disabled || pending || authority === null
  const applyDisabled = effectiveDisabled || draftInvalid || unchanged
  const errorMessage = operationError || authority === null
    ? text(HISTORY_LIMIT_TEXT.applyError)
    : draftInvalid
      ? text(HISTORY_LIMIT_TEXT.invalidValueError)
      : null
  const errorId = errorMessage ? 'history-limit-error' : null
  const describedBy = errorId
    ? `history-limit-description ${errorId}`
    : 'history-limit-description'

  const changeDraft = (event: ChangeEvent<HTMLInputElement>) => {
    setDraft(event.currentTarget.value)
    setOperationError(authority === null)
  }

  const preventImplicitApply = (event: KeyboardEvent<HTMLInputElement>) => {
    if (
      event.key === 'Enter'
      && !event.nativeEvent.isComposing
    ) event.preventDefault()
  }

  const apply = async () => {
    if (
      submittingRef.current
      || disabledRef.current
      || authority === null
      || candidate === null
      || candidate === authority.settings.historyEntryLimit
    ) return

    submittingRef.current = true
    const operation = ++operationSequenceRef.current
    const request: SetHistoryEntryLimitRequest = Object.freeze({
      schemaVersion: HISTORY_LIMIT_SCHEMA_VERSION,
      expectedProjectInstanceId:
        authority.expected.expectedProjectInstanceId,
      expectedProjectId: authority.expected.expectedProjectId,
      expectedRevision: authority.expected.expectedRevision,
      historyEntryLimit: candidate,
    })
    const requestAuthorityKey = authority.key
    const requestClient = client
    setPending(true)
    setOperationError(false)

    const isCurrent = () => (
      mountedRef.current
      && operation === operationSequenceRef.current
      && requestAuthorityKey === authorityKeyRef.current
      && requestClient === clientRef.current
    )

    try {
      const rawResponse = await requestClient.set(request)
      if (!isCurrent()) return
      const response = parseHistoryLimitSettings(rawResponse)
      if (
        !response
        || !historyLimitSettingsMatchExpectedBinding(response, {
          expectedProjectInstanceId: request.expectedProjectInstanceId,
          expectedProjectId: request.expectedProjectId,
          expectedRevision: request.expectedRevision,
        })
        || response.historyEntryLimit !== request.historyEntryLimit
      ) {
        setOperationError(true)
        return
      }
      await onAppliedRef.current(response)
    } catch {
      if (isCurrent()) setOperationError(true)
    } finally {
      if (isCurrent()) {
        submittingRef.current = false
        setPending(false)
      }
    }
  }

  return (
    <fieldset
      className="history-limit-control"
      aria-busy={pending}
    >
      <legend>{text(HISTORY_LIMIT_TEXT.legend)}</legend>
      <p>
        {text(HISTORY_LIMIT_TEXT.currentLimit)}
        {' '}
        <output aria-label={text(HISTORY_LIMIT_TEXT.currentLimitAriaLabel)}>
          {authority
            ? formatLocalizedText(locale, HISTORY_LIMIT_TEXT.entryCount, {
              count: authority.settings.historyEntryLimit,
            })
            : text(HISTORY_LIMIT_TEXT.unavailable)}
        </output>
      </p>
      <label htmlFor="history-entry-limit-input">
        {text(HISTORY_LIMIT_TEXT.inputLabel)}
      </label>
      <input
        id="history-entry-limit-input"
        name="history_entry_limit"
        type="number"
        inputMode="numeric"
        min={MIN_HISTORY_ENTRY_LIMIT}
        max={MAX_HISTORY_ENTRY_LIMIT}
        step={1}
        value={draft}
        disabled={effectiveDisabled}
        aria-invalid={authority === null || draftInvalid}
        aria-describedby={describedBy}
        onChange={changeDraft}
        onKeyDown={preventImplicitApply}
      />
      <button
        type="button"
        disabled={applyDisabled}
        onClick={() => void apply()}
      >
        {pending
          ? text(HISTORY_LIMIT_TEXT.applying)
          : text(HISTORY_LIMIT_TEXT.apply)}
      </button>
      <p id="history-limit-description">
        {text(HISTORY_LIMIT_TEXT.description)}
      </p>
      {errorMessage && (
        <p
          id="history-limit-error"
          role="alert"
          aria-live="assertive"
        >
          {errorMessage}
        </p>
      )}
    </fieldset>
  )
}

const HISTORY_LIMIT_TEXT = Object.freeze({
  invalidValueError: Object.freeze({
    ja: '履歴件数は1から128までの整数で入力してください。',
    en: 'Enter a whole-number history limit from 1 to 128.',
  }),
  applyError: Object.freeze({
    ja: '履歴件数を変更できませんでした。現在のプロジェクトを確認して、もう一度お試しください。',
    en: 'The history limit could not be changed. Check the current project and try again.',
  }),
  legend: Object.freeze({
    ja: 'Undo・Redo履歴の上限',
    en: 'Undo/Redo history limit',
  }),
  currentLimit: Object.freeze({ ja: '現在の上限:', en: 'Current limit:' }),
  currentLimitAriaLabel: Object.freeze({
    ja: '現在の履歴件数上限',
    en: 'Current history entry limit',
  }),
  entryCount: Object.freeze({ ja: '{count}件', en: '{count} entries' }),
  unavailable: Object.freeze({
    ja: '確認できません',
    en: 'Unavailable',
  }),
  inputLabel: Object.freeze({
    ja: '履歴件数の上限',
    en: 'History entry limit',
  }),
  applying: Object.freeze({ ja: '適用中…', en: 'Applying…' }),
  apply: Object.freeze({ ja: '適用', en: 'Apply' }),
  description: Object.freeze({
    ja: '上限を減らすと、古いUndo/Redo履歴は直ちに削除されます。あとで上限を増やしても、削除された履歴は戻りません。',
    en: 'Reducing the limit immediately removes older Undo/Redo entries. Increasing it later does not restore removed history.',
  }),
})

function prepareControlAuthority(
  settings: unknown,
  expectedProjectInstanceId: unknown,
  expectedProjectId: unknown,
  expectedRevision: unknown,
): ControlAuthority | null {
  const expected = {
    expectedProjectInstanceId,
    expectedProjectId,
    expectedRevision,
  }
  const parsedSettings = parseHistoryLimitSettings(settings)
  if (
    !parsedSettings
    || !historyLimitSettingsMatchExpectedBinding(parsedSettings, expected)
  ) return null

  const parsedExpected: HistoryLimitExpectedProjectBinding = Object.freeze({
    expectedProjectInstanceId: parsedSettings.projectInstanceId,
    expectedProjectId: parsedSettings.projectId,
    expectedRevision: parsedSettings.revision,
  })
  return Object.freeze({
    settings: parsedSettings,
    expected: parsedExpected,
    key: [
      parsedSettings.projectInstanceId,
      parsedSettings.projectId,
      String(parsedSettings.revision),
      String(parsedSettings.historyEntryLimit),
    ].join(':'),
  })
}

function parseDraftHistoryEntryLimit(value: string): number | null {
  if (!/^(?:[1-9]|[1-9][0-9]|1[01][0-9]|12[0-8])$/u.test(value)) {
    return null
  }
  const candidate = Number(value)
  return isHistoryEntryLimit(candidate) ? candidate : null
}
