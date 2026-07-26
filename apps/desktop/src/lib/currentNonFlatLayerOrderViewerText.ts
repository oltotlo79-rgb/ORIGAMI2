import type { LocalizedText } from './i18n.ts'

export type CurrentNonFlatLayerOrderViewerText = Readonly<Record<
  | 'title'
  | 'readOnlyBadge'
  | 'noMutationAuthority'
  | 'loading'
  | 'absent'
  | 'failedStaleAuthority'
  | 'failedInvalidEvidence'
  | 'failedResourceLimit'
  | 'failedInternalFailure'
  | 'reload'
  | 'worldPaneHeading'
  | 'worldPaneAria'
  | 'projectionPaneHeading'
  | 'projectionPaneAria'
  | 'axisX'
  | 'axisY'
  | 'axisZ'
  | 'axisU'
  | 'axisV'
  | 'droppedAxisX'
  | 'droppedAxisY'
  | 'droppedAxisZ'
  | 'lowerFace'
  | 'upperFace'
  | 'selectedFace'
  | 'selectedCell'
  | 'selectFace'
  | 'selectCell'
  | 'exactBoundaryDigest'
  | 'faceCount'
  | 'cellCount'
  | 'zeroCellWarning'
  | 'distinctCoordinateSystems',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const CURRENT_NON_FLAT_LAYER_ORDER_VIEWER_TEXT =
  Object.freeze({
    title: text(
      '適用済み非平坦層順（読み取り専用）',
      'Applied non-flat layer order (read-only)',
    ),
    readOnlyBadge: text('読み取り専用', 'Read-only'),
    noMutationAuthority: text(
      'この表示は現在の姿勢に結合した観測結果です。プロジェクトを変更する権限はありません。',
      'This view observes the current pose only. It carries no authority to modify the project.',
    ),
    loading: text('層順を取得中…', 'Loading layer order…'),
    absent: text(
      '現在の姿勢に結合した非平坦層順はありません。',
      'No non-flat layer order is bound to the current pose.',
    ),
    failedStaleAuthority: text(
      '姿勢またはプロジェクトが変化したため表示できません。',
      'The pose or project changed, so the view is unavailable.',
    ),
    failedInvalidEvidence: text(
      '層順の証拠が要件を満たさないため表示できません。',
      'The layer-order evidence did not satisfy the contract.',
    ),
    failedResourceLimit: text(
      '層順が表示上限を超えるため表示できません。',
      'The layer order exceeds the viewer limits.',
    ),
    failedInternalFailure: text(
      '層順を読み取れませんでした。',
      'The layer order could not be read.',
    ),
    reload: text('再読込', 'Reload'),
    worldPaneHeading: text('面の外周 World XYZ (mm)', 'Face outlines, world XYZ (mm)'),
    worldPaneAria: text(
      '面の外周 World XYZ (mm)',
      'Face outlines in world XYZ millimetres',
    ),
    projectionPaneHeading: text(
      '選択セルの投影 Projection UV (mm)',
      'Selected cell projection UV (mm)',
    ),
    projectionPaneAria: text(
      '選択セルの投影 Projection UV (mm)',
      'Selected cell projection in UV millimetres',
    ),
    axisX: text('X 軸', 'X axis'),
    axisY: text('Y 軸', 'Y axis'),
    axisZ: text('Z 軸', 'Z axis'),
    axisU: text('U 軸', 'U axis'),
    axisV: text('V 軸', 'V axis'),
    droppedAxisX: text(
      'World X を落とした平面です。U は World Y、V は World Z に対応します。',
      'World X is dropped; U is world Y and V is world Z.',
    ),
    droppedAxisY: text(
      'World Y を落とした平面です。U は World X、V は World Z に対応します。',
      'World Y is dropped; U is world X and V is world Z.',
    ),
    droppedAxisZ: text(
      'World Z を落とした平面です。U は World X、V は World Y に対応します。',
      'World Z is dropped; U is world X and V is world Y.',
    ),
    lowerFace: text('下の面', 'Lower face'),
    upperFace: text('上の面', 'Upper face'),
    selectedFace: text('選択中の面', 'Selected face'),
    selectedCell: text('選択中のセル', 'Selected cell'),
    selectFace: text('面 {label} を選択', 'Select face {label}'),
    selectCell: text('セル {label} を選択', 'Select cell {label}'),
    exactBoundaryDigest: text('厳密境界ダイジェスト', 'Exact boundary digest'),
    faceCount: text('面 {count} 件', '{count} faces'),
    cellCount: text('重なりセル {count} 件', '{count} overlap cells'),
    zeroCellWarning: text(
      '表示できる重なりセルはありません。これは衝突がないことの証明ではありません。',
      'There are no overlap cells to show. This is not a proof that nothing collides.',
    ),
    distinctCoordinateSystems: text(
      '面の外周は World XYZ、セルは各面の投影平面 UV です。同じ座標系ではありません。',
      'Face outlines are world XYZ; cells are UV in each face projection plane. They are not the same coordinate system.',
    ),
  }) satisfies CurrentNonFlatLayerOrderViewerText
