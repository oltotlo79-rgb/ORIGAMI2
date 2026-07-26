import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  SpeculativeStackedFoldApplyControl,
} from '../src/components/SpeculativeStackedFoldApplyControl.tsx'

afterEach(cleanup)

describe('SpeculativeStackedFoldApplyControl', () => {
  it('requires its dedicated explicit confirmation before apply', () => {
    const onApply = vi.fn()
    const onConfirmedChange = vi.fn()
    const props = {
      locale: 'en' as const,
      confirmed: false,
      onApply,
      onConfirmedChange,
    }
    const { rerender } = render(
      <SpeculativeStackedFoldApplyControl {...props} />,
    )
    const group = screen.getByRole('group', {
      name: 'Unproven speculative Apply',
    })
    const button = within(group).getByRole('button', {
      name: 'Apply unproven stacked fold',
    }) as HTMLButtonElement
    expect(button.disabled).toBe(true)
    fireEvent.click(button)
    expect(onApply).not.toHaveBeenCalled()

    fireEvent.click(within(group).getByRole('checkbox'))
    expect(onConfirmedChange).toHaveBeenCalledWith(true)
    rerender(
      <SpeculativeStackedFoldApplyControl {...props} confirmed />,
    )
    expect(button.disabled).toBe(false)
    fireEvent.click(button)
    expect(onApply).toHaveBeenCalledTimes(1)
    expect(button.dataset.applyMode).toBe('speculative_unproven')
  })

  it('preserves controlled confirmation across locale changes', () => {
    const onApply = vi.fn()
    const onConfirmedChange = vi.fn()
    const props = {
      confirmed: true,
      onApply,
      onConfirmedChange,
    }
    const { rerender } = render(
      <SpeculativeStackedFoldApplyControl {...props} locale="en" />,
    )

    rerender(
      <SpeculativeStackedFoldApplyControl {...props} locale="ja" />,
    )
    const group = screen.getByRole('group', {
      name: '未証明の投機的適用',
    })
    expect((within(group).getByRole('checkbox') as HTMLInputElement).checked)
      .toBe(true)
    expect(onApply).not.toHaveBeenCalled()
    expect(onConfirmedChange).not.toHaveBeenCalled()
    expect(within(group).getAllByText(/安全性の証明では/u)).toHaveLength(2)
  })

  it('disables both controls while unavailable or busy', () => {
    const { rerender } = render(
      <SpeculativeStackedFoldApplyControl
        locale="en"
        confirmed
        disabled
        onApply={vi.fn()}
        onConfirmedChange={vi.fn()}
      />,
    )
    expect((screen.getByRole('checkbox') as HTMLInputElement).disabled).toBe(true)
    expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true)

    rerender(
      <SpeculativeStackedFoldApplyControl
        locale="en"
        confirmed
        busy
        onApply={vi.fn()}
        onConfirmedChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('group').getAttribute('aria-busy')).toBe('true')
    expect(screen.getByRole('button', {
      name: 'Applying unproven stacked fold…',
    })).toBeTruthy()
  })
})
