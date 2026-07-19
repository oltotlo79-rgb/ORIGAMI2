import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'

import { RecoveryStartupOverlay } from '../src/components/RecoveryStartupOverlay'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('RecoveryStartupOverlay DOM interactions', () => {
  it('shows a non-dismissible checking gate without actions', () => {
    const onRetry = vi.fn()
    render(
      <RecoveryStartupOverlay
        phase="checking"
        busy={false}
        onRetry={onRetry}
      />,
    )

    const dialog = screen.getByRole('dialog')
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(dialog.getAttribute('aria-busy')).toBe('true')
    expect(screen.getByRole('status').textContent).toContain(
      '編集を安全に開始できるか確認しています',
    )
    expect(screen.queryByRole('button')).toBeNull()

    fireEvent.keyDown(document, { key: 'Escape' })
    fireEvent.click(screen.getByTestId('recovery-startup-backdrop'))
    expect(onRetry).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog')).toBeTruthy()
  })

  it('shows one fixed retry action after discovery failure', async () => {
    const onRetry = vi.fn()
    render(
      <RecoveryStartupOverlay
        phase="failed"
        busy={false}
        onRetry={onRetry}
      />,
    )

    expect(screen.getByRole('alert').textContent).toBe(
      '編集を開始する前に復旧データの確認が必要です。再試行してください。',
    )
    const retry = screen.getByRole('button', { name: '再試行' })
    await waitFor(() => expect(document.activeElement).toBe(retry))
    fireEvent.click(retry)
    await waitFor(() => expect(onRetry).toHaveBeenCalledTimes(1))
  })

  it('prevents double retry while the parent request is pending', async () => {
    const deferred = promiseWithResolvers<void>()
    const onRetry = vi.fn(() => deferred.promise)
    render(
      <RecoveryStartupOverlay
        phase="failed"
        busy={false}
        onRetry={onRetry}
      />,
    )

    const retry = screen.getByRole('button', { name: '再試行' })
    fireEvent.click(retry)
    fireEvent.click(retry)
    expect(onRetry).toHaveBeenCalledTimes(1)
    expect(
      (screen.getByRole('button', { name: '再確認中…' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true)

    deferred.resolve()
    await waitFor(() => {
      expect(
        (screen.getByRole('button', { name: '再試行' }) as HTMLButtonElement)
          .disabled,
      ).toBe(false)
    })
  })

  it('contains retry rejection and clears local busy state under StrictMode', async () => {
    const onRetry = vi.fn(async () => {
      throw new Error('C:\\private\\recovery-slot.ori2')
    })
    render(
      <StrictMode>
        <RecoveryStartupOverlay
          phase="failed"
          busy={false}
          onRetry={onRetry}
        />
      </StrictMode>,
    )

    fireEvent.click(screen.getByRole('button', { name: '再試行' }))
    await waitFor(() => {
      expect(onRetry).toHaveBeenCalledTimes(1)
      expect(
        (screen.getByRole('button', { name: '再試行' }) as HTMLButtonElement)
          .disabled,
      ).toBe(false)
    })
    expect(document.body.textContent).not.toContain('private')
    expect(document.body.textContent).not.toContain('recovery-slot')
  })

  it('keeps focus inside the failed gate and restores the previous target', async () => {
    const outside = document.createElement('button')
    outside.textContent = 'outside'
    document.body.append(outside)
    outside.focus()
    const { unmount } = render(
      <RecoveryStartupOverlay
        phase="failed"
        busy={false}
        onRetry={vi.fn()}
      />,
    )
    const retry = screen.getByRole('button', { name: '再試行' })
    await waitFor(() => expect(document.activeElement).toBe(retry))

    outside.focus()
    expect(document.activeElement).toBe(retry)
    fireEvent.keyDown(retry, { key: 'Tab' })
    expect(document.activeElement).toBe(retry)

    unmount()
    expect(document.activeElement).toBe(outside)
  })

  it('does not take ownership of background inert state', () => {
    const background = document.createElement('main')
    background.textContent = 'editor'
    document.body.append(background)
    render(
      <RecoveryStartupOverlay
        phase="failed"
        busy={false}
        onRetry={vi.fn()}
      />,
    )
    expect(background.hasAttribute('inert')).toBe(false)
    expect(document.body.hasAttribute('inert')).toBe(false)
  })
})

function promiseWithResolvers<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
