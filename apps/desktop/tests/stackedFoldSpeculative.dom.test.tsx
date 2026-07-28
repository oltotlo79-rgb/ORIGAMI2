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
  SPECULATIVE_INSTANCE_ID,
  SPECULATIVE_PROJECT_ID,
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
const POST_APPLY_JOB_TOKEN = '018f47a2-4b7a-7cc1-8abc-aabbccddeeff'

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

  it('starts exact post-Apply proof authority and renders a coarse blocked result', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockResolvedValue(4)
    const refreshed = {
      ...makeSpeculativeSnapshot({
        applied: { ...EMPTY_UNPROVEN_COUNTS, awaitingProof: 1 },
        unappliedRedo: EMPTY_UNPROVEN_COUNTS,
      }),
      revision: 4,
    } as ProjectSnapshot
    const reverted = {
      ...refreshed,
      revision: 5,
    } as ProjectSnapshot
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === 'start_post_apply_proof_job_v1') {
        return {
          ...(args as { request: Record<string, unknown> }).request,
          jobToken: POST_APPLY_JOB_TOKEN,
          status: 'blocked',
          provenPairCount: 0,
          totalPairCount: 3,
          proofFailure: {
            location: 'applied_retained_undo',
            outcome: 'blocked',
            reason: null,
            subsequentEditCount: 0,
            undoStepsToRevert: 1,
          },
        }
      }
      if (command === 'revert_post_apply_proof_failure_v1') return 5
      if (command === 'cancel_post_apply_proof_job_v1') return undefined
      throw new Error('unexpected native command')
    })
    const onApplied = vi.fn()
    const refreshSnapshot = vi.fn()
      .mockResolvedValueOnce(refreshed)
      .mockResolvedValueOnce(refreshed)
      .mockResolvedValueOnce(reverted)
    renderPanel({
      refreshSnapshot,
      onApplied,
    })

    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))

    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'start_post_apply_proof_job_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          revision: 4,
        },
      },
    ))
    const proofPanel = await screen.findByLabelText('Proof progress')
    const blocked = proofPanel.querySelector(
      '[data-proof-status="blocked"]',
    )
    expect(blocked).not.toBeNull()
    expect(blocked?.getAttribute('role')).toBe('alert')
    expect(blocked?.getAttribute('aria-live')).toBe('assertive')
    expect(blocked?.getAttribute('data-proof-status')).toBe('blocked')
    expect(blocked?.textContent).toContain('Proven pairs 0 / total pairs 3')
    expect(proofPanel.textContent).not.toMatch(
      /018f47|geometry|coordinate|stack trace|[A-Z]:\\/iu,
    )
    const revert = within(proofPanel).getByRole('button', {
      name: 'Request revert by 1 undo step(s)',
    }) as HTMLButtonElement
    expect(revert.disabled).toBe(true)
    fireEvent.click(revert)
    expect(nativeInvoke).not.toHaveBeenCalledWith(
      'revert_post_apply_proof_failure_v1',
      expect.anything(),
    )
    fireEvent.click(within(proofPanel).getByRole('checkbox'))
    expect(revert.disabled).toBe(false)
    fireEvent.click(revert)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'revert_post_apply_proof_failure_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          expectedRevision: 4,
          jobToken: POST_APPLY_JOB_TOKEN,
          expectedLocation: 'applied_retained_undo',
          expectedOutcome: 'blocked',
          expectedReason: null,
          expectedSubsequentEditCount: 0,
          expectedUndoStepsToRevert: 1,
          explicitConfirmation: true,
        },
      },
    ))
    expect(onApplied).toHaveBeenCalledWith(refreshed)
    await waitFor(() => expect(onApplied).toHaveBeenCalledWith(reverted))
    expect(refreshSnapshot).toHaveBeenCalledTimes(3)
  })

  it('cancels the exact post-Apply job on project change', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockResolvedValue(4)
    const refreshed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === 'start_post_apply_proof_job_v1') {
        return {
          ...(args as { request: Record<string, unknown> }).request,
          jobToken: POST_APPLY_JOB_TOKEN,
          status: 'proving',
          provenPairCount: 0,
          totalPairCount: 3,
          proofFailure: null,
        }
      }
      if (command === 'cancel_post_apply_proof_job_v1') return undefined
      throw new Error('unexpected native command')
    })
    const view = renderPanel({
      refreshSnapshot: vi.fn().mockResolvedValue(refreshed),
    })
    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))
    await screen.findByText('Status: Proving')

    view.rerender(
      <StackedFoldPanel {...view.props} snapshot={refreshed} />,
    )
    expect(nativeInvoke).not.toHaveBeenCalledWith(
      'cancel_post_apply_proof_job_v1',
      expect.anything(),
    )
    view.rerender(
      <StackedFoldPanel
        {...view.props}
        snapshot={{
          ...refreshed,
          project_id: '018f47a2-4b7a-7cc1-8abc-0123456789ab',
        } as ProjectSnapshot}
      />,
    )
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'cancel_post_apply_proof_job_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          revision: 4,
          jobToken: POST_APPLY_JOB_TOKEN,
        },
      },
    ))
  })

  it('cancels an active post-Apply job on unmount', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockResolvedValue(4)
    const refreshed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    nativeInvoke.mockImplementation(async (command, args) => {
      if (command === 'start_post_apply_proof_job_v1') {
        return {
          ...(args as { request: Record<string, unknown> }).request,
          jobToken: POST_APPLY_JOB_TOKEN,
          status: 'proving',
          provenPairCount: 0,
          totalPairCount: 3,
          proofFailure: null,
        }
      }
      if (command === 'cancel_post_apply_proof_job_v1') return undefined
      throw new Error('unexpected native command')
    })
    const view = renderPanel({
      refreshSnapshot: vi.fn().mockResolvedValue(refreshed),
    })
    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))
    await screen.findByText('Status: Proving')
    nativeInvoke.mockClear()
    view.unmount()
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'cancel_post_apply_proof_job_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          revision: 4,
          jobToken: POST_APPLY_JOB_TOKEN,
        },
      },
    ))
  })

  it('cancels the previous proof job before a newly accepted Apply', async () => {
    const secondTransactionToken =
      '018f47a2-4b7a-7cc1-8abc-1234567890ab'
    const firstResponse = makeSpeculativeStackedFoldResponse()
    const secondResponse = makeSpeculativeStackedFoldResponse()
    secondResponse.binding.sourceRevision = 4
    secondResponse.transactionProposal.transactionToken =
      secondTransactionToken
    secondResponse.transactionProposal.sourceRevision = 4
    secondResponse.transactionProposal.targetRevision = 5
    transport.preview
      .mockResolvedValueOnce(firstResponse)
      .mockResolvedValueOnce(secondResponse)
    transport.speculativeApply
      .mockResolvedValueOnce(4)
      .mockResolvedValueOnce(5)
    const revision4 = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    const revision5 = {
      ...makeSpeculativeSnapshot(),
      revision: 5,
    } as ProjectSnapshot
    const refreshSnapshot = vi.fn()
      .mockResolvedValueOnce(revision4)
      .mockResolvedValueOnce(revision5)
    let startedJobs = 0
    nativeInvoke.mockImplementation(async (command, args) => {
      const request =
        (args as { request: Record<string, unknown> } | undefined)?.request
      if (command === 'start_post_apply_proof_job_v1') {
        startedJobs += 1
        return {
          ...request,
          jobToken: startedJobs === 1
            ? POST_APPLY_JOB_TOKEN
            : '018f47a2-4b7a-7cc1-8abc-bbccddeeff00',
          status: 'proving',
          provenPairCount: 0,
          totalPairCount: 3,
          proofFailure: null,
        }
      }
      if (command === 'poll_post_apply_proof_job_v1') {
        return {
          ...request,
          status: 'proving',
          provenPairCount: 0,
          totalPairCount: 3,
          proofFailure: null,
        }
      }
      if (command === 'cancel_post_apply_proof_job_v1') return undefined
      throw new Error('unexpected native command')
    })
    const view = renderPanel({ refreshSnapshot })

    let group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))
    await screen.findByText('Status: Proving')
    view.rerender(
      <StackedFoldPanel {...view.props} snapshot={revision4} />,
    )

    group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'cancel_post_apply_proof_job_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          revision: 4,
          jobToken: POST_APPLY_JOB_TOKEN,
        },
      },
    ))
    const cancelCall = nativeInvoke.mock.invocationCallOrder.find(
      (_order, index) =>
        nativeInvoke.mock.calls[index]?.[0]
          === 'cancel_post_apply_proof_job_v1',
    )
    expect(cancelCall).toBeLessThan(
      transport.speculativeApply.mock.invocationCallOrder[1],
    )
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledWith(
      'start_post_apply_proof_job_v1',
      {
        request: {
          version: 1,
          projectInstanceId: SPECULATIVE_INSTANCE_ID,
          projectId: SPECULATIVE_PROJECT_ID,
          revision: 5,
        },
      },
    ))
  })

  it('uses a fixed redacted message when post-Apply transport is unavailable', async () => {
    transport.preview.mockResolvedValue(makeSpeculativeStackedFoldResponse())
    transport.speculativeApply.mockResolvedValue(4)
    const refreshed = {
      ...makeSpeculativeSnapshot(),
      revision: 4,
    } as ProjectSnapshot
    renderPanel({
      refreshSnapshot: vi.fn().mockResolvedValue(refreshed),
    })
    const group = await openSpeculativeApplyControl()
    fireEvent.click(within(group).getByRole('checkbox'))
    fireEvent.click(within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }))

    expect((await screen.findByTestId(
      'post-apply-proof-unavailable',
    )).textContent).toBe(
      'Post-Apply proof progress is unavailable. The fold remains unproven.',
    )
    expect(screen.getByTestId('unproven-proof-badge')).toBeTruthy()
    expect(screen.getByTestId('unproven-summary-unavailable').textContent)
      .toContain('could not be verified safely')
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
