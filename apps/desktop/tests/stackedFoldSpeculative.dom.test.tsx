import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { StackedFoldPanel } from '../src/components/StackedFoldPanel.tsx'
import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import {
  EMPTY_UNPROVEN_COUNTS,
  makeCertifiedStackedFoldResponse,
  makeSpeculativeSnapshot,
  makeSpeculativeStackedFoldResponse,
  SPECULATIVE_TOKEN,
} from './stackedFoldSpeculativeFixture.ts'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

const transport = vi.hoisted(() => ({
  preview: vi.fn(),
  certifiedApply: vi.fn(),
  speculativeApply: vi.fn(),
  cancel: vi.fn(),
  cancelRead: vi.fn(),
  registry: vi.fn(),
  evenCandidates: vi.fn(),
}))

vi.mock('../src/lib/coreClient', async (importOriginal) => ({
  ...await importOriginal<typeof import('../src/lib/coreClient')>(),
  proposeCurrentStackedFoldRead: transport.preview,
  applyStackedFoldTransaction: transport.certifiedApply,
  applySpeculativeStackedFoldTransaction: transport.speculativeApply,
  cancelStackedFoldTransactionPreview: transport.cancel,
  cancelCurrentStackedFoldReadV1: transport.cancelRead,
  readLiveHingeRegistryV1: transport.registry,
  readEvenCycleCandidatesV1: transport.evenCandidates,
  listenStackedFoldReadProgressV1: vi.fn(async () => () => undefined),
  listenCurrentCyclePoseProgressV1: vi.fn(async () => () => undefined),
}))

const selectedLine = {
  id: 'edge',
  start: { x: 1, y: 2 },
  end: { x: 3, y: 4 },
}

beforeEach(() => {
  vi.clearAllMocks()
  nativeInvoke.mockRejectedValue(new Error('no native transport'))
  transport.cancel.mockResolvedValue(undefined)
  transport.cancelRead.mockResolvedValue(undefined)
  transport.registry.mockRejectedValue(new Error('registry unavailable'))
  transport.evenCandidates.mockRejectedValue(new Error('cycle unavailable'))
})

afterEach(cleanup)

function renderPanel(options: {
  snapshot?: ProjectSnapshot
  refreshSnapshot?: () => Promise<ProjectSnapshot>
  onApplied?: (snapshot: ProjectSnapshot) => void
  selected?: typeof selectedLine | null
} = {}) {
  const snapshot = options.snapshot ?? makeSpeculativeSnapshot()
  const props = {
    locale: 'en' as const,
    snapshot,
    selectedLine: options.selected === undefined ? selectedLine : options.selected,
    disabled: false,
    refreshSnapshot:
      options.refreshSnapshot ?? vi.fn().mockResolvedValue(snapshot),
    onApplied: options.onApplied ?? vi.fn(),
  }
  return {
    ...render(<StackedFoldPanel {...props} />),
    props,
  }
}

async function openSpeculativeApplyControl() {
  fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
  return screen.findByRole('group', {
    name: 'Unproven speculative Apply',
  })
}

