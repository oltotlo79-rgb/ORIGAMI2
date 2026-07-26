import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { GeometricConstraintPanel } from '../src/components/GeometricConstraintPanel'
import type {
  GeometricConstraintPreflightResult,
  GeometricConstraintSemanticMus,
} from '../src/lib/coreClient'
import { localeFixture } from './localeTestFixture'

const uuid = (index: number) =>
  `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
const CORE = [uuid(1), uuid(2)]
const DIRECT: GeometricConstraintPreflightResult = {
  status: 'direct_conflict',
  conflicts: [{
    conflict: { kind: 'different_fixed_lengths', edge: uuid(20) },
    constraint_ids: CORE,
  }],
  bounded_direct_mus: {
    status: 'proven_unsatisfiable',
    constraint_ids: CORE,
    oracle_calls: 7,
  },
}
const CERTIFIED: GeometricConstraintSemanticMus = {
  status: 'certified',
  model_id: 'geometric_constraint_current_runtime_semantic_mus_v1',
  constraint_ids: CORE,
  constraint_count: 2,
  direct_oracle_calls: 7,
  deletion_witness_checks: 2,
  deletion_witness_work: 100,
  current_assignment_witness_count: 1,
  axis_exactification_witness_count: 0,
  single_constraint_constructive_witness_count: 1,
  authorizes_project_mutation: false,
  replayable_across_runtimes: false,
}

afterEach(cleanup)

describe('geometric-constraint semantic MUS status', () => {
  it('labels a certified result as current-runtime only and disclaims authority', () => {
    renderPanel(CERTIFIED, 'en')

    const region = screen.getByRole('region', {
      name: 'Current-runtime semantic minimal-core certification',
    })
    expect(region.textContent).toContain(
      'Certified a semantic minimal core in the current runtime',
    )
    expect(region.textContent).toContain(
      '1 current-assignment, 0 axis-exactification, 1 single-constraint constructive',
    )
    expect(region.textContent).toContain(
      'does not authorize project mutation and cannot be replayed across runtimes',
    )
    expect(region.textContent).not.toContain('not certified')
  })

  it('does not promote an Unknown direct core to semantic minimality', () => {
    const unknown: GeometricConstraintSemanticMus = {
      status: 'unknown',
      model_id: 'geometric_constraint_current_runtime_semantic_mus_v1',
      reason: 'deletion_witness_unavailable',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_checks: 1,
      certified_deletion_witnesses: 0,
      deletion_witness_work: 99,
      max_deletion_witness_checks: 16,
      max_deletion_witness_work: 20_000_000,
      authorizes_project_mutation: false,
      replayable_across_runtimes: false,
    }
    renderPanel(unknown, 'en')

    const region = screen.getByRole('region', {
      name: 'Current-runtime semantic minimal-core certification',
    })
    expect(region.textContent).toContain(
      'A direct-conflict core (2 constraints) was found',
    )
    expect(region.textContent).toContain(
      'semantic minimality is not certified',
    )
    expect(region.textContent).toContain(
      'A required deletion witness could not be certified.',
    )
    expect(region.textContent).toContain(
      'does not authorize project mutation and cannot be replayed across runtimes',
    )
  })

  it('reports pre-core Unknown and legacy responses without a semantic claim', () => {
    const unknown: GeometricConstraintSemanticMus = {
      status: 'unknown',
      model_id: 'geometric_constraint_current_runtime_semantic_mus_v1',
      reason: 'cancelled',
      direct_core_constraint_ids: [],
      direct_oracle_calls: 0,
      deletion_witness_checks: 0,
      certified_deletion_witnesses: 0,
      deletion_witness_work: 0,
      max_deletion_witness_checks: 16,
      max_deletion_witness_work: 20_000_000,
      authorizes_project_mutation: false,
      replayable_across_runtimes: false,
    }
    const { rerender } = renderPanel(unknown, 'ja')
    let region = screen.getByRole('region', {
      name: '現在の実行環境で意味論的最小コア認証',
    })
    expect(region.textContent).toContain('意味論的最小コアは認証されていません')
    expect(region.textContent).toContain('直接コアの確定前に停止しました')
    expect(region.textContent).toContain(
      'プロジェクト変更を許可せず、別の実行環境で再利用できません',
    )

    rerender(panel(null, 'ja'))
    region = screen.getByRole('region', {
      name: '現在の実行環境で意味論的最小コア認証',
    })
    expect(region.textContent).toContain(
      'この応答には意味論的最小コア認証情報がありません',
    )
    expect(region.textContent).not.toContain('意味論的最小コアを認証しました')
  })
})

function renderPanel(
  semanticMus: GeometricConstraintSemanticMus | null,
  locale: 'ja' | 'en',
) {
  return render(panel(semanticMus, locale))
}

function panel(
  semanticMus: GeometricConstraintSemanticMus | null,
  locale: 'ja' | 'en',
) {
  return (
    <GeometricConstraintPanel
      document={{ schema_version: 1, constraints: [] }}
      preflight={DIRECT}
      semanticMus={semanticMus}
      analyzing={false}
      analysisFailed={false}
      selectedEdgeId={null}
      disabled={false}
      onAddOrientation={() => undefined}
      onAddConstraint={() => undefined}
      onRemove={() => undefined}
      onSelectEdge={() => undefined}
      onRetryAnalysis={() => undefined}
      localeStore={localeFixture(locale)}
    />
  )
}
