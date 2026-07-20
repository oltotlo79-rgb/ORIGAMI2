import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'

import {
  GlobalFlatFoldabilityPanel,
  type GlobalFlatFoldabilityPanelProps,
} from '../src/components/GlobalFlatFoldabilityPanel'
import {
  GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
} from '../src/lib/globalFlatFoldability'
import { localeFixture } from './localeTestFixture'

const COUNTS = {
  face_count: 6,
  overlap_cell_count: 12,
  constraint_count: 345,
  search_node_count: 6_789,
}
const SUMMARY = {
  model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  elapsed_ms: 3_500,
  counts: COUNTS,
}
const RUNNING = {
  state: 'running',
  cancel_requested: false,
  progress: {
    model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    phase: 'building_overlap_arrangement',
    completed_work: 12_340,
    total_work: null,
    elapsed_ms: 2_000,
    counts: COUNTS,
  },
}

afterEach(() => {
  cleanup()
  document.body.replaceChildren()
})

describe('GlobalFlatFoldabilityPanel DOM interactions', () => {
  it('shares layer face selection and hover, then clears hover on revision drift', async () => {
    const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
    const project = '018f47a2-4b7a-7cc1-8abc-665544332211'
    const loadLayerOrderView = vi.fn(async (authority) => ({
      ...authority,
      layerOrderGeneration: 1,
      cells: [{
        cellKeySha256: 'a'.repeat(64),
        bottomToTopFaces: [instance, project],
        boundaryWorld: [[0, 0, 0], [1, 0, 0], [0, 0, -1]],
      }],
      readOnly: true,
    }))
    const onSelectFace = vi.fn()
    const onHoverFace = vi.fn()
    const { rerender } = renderPanel({
      job: terminalJob('possible'),
      authority: { projectInstanceId: instance, projectId: project, revision: 3 },
      onSelectFace,
      onHoverFace,
      loadLayerOrderView,
      localeStore: localeFixture('en'),
    })
    const top = await screen.findByRole('button', { name: /Front \/ top/u })
    fireEvent.mouseEnter(top)
    expect(onHoverFace).toHaveBeenLastCalledWith(project)
    fireEvent.click(top)
    expect(onSelectFace).toHaveBeenCalledWith(project)

    rerender(panelElement({
      job: terminalJob('possible'),
      authority: { projectInstanceId: instance, projectId: project, revision: 4 },
      onSelectFace,
      onHoverFace,
      loadLayerOrderView,
      localeStore: localeFixture('en'),
    }))
    await waitFor(() => expect(onHoverFace).toHaveBeenLastCalledWith(null))
    expect(onSelectFace).toHaveBeenLastCalledWith(null)
  })

  it('starts idle with the three time presets and the permanent scope caution', () => {
    renderPanel()

    const panel = screen.getByRole('region', { name: '全体平坦折り判定' })
    expect(
      screen.getByRole('group', { name: '未判定' }).getAttribute('aria-busy'),
    ).toBe('false')
    const timeLimit = screen.getByRole(
      'combobox',
      { name: '時間制限' },
    ) as HTMLSelectElement
    expect(timeLimit.value).toBe('30')
    expect(
      screen.getAllByRole('option').map((option) => option.textContent),
    ).toEqual(['5秒', '30秒', '120秒'])
    expect(screen.getByRole('button', { name: '判定を開始' })).toBeTruthy()
    expect(screen.getAllByText('未判定').length).toBeGreaterThan(0)
    expect(screen.getByText(GLOBAL_FLAT_FOLDABILITY_MODEL_ID)).toBeTruthy()
    expect(screen.getByText('凸多角形面（切断・穴・未接続材料なし）')).toBeTruthy()
    expect(screen.getByText('「可」が保証しないこと')).toBeTruthy()
    expect(panel.textContent).toContain('紙厚や層ずれ')
    expect(panel.textContent).toContain('手で折りやすい')
    expect(panel.textContent).toContain('安全にたどれる連続した折り経路')
  })

  it('switches controls, progress, results and accessibility text live to English', () => {
    const localeStore = localeFixture('ja')
    const { rerender } = renderPanel({
      job: RUNNING,
      localeStore,
    })
    expect(
      screen.getByRole('region', { name: '全体平坦折り判定' }),
    ).toBeTruthy()

    act(() => {
      localeStore.setLocale('en')
    })

    const panel = screen.getByRole(
      'region',
      { name: 'Global flat-foldability check' },
    )
    expect(
      screen.getByText('Time-limited three-way result'),
    ).toBeTruthy()
    const timeLimit = screen.getByRole(
      'combobox',
      { name: 'Time limit' },
    ) as HTMLSelectElement
    expect(timeLimit.disabled).toBe(true)
    expect(
      screen.getAllByRole('option').map((option) => option.textContent),
    ).toEqual(['5 seconds', '30 seconds', '120 seconds'])
    expect(screen.getByRole('button', { name: 'Checking…' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Cancel check' })).toBeTruthy()
    expect(
      screen.getByRole('group', { name: 'Checking' })
        .getAttribute('aria-busy'),
    ).toBe('true')
    expect(screen.getByText('Building overlap regions')).toBeTruthy()
    expect(
      screen.getByText('12,340 completed (total still being calculated)'),
    ).toBeTruthy()
    expect(panel.textContent).toContain('Elapsed time')
    expect(panel.textContent).toContain('Overlap cells')
    expect(panel.textContent).toContain('Search nodes')
    const live = screen.getByRole('status')
    expect(live.getAttribute('aria-live')).toBe('polite')
    expect(live.getAttribute('aria-atomic')).toBe('true')
    expect(live.textContent).toBe('Checking. Building overlap regions.')
    const caution = screen.getByRole('complementary', {
      name: 'Important limitations of the result',
    })
    expect(caution.textContent).toContain(
      'What “Possible” does not guarantee',
    )
    expect(caution.textContent).toContain(
      'This check uses an ideal zero-thickness model.',
    )

    rerender(panelElement({
      localeStore,
      job: {
        ...RUNNING,
        cancel_requested: true,
      },
    }))
    expect(
      screen.getByRole('group', { name: 'Cancelling' }),
    ).toBeTruthy()
    expect(
      screen.getByRole('button', { name: 'Cancel requested' }),
    ).toBeTruthy()
    expect(screen.getByRole('status').textContent).toBe(
      'Cancelling. Building overlap regions.',
    )

    for (const [kind, label] of [
      ['possible', 'Possible'],
      ['impossible', 'Impossible'],
      ['unknown', 'Unknown'],
      ['cancelled', 'Cancelled'],
      ['failed', 'Calculation error'],
      ['stale', 'Outdated result'],
    ] as const) {
      rerender(panelElement({
        localeStore,
        job: terminalJob(kind),
      }))
      expect(
        document.querySelector(`[data-result-kind="${kind}"]`),
      ).toBeTruthy()
      expect(screen.getByRole('group', { name: label })).toBeTruthy()
      expect(screen.getByRole('button', { name: 'Run again' })).toBeTruthy()
      expect(screen.getByRole('status').textContent?.length).toBeGreaterThan(0)
    }

    rerender(panelElement({
      localeStore,
      job: terminalJob('unknown'),
    }))
    expect(screen.getByText('Reason for indeterminate result')).toBeTruthy()
    expect(screen.getAllByText(/selected time limit/u).length).toBeGreaterThan(0)

    const privateValue =
      'C:\\Users\\alice\\秘密\\作品.ori; face_uuid=private; point=(12.3,45.6)'
    rerender(panelElement({
      localeStore,
      job: {
        state: 'failed',
        summary: SUMMARY,
        error_category: 'internal_failure',
        raw_error: privateValue,
      },
    }))
    expect(screen.getByText('Calculation error')).toBeTruthy()
    expect(
      screen.getByText(
        'The result format could not be verified safely. Its contents are hidden; run the check again with the current edits.',
      ),
    ).toBeTruthy()
    expect(document.body.textContent).not.toMatch(
      /alice|秘密|face_uuid|12\.3|45\.6/iu,
    )

    rerender(panelElement({ localeStore }))
    expect(screen.getByRole('group', { name: 'Not checked' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Start check' })).toBeTruthy()
  })

  it('changes only to an allowlisted preset and starts with the controlled value', () => {
    const onTimeLimitChange = vi.fn()
    const onStart = vi.fn()
    const { rerender } = renderPanel({ onTimeLimitChange, onStart })

    fireEvent.change(screen.getByLabelText('時間制限'), {
      target: { value: '120' },
    })
    expect(onTimeLimitChange).toHaveBeenCalledWith(120)

    rerender(panelElement({
      timeLimitSeconds: 120,
      onTimeLimitChange,
      onStart,
    }))
    fireEvent.click(screen.getByRole('button', { name: '判定を開始' }))
    expect(onStart).toHaveBeenCalledWith(120)

    fireEvent.change(screen.getByLabelText('時間制限'), {
      target: { value: '60' },
    })
    expect(onTimeLimitChange).toHaveBeenCalledTimes(1)
  })

  it('shows phase and bounded counts without a fabricated percentage', () => {
    const { rerender } = renderPanel({ job: RUNNING })

    const panel = screen.getByRole('region', { name: '全体平坦折り判定' })
    expect(
      screen.getByRole('group', { name: '判定中' }).getAttribute('aria-busy'),
    ).toBe('true')
    expect(screen.getByText('重なり領域を構築しています')).toBeTruthy()
    expect(screen.getByText('12,340件完了（総数は計算中）')).toBeTruthy()
    expect(screen.getByText('6件')).toBeTruthy()
    expect(screen.getByText('12件')).toBeTruthy()
    expect(screen.getByText('345件')).toBeTruthy()
    expect(screen.getByText('6,789件')).toBeTruthy()
    expect(panel.textContent).not.toContain('%')
    expect(screen.getByRole('status').textContent).toContain(
      '重なり領域を構築しています',
    )
    const announcement = screen.getByRole('status').textContent
    rerender(panelElement({
      job: {
        ...RUNNING,
        progress: {
          ...RUNNING.progress,
          completed_work: 12_341,
          elapsed_ms: 2_250,
        },
      },
    }))
    expect(screen.getByText(/12,341/u)).toBeTruthy()
    expect(screen.getByRole('status').textContent).toBe(announcement)
  })

  it('mutates the live region for phase changes but not for same-phase work ticks', () => {
    const { rerender } = renderPanel({ job: RUNNING })
    const liveRegion = screen.getByRole('status')
    const observer = new MutationObserver(() => undefined)
    observer.observe(liveRegion, {
      childList: true,
      characterData: true,
      subtree: true,
    })

    rerender(panelElement({
      job: {
        ...RUNNING,
        progress: {
          ...RUNNING.progress,
          completed_work: 12_341,
          elapsed_ms: 2_250,
        },
      },
    }))
    expect(observer.takeRecords()).toHaveLength(0)

    rerender(panelElement({
      job: {
        ...RUNNING,
        progress: {
          ...RUNNING.progress,
          phase: 'searching',
          completed_work: 12_342,
          elapsed_ms: 2_500,
        },
      },
    }))
    expect(observer.takeRecords().length).toBeGreaterThan(0)
    expect(liveRegion.textContent).toContain('層順序を探索しています')
    observer.disconnect()
  })

  it('keeps an enabled native cancel button reachable during running and cancellation', () => {
    const onCancel = vi.fn()
    const { rerender } = renderPanel({ job: RUNNING, onCancel })
    const cancel = screen.getByRole(
      'button',
      { name: '判定を中止' },
    ) as HTMLButtonElement
    expect(cancel.tagName).toBe('BUTTON')
    expect(cancel.disabled).toBe(false)
    cancel.focus()
    expect(document.activeElement).toBe(cancel)
    fireEvent.click(cancel)
    expect(onCancel).toHaveBeenCalledTimes(1)

    rerender(panelElement({
      job: {
        ...RUNNING,
        cancel_requested: true,
      },
      onCancel,
    }))
    const requested = screen.getByRole(
      'button',
      { name: '中止（要求済み）' },
    ) as HTMLButtonElement
    expect(requested.disabled).toBe(false)
    fireEvent.click(requested)
    expect(onCancel).toHaveBeenCalledTimes(2)
    expect(screen.getByRole('status').textContent).toContain('中止しています')
  })

  it('moves focus only from an explicit start to cancel and never steals it on completion', () => {
    const { rerender } = renderPanel()
    screen.getByRole('button', { name: '判定を開始' }).focus()

    rerender(panelElement({ job: RUNNING }))
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: '判定を中止' }),
    )

    const outside = document.createElement('button')
    outside.textContent = '編集中の操作'
    document.body.append(outside)
    outside.focus()
    rerender(panelElement({ job: terminalJob('possible') }))
    const result = document.querySelector<HTMLElement>(
      '[data-result-kind="possible"]',
    )
    expect(result).toBeTruthy()
    expect(document.activeElement).toBe(outside)
    expect(screen.getByRole('status').textContent).toContain('結果は、可です')

    rerender(panelElement())
    outside.focus()
    rerender(panelElement({ job: RUNNING }))
    expect(document.activeElement).toBe(outside)
  })

  it.each([
    ['possible', '可'],
    ['impossible', '不可'],
    ['unknown', '不明'],
    ['cancelled', '中止'],
    ['failed', '計算エラー'],
    ['stale', '古い結果'],
  ] as const)('renders %s as the distinct terminal label %s', (kind, label) => {
    renderPanel({ job: terminalJob(kind) })

    const result = document.querySelector(
      `[data-result-kind="${kind}"]`,
    )
    expect(result).toBeTruthy()
    expect(result?.textContent).toContain(label)
    expect(screen.getByRole('status').textContent?.length).toBeGreaterThan(0)
    expect(screen.getByText('「可」が保証しないこと')).toBeTruthy()
  })

  it('shows only public possible and impossible evidence', () => {
    const { rerender } = renderPanel({ job: terminalJob('possible') })
    expect(screen.getByText(GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID))
      .toBeTruthy()
    expect(screen.getByText('6層')).toBeTruthy()
    expect(screen.getByText('4 ply')).toBeTruthy()
    expect(screen.getByText('面 1')).toBeTruthy()
    expect(screen.getByText('利用できます')).toBeTruthy()

    rerender(panelElement({ job: terminalJob('impossible') }))
    expect(screen.getByText('層順序制約の矛盾')).toBeTruthy()
    expect(screen.getByText('面 2、面 5')).toBeTruthy()
  })

  it('fails closed on hostile DTOs without showing raw errors, coordinates or IDs', () => {
    const privateValue =
      'C:\\Users\\alice\\秘密の作品.ori; face_uuid=private; point=(12.3,45.6)'
    renderPanel({
      job: {
        state: 'failed',
        summary: SUMMARY,
        error_category: 'internal_failure',
        raw_error: privateValue,
      },
    })

    expect(screen.getByText('計算エラー')).toBeTruthy()
    expect(screen.getByText(/形式を安全に確認できませんでした/u)).toBeTruthy()
    expect(document.body.textContent).not.toMatch(
      /alice|秘密の作品|face_uuid|12\.3|45\.6/iu,
    )
  })

  it('uses wrapping, unbounded-height structures that keep results and cancel in one panel', () => {
    renderPanel({ job: RUNNING })
    const panel = screen.getByRole('region', { name: '全体平坦折り判定' })
    const cancel = screen.getByRole('button', { name: '判定を中止' })
    expect(panel.classList.contains('global-flat-foldability-panel')).toBe(true)
    expect(cancel.closest('.global-flat-foldability-running')).toBeTruthy()
    expect(panel.getAttribute('style')).toBeNull()
    expect(panel.querySelector('.global-flat-foldability-summary')).toBeTruthy()
    expect(panel.querySelector('.global-flat-foldability-caution')).toBeTruthy()
  })
})

function renderPanel(overrides: Partial<GlobalFlatFoldabilityPanelProps> = {}) {
  return render(panelElement(overrides))
}

function panelElement(
  overrides: Partial<GlobalFlatFoldabilityPanelProps> = {},
) {
  return (
    <GlobalFlatFoldabilityPanel
      job={overrides.job === undefined ? null : overrides.job}
      timeLimitSeconds={overrides.timeLimitSeconds ?? 30}
      startDisabled={overrides.startDisabled ?? false}
      onTimeLimitChange={overrides.onTimeLimitChange ?? vi.fn()}
      onStart={overrides.onStart ?? vi.fn()}
      onCancel={overrides.onCancel ?? vi.fn()}
      localeStore={overrides.localeStore}
      authority={overrides.authority}
      selectedFaceId={overrides.selectedFaceId}
      onSelectFace={overrides.onSelectFace}
      onHoverFace={overrides.onHoverFace}
      loadLayerOrderView={overrides.loadLayerOrderView}
    />
  )
}

function terminalJob(
  kind:
    | 'possible'
    | 'impossible'
    | 'unknown'
    | 'cancelled'
    | 'failed'
    | 'stale',
) {
  switch (kind) {
    case 'possible':
      return {
        state: 'completed',
        result: {
          verdict: 'possible',
          summary: SUMMARY,
          layer_order: {
            model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
            layer_count: 6,
            max_ply: 4,
            reference_face_number: 1,
            layer_view_available: true,
          },
        },
      }
    case 'impossible':
      return {
        state: 'completed',
        result: {
          verdict: 'impossible',
          summary: SUMMARY,
          proof: {
            category: 'layer_constraints_contradictory',
            face_numbers: [2, 5],
          },
        },
      }
    case 'unknown':
      return {
        state: 'completed',
        result: {
          verdict: 'unknown',
          summary: SUMMARY,
          reason: 'time_limit_reached',
        },
      }
    case 'cancelled':
      return { state: 'cancelled', summary: SUMMARY }
    case 'failed':
      return {
        state: 'failed',
        summary: SUMMARY,
        error_category: 'internal_failure',
      }
    case 'stale':
      return { state: 'stale', summary: SUMMARY }
  }
}
