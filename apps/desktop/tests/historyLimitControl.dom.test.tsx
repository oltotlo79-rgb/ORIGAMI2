import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  HistoryLimitControl,
  type HistoryLimitControlProps,
} from '../src/components/HistoryLimitControl.tsx'
import {
  type HistoryLimitClient,
  type HistoryLimitSettings,
  type SetHistoryEntryLimitRequest,
} from '../src/lib/historyLimitClient.ts'

const INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'
const NEXT_INSTANCE_ID = '30000000-0000-4000-8000-000000000003'
const NEXT_PROJECT_ID = '40000000-0000-4000-8000-000000000004'

const SETTINGS: HistoryLimitSettings = Object.freeze({
  schemaVersion: 1,
  projectInstanceId: INSTANCE_ID,
  projectId: PROJECT_ID,
  revision: 12,
  historyEntryLimit: 128,
})

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('HistoryLimitControl', () => {
  it('shows the current limit, exact number bounds, and the destructive trim warning', () => {
    renderControl()

    expect(screen.getByRole('status', {
      name: '現在の履歴件数上限',
    }).textContent).toBe('128件')
    const input = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    }) as HTMLInputElement
    expect(input.value).toBe('128')
    expect(input.min).toBe('1')
    expect(input.max).toBe('128')
    expect(input.step).toBe('1')
    expect(screen.getByText(
      /古いUndo\/Redo履歴は直ちに削除されます/u,
    ).textContent).toContain(
      'あとで上限を増やしても、削除された履歴は戻りません',
    )
    expect(input.getAttribute('aria-describedby')).toContain(
      'history-limit-description',
    )
  })

  it('applies only from the explicit button and sends the exact project-bound request', async () => {
    const requests: SetHistoryEntryLimitRequest[] = []
    const onApplied = vi.fn()
    const client = clientWithSet(async (request) => {
      requests.push(request)
      return { ...SETTINGS, historyEntryLimit: request.historyEntryLimit }
    })
    let formSubmissions = 0
    render(
      <form
        onSubmit={(event) => {
          event.preventDefault()
          formSubmissions += 1
        }}
      >
        {controlElement({ client, onApplied })}
      </form>,
    )
    const input = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })

    fireEvent.change(input, { target: { value: '64' } })
    fireEvent.blur(input)
    fireEvent.compositionStart(input)
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true })
    fireEvent.compositionEnd(input)
    fireEvent.keyDown(input, { key: 'Enter' })
    await Promise.resolve()
    expect(requests).toHaveLength(0)
    expect(onApplied).not.toHaveBeenCalled()
    expect(formSubmissions).toBe(0)

    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledTimes(1))
    expect(requests).toEqual([{
      schemaVersion: 1,
      expectedProjectInstanceId: INSTANCE_ID,
      expectedProjectId: PROJECT_ID,
      expectedRevision: 12,
      historyEntryLimit: 64,
    }])
    expect(onApplied).toHaveBeenCalledWith({
      ...SETTINGS,
      historyEntryLimit: 64,
    })
  })

  it('rejects non-integer and out-of-range drafts before calling the client', () => {
    const set = vi.fn()
    renderControl({ client: clientWithSet(set) })
    const input = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })

    for (const value of ['', '0', '129', '1.5']) {
      fireEvent.change(input, { target: { value } })
      expect(input.getAttribute('aria-invalid')).toBe('true')
      expect(screen.getByRole('alert').textContent).toBe(
        '履歴件数は1から128までの整数で入力してください。',
      )
      const apply = screen.getByRole('button', { name: '適用' })
      expect((apply as HTMLButtonElement).disabled).toBe(true)
      fireEvent.click(apply)
    }
    expect(set).not.toHaveBeenCalled()
  })

  it('locks synchronously against double submission and disables editing while pending', async () => {
    const deferred = promiseWithResolvers<HistoryLimitSettings>()
    const set = vi.fn(() => deferred.promise)
    const onApplied = vi.fn()
    renderControl({ client: clientWithSet(set), onApplied })
    const input = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })
    fireEvent.change(input, { target: { value: '64' } })
    const apply = screen.getByRole('button', { name: '適用' })

    fireEvent.click(apply)
    fireEvent.click(apply)

    expect(set).toHaveBeenCalledTimes(1)
    expect((input as HTMLInputElement).disabled).toBe(true)
    expect(screen.getByRole('group', {
      name: 'Undo・Redo履歴の上限',
    }).getAttribute('aria-busy')).toBe('true')
    expect((screen.getByRole('button', {
      name: '適用中…',
    }) as HTMLButtonElement).disabled).toBe(true)

    deferred.resolve({ ...SETTINGS, historyEntryLimit: 64 })
    await waitFor(() => expect(onApplied).toHaveBeenCalledTimes(1))
    await waitFor(() => {
      expect((screen.getByRole('button', {
        name: '適用',
      }) as HTMLButtonElement).disabled).toBe(false)
    })
  })

  it('redacts client failures, clears busy state, and permits a retry', async () => {
    const set = vi.fn()
      .mockRejectedValueOnce(
        new Error('C:\\Users\\private\\project.ori2: raw native error'),
      )
      .mockResolvedValueOnce({ ...SETTINGS, historyEntryLimit: 64 })
    const onApplied = vi.fn()
    renderControl({ client: clientWithSet(set), onApplied })
    const input = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })
    fireEvent.change(input, { target: { value: '64' } })

    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    await waitFor(() => {
      expect(screen.getByRole('alert').textContent).toBe(
        '履歴件数を変更できませんでした。現在のプロジェクトを確認して、もう一度お試しください。',
      )
    })
    expect(document.body.textContent).not.toContain('private')
    expect(document.body.textContent).not.toContain('project.ori2')

    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledTimes(1))
    expect(set).toHaveBeenCalledTimes(2)
  })

  it('invalidates an older response immediately when project props change', async () => {
    const first = promiseWithResolvers<HistoryLimitSettings>()
    const nextSettings: HistoryLimitSettings = {
      schemaVersion: 1,
      projectInstanceId: NEXT_INSTANCE_ID,
      projectId: NEXT_PROJECT_ID,
      revision: 0,
      historyEntryLimit: 32,
    }
    let call = 0
    const client = clientWithSet((request) => {
      call += 1
      if (call === 1) return first.promise
      return Promise.resolve({
        ...nextSettings,
        historyEntryLimit: request.historyEntryLimit,
      })
    })
    const onApplied = vi.fn()
    const rendered = renderControl({ client, onApplied })
    const firstInput = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })
    fireEvent.change(firstInput, { target: { value: '64' } })
    fireEvent.click(screen.getByRole('button', { name: '適用' }))

    rendered.rerender(controlElement({
      client,
      onApplied,
      settings: nextSettings,
      expectedProjectInstanceId: NEXT_INSTANCE_ID,
      expectedProjectId: NEXT_PROJECT_ID,
      expectedRevision: 0,
    }))
    await waitFor(() => {
      expect((screen.getByRole('spinbutton', {
        name: '履歴件数の上限',
      }) as HTMLInputElement).value).toBe('32')
    })

    first.resolve({ ...SETTINGS, historyEntryLimit: 64 })
    await Promise.resolve()
    await Promise.resolve()
    expect(onApplied).not.toHaveBeenCalled()
    expect(screen.queryByRole('alert')).toBeNull()

    const nextInput = screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    })
    fireEvent.change(nextInput, { target: { value: '16' } })
    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    await waitFor(() => {
      expect(onApplied).toHaveBeenCalledWith({
        ...nextSettings,
        historyEntryLimit: 16,
      })
    })
  })

  it('rejects a stale custom-client response before the success callback', async () => {
    const onApplied = vi.fn()
    const client = clientWithSet(async () => ({
      ...SETTINGS,
      projectId: NEXT_PROJECT_ID,
      historyEntryLimit: 64,
    }))
    renderControl({ client, onApplied })
    fireEvent.change(screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    }), { target: { value: '64' } })
    fireEvent.click(screen.getByRole('button', { name: '適用' }))

    await waitFor(() => expect(screen.getByRole('alert').textContent).toBe(
      '履歴件数を変更できませんでした。現在のプロジェクトを確認して、もう一度お試しください。',
    ))
    expect(onApplied).not.toHaveBeenCalled()
  })

  it('ignores pending completion after unmount and remains single-shot in StrictMode', async () => {
    const deferred = promiseWithResolvers<HistoryLimitSettings>()
    const onUnmountedApplied = vi.fn()
    const rendered = renderControl({
      client: clientWithSet(() => deferred.promise),
      onApplied: onUnmountedApplied,
    })
    fireEvent.change(screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    }), { target: { value: '64' } })
    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    rendered.unmount()
    deferred.resolve({ ...SETTINGS, historyEntryLimit: 64 })
    await Promise.resolve()
    await Promise.resolve()
    expect(onUnmountedApplied).not.toHaveBeenCalled()

    const set = vi.fn(async () => ({ ...SETTINGS, historyEntryLimit: 64 }))
    const onApplied = vi.fn()
    render(
      <StrictMode>
        {controlElement({ client: clientWithSet(set), onApplied })}
      </StrictMode>,
    )
    fireEvent.change(screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    }), { target: { value: '64' } })
    fireEvent.click(screen.getByRole('button', { name: '適用' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledTimes(1))
    expect(set).toHaveBeenCalledTimes(1)
  })

  it('fails closed for mismatched settings props and respects external disablement', () => {
    const set = vi.fn()
    renderControl({
      settings: { ...SETTINGS, projectId: NEXT_PROJECT_ID },
      disabled: true,
      client: clientWithSet(set),
    })

    expect(screen.getByRole('alert').textContent).toBe(
      '履歴件数を変更できませんでした。現在のプロジェクトを確認して、もう一度お試しください。',
    )
    expect((screen.getByRole('spinbutton', {
      name: '履歴件数の上限',
    }) as HTMLInputElement).disabled).toBe(true)
    expect((screen.getByRole('button', { name: '適用' }) as HTMLButtonElement)
      .disabled).toBe(true)
    expect(set).not.toHaveBeenCalled()
  })
})

function renderControl(overrides: Partial<HistoryLimitControlProps> = {}) {
  return render(controlElement(overrides))
}

function controlElement(overrides: Partial<HistoryLimitControlProps> = {}) {
  return (
    <HistoryLimitControl
      settings={overrides.settings ?? SETTINGS}
      expectedProjectInstanceId={
        overrides.expectedProjectInstanceId ?? INSTANCE_ID
      }
      expectedProjectId={overrides.expectedProjectId ?? PROJECT_ID}
      expectedRevision={overrides.expectedRevision ?? 12}
      client={overrides.client ?? clientWithSet(async (candidate) => ({
        ...SETTINGS,
        historyEntryLimit: candidate.historyEntryLimit,
      }))}
      disabled={overrides.disabled ?? false}
      onApplied={overrides.onApplied ?? vi.fn()}
    />
  )
}

function clientWithSet(
  set: HistoryLimitClient['set'],
): HistoryLimitClient {
  return {
    get: async () => SETTINGS,
    set,
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
