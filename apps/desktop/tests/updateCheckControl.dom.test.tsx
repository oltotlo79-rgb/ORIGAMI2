import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  UpdateCheckControl,
  UpdateCheckPopover,
  type InstalledVersionProvider,
  type UpdateCheckControlProps,
} from '../src/components/UpdateCheckControl.tsx'
import {
  createUpdateCheckClient,
  type UpdateCheckClient,
  type UpdateCheckResult,
  type UpdateCheckTransportResponse,
} from '../src/lib/githubReleaseUpdate.ts'
import {
  createUpdateCheckSettingsStore,
  encodeUpdateCheckSettings,
  type UpdateCheckSettingsEnvironment,
  type UpdateCheckSettingsStore,
} from '../src/lib/updateCheckSettings.ts'
import { localeFixture } from './localeTestFixture.ts'

const OFFICIAL_RELEASE_URL =
  'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3'

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

describe('UpdateCheckControl', () => {
  it('opens its native localized popover without communicating', () => {
    const localeStore = localeFixture('en')
    const getVersion = vi.fn(async () => '1.0.0')
    const checkNow = vi.fn(async () => upToDate())
    render(
      <StrictMode>
        <UpdateCheckPopover
          localeStore={localeStore}
          versionProvider={provider(getVersion)}
          client={client(checkNow)}
          settingsStore={settingsFixture().store}
        />
      </StrictMode>,
    )
    const details = document.querySelector('.update-check-popover')
    const summary = screen.getByText('Updates')
    expect(details).toBeInstanceOf(HTMLDetailsElement)
    expect((details as HTMLDetailsElement).open).toBe(false)

    fireEvent.click(summary)
    expect((details as HTMLDetailsElement).open).toBe(true)
    act(() => {
      localeStore.setLocale('ja')
    })

    expect(screen.getByText('更新')).toBeTruthy()
    expect(getVersion).not.toHaveBeenCalled()
    expect(checkNow).not.toHaveBeenCalled()
  })

  it('performs no startup communication even under StrictMode', async () => {
    const getVersion = vi.fn(async () => '1.0.0')
    const checkNow = vi.fn(async () => upToDate())

    render(
      <StrictMode>
        {controlElement({
          versionProvider: provider(getVersion),
          client: client(checkNow),
        })}
      </StrictMode>,
    )
    await act(async () => {
      await Promise.resolve()
    })

    expect(getVersion).not.toHaveBeenCalled()
    expect(checkNow).not.toHaveBeenCalled()
    expect(control().dataset.updateState).toBe('idle')
    expect(screen.getByText(
      /GitHub is contacted only when you choose “Check now”/u,
    )).toBeTruthy()
    expect(screen.getByText(
      /Project data, usage data, and the installed version are not sent/u,
    )).toBeTruthy()
    expect(screen.getByText(
      /Nothing is downloaded or installed automatically/u,
    )).toBeTruthy()
  })

  it('checks only from the explicit button and locks synchronous double clicks', async () => {
    const version = promiseWithResolvers<unknown>()
    const getVersion = vi.fn(() => version.promise)
    const checkNow = vi.fn(async () => upToDate())
    renderControl({
      versionProvider: provider(getVersion),
      client: client(checkNow),
    })
    const button = screen.getByRole('button', { name: 'Check now' })

    fireEvent.click(button)
    fireEvent.click(button)

    expect(getVersion).toHaveBeenCalledTimes(1)
    expect(checkNow).not.toHaveBeenCalled()
    expect(control().dataset.updateState).toBe('checking')
    expect(control().getAttribute('aria-busy')).toBe('true')
    expect((screen.getByRole('button', {
      name: 'Checking…',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe(
      'polite',
    )

    version.resolve('1.0.0')
    await waitFor(() => expect(checkNow).toHaveBeenCalledTimes(1))
    await waitFor(() => {
      expect(control().dataset.updateState).toBe('up_to_date')
    })
    expect(checkNow).toHaveBeenCalledWith('1.0.0', { enabled: true })
    expect(screen.getByRole('status').textContent).toBe(
      'Up to date. Installed 1.0.0; latest release 1.0.0.',
    )
  })

  it.each([
    {
      label: 'no published release',
      result: {
        kind: 'unavailable',
        reason: 'no_published_release',
      } satisfies UpdateCheckResult,
      state: 'no_published_release',
      message: 'No published release is available.',
    },
    {
      label: 'temporary unavailability',
      result: {
        kind: 'unavailable',
        reason: 'service_unavailable',
      } satisfies UpdateCheckResult,
      state: 'unavailable',
      message:
        'Update information could not be checked. Please try again later.',
    },
  ])('distinguishes $label in the live status', async ({
    result,
    state,
    message,
  }) => {
    renderControl({ client: client(async () => result) })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))

    await waitFor(() => {
      expect(control().dataset.updateState).toBe(state)
    })
    expect(screen.getByRole('status').textContent).toBe(message)
    expect(screen.queryByRole('link')).toBeNull()
  })

  it('shows an authenticated release as an explicit safe external link only', async () => {
    const open = vi.spyOn(window, 'open')
    renderControl({
      client: client(async () => updateAvailable()),
    })

    expect(screen.queryByRole('link')).toBeNull()
    expect(open).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))

    const link = await screen.findByRole('link', {
      name: 'Open release 1.2.3 on GitHub',
    })
    expect(control().dataset.updateState).toBe('update_available')
    expect(link.getAttribute('href')).toBe(OFFICIAL_RELEASE_URL)
    expect(link.getAttribute('target')).toBe('_blank')
    expect(link.getAttribute('rel')?.split(/\s+/u).sort()).toEqual([
      'noopener',
      'noreferrer',
    ])
    expect(open).not.toHaveBeenCalled()
    expect(document.body.querySelector('[download]')).toBeNull()
  })

  it.each([
    'javascript:alert(1)',
    'https://evil.example/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    'https://alice@github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.2.3',
    `${OFFICIAL_RELEASE_URL}?download=1`,
    `${OFFICIAL_RELEASE_URL}#notes`,
    `${OFFICIAL_RELEASE_URL}/payload`,
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v9.9.9',
  ])('refuses a forged release URL: %s', async (releasePageUrl) => {
    renderControl({
      client: client(async () => updateAvailable({ releasePageUrl })),
    })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))

    await waitFor(() => {
      expect(control().dataset.updateState).toBe('unavailable')
    })
    expect(screen.queryByRole('link')).toBeNull()
    expect(document.body.innerHTML).not.toContain(releasePageUrl)
  })

  it('rejects accessor proxy and extra-field results without reading hostile data', async () => {
    let kindRead = false
    const accessor = Object.defineProperty({}, 'kind', {
      enumerable: true,
      get() {
        kindRead = true
        return 'update_available'
      },
    })
    const hostileResults: unknown[] = [
      accessor,
      new Proxy({}, {
        getPrototypeOf() {
          throw new Error('C:\\Users\\private\\project.ori2')
        },
      }),
      {
        ...updateAvailable(),
        downloadUrl: 'https://evil.example/payload.exe',
      },
    ]

    for (const result of hostileResults) {
      const rendered = renderControl({
        client: client(async () => result),
      })
      fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
      await waitFor(() => {
        expect(control().dataset.updateState).toBe('unavailable')
      })
      expect(screen.queryByRole('link')).toBeNull()
      expect(document.body.textContent).not.toMatch(
        /private|project\.ori2|payload/iu,
      )
      rendered.unmount()
    }
    expect(kindRead).toBe(false)
  })

  it('persists enablement without checking and blocks every disabled request', () => {
    const settings = settingsFixture({ enabled: false })
    const getVersion = vi.fn(async () => '1.0.0')
    const checkNow = vi.fn(async () => upToDate())
    renderControl({
      settingsStore: settings.store,
      versionProvider: provider(getVersion),
      client: client(checkNow),
    })
    const toggle = screen.getByRole('switch', {
      name: 'Enable update checks',
    }) as HTMLInputElement
    const check = screen.getByRole('button', { name: 'Check now' })

    expect(toggle.checked).toBe(false)
    expect((check as HTMLButtonElement).disabled).toBe(true)
    expect(control().dataset.updateState).toBe('disabled')
    fireEvent.click(check)
    expect(getVersion).not.toHaveBeenCalled()
    expect(checkNow).not.toHaveBeenCalled()

    fireEvent.click(toggle)

    expect(toggle.checked).toBe(true)
    expect((check as HTMLButtonElement).disabled).toBe(false)
    expect(settings.writes).toHaveLength(1)
    expect(settings.writes[0]).toBe(
      encodeUpdateCheckSettings({ enabled: true }),
    )
    expect(getVersion).not.toHaveBeenCalled()
    expect(checkNow).not.toHaveBeenCalled()
  })

  it('keeps the session setting and clearly reports a storage failure', () => {
    const settings = settingsFixture({
      enabled: true,
      writeFails: true,
    })
    renderControl({ settingsStore: settings.store })
    const toggle = screen.getByRole('switch', {
      name: 'Enable update checks',
    }) as HTMLInputElement

    fireEvent.click(toggle)

    expect(toggle.checked).toBe(false)
    expect(control().dataset.updateState).toBe('disabled')
    expect(screen.getByRole('alert').textContent).toBe(
      'The update-check setting could not be saved on this PC. It applies only for this session.',
    )
    expect(document.body.textContent).not.toMatch(
      /private|localStorage|project\.ori2/iu,
    )

    settings.setWriteFails(false)
    fireEvent.click(toggle)
    expect(toggle.checked).toBe(true)
    expect(screen.queryByRole('alert')).toBeNull()
    expect(settings.writes).toHaveLength(1)
  })

  it('redacts a Tauri version failure and never reaches the update client', async () => {
    const getVersion = vi.fn(async () => {
      throw new Error('C:\\Users\\private\\project.ori2 raw IPC failure')
    })
    const checkNow = vi.fn(async () => upToDate())
    renderControl({
      versionProvider: provider(getVersion),
      client: client(checkNow),
    })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))

    await waitFor(() => {
      expect(control().dataset.updateState).toBe('unavailable')
    })
    expect(getVersion).toHaveBeenCalledTimes(1)
    expect(checkNow).not.toHaveBeenCalled()
    expect(document.body.textContent).not.toMatch(
      /private|project\.ori2|raw IPC/iu,
    )
    expect(screen.getByRole('status').textContent).toBe(
      'Update information could not be checked. Please try again later.',
    )
  })

  it('redacts a client rejection, clears busy state, and permits a retry', async () => {
    const checkNow = vi.fn()
      .mockRejectedValueOnce(
        new Error('C:\\Users\\private\\project.ori2 raw network failure'),
      )
      .mockResolvedValueOnce(upToDate())
    renderControl({ client: client(checkNow) })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    await waitFor(() => {
      expect(control().dataset.updateState).toBe('unavailable')
    })
    expect(document.body.textContent).not.toMatch(
      /private|project\.ori2|raw network/iu,
    )
    expect((screen.getByRole('button', {
      name: 'Check now',
    }) as HTMLButtonElement).disabled).toBe(false)

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    await waitFor(() => {
      expect(control().dataset.updateState).toBe('up_to_date')
    })
    expect(checkNow).toHaveBeenCalledTimes(2)
  })

  it('hides hostile latest responses with localized errors and permits a safe retry', async () => {
    const localeStore = localeFixture('en')
    const stable = {
      tag_name: 'v1.2.3',
      html_url: OFFICIAL_RELEASE_URL,
      name: 'ORIGAMI2 v1.2.3',
      body: 'Canonical generated release notes.',
      draft: false,
      prerelease: false,
      assets: Array.from({ length: 9 }, (_, id) => ({ id: id + 1 })),
    }
    const bodies = [
      JSON.stringify({
        ...stable,
        name: 'QUARANTINED v1.2.3',
        body: '## QUARANTINED RELEASE\n\nDo not install.',
      }),
      `${'['.repeat(33)}null${']'.repeat(33)}`,
      'x'.repeat(128 * 1024 + 1),
      JSON.stringify(stable),
    ]
    const requestLatestRelease = vi.fn(async () => ({
      status: 200,
      contentType: 'application/vnd.github+json; charset=utf-8',
      body: bodies.shift() ?? '',
      finalUrl:
        'https://api.github.com/repos/oltotlo79-rgb/ORIGAMI2/releases/latest',
      redirected: false,
    } satisfies UpdateCheckTransportResponse))
    renderControl({
      localeStore,
      client: createUpdateCheckClient({ requestLatestRelease }),
    })
    const check = () => fireEvent.click(screen.getByRole('button', {
      name: /^(Check now|今すぐ確認)$/u,
    }))

    check()
    await waitFor(() => expect(control().dataset.updateState).toBe('unavailable'))
    expect(screen.getByRole('status').textContent).toBe(
      'Update information could not be checked. Please try again later.',
    )
    expect(screen.queryByRole('link')).toBeNull()
    expect(document.body.textContent).not.toMatch(/QUARANTINED|Do not install/iu)

    act(() => localeStore.setLocale('ja'))
    check()
    await waitFor(() => expect(requestLatestRelease).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('status').textContent).toBe(
      '更新情報を確認できませんでした。時間をおいてもう一度お試しください。',
    )
    expect(screen.queryByRole('link')).toBeNull()

    check()
    await waitFor(() => expect(requestLatestRelease).toHaveBeenCalledTimes(3))
    expect(control().dataset.updateState).toBe('unavailable')
    expect(screen.queryByRole('link')).toBeNull()

    act(() => localeStore.setLocale('en'))
    check()
    await waitFor(() => expect(control().dataset.updateState).toBe('update_available'))
    expect(requestLatestRelease).toHaveBeenCalledTimes(4)
    expect(screen.getByRole('link', {
      name: 'Open release 1.2.3 on GitHub',
    }).getAttribute('href')).toBe(OFFICIAL_RELEASE_URL)
  })

  it('does not start a request after unmount while version lookup is pending', async () => {
    const version = promiseWithResolvers<unknown>()
    const checkNow = vi.fn(async () => upToDate())
    const rendered = renderControl({
      versionProvider: provider(() => version.promise),
      client: client(checkNow),
    })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    rendered.unmount()
    version.resolve('1.0.0')
    await act(async () => {
      await version.promise
      await Promise.resolve()
    })

    expect(checkNow).not.toHaveBeenCalled()
  })

  it('ignores an old result after disable re-enable and a newer check', async () => {
    const oldResult = promiseWithResolvers<UpdateCheckResult>()
    let call = 0
    const checkNow = vi.fn(() => {
      call += 1
      return call === 1
        ? oldResult.promise
        : Promise.resolve(upToDate())
    })
    renderControl({ client: client(checkNow) })
    const toggle = screen.getByRole('switch', {
      name: 'Enable update checks',
    })

    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    await waitFor(() => expect(checkNow).toHaveBeenCalledTimes(1))
    fireEvent.click(toggle)
    expect(control().dataset.updateState).toBe('disabled')
    fireEvent.click(toggle)
    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    await waitFor(() => {
      expect(control().dataset.updateState).toBe('up_to_date')
    })

    oldResult.resolve(updateAvailable())
    await act(async () => {
      await oldResult.promise
      await Promise.resolve()
    })

    expect(checkNow).toHaveBeenCalledTimes(2)
    expect(control().dataset.updateState).toBe('up_to_date')
    expect(screen.queryByRole('link')).toBeNull()
  })

  it('retranslates a completed semantic result without another check', async () => {
    const localeStore = localeFixture('en')
    const getVersion = vi.fn(async () => '1.0.0')
    const checkNow = vi.fn(async () => upToDate())
    renderControl({
      localeStore,
      versionProvider: provider(getVersion),
      client: client(checkNow),
    })
    fireEvent.click(screen.getByRole('button', { name: 'Check now' }))
    await waitFor(() => {
      expect(screen.getByRole('status').textContent).toBe(
        'Up to date. Installed 1.0.0; latest release 1.0.0.',
      )
    })

    act(() => {
      localeStore.setLocale('ja')
    })

    expect(screen.getByRole('heading', {
      name: 'ソフトウェア更新',
    })).toBeTruthy()
    expect(screen.getByRole('status').textContent).toBe(
      '最新版です。現在 1.0.0、公開版 1.0.0。',
    )
    expect(getVersion).toHaveBeenCalledTimes(1)
    expect(checkNow).toHaveBeenCalledTimes(1)
  })
})

