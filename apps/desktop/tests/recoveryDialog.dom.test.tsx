import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'

import {
  RecoveryDialog,
  type RecoveryDialogProps,
} from '../src/components/RecoveryDialog'
import type {
  RecoveryCandidateAvailable,
  RecoveryCandidateInvalid,
} from '../src/lib/recoveryClient'

const RECOVERY_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'

const AVAILABLE: RecoveryCandidateAvailable = {
  schema_version: 1,
  status: 'available',
  recovery_id: RECOVERY_ID,
  project_id: PROJECT_ID,
  updated_at_unix_ms: 1_753_000_000_000,
}

const INVALID: RecoveryCandidateInvalid = {
  schema_version: 1,
  status: 'invalid',
  recovery_id: RECOVERY_ID,
}

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('RecoveryDialog DOM interactions', () => {
  it('shows only safe available metadata and requires an explicit decision', async () => {
    const onRestore = vi.fn()
    const onDiscard = vi.fn()
    const onRetry = vi.fn()
    renderDialog({ onRestore, onDiscard, onRetry })

    const dialog = screen.getByRole('dialog')
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(screen.getByText('未保存の編集内容を復元しますか？')).toBeTruthy()
    expect(screen.getByText('最終更新')).toBeTruthy()
    expect(screen.getByText(/元のファイルを自動で上書き/u)).toBeTruthy()
    expect(screen.getByRole('button', { name: '復元する' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '再確認' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '破棄する' })).toBeTruthy()

    const visible = document.body.textContent ?? ''
    expect(visible).not.toContain(RECOVERY_ID)
    expect(visible).not.toContain(PROJECT_ID)
    expect(visible).not.toContain('C:\\')
    expect(visible).not.toContain('document')

    fireEvent.keyDown(document, { key: 'Escape' })
    fireEvent.click(screen.getByTestId('recovery-dialog-backdrop'))
    expect(onRestore).not.toHaveBeenCalled()
    expect(onDiscard).not.toHaveBeenCalled()
    expect(onRetry).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog')).toBeTruthy()

    await waitFor(() => {
      expect(document.activeElement).toBe(
        screen.getByRole('button', { name: '復元する' }),
      )
    })
  })

  it('offers retry and discard, but never restore, for an invalid candidate', async () => {
    const onDiscard = vi.fn()
    const onRetry = vi.fn()
    renderDialog({ candidate: INVALID, onDiscard, onRetry })

    expect(screen.getByText('復旧データを確認できません')).toBeTruthy()
    expect(screen.getByText(/破損しているか/u)).toBeTruthy()
    expect(screen.queryByRole('button', { name: '復元する' })).toBeNull()

    const retry = screen.getByRole('button', { name: '再確認' })
    await waitFor(() => expect(document.activeElement).toBe(retry))
    fireEvent.click(retry)
    await waitFor(() => {
      expect(onRetry).toHaveBeenCalledTimes(1)
      expect((screen.getByRole('button', { name: '再確認' }) as HTMLButtonElement).disabled)
        .toBe(false)
    })

    fireEvent.click(screen.getByRole('button', { name: '破棄する' }))
    await waitFor(() => {
      expect(onDiscard).toHaveBeenCalledTimes(1)
      expect(onDiscard).toHaveBeenCalledWith(INVALID)
    })
  })

  it('prevents synchronous double submission and disables every action while pending', async () => {
    const deferred = promiseWithResolvers<void>()
    const onRestore = vi.fn(() => deferred.promise)
    renderDialog({ onRestore })

    const restore = screen.getByRole('button', { name: '復元する' })
    fireEvent.click(restore)
    fireEvent.click(restore)
    expect(onRestore).toHaveBeenCalledTimes(1)
    expect(onRestore).toHaveBeenCalledWith(AVAILABLE)
    expect(screen.getByRole('button', { name: '復元中…' })).toBeTruthy()
    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true)
    }

    deferred.resolve()
    await waitFor(() => {
      expect(screen.getByRole('button', { name: '復元する' })).toBeTruthy()
    })
  })

  it('redacts callback failures and recovers busy state under StrictMode', async () => {
    const onRestore = vi.fn(async () => {
      throw new Error('C:\\Users\\private\\recovery.json: raw native failure')
    })
    render(
      <StrictMode>
        {dialogElement({ onRestore })}
      </StrictMode>,
    )

    fireEvent.click(screen.getByRole('button', { name: '復元する' }))
    await waitFor(() => {
      expect(screen.getByRole('alert').textContent).toBe(
        '復旧データを処理できませんでした。もう一度お試しください。',
      )
      expect(
        (screen.getByRole('button', { name: '復元する' }) as HTMLButtonElement)
          .disabled,
      ).toBe(false)
    })
    expect(document.body.textContent).not.toContain('private')
    expect(document.body.textContent).not.toContain('recovery.json')

    fireEvent.click(screen.getByRole('button', { name: '復元する' }))
    await waitFor(() => expect(onRestore).toHaveBeenCalledTimes(2))
  })

  it('renders only fixed copy for a parent-reported error', () => {
    renderDialog({ error: true })
    expect(screen.getByRole('alert').textContent).toBe(
      '復旧データを処理できませんでした。もう一度お試しください。',
    )
  })

  it('traps focus, blocks Escape, and restores focus when removed', async () => {
    const outside = document.createElement('button')
    outside.textContent = 'outside'
    document.body.append(outside)
    outside.focus()

    const onRestore = vi.fn()
    const onDiscard = vi.fn()
    const { unmount } = renderDialog({ onRestore, onDiscard })
    const restore = screen.getByRole('button', { name: '復元する' })
    const discard = screen.getByRole('button', { name: '破棄する' })
    await waitFor(() => expect(document.activeElement).toBe(restore))

    restore.focus()
    fireEvent.keyDown(restore, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(discard)
    fireEvent.keyDown(discard, { key: 'Tab' })
    expect(document.activeElement).toBe(restore)

    outside.focus()
    expect(document.activeElement).toBe(restore)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onRestore).not.toHaveBeenCalled()
    expect(onDiscard).not.toHaveBeenCalled()

    unmount()
    expect(document.activeElement).toBe(outside)
  })

  it('leaves background inert ownership entirely with the parent', () => {
    const background = document.createElement('main')
    background.textContent = 'editor'
    document.body.append(background)
    renderDialog()
    expect(background.hasAttribute('inert')).toBe(false)
    expect(document.body.hasAttribute('inert')).toBe(false)
  })

  it('respects parent busy state without dispatching actions', () => {
    const onRestore = vi.fn()
    const onDiscard = vi.fn()
    const onRetry = vi.fn()
    renderDialog({
      busy: true,
      onRestore,
      onDiscard,
      onRetry,
    })

    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true)
      fireEvent.click(button)
    }
    expect(onRestore).not.toHaveBeenCalled()
    expect(onDiscard).not.toHaveBeenCalled()
    expect(onRetry).not.toHaveBeenCalled()
  })
})

function renderDialog(overrides: Partial<RecoveryDialogProps> = {}) {
  return render(dialogElement(overrides))
}

function dialogElement(overrides: Partial<RecoveryDialogProps> = {}) {
  return (
    <RecoveryDialog
      candidate={overrides.candidate ?? AVAILABLE}
      busy={overrides.busy ?? false}
      error={overrides.error ?? false}
      onRestore={overrides.onRestore ?? vi.fn()}
      onDiscard={overrides.onDiscard ?? vi.fn()}
      onRetry={overrides.onRetry ?? vi.fn()}
    />
  )
}

function promiseWithResolvers<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
