import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ProofScopeSummary } from '../src/components/ProofScopeSummary'
import { GLOBAL_FLAT_FOLDABILITY_MODEL_ID } from '../src/lib/globalFlatFoldability'
import { LOCAL_SUFFICIENCY_CERTIFICATE_MODEL } from '../src/lib/proofScopePresentation'
import { localeFixture } from './localeTestFixture'

const localSummary = {
  version: 1 as const,
  projectInstanceId: 'instance',
  projectId: 'project',
  revision: 1,
  foldModelFingerprint: 'a'.repeat(64),
  vertices: [
    { vertex: 'vertex-a', status: 'necessary_failed' as const },
    {
      vertex: 'vertex-b',
      status: 'sufficient_proven' as const,
      model_id: LOCAL_SUFFICIENCY_CERTIFICATE_MODEL,
      reduction_steps: 0,
    },
    {
      vertex: 'vertex-c',
      status: 'indeterminate' as const,
      reason: 'resource_limit' as const,
    },
  ],
  totalReductionSteps: 0,
  authorizesProjectMutation: false as const,
}
const globalJob = {
  state: 'completed',
  result: {
    verdict: 'unknown',
    summary: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      elapsed_ms: 1,
      counts: {
        face_count: 2,
        overlap_cell_count: 0,
        constraint_count: 0,
        search_node_count: 1,
      },
    },
    reason: 'proof_not_completed',
  },
}

afterEach(cleanup)

describe('ProofScopeSummary', () => {
  it('renders distinct English proof ranges and supports related selection', () => {
    const onSelectVertex = vi.fn()
    render(
      <ProofScopeSummary
        globalJob={globalJob}
        localSummary={localSummary}
        localeStore={localeFixture('en')}
        selectedVertexId="vertex-b"
        onSelectVertex={onSelectVertex}
      />,
    )
    const region = screen.getByRole('region', { name: 'Proof coverage' })
    expect(region.textContent).toContain('Unknown')
    expect(region.textContent).toContain('Necessary failed 1')
    expect(region.textContent).toContain('sufficiency proven 1')
    expect(region.textContent).toContain('indeterminate 1')
    expect(region.textContent).toContain(GLOBAL_FLAT_FOLDABILITY_MODEL_ID)
    expect(region.textContent).toContain(LOCAL_SUFFICIENCY_CERTIFICATE_MODEL)
    const sufficient = screen.getByRole('button', {
      name: 'Vertex 2 · Sufficiency proven',
    })
    expect(sufficient.getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(screen.getByRole('button', {
      name: 'Vertex 1 · Necessary failed',
    }))
    expect(onSelectVertex).toHaveBeenCalledWith('vertex-a')
    const diagnostics = screen.getByLabelText(
      'Proof coverage diagnostics JSON',
    ) as HTMLTextAreaElement
    expect(diagnostics.readOnly).toBe(true)
    expect(diagnostics.value).not.toMatch(/vertex-a|instance|project|fingerprint/u)
  })

  it('renders Japanese copy without treating unavailable local data as proof', () => {
    render(
      <ProofScopeSummary
        globalJob={null}
        localSummary={null}
        localeStore={localeFixture('ja')}
      />,
    )
    const region = screen.getByRole('region', { name: '証明範囲' })
    expect(region.textContent).toContain('未判定')
    expect(region.textContent).toContain('未取得')
    expect(region.textContent).toContain(
      '全体判定・局所必要条件・局所十分性は、互いに別の証明です。',
    )
  })

  it('switches locale in place without changing proof identity or selection', () => {
    const onSelectVertex = vi.fn()
    const store = localeFixture('en')
    render(
      <ProofScopeSummary
        globalJob={globalJob}
        localSummary={localSummary}
        localeStore={store}
        selectedVertexId="vertex-b"
        onSelectVertex={onSelectVertex}
      />,
    )
    const english = screen.getByRole('region', { name: 'Proof coverage' })
    const diagnostics = screen.getByLabelText(
      'Proof coverage diagnostics JSON',
    ) as HTMLTextAreaElement
    const diagnosticsJson = diagnostics.value
    expect(english.textContent).toContain('Unknown')
    expect(english.textContent).toContain(
      'Necessary failed 1; sufficiency proven 1; indeterminate 1',
    )
    act(() => {
      store.setLocale('ja')
    })
    const japanese = screen.getByRole('region', { name: '証明範囲' })
    expect(japanese).toBe(english)
    expect(japanese.textContent).toContain('不明')
    expect(japanese.textContent).toContain(
      '必要条件不成立 1・十分性証明 1・判定不能 1',
    )
    expect(japanese.textContent).toContain(GLOBAL_FLAT_FOLDABILITY_MODEL_ID)
    expect(japanese.textContent).toContain(LOCAL_SUFFICIENCY_CERTIFICATE_MODEL)
    const selected = screen.getByRole('button', { name: '頂点 2 · 十分性証明' })
    expect(selected.getAttribute('aria-pressed')).toBe('true')
    const japaneseDiagnostics = screen.getByLabelText(
      '証明範囲diagnostics JSON',
    ) as HTMLTextAreaElement
    expect(japaneseDiagnostics).toBe(diagnostics)
    expect(japaneseDiagnostics.value).toBe(diagnosticsJson)
    expect(japaneseDiagnostics.value).not.toMatch(
      /vertex-a|instance|project|fingerprint/u,
    )
    act(() => {
      store.setLocale('en')
    })
    expect(screen.getByRole('region', { name: 'Proof coverage' })).toBe(english)
    expect(screen.getByRole('button', {
      name: 'Vertex 2 · Sufficiency proven',
    }).getAttribute('aria-pressed')).toBe('true')
    expect(onSelectVertex).not.toHaveBeenCalled()
  })
})