function renderControl(overrides: Partial<UpdateCheckControlProps> = {}) {
  return render(controlElement(overrides))
}

function controlElement(overrides: Partial<UpdateCheckControlProps> = {}) {
  return (
    <UpdateCheckControl
      client={overrides.client ?? client(async () => upToDate())}
      versionProvider={
        overrides.versionProvider ?? provider(async () => '1.0.0')
      }
      settingsStore={
        overrides.settingsStore ?? settingsFixture().store
      }
      localeStore={overrides.localeStore ?? localeFixture('en')}
    />
  )
}

function control() {
  return screen.getByTestId('update-check-control')
}

function provider(
  getVersion: InstalledVersionProvider['getVersion'],
): InstalledVersionProvider {
  return { getVersion }
}

function client(
  checkNow: (...arguments_: Parameters<UpdateCheckClient['checkNow']>) =>
    Promise<unknown>,
): UpdateCheckClient {
  return {
    checkNow: (...arguments_) =>
      checkNow(...arguments_) as Promise<UpdateCheckResult>,
  }
}

function upToDate(): UpdateCheckResult {
  return {
    kind: 'up_to_date',
    currentVersion: '1.0.0',
    latestVersion: '1.0.0',
  }
}

function updateAvailable(
  overrides: Readonly<{ releasePageUrl?: string }> = {},
): UpdateCheckResult {
  return {
    kind: 'update_available',
    currentVersion: '1.0.0',
    latestVersion: '1.2.3',
    releasePageUrl: overrides.releasePageUrl ?? OFFICIAL_RELEASE_URL,
  }
}

function settingsFixture(options: Readonly<{
  enabled?: boolean
  writeFails?: boolean
}> = {}): {
  store: UpdateCheckSettingsStore
  writes: string[]
  setWriteFails: (value: boolean) => void
} {
  const writes: string[] = []
  let writeFails = options.writeFails ?? false
  let stored = options.enabled === undefined
    ? null
    : encodeUpdateCheckSettings({ enabled: options.enabled })
  const environment: UpdateCheckSettingsEnvironment = {
    readStoredSettings: () => stored,
    writeStoredSettings(serialized) {
      if (writeFails) {
        throw new Error(
          'C:\\Users\\private\\project.ori2 localStorage blocked',
        )
      }
      writes.push(serialized)
      stored = serialized
    },
  }
  const store = createUpdateCheckSettingsStore(environment)
  store.initialize()
  return {
    store,
    writes,
    setWriteFails(value) {
      writeFails = value
    },
  }
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
