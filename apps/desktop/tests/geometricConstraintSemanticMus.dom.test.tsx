import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { GeometricConstraintPanel } from '../src/components/GeometricConstraintPanel'
import type {
  GeometricConstraintPreflightResult,
  GeometricConstraintSemanticMus,
} from '../src/lib/coreClient'
import { normalizeGeometricConstraintPreflightResponse } from '../src/lib/geometricConstraints'
import {
  BINDING,
  envelope,
  provenDirect,
} from './geometricConstraintSemanticMusTestSupport'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel'
import {
  GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
} from '../src/lib/geometricConstraintSemanticMus'
import { localeFixture } from './localeTestFixture'

const uuid = (index: number) =>
  `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
const CORE = Object.freeze([uuid(1), uuid(2)])
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
const CERTIFIED_WIRE = {
  status: 'certified',
  model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
  transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
  constraint_ids: CORE,
  constraint_count: 2,
  direct_oracle_calls: 7,
  deletion_witness_checks: 2,
  deletion_witness_work: 100,
  current_assignment_witness_count: 0,
  axis_exactification_witness_count: 0,
  single_constraint_constructive_witness_count: 0,
  pair_constraint_constructive_witness_count: 1,
  pair_constraint_algebraic_witness_count: 1,
  length_constraint_constructive_witness_count: 0,
  zero_length_closure_constructive_witness_count: 0,
  authorizes_project_mutation: false,
  replayable_across_runtimes: true,
}

function parsedCertified(
  overrides: Partial<Extract<
    GeometricConstraintSemanticMus,
    { status: 'certified' }
  >>,
): GeometricConstraintSemanticMus {
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(
      { ...CERTIFIED_WIRE, ...overrides, constraint_ids: CORE },
      provenDirect(),
    ),
    BINDING,
  )
  if (normalized?.semantic_mus?.status !== 'certified') {
    throw new Error('expected parsed certified semantic MUS')
  }
  return normalized.semantic_mus
}

const CERTIFIED = parsedCertified({})

afterEach(cleanup)

describe('geometric-constraint semantic MUS status', () => {
  it('labels a certified result as deterministic and disclaims witness portability', () => {
    renderPanel(CERTIFIED, 'en')

    const region = screen.getByRole('region', {
      name: 'Deterministic-binary64 semantic minimal-core certification',
    })
    expect(region.textContent).toContain(
      'Certified a deterministic-binary64 semantic minimal core',
    )
    expect(region.textContent).toContain(
      '1 pair-constraint constructive, 1 pair-constraint algebraic-collapse',
    )
    expect(region.textContent).toContain('0 bounded length-only constructive')
    expect(region.textContent).toContain(
      '0 bounded zero-length-closure constructive',
    )
    expect(region.textContent).toContain(
      'Re-certifiable under the frozen deterministic model',
    )
    expect(region.textContent).toContain(
      'does not claim portability of a serialized witness',
    )
    expect(region.textContent).toContain(
      'does not authorize project mutation and makes no portability claim',
    )
    expect(region.textContent).not.toContain('not certified')
  })

  it('labels a certified unsupported-target result as current-runtime fallback', () => {
    renderPanel(parsedCertified({
      replayable_across_runtimes: false,
    }), 'en')

    const region = screen.getByRole('region', {
      name: 'Deterministic-binary64 semantic minimal-core certification',
    })
    expect(region.textContent).toContain(
      'On this target, this is a current-runtime-only fallback.',
    )
    expect(region.textContent).not.toContain(
      'Re-certifiable under the frozen deterministic model',
    )
  })

  it('shows both pair methods in Japanese with the same labelled region', () => {
    renderPanel(CERTIFIED, 'ja')

    const region = screen.getByRole('region', {
      name: '決定論的binary64意味論的最小コア認証',
    })
    expect(region.textContent).toContain('二制約構成1件')
    expect(region.textContent).toContain('二制約代数縮退1件')
    expect(region.textContent).toContain('有界長さ制約構成0件')
    expect(region.textContent).toContain('ゼロ長閉包構成0件')
  })

  it('shows a bounded length-only witness count without inflating its total', () => {
    renderPanel(parsedCertified({
      pair_constraint_constructive_witness_count: 0,
      pair_constraint_algebraic_witness_count: 0,
      length_constraint_constructive_witness_count: 2,
      zero_length_closure_constructive_witness_count: 0,
    }), 'ja')

    const region = screen.getByRole('region', {
      name: '決定論的binary64意味論的最小コア認証',
    })
    expect(region.textContent).toContain('削除証人2件')
    expect(region.textContent).toContain('有界長さ制約構成2件')
    expect(region.textContent).not.toContain('有界長さ制約構成3件')
  })

  it('shows the zero-length-closure method in Japanese without inflating the total', () => {
    renderPanel(parsedCertified({
      pair_constraint_constructive_witness_count: 0,
      pair_constraint_algebraic_witness_count: 0,
      zero_length_closure_constructive_witness_count: 2,
    }), 'ja')

    const region = screen.getByRole('region', {
      name: '決定論的binary64意味論的最小コア認証',
    })
    expect(region.textContent).toContain('削除証人2件')
    expect(region.textContent).toContain('ゼロ長閉包構成2件')
    expect(region.textContent).not.toContain('ゼロ長閉包構成3件')
  })

  it('fails closed instead of overstating forged in-process witness counts', () => {
    renderPanel(Object.freeze({
      ...CERTIFIED,
      current_assignment_witness_count: 2,
    }), 'en')

    const region = screen.getByRole('region', {
      name: 'Deterministic-binary64 semantic minimal-core certification',
    })
    expect(region.textContent).toContain(
      'does not contain semantic minimal-core certification information',
    )
    expect(region.textContent).not.toContain(
      'Certified a semantic minimal core',
    )
    expect(region.textContent).not.toContain('3 deletion witnesses')
  })

  it('does not promote an Unknown direct core to semantic minimality', () => {
    const unknown: GeometricConstraintSemanticMus = {
      status: 'unknown',
      model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
      transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
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
      name: 'Deterministic-binary64 semantic minimal-core certification',
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
      'does not authorize project mutation and makes no portability claim',
    )
  })

  it('reports pre-core Unknown and legacy responses without a semantic claim', () => {
    const unknown: GeometricConstraintSemanticMus = {
      status: 'unknown',
      model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
      transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
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
      name: '決定論的binary64意味論的最小コア認証',
    })
    expect(region.textContent).toContain('意味論的最小コアは認証されていません')
    expect(region.textContent).toContain('直接コアの確定前に停止しました')
    expect(region.textContent).toContain(
      'プロジェクト変更を許可せず、シリアライズ済み証人の可搬性も主張しません',
    )

    rerender(panel(null, 'ja'))
    region = screen.getByRole('region', {
      name: '決定論的binary64意味論的最小コア認証',
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
