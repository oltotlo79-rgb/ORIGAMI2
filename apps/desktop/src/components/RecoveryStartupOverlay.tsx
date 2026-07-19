import { useEffect, useRef, useState } from 'react'

import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import './RecoveryDialog.css'

export type RecoveryStartupOverlayProps = Readonly<{
  phase: 'checking' | 'failed'
  busy: boolean
  onRetry: () => void | Promise<void>
  localeStore?: LocaleStore
}>

/**
 * Blocks the editor while startup recovery discovery has no safe result.
 * There is intentionally no close or continue-without-checking action.
 */
export function RecoveryStartupOverlay({
  phase,
  busy,
  onRetry,
  localeStore: localeStore_ = localeStore,
}: RecoveryStartupOverlayProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const [retryPending, setRetryPending] = useState(false)
  const retryingRef = useRef(false)
  const dialogRef = useRef<HTMLElement>(null)
  const retryButtonRef = useRef<HTMLButtonElement>(null)
  const mountedRef = useRef(true)
  const effectiveBusy = busy || retryPending

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  useEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
    const focusRetry = () => {
      if (phase !== 'failed') return
      if (retryButtonRef.current && !retryButtonRef.current.disabled) {
        retryButtonRef.current.focus()
      } else {
        dialogRef.current?.focus()
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        event.stopImmediatePropagation()
        return
      }
      if (event.key !== 'Tab' || phase !== 'failed') return
      event.preventDefault()
      focusRetry()
    }
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      if (
        phase === 'failed'
        && dialog
        && event.target instanceof Node
        && !dialog.contains(event.target)
      ) focusRetry()
    }
    const frame = requestAnimationFrame(focusRetry)
    document.addEventListener('keydown', handleKeyDown, true)
    document.addEventListener('focusin', handleFocusIn, true)
    return () => {
      cancelAnimationFrame(frame)
      document.removeEventListener('keydown', handleKeyDown, true)
      document.removeEventListener('focusin', handleFocusIn, true)
      if (previouslyFocused?.isConnected) previouslyFocused.focus()
    }
  }, [phase])

  const retry = async () => {
    if (retryingRef.current || effectiveBusy) return
    retryingRef.current = true
    setRetryPending(true)
    try {
      await onRetry()
    } catch {
      // The parent owns the fixed failed state. Raw errors never enter props.
    } finally {
      retryingRef.current = false
      if (mountedRef.current) setRetryPending(false)
    }
  }

  const checking = phase === 'checking'
  return (
    <div
      className="dialog-backdrop recovery-dialog-backdrop"
      data-testid="recovery-startup-backdrop"
    >
      <section
        ref={dialogRef}
        className="recovery-startup-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="recovery-startup-title"
        aria-describedby="recovery-startup-description"
        aria-busy={checking || effectiveBusy}
        tabIndex={-1}
      >
        <header className="recovery-dialog-header">
          <span className="recovery-dialog-eyebrow">
            {text(RECOVERY_STARTUP_TEXT.eyebrow)}
          </span>
          <h2 id="recovery-startup-title">
            {checking
              ? text(RECOVERY_STARTUP_TEXT.checkingTitle)
              : text(RECOVERY_STARTUP_TEXT.failedTitle)}
          </h2>
        </header>
        <div className="recovery-dialog-body">
          <p id="recovery-startup-description" role={checking ? 'status' : 'alert'}>
            {checking
              ? text(RECOVERY_STARTUP_TEXT.checkingDescription)
              : text(RECOVERY_STARTUP_TEXT.failedDescription)}
          </p>
        </div>
        {!checking && (
          <footer className="recovery-dialog-actions">
            <button
              ref={retryButtonRef}
              type="button"
              className="recovery-dialog-primary"
              disabled={effectiveBusy}
              onClick={() => void retry()}
            >
              {effectiveBusy
                ? text(RECOVERY_STARTUP_TEXT.retrying)
                : text(RECOVERY_STARTUP_TEXT.retry)}
            </button>
          </footer>
        )}
      </section>
    </div>
  )
}

const RECOVERY_STARTUP_TEXT = Object.freeze({
  eyebrow: Object.freeze({ ja: '起動時の復旧', en: 'Startup recovery' }),
  checkingTitle: Object.freeze({
    ja: '復旧データを確認しています',
    en: 'Checking recovery data',
  }),
  failedTitle: Object.freeze({
    ja: '復旧データを確認できません',
    en: 'Recovery data could not be checked',
  }),
  checkingDescription: Object.freeze({
    ja: '編集を安全に開始できるか確認しています。しばらくお待ちください。',
    en: 'Checking whether editing can start safely. Please wait.',
  }),
  failedDescription: Object.freeze({
    ja: '編集を開始する前に復旧データの確認が必要です。再試行してください。',
    en: 'Recovery data must be checked before editing can begin. Try again.',
  }),
  retrying: Object.freeze({ ja: '再確認中…', en: 'Checking again…' }),
  retry: Object.freeze({ ja: '再試行', en: 'Try again' }),
})
