import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
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

  it.each(['offline', 'rollback', 'signature', 'disk'] as const)('fails closed for %s', async (error) => {
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

  it('honors disabled settings, discloses privacy, and renders injected English locale', async () => {
    const recoverPending = vi.fn(async () => 'ready' as const)
    const english = createLocaleStore({
      readStoredLocale: () => 'en', writeStoredLocale() {}, applyDocumentLanguage() {},
    })
    english.initialize()
    render(<RuntimeUpdaterControl controller={controller({ recoverPending })} enabled={false} localeStore={english} />)
    expect(await screen.findByText('Update checks are disabled')).toBeTruthy()
    expect(screen.getByText(/Checks never send project data/u)).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Check for updates' })).toBeNull()
    expect(recoverPending).not.toHaveBeenCalled()
    english.dispose()
  })
})
