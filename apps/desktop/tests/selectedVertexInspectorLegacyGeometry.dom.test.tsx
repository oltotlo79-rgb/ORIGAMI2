import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SelectedVertexInspector } from '../src/components/SelectedVertexInspector.tsx'
import { APP_TEXT } from '../src/lib/appText.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
  type VertexCoordinateExpressionBinding,
} from '../src/lib/coreClient.ts'
import { selectLocalizedText, type Locale } from '../src/lib/i18n.ts'
import {
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
} from '../src/lib/lengthUnit.ts'

afterEach(cleanup)

const VERTEX_ID = '1aaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'
const EDGE_ID = '2bbbbbbb-bbbb-4bbb-9bbb-bbbbbbbbbbb2'

const LEGACY_BINDING = Object.freeze({
  schema_version: 1,
  vertex: VERTEX_ID,
  x_source: `e.${EDGE_ID}.length`,
  y_source: '2',
  adopted_x_mm: 1,
  adopted_y_mm: 2,
} satisfies VertexCoordinateExpressionBinding)

const LEGACY_POLAR_BINDING = Object.freeze({
  schema_version: 1,
  vertex: VERTEX_ID,
  x_source: '4.224051852714362',
  y_source: '0.5328697807033036',
  adopted_x_mm: 4.224051852714362,
  adopted_y_mm: 0.5328697807033036,
  polar_construction: {
    schema_version: 1,
    start_vertex: EDGE_ID,
    adopted_start_x_mm: 1.25,
    adopted_start_y_mm: -2.5,
    length_source: '3.75',
    angle_degrees_source: '37.5',
    adopted_length_mm: 3.75,
    adopted_angle_degrees: 37.5,
  },
} satisfies VertexCoordinateExpressionBinding)

describe('SelectedVertexInspector legacy edge geometry disclosure', () => {
  it('localizes the warning for a valid unverified legacy V1 binding', () => {
    const view = render(inspector(LEGACY_BINDING, 'en'))
    const note = screen.getByRole('note')
    expect(note.hasAttribute(
      'data-unverified-legacy-edge-geometry-binding',
    )).toBe(true)
    expect(note.textContent).toBe(
      'This legacy V1 edge-geometry reference is unverified. '
      + 'The saved coordinates remain canonical for display and have not '
      + 'been reevaluated from the current edge geometry. '
      + 'Updating the coordinates or upgrading to V2 requires explicit '
      + 'coordinate reevaluation by the user. '
      + 'This notice grants no mutation authority.',
    )

    view.rerender(inspector(LEGACY_BINDING, 'ja'))
    expect(note.textContent).toBe(
      '旧V1の辺形状参照式は未検証です。'
      + '保存済み座標を正本として表示していますが、'
      + '現在の辺形状から再評価していません。'
      + '座標の更新またはV2への昇格には、'
      + 'ユーザーによる明示的な座標再評価が必要です。'
      + 'この表示は変更権限を与えません。',
    )
  })

  it('omits the warning for V2, plain V1, and malformed mixed references', () => {
    const deterministic = {
      ...LEGACY_BINDING,
      schema_version: 2,
      transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    } satisfies VertexCoordinateExpressionBinding
    const view = render(inspector(deterministic, 'en'))
    expect(screen.queryByRole('note')).toBeNull()

    view.rerender(inspector({
      ...LEGACY_BINDING,
      x_source: '1',
    }, 'en'))
    expect(screen.queryByRole('note')).toBeNull()

    view.rerender(inspector({
      ...LEGACY_BINDING,
      x_source: `${LEGACY_BINDING.x_source} + e.bad`,
    }, 'en'))
    expect(screen.queryByRole('note')).toBeNull()
  })
})

describe('SelectedVertexInspector legacy polar construction disclosure', () => {
  it('uses a dedicated localized saved-endpoint warning for legacy V1 polar data', () => {
    const view = render(inspector(LEGACY_POLAR_BINDING, 'en'))
    const note = screen.getByRole('note')
    expect(note.hasAttribute(
      'data-unverified-legacy-polar-construction-binding',
    )).toBe(true)
    expect(note.hasAttribute(
      'data-unverified-legacy-edge-geometry-binding',
    )).toBe(false)
    expect(note.textContent).toBe(selectLocalizedText(
      'en',
      APP_TEXT.legacyV1PolarConstructionIsUnverified,
    ))

    view.rerender(inspector(LEGACY_POLAR_BINDING, 'ja'))
    expect(note.textContent).toBe(selectLocalizedText(
      'ja',
      APP_TEXT.legacyV1PolarConstructionIsUnverified,
    ))
  })

  it('omits the legacy polar warning after deterministic V2 adoption', () => {
    render(inspector({
      ...LEGACY_POLAR_BINDING,
      schema_version: 2,
      transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    }, 'en'))
    expect(screen.queryByRole('note')).toBeNull()
  })
})

function inspector(
  expression: VertexCoordinateExpressionBinding,
  locale: Locale,
) {
  return (
    <SelectedVertexInspector
      locale={locale}
      vertex={{
        id: VERTEX_ID,
        position: { x: 1, y: 2 },
      }}
      expression={expression}
      displayUnit={MILLIMETRE_LENGTH_DISPLAY_UNIT}
      displayUnitLabel="mm"
      coreBusy={false}
      locked={false}
      boundary={false}
      boundaryVertexCount={0}
      cuttingAllowed={false}
      compassCircleCount={0}
      onSubmit={vi.fn()}
      onDeleteSelection={vi.fn()}
      onAddCompassCircle={vi.fn()}
      onClearCompassCircles={vi.fn()}
    />
  )
}