describe('StackedFoldPanel speculative-unproven apply', () => {
  it('speculativeApplyRequiresExplicitConfirmation', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockResolvedValue(4)
    const refreshed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    const refreshSnapshot = vi.fn().mockResolvedValue(refreshed)
    const onApplied = vi.fn()
    renderPanel({ refreshSnapshot, onApplied })

    const group = await openSpeculativeApplyControl()
    const apply = within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }) as HTMLButtonElement
    expect(apply.disabled).toBe(true)
    fireEvent.click(apply)
    expect(transport.speculativeApply).not.toHaveBeenCalled()

    fireEvent.click(within(group).getByRole('checkbox'))
    expect(apply.disabled).toBe(false)
    fireEvent.click(apply)
    await waitFor(() => expect(transport.speculativeApply).toHaveBeenCalledWith({
      transactionToken: SPECULATIVE_TOKEN,
      explicitConfirmation: true,
    }))
    expect(transport.certifiedApply).not.toHaveBeenCalled()
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(refreshed))
  })

  it('consumes a failed speculative token and requires a fresh analysis', async () => {
    const secondToken = '018f47a2-4b7a-7cc1-8abc-aabbccddeeff'
    const second = makeSpeculativeStackedFoldResponse()
    second.transactionProposal.transactionToken = secondToken
    transport.preview
      .mockResolvedValueOnce(makeSpeculativeStackedFoldResponse())
      .mockResolvedValueOnce(second)
    transport.speculativeApply
      .mockRejectedValueOnce(new Error('stale authority'))
      .mockRejectedValueOnce(new Error('second failure'))
    renderPanel()

    const firstGroup = await openSpeculativeApplyControl()
    fireEvent.click(within(firstGroup).getByRole('checkbox'))
    const consumedButton = within(firstGroup).getByRole('button', {
      name: 'Apply unproven stacked fold',
    })
    fireEvent.click(consumedButton)
    expect(await screen.findByText(
      'Apply failed; the preview is no longer trusted.',
    )).toBeTruthy()
    expect(screen.queryByRole('group', {
      name: 'Unproven speculative Apply',
    })).toBeNull()

    fireEvent.click(consumedButton)
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)
    expect(transport.preview).toHaveBeenCalledTimes(1)

    const secondGroup = await openSpeculativeApplyControl()
    expect(transport.preview).toHaveBeenCalledTimes(2)
    fireEvent.click(within(secondGroup).getByRole('checkbox'))
    fireEvent.click(within(secondGroup).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))
    await waitFor(() => expect(transport.speculativeApply).toHaveBeenNthCalledWith(
      2,
      { transactionToken: secondToken, explicitConfirmation: true },
    ))
  })

  it('reconciles a rejected response that committed exactly one revision', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockRejectedValue(
      new Error('transport response lost after commit'),
    )
    const committed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    const refreshSnapshot = vi.fn().mockResolvedValue(committed)
    const onApplied = vi.fn()
    renderPanel({ refreshSnapshot, onApplied })

    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))

    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(committed))
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)
    expect(refreshSnapshot).toHaveBeenCalledTimes(1)
    expect(screen.queryByText(
      'Apply failed; the preview is no longer trusted.',
    )).toBeNull()
  })

  it('keeps an ambiguous response fail closed and retries only refresh', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockRejectedValue(
      new Error('transport response lost'),
    )
    const committed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    const refreshSnapshot = vi.fn()
      .mockRejectedValueOnce(new Error('refresh unavailable'))
      .mockResolvedValueOnce(committed)
    const onApplied = vi.fn()
    renderPanel({ refreshSnapshot, onApplied })

    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))

    expect(await screen.findByText(
      /speculative Apply outcome could not be confirmed/iu,
    )).toBeTruthy()
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'Retry refresh' }))
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(committed))
    expect(refreshSnapshot).toHaveBeenCalledTimes(2)
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)
  })

  it('cannot reuse an in-flight token after the snapshot becomes stale', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    let rejectApply!: (reason: Error) => void
    transport.speculativeApply.mockReturnValue(new Promise((_resolve, reject) => {
      rejectApply = reject
    }))
    const view = renderPanel()
    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    const consumedButton = within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    })
    fireEvent.click(consumedButton)
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)

    view.rerender(
      <StackedFoldPanel
        {...view.props}
        snapshot={{ ...view.props.snapshot, revision: 4 } as ProjectSnapshot}
      />,
    )
    rejectApply(new Error('stale authority'))
    await waitFor(() => expect(screen.queryByRole('group', {
      name: 'Unproven speculative Apply',
    })).toBeNull())
    fireEvent.click(consumedButton)
    expect(transport.speculativeApply).toHaveBeenCalledTimes(1)
  })

  it('keeps certified apply on the existing separate command', async () => {
    transport.preview.mockResolvedValue(makeCertifiedStackedFoldResponse())
    transport.certifiedApply.mockRejectedValue(new Error('temporary failure'))
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: 'Verify safety' }))
    const apply = await screen.findByRole('button', {
      name: 'Apply stacked fold',
    })
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(apply)
    await waitFor(() =>
      expect(transport.certifiedApply).toHaveBeenCalledWith(SPECULATIVE_TOKEN))
    expect(transport.speculativeApply).not.toHaveBeenCalled()
    expect(await screen.findByText(
      'Apply failed. You can retry with the same certified preview.',
    )).toBeTruthy()
  })
})

describe('StackedFoldPanel persisted unproven history', () => {
  it('warns for applied history and distinguishes unapplied redo history', () => {
    const applied = {
      ...EMPTY_UNPROVEN_COUNTS,
      awaitingProof: 1,
    }
    const view = renderPanel({
      selected: null,
      snapshot: makeSpeculativeSnapshot({
        applied,
        unappliedRedo: EMPTY_UNPROVEN_COUNTS,
      }),
    })
    expect(screen.getByRole('alert').textContent).toContain(
      '1 unproven fold operation(s) are applied',
    )
    expect(screen.queryByRole('button', {
      name: /request revert|戻すよう要求/iu,
    })).toBeNull()

    view.rerender(
      <StackedFoldPanel
        {...view.props}
        snapshot={makeSpeculativeSnapshot({
          applied: EMPTY_UNPROVEN_COUNTS,
          unappliedRedo: {
            ...EMPTY_UNPROVEN_COUNTS,
            unknownDeadlineReached: 2,
          },
        })}
      />,
    )
    expect(screen.queryByText(/are applied to the current document/u)).toBeNull()
    expect(screen.getByText(
      /exist only in redo history and are currently unapplied/u,
    )).toBeTruthy()
  })

  it('fails closed when persisted unproven counts are malformed', () => {
    renderPanel({
      selected: null,
      snapshot: makeSpeculativeSnapshot({
        applied: {
          ...EMPTY_UNPROVEN_COUNTS,
          awaitingProof: '1',
        },
        unappliedRedo: EMPTY_UNPROVEN_COUNTS,
      }),
    })
    expect(screen.getByRole('alert').textContent).toContain(
      'Unproven-state counts could not be verified safely',
    )
  })
})
