import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ProofProgressPanel } from '../src/components/ProofProgressPanel.tsx'
import type { ProofProgressPanelModel } from '../src/lib/proofProgressModel.ts'

const emptyCounts = Object.freeze({
  awaitingProof: 0,
  proofBlocked: 0,
  unknownEvidenceInsufficient: 0,
  unknownResourceLimit: 0,
  unknownCancelled: 0,
  unknownDeadlineReached: 0,
})

const baseModel: ProofProgressPanelModel = Object.freeze({
  status: 'certified',
  provenPairCount: 6,
  totalPairCount: 6,
  unprovenHistory: Object.freeze({
    kind: 'known',
    applied: emptyCounts,
    unappliedRedo: emptyCounts,
    appliedTotal: 0,
    unappliedRedoTotal: 0,
  }),
  speculativeApplyAvailable: false,
  proofFailure: null,
})

afterEach(cleanup)

describe('ProofProgressPanel', () => {
  it('unprovenBadgeDistinctFromProvenBadge', () => {
    const view = render(<ProofProgressPanel locale="en" model={baseModel} />)
    const proven = screen.getByTestId('proven-proof-badge')
    expect(proven.textContent).toBe('Proven')
    expect(proven.className).toContain('proof-badge--proven')
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite')

    view.rerender(<ProofProgressPanel locale="en" model={{
      ...baseModel,
      status: 'evidence_insufficient',
      provenPairCount: 0,
      speculativeApplyAvailable: true,
    }} />)
    const unproven = screen.getByTestId('unproven-proof-badge')
    expect(unproven).toBe(proven)
    expect(unproven.textContent).toBe('Unproven')
    expect(unproven.className).toContain('proof-badge--unproven')
    expect(unproven.className).not.toContain('proof-badge--proven')
    expect(screen.getByText(/not a safety certificate/iu)).toBeTruthy()
  })

  it('unprovenDocumentOnLoadShowsExplicitWarning', () => {
    render(<ProofProgressPanel locale="en" model={{
      ...baseModel,
      status: null,
      provenPairCount: 0,
      totalPairCount: null,
      unprovenHistory: {
        kind: 'known',
        applied: { ...emptyCounts, awaitingProof: 2 },
        unappliedRedo: { ...emptyCounts, proofBlocked: 1 },
        appliedTotal: 2,
        unappliedRedoTotal: 1,
      },
    }} />)
    const warning = screen.getByTestId('applied-unproven-warning')
    expect(warning.getAttribute('role')).toBe('alert')
    expect(warning.textContent).toContain(
      '2 unproven fold operation(s) are applied to the current document.',
    )
    expect(screen.getByTestId('redo-unproven-notice').textContent).toContain(
      'currently unapplied',
    )
  })

  it('distinguishes redo-only marks from currently applied marks', () => {
    render(<ProofProgressPanel locale="ja" model={{
      ...baseModel,
      status: null,
      unprovenHistory: {
        kind: 'known',
        applied: emptyCounts,
        unappliedRedo: { ...emptyCounts, unknownResourceLimit: 3 },
        appliedTotal: 0,
        unappliedRedoTotal: 3,
      },
    }} />)
    expect(screen.queryByTestId('applied-unproven-warning')).toBeNull()
    expect(screen.getByTestId('redo-unproven-notice').textContent).toContain(
      '現在は未適用',
    )
    const applied = screen.getByLabelText('適用中の未証明内訳')
    const redo = screen.getByLabelText('現在は未適用のRedo内訳')
    expect(applied.querySelectorAll('dt')).toHaveLength(6)
    expect(redo.querySelectorAll('dt')).toHaveLength(6)
    expect(redo.textContent).toContain('資源上限')
  })

  it('renders every terminal state independently and fails unknown closed', () => {
    const cases = [
      ['proving', 'status', 'polite'],
      ['certified', 'status', 'polite'],
      ['blocked', 'alert', 'assertive'],
      ['evidence_insufficient', 'status', 'polite'],
      ['resource_limit', 'status', 'polite'],
      ['cancelled', 'status', 'polite'],
      ['deadline', 'status', 'polite'],
      ['stale', 'status', 'polite'],
      ['future_success', 'status', 'polite'],
    ] as const
    const view = render(<ProofProgressPanel locale="en" model={baseModel} />)
    for (const [status, role, live] of cases) {
      view.rerender(<ProofProgressPanel locale="en" model={{
        ...baseModel,
        status: status as ProofProgressPanelModel['status'],
      }} />)
      const announcement = screen.getByRole(role)
      expect(announcement.getAttribute('aria-live')).toBe(live)
      expect(announcement.getAttribute('data-proof-status')).toBe(
        status === 'future_success' ? 'evidence_insufficient' : status,
      )
      if (status === 'future_success') {
        expect(screen.getByTestId('unproven-proof-badge')).toBeTruthy()
      }
    }
  })

  it('announces post-Apply start and transport failure with fixed redacted text', () => {
    const view = render(<ProofProgressPanel locale="en" model={{
      ...baseModel,
      status: 'proving',
      provenPairCount: 0,
      totalPairCount: null,
      postApplyNotice: 'starting',
      unprovenHistory: {
        kind: 'known',
        applied: { ...emptyCounts, awaitingProof: 1 },
        unappliedRedo: emptyCounts,
        appliedTotal: 1,
        unappliedRedoTotal: 0,
      },
    }} />)
    const starting = screen.getByTestId('post-apply-proof-starting')
    expect(starting.textContent).toBe('Starting the post-Apply proof job.')
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite')
    expect(screen.getByTestId('applied-unproven-warning').textContent).toContain(
      '1 unproven fold operation(s)',
    )

    view.rerender(<ProofProgressPanel locale="en" model={{
      ...baseModel,
      status: 'evidence_insufficient',
      provenPairCount: 0,
      totalPairCount: null,
      postApplyNotice: 'unavailable',
    }} />)
    expect(screen.getByTestId('post-apply-proof-unavailable').textContent).toBe(
      'Post-Apply proof progress is unavailable. The fold remains unproven.',
    )
    expect(screen.getByTestId('unproven-proof-badge')).toBeTruthy()
    expect(view.container.textContent).not.toMatch(
      /stack trace|[A-Z]:\\|authority|geometry|coordinate/iu,
    )
  })

  it('proofFailureOffersRevertButDoesNotAutoRevert', () => {
    const onRequestRevert = vi.fn()
    render(<ProofProgressPanel locale="en" model={{
      ...baseModel,
      status: 'blocked',
      proofFailure: {
        location: 'applied_retained_undo',
        reason: 'blocked',
        subsequentEditCount: 2,
        undoStepsToRevert: 3,
      },
    }} onRequestRevert={onRequestRevert} />)
    const button = screen.getByRole('button', {
      name: 'Request revert by 3 undo step(s)',
    }) as HTMLButtonElement
    expect(button.className).toContain('secondary')
    expect(button.disabled).toBe(true)
    expect(onRequestRevert).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('checkbox'))
    expect(button.disabled).toBe(false)
    fireEvent.click(button)
    fireEvent.click(button)
    expect(onRequestRevert).toHaveBeenCalledTimes(1)
    expect(screen.getByText('2 edit(s) were made after this operation.')).toBeTruthy()
  })

  it('locale changes preserve destructive confirmation and callback count', () => {
    const onRequestRevert = vi.fn()
    const model: ProofProgressPanelModel = {
      ...baseModel,
      status: 'resource_limit',
      proofFailure: {
        location: 'applied_retained_undo',
        reason: 'resource_limit',
        subsequentEditCount: 1,
        undoStepsToRevert: 2,
      },
    }
    const view = render(
      <ProofProgressPanel
        locale="en"
        model={model}
        onRequestRevert={onRequestRevert}
      />,
    )
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement
    fireEvent.click(checkbox)
    expect(checkbox.checked).toBe(true)
    view.rerender(
      <ProofProgressPanel
        locale="ja"
        model={model}
        onRequestRevert={onRequestRevert}
      />,
    )
    expect(screen.getByRole('checkbox')).toBe(checkbox)
    expect(checkbox.checked).toBe(true)
    expect(onRequestRevert).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '2 手分を戻すよう要求' }))
    expect(onRequestRevert).toHaveBeenCalledTimes(1)
  })

  it('a recomputed terminal report resets destructive confirmation', async () => {
    const initial: ProofProgressPanelModel = {
      ...baseModel,
      status: 'blocked',
      proofFailure: {
        location: 'applied_retained_undo',
        reason: 'blocked',
        subsequentEditCount: 1,
        undoStepsToRevert: 2,
      },
    }
    const view = render(
      <ProofProgressPanel locale="en" model={initial} onRequestRevert={vi.fn()} />,
    )
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement
    fireEvent.click(checkbox)
    expect(checkbox.checked).toBe(true)

    view.rerender(
      <ProofProgressPanel locale="en" model={{
        ...initial,
        proofFailure: {
          ...initial.proofFailure!,
          subsequentEditCount: 2,
          undoStepsToRevert: 3,
        },
      }} onRequestRevert={vi.fn()} />,
    )
    await waitFor(() => expect(
      (screen.getByRole('checkbox') as HTMLInputElement).checked,
    ).toBe(false))
    expect((screen.getByRole('button', {
      name: 'Request revert by 3 undo step(s)',
    }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('does not expose raw identifiers, paths, errors, coordinates, or shape data', () => {
    const model = {
      ...baseModel,
      status: 'blocked' as const,
      proofFailure: {
        location: 'applied_trimmed_base' as const,
        reason: 'blocked' as const,
        subsequentEditCount: 4,
        undoStepsToRevert: null,
      },
    }
    const { container } = render(
      <ProofProgressPanel locale="en" model={model} />,
    )
    expect(container.textContent).not.toMatch(
      /018f47|[A-Z]:\\|stack trace|coordinate|shape/iu,
    )
    expect(screen.queryByRole('button')).toBeNull()
  })
})
