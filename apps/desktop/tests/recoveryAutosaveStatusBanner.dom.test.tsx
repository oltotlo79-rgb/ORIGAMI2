import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  RECOVERY_AUTOSAVE_MONITOR_WARNING,
  RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
  RecoveryAutosaveStatusBanner,
} from '../src/components/RecoveryAutosaveStatusBanner.tsx'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('RecoveryAutosaveStatusBanner', () => {
  it('localizes every autosave transition announcement in English', () => {
    const english = localeFixture('en')
    const rendered = render(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'persistence_failed', transition_id: 7 }}
        localeStore={english}
      />,
    )
    expect(screen.getByRole('alert').textContent).toBe(
      RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
    )

    rendered.rerender(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'monitor_unavailable' }}
        localeStore={english}
      />,
    )
    expect(screen.getByRole('alert').textContent).toBe(
      RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
    )

    rendered.rerender(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'operational', transition_id: 8, recovered: true }}
        localeStore={english}
      />,
    )
    expect(screen.getByRole('status').textContent).toBe(
      RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
    )
  })

  it('keeps persistence failure visible as a fixed assertive warning', () => {
    render(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'persistence_failed', transition_id: 7 }}
      />,
    )
    const warning = screen.getByRole('alert')
    expect(warning.textContent).toBe(RECOVERY_AUTOSAVE_PERSISTENCE_WARNING)
    expect(warning.getAttribute('aria-live')).toBe('assertive')
    expect(warning.getAttribute('aria-atomic')).toBe('true')
    expect(warning.classList.contains('is-persistence-failed')).toBe(true)
    expect(warning.textContent).not.toMatch(
      /(?:[A-Z]:\\|\/Users\/|permission|denied|\.ori2)/iu,
    )
  })

  it('uses a separate fixed warning when monitoring itself is unavailable', () => {
    render(
      <RecoveryAutosaveStatusBanner view={{ kind: 'monitor_unavailable' }} />,
    )
    const warning = screen.getByRole('alert')
    expect(warning.textContent).toBe(RECOVERY_AUTOSAVE_MONITOR_WARNING)
    expect(warning.classList.contains('is-monitor-unavailable')).toBe(true)
  })

  it('announces recovery politely only for a failure-to-operational transition', () => {
    const { rerender } = render(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'operational', transition_id: 1, recovered: false }}
      />,
    )
    expect(screen.queryByRole('status')).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()

    rerender(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'operational', transition_id: 2, recovered: true }}
      />,
    )
    const recovered = screen.getByRole('status')
    expect(recovered.textContent).toBe(RECOVERY_AUTOSAVE_RECOVERED_NOTICE)
    expect(recovered.getAttribute('aria-live')).toBe('polite')
    expect(recovered.classList.contains('visually-hidden')).toBe(true)
  })

  it('renders no warning before the first attempt or in browser mode', () => {
    const { rerender } = render(
      <RecoveryAutosaveStatusBanner
        view={{ kind: 'pending_first_attempt', transition_id: 0 }}
      />,
    )
    expect(document.body.textContent).toBe('')
    rerender(<RecoveryAutosaveStatusBanner view={{ kind: 'inactive' }} />)
    expect(document.body.textContent).toBe('')
  })
})
