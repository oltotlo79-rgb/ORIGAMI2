import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList'

const target = (id: number, count: 1 | 2 | 4, symmetry: 'none' | 'bilateral') => ({
  id, count, symmetry, length_tenths_mm: id * 100, thickness_tenths_mm: id * 10,
  position_tenths_mm: [0, 0, 0] as [number, number, number],
  direction_milli: [1000, 0, 0] as [number, number, number], curvature_degrees: 0,
  joint: 'fixed' as const, motion_degrees: [0, 0] as [number, number],
  side: 'either' as const, priority: 50,
})
const valid = [target(1, 4, 'bilateral'), target(2, 2, 'bilateral')]
afterEach(cleanup)

describe('GenericTargetBindingList', () => {
  it('renders a bounded recognized combination in both locales', () => {
    const { rerender } = render(<GenericTargetBindingList locale="ja" protrusions={valid} />)
    expect(screen.getByRole('list', { name: '上限付き汎用対象binding寸法' }).children).toHaveLength(2)
    rerender(<GenericTargetBindingList locale="en" protrusions={valid} />)
    expect(screen.getByText('Binding 2 · bilateral · count 2 · length 200 · thickness 20')).toBeTruthy()
  })
  it('rejects singleton, overflow, noncanonical, and unsupported radial input', () => {
    for (const forged of [valid.slice(0, 1), Array.from({ length: 9 }, (_, i) => target(i + 1, 2, 'bilateral')),
      [valid[1], valid[0]], [target(1, 2, 'none'), valid[1]]]) {
      const { unmount } = render(<GenericTargetBindingList locale="en" protrusions={forged} />)
      expect(screen.queryByRole('list')).toBeNull(); unmount()
    }
  })
})
