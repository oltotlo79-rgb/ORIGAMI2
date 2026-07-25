import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  RuntimeUpdaterControl,
  type RuntimeUpdaterUiController,
} from '../src/components/RuntimeUpdaterControl.tsx'
import { createLocaleStore } from '../src/lib/i18n.ts'

afterEach(cleanup)
const candidate = { version: '2.0.0', releaseNotes: '安全性と安定性を改善', platform: 'windows-x64' as const, byteLength: 25 * 1024 * 1024 }
const controller = (overrides: Partial<RuntimeUpdaterUiController> = {}): RuntimeUpdaterUiController => ({
  async recoverPending() { return 'ready' }, async check() { return candidate },
  async downloadAndVerify() { return 'verified' }, async restartAndApply() { return 'applied' }, ...overrides,
})

describe('RuntimeUpdaterControl', () => {
  it('requires explicit download and restart actions while showing release metadata', async () => {
    const value = controller()
    render(<RuntimeUpdaterControl controller={value} />)
    await screen.findByText('更新を手動で確認できます')
    fireEvent.click(screen.getByRole('button', { name: '更新を確認' }))
    await screen.findByText('2.0.0')
    expect(screen.getByText('windows-x64')).toBeTruthy()
    expect(screen.getByText('25.0 MB')).toBeTruthy()
    expect(screen.getByText('安全性と安定性を改善')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'ダウンロードして検証' }))
    await screen.findByText(/検証済み/u)
    fireEvent.click(screen.getByRole('button', { name: '再起動して適用' }))
    await screen.findByText('更新の適用を確認しました')
  })

  it('preserves busy, action, detail, and live-region semantics across the happy path', async () => {
    const recovery = promiseWithResolvers<'ready'>()
    const checked = promiseWithResolvers<typeof candidate>()
    const downloaded = promiseWithResolvers<'verified'>()
    const applied = promiseWithResolvers<'applied'>()
    render(<RuntimeUpdaterControl controller={controller({
      recoverPending: () => recovery.promise,
      check: () => checked.promise,
      downloadAndVerify: () => downloaded.promise,
      restartAndApply: () => applied.promise,
    })} />)

    const heading = screen.getByRole('heading', { name: 'アプリ更新' })
    const region = heading.closest('section')
    const status = screen.getByRole('status')
    expect(region?.getAttribute('aria-labelledby')).toBe(heading.id)
    expect(region?.getAttribute('aria-busy')).toBe('true')
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.textContent).toBe('保留中の更新を確認しています')
    expect(screen.queryByRole('button')).toBeNull()

    recovery.resolve('ready')
    await screen.findByText('更新を手動で確認できます')
    expect(region?.getAttribute('aria-busy')).toBe('false')
    fireEvent.click(screen.getByRole('button', { name: '更新を確認' }))
    expect(status.textContent).toBe('更新を確認しています')
    expect(region?.getAttribute('aria-busy')).toBe('true')
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeTruthy()

    checked.resolve(candidate)
    await screen.findByText(
      '更新を利用できます。内容を確認してダウンロードしてください',
    )
    expect(region?.getAttribute('aria-busy')).toBe('false')
    expect(screen.getByLabelText('更新内容')).toBeTruthy()
    expect(screen.getByText('バージョン')).toBeTruthy()
    expect(screen.getByText('プラットフォーム')).toBeTruthy()
    expect(screen.getByText('サイズ')).toBeTruthy()
    expect(screen.getByText('リリースノート')).toBeTruthy()
    expect(screen.getByText('25.0 MB')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', {
      name: 'ダウンロードして検証',
    }))
    expect(status.textContent).toBe(
      'ダウンロードして署名とchecksumを検証しています',
    )
    expect(region?.getAttribute('aria-busy')).toBe('true')
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeTruthy()

    downloaded.resolve('verified')
    await screen.findByText(
      '検証済みです。明示的に再起動して適用できます',
    )
    expect(region?.getAttribute('aria-busy')).toBe('false')
    fireEvent.click(screen.getByRole('button', { name: '再起動して適用' }))
    expect(status.textContent).toBe('再起動と適用を準備しています')
    expect(region?.getAttribute('aria-busy')).toBe('true')
    expect(screen.queryByRole('button')).toBeNull()

    applied.resolve('applied')
    await screen.findByText('更新の適用を確認しました')
    expect(region?.getAttribute('aria-busy')).toBe('false')
    expect(screen.queryByRole('button')).toBeNull()
  })

  it.each(['offline', 'rollback', 'signature', 'disk', 'malformed'] as const)('fails closed for %s', async (error) => {
    render(<RuntimeUpdaterControl controller={controller({ async check() { return error } })} />)
    await screen.findByRole('button', { name: '更新を確認' })
    fireEvent.click(screen.getByRole('button', { name: '更新を確認' }))
    await screen.findByText(`更新を安全に停止しました: ${error}`)
  })

  it('recovers pending state before enabling checks and cancels an in-flight request', async () => {
    let resolveRecovery!: (value: 'ready') => void
    const recoverPending = vi.fn(() => new Promise<'ready'>((resolve) => { resolveRecovery = resolve }))
    const check = vi.fn((_signal: AbortSignal) => new Promise<typeof candidate>(() => undefined))
    render(<RuntimeUpdaterControl controller={controller({ recoverPending, check })} />)
    expect(screen.queryByRole('button', { name: '更新を確認' })).toBeNull()
    resolveRecovery('ready')
    await screen.findByRole('button', { name: '更新を確認' })
    fireEvent.click(screen.getByRole('button', { name: '更新を確認' }))
    fireEvent.click(await screen.findByRole('button', { name: 'キャンセル' }))
    await waitFor(() => expect(screen.getByText('操作をキャンセルしました')).toBeTruthy())
    expect(check.mock.calls[0]?.[0].aborted).toBe(true)
  })

  it('retranslates available details and errors without repeating an operation', async () => {
    const locales = createLocaleStore({
      readStoredLocale: () => 'ja',
      writeStoredLocale() {},
      applyDocumentLanguage() {},
    })
    const recoverPending = vi.fn(async () => 'ready' as const)
    const check = vi.fn(async () => candidate)
    const downloadAndVerify = vi.fn(async () => 'signature' as const)
    locales.initialize()
    render(<RuntimeUpdaterControl
      controller={controller({ recoverPending, check, downloadAndVerify })}
      localeStore={locales}
    />)

    fireEvent.click(await screen.findByRole('button', {
      name: '更新を確認',
    }))
    await screen.findByLabelText('更新内容')
    expect(screen.getByRole('status').textContent).toBe(
      '更新を利用できます。内容を確認してダウンロードしてください',
    )

    act(() => {
      locales.setLocale('en')
    })
    expect(screen.getByRole('heading', { name: 'App update' })).toBeTruthy()
    expect(screen.getByText(
      'Checks never send project data. Payloads are fetched only after an explicit action and are verified by signature and checksum.',
    )).toBeTruthy()
    expect(screen.getByLabelText('Update details')).toBeTruthy()
    expect(screen.getByText('Version')).toBeTruthy()
    expect(screen.getByText('Platform')).toBeTruthy()
    expect(screen.getByText('Size')).toBeTruthy()
    expect(screen.getByText('Release notes')).toBeTruthy()
    expect(screen.getByText('25.0 MB')).toBeTruthy()
    expect(screen.getByRole('status').textContent).toBe(
      'An update is available. Review it before downloading.',
    )

    fireEvent.click(screen.getByRole('button', {
      name: 'Download and verify',
    }))
    await screen.findByText('Update stopped safely: signature')
    act(() => {
      locales.setLocale('ja')
    })
    expect(screen.getByRole('status').textContent).toBe(
      '更新を安全に停止しました: signature',
    )
    expect(screen.getByRole('button', { name: '更新を確認' })).toBeTruthy()
    expect(recoverPending).toHaveBeenCalledTimes(1)
    expect(check).toHaveBeenCalledTimes(1)
    expect(downloadAndVerify).toHaveBeenCalledTimes(1)
    locales.dispose()
  })

  it('honors disabled settings, discloses privacy, and renders injected English locale', async () => {
    const recoverPending = vi.fn(async () => 'ready' as const)
    const english = createLocaleStore({
      readStoredLocale: () => 'en', writeStoredLocale() {}, applyDocumentLanguage() {},
    })
    english.initialize()
    render(<RuntimeUpdaterControl controller={controller({ recoverPending })} enabled={false} localeStore={english} />)
    expect(await screen.findByText('Update checks are disabled')).toBeTruthy()
    const heading = screen.getByRole('heading', { name: 'App update' })
    const region = heading.closest('section')
    expect(region?.getAttribute('aria-labelledby')).toBe(heading.id)
    expect(region?.getAttribute('aria-busy')).toBe('false')
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite')
    expect(screen.getByText(/Checks never send project data/u)).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Check for updates' })).toBeNull()
    expect(recoverPending).not.toHaveBeenCalled()
    english.dispose()
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
