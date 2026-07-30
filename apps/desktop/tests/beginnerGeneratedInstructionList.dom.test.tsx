import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  BeginnerGeneratedInstructionList,
} from '../src/components/BeginnerCandidateResults.tsx'

afterEach(cleanup)

describe('beginner generated instruction presentation', () => {
  it('renders the complete generic candidate grammar in English', () => {
    render(
      <BeginnerGeneratedInstructionList
        locale="en"
        instructionCodes={[
          'bounded_tree_river_axial_v1:4000000,1000000',
          'bounded_radial_corner_support_v1:added=5:covered=4',
          'bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2',
          'bounded_tree_paper_orientation_v1:horizontal',
        ]}
      />,
    )

    const list = screen.getByRole('list', {
      name: 'Candidate folding instructions',
    })
    expect(list.getAttribute('aria-live')).toBeNull()
    expect(list.getAttribute('aria-atomic')).toBeNull()
    expect(within(list).getAllByRole('listitem').map(
      (item) => item.textContent,
    )).toEqual([
      'Use bounded-tree river/axial ratios (millionths): 4000000,1000000.',
      'Cover all four paper corners with bounded radial support (5 support creases added).',
      'Use bounded-tree branch topology: 3 nodes, 2 leaves, 2 bars.',
      'Evaluate the bounded tree in horizontal paper orientation.',
    ])
  })

  it('renders generic topology and vertical orientation in Japanese', () => {
    render(
      <BeginnerGeneratedInstructionList
        locale="ja"
        instructionCodes={[
          'bounded_tree_branch_topology_v1:nodes=5:leaves=3:bars=4',
          'bounded_tree_paper_orientation_v1:vertical',
        ]}
      />,
    )

    expect(screen.getByText(
      '一般木の分岐構造を使用します: 節点 5、葉 3、枝 4。',
    )).toBeTruthy()
    expect(screen.getByText(
      '一般木を用紙の縦向き配置で評価します。',
    )).toBeTruthy()
  })

  it('keeps ten semantic insect bindings distinct from seven physical targets', () => {
    render(
      <BeginnerGeneratedInstructionList
        locale="en"
        instructionCodes={['asymmetric_insect_landmark_base']}
      />,
    )
    expect(screen.getByText(
      'Bind ten ordered insect landmarks to the certified four-ray base.',
    )).toBeTruthy()
    expect(screen.queryByText(/seven physical/u)).toBeNull()
  })

  it('shows an explicit unknown instruction instead of a diagonal fallback', () => {
    render(
      <BeginnerGeneratedInstructionList
        locale="en"
        instructionCodes={['future_instruction_v2']}
      />,
    )
    expect(screen.getByText(
      'Unknown generated instruction: future_instruction_v2',
    )).toBeTruthy()
    expect(screen.queryByText('Fold on the diagonal.')).toBeNull()
  })

  it('announces an empty validated instruction set without an empty list', () => {
    render(
      <BeginnerGeneratedInstructionList
        locale="en"
        instructionCodes={[]}
      />,
    )

    const status = screen.getByRole('status')
    expect(status.textContent).toBe(
      'No validated candidate folding instructions are available.',
    )
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.getAttribute('aria-atomic')).toBe('true')
    expect(screen.queryByRole('list')).toBeNull()
  })
})
