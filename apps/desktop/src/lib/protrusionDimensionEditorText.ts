import type { LocalizedText } from './i18n.ts'

export type ProtrusionDimensionEditorText = Readonly<Record<
  | 'bindingSummary'
  | 'symmetryNone'
  | 'symmetryBilateral'
  | 'symmetryRadial'
  | 'partKind'
  | 'unassigned'
  | 'symmetry'
  | 'count'
  | 'rootWidth'
  | 'tipWidth'
  | 'length'
  | 'bilateralSpacing'
  | 'thickness'
  | 'mountVertical'
  | 'mountForeAft'
  | 'directionHorizontal'
  | 'directionVertical'
  | 'curvature'
  | 'motionMinimum'
  | 'motionMaximum'
  | 'joint'
  | 'side'
  | 'priority'
  | 'rootWidthLabel'
  | 'tipWidthLabel'
  | 'lengthLabel'
  | 'bilateralSpacingLabel'
  | 'thicknessLabel'
  | 'mountVerticalLabel'
  | 'mountForeAftLabel'
  | 'curvatureLabel'
  | 'motionMinimumLabel'
  | 'motionMaximumLabel'
  | 'ariaBinding'
  | 'ariaBindingMillimetres'
  | 'ariaBindingDegrees'
  | 'jointFixed'
  | 'jointHinge'
  | 'jointBall'
  | 'sideFront'
  | 'sideBack'
  | 'sideEither'
  | 'remove'
  | 'moveUp'
  | 'moveDown',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const PROTRUSION_DIMENSION_EDITOR_TEXT =
  Object.freeze({
    bindingSummary: text(
      'binding {id}・{symmetry}・数 {count}',
      'Binding {id} · {symmetry} · count {count}',
    ),
    symmetryNone: text('非対称単独', 'Asymmetric single'),
    symmetryBilateral: text('左右対称', 'Bilateral'),
    symmetryRadial: text('放射対称', 'Radial'),
    partKind: text('種類', 'Part kind'),
    unassigned: text('未割り当て', 'Unassigned'),
    symmetry: text('対称性', 'Symmetry'),
    count: text('個数', 'Count'),
    rootWidth: text('根元幅', 'Root width'),
    tipWidth: text('先端幅', 'Tip width'),
    length: text('長さ', 'Length'),
    bilateralSpacing: text('左右間隔', 'Bilateral spacing'),
    thickness: text('厚さ', 'Thickness'),
    mountVertical: text('取付位置 上下', 'Mount vertical'),
    mountForeAft: text('取付位置 前後', 'Mount fore-aft'),
    directionHorizontal: text('向き 左右', 'Direction horizontal'),
    directionVertical: text('向き 上下', 'Direction vertical'),
    curvature: text('曲率', 'Curvature'),
    motionMinimum: text('可動範囲の最小', 'Motion minimum'),
    motionMaximum: text('可動範囲の最大', 'Motion maximum'),
    joint: text('関節', 'Joint'),
    side: text('面', 'Side'),
    priority: text('優先度', 'Priority'),
    rootWidthLabel: text('根元幅 (mm、任意)', 'Root width (mm, optional)'),
    tipWidthLabel: text('先端幅 (mm、任意)', 'Tip width (mm, optional)'),
    lengthLabel: text('長さ (mm)', 'Length (mm)'),
    bilateralSpacingLabel: text('左右間隔 (mm)', 'Bilateral spacing (mm)'),
    thicknessLabel: text('厚さ (mm)', 'Thickness (mm)'),
    mountVerticalLabel: text('取付位置 上下 (mm)', 'Mount vertical (mm)'),
    mountForeAftLabel: text('取付位置 前後 (mm)', 'Mount fore-aft (mm)'),
    curvatureLabel: text('曲率 (度)', 'Curvature (degrees)'),
    motionMinimumLabel: text(
      '可動範囲の最小 (度)',
      'Motion minimum (degrees)',
    ),
    motionMaximumLabel: text(
      '可動範囲の最大 (度)',
      'Motion maximum (degrees)',
    ),
    ariaBinding: text('{name} binding {id}', '{name} binding {id}'),
    ariaBindingMillimetres: text(
      '{name} binding {id} (mm)',
      '{name} binding {id} (mm)',
    ),
    ariaBindingDegrees: text(
      '{name} binding {id} (度)',
      '{name} binding {id} (degrees)',
    ),
    jointFixed: text('固定', 'Fixed'),
    jointHinge: text('ヒンジ', 'Hinge'),
    jointBall: text('球状', 'Ball'),
    sideFront: text('表', 'Front'),
    sideBack: text('裏', 'Back'),
    sideEither: text('どちらでも可', 'Either'),
    remove: text('削除', 'Remove'),
    moveUp: text('上へ', 'Move up'),
    moveDown: text('下へ', 'Move down'),
  }) satisfies ProtrusionDimensionEditorText
