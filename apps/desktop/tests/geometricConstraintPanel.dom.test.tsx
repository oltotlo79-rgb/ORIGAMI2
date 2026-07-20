import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'

import { GeometricConstraintPanel } from '../src/components/GeometricConstraintPanel'
import type {
  GeometricConstraintDocument,
  GeometricConstraintPreflightResult,
} from '../src/lib/coreClient'
import type { LocaleStore } from '../src/lib/i18n'
import { localeFixture } from './localeTestFixture'

const IDS = Array.from(
  { length: 24 },
  (_, index) => `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
)

afterEach(cleanup)

describe('GeometricConstraintPanel', () => {
  it('adds only a horizontal or vertical constraint to an explicitly selected edge', () => {
    const onAddOrientation = vi.fn()
    const { rerender } = renderPanel({ onAddOrientation })

    expect(
      (screen.getByRole('button', { name: '選択線を水平に制約' }) as HTMLButtonElement).disabled,
    ).toBe(true)
    expect(screen.getByText('水平・垂直制約を追加するには線を選択してください。')).toBeTruthy()

    rerender(panel({
      selectedEdgeId: IDS[0],
      onAddOrientation,
    }))
    fireEvent.click(screen.getByRole('button', { name: '選択線を水平に制約' }))
    fireEvent.click(screen.getByRole('button', { name: '選択線を垂直に制約' }))
    expect(onAddOrientation.mock.calls).toEqual([['horizontal'], ['vertical']])
  })

  it('lists and allows deleting every persisted V1 constraint kind', () => {
    const onRemove = vi.fn()
    const onSelectEdge = vi.fn()
    renderPanel({
      document: allKinds(),
      onRemove,
      onSelectEdge,
    })

    for (const label of [
      '長さを固定',
      '角度を固定',
      '水平',
      '垂直',
      '等しい長さ',
      '平行',
      '点を直線上に配置',
      '線対称',
      '回転対称',
      '角の二等分',
      '長さの比',
    ]) {
      expect(screen.getByText(label)).toBeTruthy()
    }
    expect(screen.getByText('11件')).toBeTruthy()
    expect(screen.getAllByRole('button', { name: /制約を削除/u })).toHaveLength(11)

    fireEvent.click(screen.getAllByRole('button', { name: /制約を削除/u })[0]!)
    expect(onRemove).toHaveBeenCalledWith(IDS[12])
    fireEvent.click(screen.getAllByRole('button', { name: '対象を選択' })[0]!)
    expect(onSelectEdge).toHaveBeenCalledWith(IDS[0])
  })

  it('switches every constraint status and control to fixed English UI text', () => {
    const localeStore = localeFixture('ja')
    const onRetryAnalysis = vi.fn()
    const direct: GeometricConstraintPreflightResult = {
      status: 'direct_conflict',
      conflicts: [{
        conflict: { kind: 'horizontal_and_vertical', edge: IDS[0]! },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }],
    }
    const { rerender } = renderPanel({
      document: allKinds(),
      preflight: direct,
      selectedEdgeId: IDS[0],
      onRetryAnalysis,
      localeStore,
    })
    expect(screen.getByRole('heading', { name: '幾何制約' })).toBeTruthy()

    act(() => {
      localeStore.setLocale('en')
    })

    expect(
      screen.getByRole('heading', { name: 'Geometric constraints' }),
    ).toBeTruthy()
    expect(screen.getByText('11 constraints')).toBeTruthy()
    expect(
      screen.getByRole('button', {
        name: 'Constrain selected line horizontally',
      }),
    ).toBeTruthy()
    expect(
      screen.getByRole('button', {
        name: 'Constrain selected line vertically',
      }),
    ).toBeTruthy()
    for (const name of [
      'Fixed length',
      'Fixed angle',
      'Horizontal',
      'Vertical',
      'Equal length',
      'Parallel',
      'Point on line',
      'Mirror symmetry',
      'Rotational symmetry',
      'Angle bisector',
      'Length ratio',
    ]) {
      expect(screen.getByText(name)).toBeTruthy()
    }
    expect(
      screen.getByRole('button', {
        name: 'Delete Fixed length constraint',
      }),
    ).toBeTruthy()
    expect(screen.getAllByRole('button', { name: 'Select target' })).toHaveLength(10)

    let alert = screen.getByRole('alert')
    expect(alert.getAttribute('aria-live')).toBe('assertive')
    expect(alert.getAttribute('aria-atomic')).toBe('true')
    expect(alert.textContent).toContain('1 direct conflicts found.')
    expect(
      screen.getByRole('list', { name: 'Direct conflict causes' }).textContent,
    ).toContain(
      'Edge 00000000…0001 is constrained as both horizontal and vertical',
    )
    expect(alert.textContent).toContain(
      'Causing constraints: 00000000…0013, 00000000…0014',
    )
    fireEvent.click(screen.getByRole('button', { name: 'Analyze again' }))
    expect(onRetryAnalysis).toHaveBeenCalledTimes(1)

    const conflictCases = [
      {
        conflict: {
          kind: 'different_fixed_lengths' as const,
          edge: IDS[0]!,
        },
        expected: 'Different lengths are assigned to the same edge',
      },
      {
        conflict: {
          kind: 'different_fixed_angles' as const,
          vertex: IDS[6]!,
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        expected: 'Different angles are assigned to the same angle',
      },
      {
        conflict: {
          kind: 'different_length_ratios' as const,
          numerator_edge: IDS[0]!,
          denominator_edge: IDS[1]!,
        },
        expected:
          'Different length ratios are assigned to the same pair of edges',
      },
      {
        conflict: {
          kind: 'equal_length_with_different_fixed_lengths' as const,
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        expected:
          'Edges constrained to equal length have different fixed lengths',
      },
      {
        conflict: {
          kind: 'parallel_with_fixed_non_parallel_angle' as const,
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        expected: 'Parallel edges have a fixed angle that is not parallel',
      },
      {
        conflict: {
          kind: 'parallel_with_perpendicular_orientations' as const,
          horizontal_edge: IDS[0]!,
          vertical_edge: IDS[1]!,
        },
        expected:
          'Parallel edges are separately constrained as horizontal and vertical',
      },
    ]
    for (const { conflict, expected } of conflictCases) {
      rerender(panel({
        localeStore,
        preflight: {
          status: 'direct_conflict',
          conflicts: [{
            conflict,
            constraint_ids: [IDS[12]!, IDS[13]!],
          }],
        },
      }))
      expect(screen.getByRole('alert').textContent).toContain(expected)
    }

    for (const [reason, expected] of [
      [
        'work_limit_exceeded',
        'Indeterminate because the analysis work limit was reached.',
      ],
      [
        'solver_required_constraint_kinds',
        'Indeterminate because a complete constraint solver is required.',
      ],
      [
        'invalid_document_or_geometry',
        'Indeterminate because the constraints or crease pattern could not be validated.',
      ],
    ] as const) {
      rerender(panel({
        localeStore,
        preflight: {
          status: 'unknown',
          reason,
          unchecked_constraint_ids: ['private-untrusted-constraint-id'],
        },
      }))
      alert = screen.getByRole('alert')
      expect(alert.textContent).toContain(expected)
      expect(alert.textContent).toContain(
        'Do not treat the constraints as safety-verified.',
      )
      expect(alert.textContent).toContain(
        'Unchecked constraints: invalid identifier',
      )
      expect(alert.textContent).not.toContain(
        'private-untrusted-constraint-id',
      )
    }

    rerender(panel({
      localeStore,
      preflight: { status: 'no_direct_conflict' },
    }))
    let status = screen.getByRole('status')
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.textContent).toContain(
      'No direct conflicts found (satisfiability of all constraints is not proven)',
    )

    rerender(panel({ localeStore, analyzing: true }))
    expect(screen.getByRole('status').textContent).toContain(
      'Analyzing constraints…',
    )
    rerender(panel({ localeStore, analysisFailed: true }))
    expect(screen.getByRole('alert').textContent).toContain(
      'Constraint analysis could not be completed.',
    )
    rerender(panel({ localeStore }))
    status = screen.getByRole('status')
    expect(status.textContent).toContain(
      'The current constraints have not been analyzed.',
    )
    expect(screen.getByText('No constraints yet.')).toBeTruthy()
  })

  it('never presents direct-conflict or unknown results as safe', () => {
    const direct: GeometricConstraintPreflightResult = {
      status: 'direct_conflict',
      conflicts: [{
        conflict: { kind: 'horizontal_and_vertical', edge: IDS[0] },
        constraint_ids: [IDS[12], IDS[13]],
      }],
    }
    const { rerender } = renderPanel({ preflight: direct })

    let alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('直接矛盾があります（1件）')
    expect(screen.getByRole('list', { name: '直接矛盾の原因' }).textContent).toContain(
      '水平と垂直が同時に指定されています',
    )
    expect(alert.textContent).toContain('原因となる制約')
    expect(alert.textContent).toContain('00000000…0013')
    expect(alert.textContent).toContain('00000000…0014')
    expect(alert.classList.contains('is-blocking')).toBe(true)

    rerender(panel({
      preflight: {
        status: 'unknown',
        reason: 'solver_required_constraint_kinds',
        unchecked_constraint_ids: [IDS[12]],
      },
    }))
    alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('判定保留')
    expect(alert.textContent).toContain('安全確認済みとして扱いません')
    expect(alert.textContent).toContain('未確認の制約: 00000000…0013')
    expect(alert.classList.contains('is-blocking')).toBe(true)

    rerender(panel({
      preflight: null,
      analysisFailed: true,
    }))
    alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('安全確認済みとして扱いません')
  })

  it('uses the exact narrow wording for a no-direct-conflict result', () => {
    renderPanel({
      preflight: { status: 'no_direct_conflict' },
    })

    const status = screen.getByRole('status')
    expect(status.textContent).toContain(
      '直接矛盾は見つかりません（全制約の充足可能性は未証明）',
    )
    expect(status.classList.contains('is-clear')).toBe(true)
  })

  it('shortens canonical constraint IDs without restricting UUID version or variant bits', () => {
    const id = 'abcdef00-0000-0000-7000-00000000abcd'
    renderPanel({
      preflight: {
        status: 'unknown',
        reason: 'solver_required_constraint_kinds',
        unchecked_constraint_ids: [id],
      },
    })

    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('abcdef00…abcd')
    expect(alert.textContent).not.toContain(id)
  })

  it('renders every admitted direct-conflict kind and unknown reason explicitly', () => {
    type Conflict = Extract<
      GeometricConstraintPreflightResult,
      { status: 'direct_conflict' }
    >['conflicts'][number]
    const conflicts: Array<[Conflict, string]> = [
      [{
        conflict: { kind: 'different_fixed_lengths', edge: IDS[0]! },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }, '異なる長さ'],
      [{
        conflict: {
          kind: 'different_fixed_angles',
          vertex: IDS[6]!,
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }, '異なる角度'],
      [{
        conflict: {
          kind: 'different_length_ratios',
          numerator_edge: IDS[0]!,
          denominator_edge: IDS[1]!,
        },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }, '異なる長さ比'],
      [{
        conflict: { kind: 'horizontal_and_vertical', edge: IDS[0]! },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }, '水平と垂直が同時'],
      [{
        conflict: {
          kind: 'equal_length_with_different_fixed_lengths',
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        constraint_ids: [IDS[12]!, IDS[13]!, IDS[14]!],
      }, '等長にした辺へ異なる固定長'],
      [{
        conflict: {
          kind: 'parallel_with_fixed_non_parallel_angle',
          first_edge: IDS[0]!,
          second_edge: IDS[1]!,
        },
        constraint_ids: [IDS[12]!, IDS[13]!],
      }, '平行にした辺へ平行でない固定角'],
      [{
        conflict: {
          kind: 'parallel_with_perpendicular_orientations',
          horizontal_edge: IDS[0]!,
          vertical_edge: IDS[1]!,
        },
        constraint_ids: [IDS[12]!, IDS[13]!, IDS[14]!],
      }, '平行にした辺へ水平と垂直'],
    ]
    const { rerender } = renderPanel()
    for (const [conflict, expected] of conflicts) {
      rerender(panel({
        preflight: { status: 'direct_conflict', conflicts: [conflict] },
      }))
      expect(screen.getByRole('alert').textContent).toContain(expected)
    }

    for (const [reason, expected] of [
      ['work_limit_exceeded', '診断の処理上限'],
      ['solver_required_constraint_kinds', '完全な制約ソルバー'],
      ['invalid_document_or_geometry', '制約または展開図を検証できない'],
    ] as const) {
      rerender(panel({
        preflight: {
          status: 'unknown',
          reason,
          unchecked_constraint_ids: [IDS[12]!],
        },
      }))
      expect(screen.getByRole('alert').textContent).toContain(expected)
    }
  })

  it('disables every mutation and retry while the project is busy', () => {
    renderPanel({
      document: allKinds(),
      selectedEdgeId: IDS[0],
      disabled: true,
      analysisFailed: true,
    })

    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true)
    }
  })

  it('bounds large persisted documents before creating interactive rows', () => {
    const constraints = Array.from({ length: 201 }, (_, index) => ({
      id: generatedId(index + 100),
      constraint: {
        kind: 'horizontal' as const,
        edge: IDS[0]!,
      },
    }))
    renderPanel({
      document: { schema_version: 1, constraints },
    })

    expect(screen.getAllByRole('button', { name: /制約を削除/u })).toHaveLength(200)
    expect(screen.getByText(/先頭200件を表示しています。残り1件/u)).toBeTruthy()
  })
})

function renderPanel(overrides: Partial<Parameters<typeof panel>[0]> = {}) {
  return render(panel(overrides))
}

function panel(overrides: Partial<{
  document: GeometricConstraintDocument
  preflight: GeometricConstraintPreflightResult | null
  analyzing: boolean
  analysisFailed: boolean
  selectedEdgeId: string | null
  disabled: boolean
  onAddOrientation: (orientation: 'horizontal' | 'vertical') => void
  onAddConstraint: (constraint: GeometricConstraintDocument['constraints'][number]['constraint']) => void
  onRemove: (id: string) => void
  onSelectEdge: (id: string) => void
  onRetryAnalysis: () => void
  localeStore: LocaleStore
}> = {}) {
  return (
    <GeometricConstraintPanel
      document={overrides.document ?? { schema_version: 1, constraints: [] }}
      preflight={overrides.preflight ?? null}
      analyzing={overrides.analyzing ?? false}
      analysisFailed={overrides.analysisFailed ?? false}
      selectedEdgeId={overrides.selectedEdgeId ?? null}
      disabled={overrides.disabled ?? false}
      onAddOrientation={overrides.onAddOrientation ?? (() => undefined)}
      onAddConstraint={overrides.onAddConstraint ?? (() => undefined)}
      onRemove={overrides.onRemove ?? (() => undefined)}
      onSelectEdge={overrides.onSelectEdge ?? (() => undefined)}
      onRetryAnalysis={overrides.onRetryAnalysis ?? (() => undefined)}
      localeStore={overrides.localeStore}
    />
  )
}

function allKinds(): GeometricConstraintDocument {
  const [e0, e1, e2, e3, e4, e5, v0, v1, v2, v3] = IDS
  return {
    schema_version: 1,
    constraints: [
      { id: IDS[12]!, constraint: { kind: 'fixed_length', edge: e0!, length_mm: 10 } },
      {
        id: IDS[13]!,
        constraint: {
          kind: 'fixed_angle',
          vertex: v0!,
          first_edge: e0!,
          second_edge: e1!,
          angle_degrees: 45,
        },
      },
      { id: IDS[14]!, constraint: { kind: 'horizontal', edge: e2! } },
      { id: IDS[15]!, constraint: { kind: 'vertical', edge: e3! } },
      {
        id: IDS[16]!,
        constraint: { kind: 'equal_length', first_edge: e0!, second_edge: e1! },
      },
      {
        id: IDS[17]!,
        constraint: { kind: 'parallel', first_edge: e2!, second_edge: e3! },
      },
      {
        id: IDS[18]!,
        constraint: { kind: 'point_on_line', vertex: v1!, line_edge: e4! },
      },
      {
        id: IDS[19]!,
        constraint: {
          kind: 'mirror_symmetry',
          first_vertex: v0!,
          second_vertex: v1!,
          axis_edge: e5!,
        },
      },
      {
        id: IDS[20]!,
        constraint: {
          kind: 'rotational_symmetry',
          center_vertex: v0!,
          source_vertex: v1!,
          target_vertex: v2!,
          angle_degrees: 120,
        },
      },
      {
        id: IDS[21]!,
        constraint: {
          kind: 'angle_bisector',
          vertex: v3!,
          first_edge: e0!,
          second_edge: e1!,
          bisector_edge: e2!,
        },
      },
      {
        id: IDS[22]!,
        constraint: {
          kind: 'length_ratio',
          numerator_edge: e4!,
          denominator_edge: e5!,
          ratio: 2,
        },
      },
    ],
  }
}

function generatedId(index: number) {
  return `10000000-0000-4000-8000-${String(index).padStart(12, '0')}`
}
