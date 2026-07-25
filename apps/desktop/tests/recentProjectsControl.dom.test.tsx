import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  RecentProjectsControl,
} from '../src/components/RecentProjectsControl'
import type { ProjectSnapshot } from '../src/lib/coreClient'
import type {
  createRecentProjectsClient,
  RecentProjectItem,
  RecentProjectOpenResult,
} from '../src/lib/recentProjectsClient'

type RecentProjectsClient = ReturnType<typeof createRecentProjectsClient>

const ITEM: RecentProjectItem = Object.freeze({
  opaque_id: `r1-${'a'.repeat(32)}`,
  display_name: 'Crane',
})

afterEach(cleanup)

describe('RecentProjectsControl', () => {
  it('refreshes the list on locale changes and localizes the same DOM', async () => {
    const client = clientStub({
      list: vi.fn(async () => Object.freeze([ITEM])),
    })
    const onOpen = vi.fn()
    const rendered = render(
      <RecentProjectsControl
        locale="en"
        onOpen={onOpen}
        client={client}
      />,
    )

    expect(
      screen.getByRole('heading', { name: 'Recent projects' }),
    ).toBeTruthy()
    expect(screen.getByText('No recent projects.')).toBeTruthy()
    await screen.findByRole('button', { name: 'Crane' })
    expect(client.list).toHaveBeenCalledTimes(1)

    rendered.rerender(
      <RecentProjectsControl
        locale="ja"
        onOpen={onOpen}
        client={client}
      />,
    )

    expect(
      screen.getByRole('heading', { name: '最近使った作品' }),
    ).toBeTruthy()
    await waitFor(() => expect(client.list).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: 'Crane' })).toBeTruthy()
    expect(onOpen).not.toHaveBeenCalled()
  })

  it('keeps the button busy, reports invalidation, and refreshes the list', async () => {
    let resolveOpen:
      | ((result: RecentProjectOpenResult) => void)
      | undefined
    const open = vi.fn(() => new Promise<RecentProjectOpenResult>((resolve) => {
      resolveOpen = resolve
    }))
    const list = vi.fn(async () => Object.freeze([ITEM]))
    const client = clientStub({ list, open })

    const rendered = render(
      <RecentProjectsControl
        locale="en"
        onOpen={() => {}}
        client={client}
      />,
    )
    const button = await screen.findByRole('button', { name: 'Crane' })

    fireEvent.click(button)
    expect((button as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByRole('status').textContent).toBe('')

    resolveOpen?.(Object.freeze({ status: 'invalidated' }))
    await waitFor(() => {
      expect(screen.getByRole('status').textContent).toBe(
        'The project moved or was replaced and was removed.',
      )
    })
    expect(list).toHaveBeenCalledTimes(2)
    expect((button as HTMLButtonElement).disabled).toBe(false)

    const status = screen.getByRole('status')
    rendered.rerender(
      <RecentProjectsControl
        locale="ja"
        onOpen={() => {}}
        client={client}
      />,
    )
    expect(screen.getByRole('status')).toBe(status)
    expect(status.textContent).toBe(
      '作品が移動または置換されたため一覧から削除しました。',
    )
    await waitFor(() => expect(list).toHaveBeenCalledTimes(3))
  })

  it('forwards an opened project and restores the button state', async () => {
    const project = Object.freeze({}) as ProjectSnapshot
    const onOpen = vi.fn()
    const client = clientStub({
      list: vi.fn(async () => Object.freeze([ITEM])),
      open: vi.fn(async () => Object.freeze({
        status: 'opened' as const,
        file: Object.freeze({ project }),
      })),
    })

    render(
      <RecentProjectsControl
        locale="en"
        onOpen={onOpen}
        client={client}
      />,
    )
    const button = await screen.findByRole('button', { name: 'Crane' })
    fireEvent.click(button)

    await waitFor(() => expect(onOpen).toHaveBeenCalledWith(project))
    expect((button as HTMLButtonElement).disabled).toBe(false)
    expect(screen.getByRole('status').textContent).toBe('')
  })

  it('reports list and open failures in the active locale', async () => {
    const listFailure = clientStub({
      list: vi.fn(async () => {
        throw new Error('unavailable')
      }),
    })
    const failedList = render(
      <RecentProjectsControl
        locale="ja"
        onOpen={() => {}}
        client={listFailure}
      />,
    )
    await waitFor(() => {
      expect(screen.getByRole('status').textContent).toBe(
        '最近使った作品を確認できません。',
      )
    })
    failedList.unmount()

    const openFailure = clientStub({
      list: vi.fn(async () => Object.freeze([ITEM])),
      open: vi.fn(async () => {
        throw new Error('unsafe')
      }),
    })
    render(
      <RecentProjectsControl
        locale="ja"
        onOpen={() => {}}
        client={openFailure}
      />,
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Crane' }))
    await waitFor(() => {
      expect(screen.getByRole('status').textContent).toBe(
        '作品を安全に開けませんでした。',
      )
    })
  })
})

function clientStub(
  overrides: Partial<RecentProjectsClient>,
): RecentProjectsClient {
  return Object.freeze({
    list: async () => Object.freeze([]),
    open: async () => Object.freeze({ status: 'invalidated' as const }),
    ...overrides,
  })
}
