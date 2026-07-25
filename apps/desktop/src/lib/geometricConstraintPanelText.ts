import type {
  GeometricConstraintKind,
  GeometricConstraintPreflightResult,
  GeometricConstraintSolvePreview,
} from './coreClient.ts'
import type { LocalizedText } from './i18n.ts'

type DirectConflictKind = Extract<
  GeometricConstraintPreflightResult,
  { status: 'direct_conflict' }
>['conflicts'][number]['conflict']['kind']

type UnknownReason = Extract<
  GeometricConstraintPreflightResult,
  { status: 'unknown' }
>['reason']

export type GeometricConstraintCreationFieldLabel =
  | 'targetLine'
  | 'angleVertex'
  | 'firstLine'
  | 'secondLine'
  | 'targetPoint'
  | 'referenceLine'
  | 'firstPoint'
  | 'secondPoint'
  | 'symmetryAxis'
  | 'rotationCenter'
  | 'sourcePoint'
  | 'correspondingPoint'
  | 'bisectorLine'
  | 'numeratorLine'
  | 'denominatorLine'

type ScalarConstraintKind =
  | 'fixed_length'
  | 'fixed_angle'
  | 'rotational_symmetry'
  | 'length_ratio'

type GeometricConstraintPanelSimpleTextKey =
  | 'title'
  | 'constraintCount'
  | 'addHorizontal'
  | 'addVertical'
  | 'moveLegend'
  | 'xAxis'
  | 'solveXAria'
  | 'yAxis'
  | 'solveYAria'
  | 'preview'
  | 'changedVertices'
  | 'iterations'
  | 'residual'
  | 'rank'
  | 'degreesOfFreedom'
  | 'condition'
  | 'detailSeparator'
  | 'movePreview'
  | 'apply'
  | 'cancel'
  | 'solveError'
  | 'edgeTransformLegend'
  | 'edgeDeltaX'
  | 'edgeDeltaY'
  | 'edgeRotation'
  | 'edgeLengthScale'
  | 'previewEdgeTransform'
  | 'reevaluateSavedExpressions'
  | 'references'
  | 'selectEdgeHint'
  | 'creationLegend'
  | 'constraintKind'
  | 'createKind'
  | 'selectPrompt'
  | 'addFormConstraint'
  | 'creationInvalid'
  | 'creationHint'
  | 'allKindsLegend'
  | 'constraintJson'
  | 'constraintJsonPlaceholder'
  | 'addConstraint'
  | 'jsonInvalid'
  | 'jsonHint'
  | 'noConstraints'
  | 'unknownConstraint'
  | 'targetUnavailable'
  | 'selectTarget'
  | 'deleteConstraint'
  | 'delete'
  | 'constraintListTruncated'
  | 'analyzing'
  | 'analysisFailed'
  | 'directConflictCount'
  | 'unknownStatus'
  | 'noDirectConflict'
  | 'unanalyzed'
  | 'directConflictCauses'
  | 'causingConstraints'
  | 'additionalDirectConflicts'
  | 'uncheckedConstraints'
  | 'analyzeAgain'
  | 'boundedMusProven'
  | 'boundedMusConstraintLimit'
  | 'boundedMusIncomplete'
  | 'invalidIdentifier'
  | 'idListSeparator'
  | 'remainingIds'

export type GeometricConstraintPanelText = Readonly<
  Record<GeometricConstraintPanelSimpleTextKey, LocalizedText>
  & {
    creationFieldLabels: Readonly<
      Record<GeometricConstraintCreationFieldLabel, LocalizedText>
    >
    constraintKindNames: Readonly<
      Record<GeometricConstraintKind['kind'], LocalizedText>
    >
    scalarLabels: Readonly<Record<ScalarConstraintKind, LocalizedText>>
    systemClassifications: Readonly<
      Record<
        GeometricConstraintSolvePreview['systemClassification'],
        LocalizedText
      >
    >
    directConflictLabels: Readonly<Record<DirectConflictKind, LocalizedText>>
    unknownReasonLabels: Readonly<Record<UnknownReason, LocalizedText>>
  }
>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const GEOMETRIC_CONSTRAINT_PANEL_TEXT = Object.freeze({
  title: text('幾何制約', 'Geometric constraints'),
  constraintCount: text('{count}件', '{count} constraints'),
  addHorizontal: text(
    '選択線を水平に制約',
    'Constrain selected line horizontally',
  ),
  addVertical: text(
    '選択線を垂直に制約',
    'Constrain selected line vertically',
  ),
  moveLegend: text('拘束を保った移動', 'Constraint-preserving move'),
  xAxis: text('X (mm)', 'X (mm)'),
  solveXAria: text(
    '制約ソルバー X座標',
    'Constraint solver X coordinate',
  ),
  yAxis: text('Y (mm)', 'Y (mm)'),
  solveYAria: text(
    '制約ソルバー Y座標',
    'Constraint solver Y coordinate',
  ),
  preview: text('プレビュー', 'Preview'),
  changedVertices: text('変更頂点', 'Changed vertices'),
  iterations: text('反復', 'Iterations'),
  residual: text('residual', 'residual'),
  rank: text('rank', 'rank'),
  degreesOfFreedom: text('DOF', 'DOF'),
  condition: text('condition', 'condition'),
  detailSeparator: text(' · ', ' · '),
  movePreview: text('移動プレビュー', 'Move preview'),
  apply: text('適用', 'Apply'),
  cancel: text('キャンセル', 'Cancel'),
  solveError: text(
    '拘束を満たす解を安全に作成できませんでした。',
    'A safe constraint solution could not be created.',
  ),
  edgeTransformLegend: text(
    '拘束を保った辺操作',
    'Constraint-preserving edge transform',
  ),
  edgeDeltaX: text('Edge delta X', 'Edge delta X'),
  edgeDeltaY: text('Edge delta Y', 'Edge delta Y'),
  edgeRotation: text(
    'Edge rotation (degrees)',
    'Edge rotation (degrees)',
  ),
  edgeLengthScale: text('Edge length scale', 'Edge length scale'),
  previewEdgeTransform: text('辺をプレビュー', 'Preview edge transform'),
  reevaluateSavedExpressions: text(
    '保存式を再評価してプレビュー',
    'Re-evaluate saved expressions',
  ),
  references: text(
    '参照: v.<正規UUID>.x/y、e.<正規UUID>.length/angle',
    'References: v.<canonical-uuid>.x/y, e.<canonical-uuid>.length/angle',
  ),
  selectEdgeHint: text(
    '水平・垂直制約を追加するには線を選択してください。',
    'Select a line before adding a horizontal or vertical constraint.',
  ),
  creationLegend: text(
    '制約をフォームから追加',
    'Add constraint from form',
  ),
  constraintKind: text('制約種別', 'Constraint kind'),
  createKind: text('{name}を作成', 'Create {name}'),
  selectPrompt: text('選択してください', 'Select…'),
  addFormConstraint: text(
    'フォームの制約を追加',
    'Add form constraint',
  ),
  creationInvalid: text(
    '必要な対象と有効な数値を指定してください。',
    'Choose every required target and enter a valid value.',
  ),
  creationHint: text(
    '対象は現在のproject要素から選択します。追加は一回のUndoで戻せます。',
    'Targets come from the current project. One Undo removes the addition.',
  ),
  allKindsLegend: text(
    '全11種の制約を追加',
    'Add any of the 11 constraint kinds',
  ),
  constraintJson: text('制約JSON', 'Constraint JSON'),
  constraintJsonPlaceholder: text(
    '{"kind":"equal_length","first_edge":"UUID","second_edge":"UUID"}',
    '{"kind":"equal_length","first_edge":"UUID","second_edge":"UUID"}',
  ),
  addConstraint: text('制約を追加', 'Add constraint'),
  jsonInvalid: text(
    '制約JSONの種別、ID、値、またはfieldが不正です。',
    'The constraint kind, IDs, values, or fields are invalid.',
  ),
  jsonHint: text(
    'fixed_length / fixed_angle / horizontal / vertical / equal_length / parallel / point_on_line / mirror_symmetry / rotational_symmetry / angle_bisector / length_ratio を厳格JSONで指定します。',
    'Use strict JSON for fixed_length, fixed_angle, horizontal, vertical, equal_length, parallel, point_on_line, mirror_symmetry, rotational_symmetry, angle_bisector, or length_ratio.',
  ),
  noConstraints: text('制約はまだありません。', 'No constraints yet.'),
  unknownConstraint: text('不明な制約', 'Unknown constraint'),
  targetUnavailable: text('対象を確認できません', 'Target unavailable'),
  selectTarget: text('対象を選択', 'Select target'),
  deleteConstraint: text(
    '{name}制約を削除',
    'Delete {name} constraint',
  ),
  delete: text('削除', 'Delete'),
  constraintListTruncated: text(
    '先頭{visible}件を表示しています。残り{remaining}件は、表示中の制約を削除すると順に表示されます。',
    'Showing the first {visible} constraints. The remaining {remaining} appear as displayed constraints are deleted.',
  ),
  analyzing: text('制約を診断しています…', 'Analyzing constraints…'),
  analysisFailed: text(
    '制約診断を完了できませんでした。安全確認済みとして扱いません。',
    'Constraint analysis could not be completed. Do not treat the constraints as safety-verified.',
  ),
  directConflictCount: text(
    '直接矛盾があります（{count}件）。',
    '{count} direct conflicts found.',
  ),
  unknownStatus: text(
    '{reason}。安全確認済みとして扱いません。',
    '{reason} Do not treat the constraints as safety-verified.',
  ),
  noDirectConflict: text(
    '直接矛盾は見つかりません（全制約の充足可能性は未証明）',
    'No direct conflicts found (satisfiability of all constraints is not proven)',
  ),
  unanalyzed: text(
    '現在の制約は未診断です。',
    'The current constraints have not been analyzed.',
  ),
  directConflictCauses: text('直接矛盾の原因', 'Direct conflict causes'),
  causingConstraints: text(
    '原因となる制約: {ids}',
    'Causing constraints: {ids}',
  ),
  additionalDirectConflicts: text(
    'ほか{count}件の直接矛盾',
    '{count} more direct conflicts',
  ),
  uncheckedConstraints: text(
    '未確認の制約: {ids}',
    'Unchecked constraints: {ids}',
  ),
  analyzeAgain: text('再診断', 'Analyze again'),
  boundedMusProven: text(
    '有界な直接矛盾オラクルで証明した最小部分集合（{count}件、呼び出し{calls}回）: {ids}',
    'Smallest subset proven by the bounded direct-conflict oracle ({count} constraints, {calls} calls): {ids}',
  ),
  boundedMusConstraintLimit: text(
    '直接矛盾は証明済みです。制約が16件を超えるため、有界な直接矛盾の最小化は実行していません。',
    'A direct conflict is proven. Bounded direct-conflict minimization was skipped because more than 16 constraints are present.',
  ),
  boundedMusIncomplete: text(
    '直接矛盾は証明済みですが、有界な直接矛盾の最小化は完了していません。',
    'A direct conflict is proven, but bounded direct-conflict minimization did not complete.',
  ),
  invalidIdentifier: text('不正な識別子', 'invalid identifier'),
  idListSeparator: text('、', ', '),
  remainingIds: text(
    '{visible}、ほか{remaining}件',
    '{visible}, {remaining} more',
  ),
  creationFieldLabels: Object.freeze({
    targetLine: text('対象線', 'Target line'),
    angleVertex: text('角の頂点', 'Angle vertex'),
    firstLine: text('1本目の線', 'First line'),
    secondLine: text('2本目の線', 'Second line'),
    targetPoint: text('対象点', 'Target point'),
    referenceLine: text('基準線', 'Reference line'),
    firstPoint: text('1点目', 'First point'),
    secondPoint: text('2点目', 'Second point'),
    symmetryAxis: text('対称軸', 'Symmetry axis'),
    rotationCenter: text('回転中心', 'Rotation center'),
    sourcePoint: text('元の点', 'Source point'),
    correspondingPoint: text('対応点', 'Target point'),
    bisectorLine: text('二等分線', 'Bisector line'),
    numeratorLine: text('分子側の線', 'Numerator line'),
    denominatorLine: text('分母側の線', 'Denominator line'),
  }),
  constraintKindNames: Object.freeze({
    fixed_length: text('長さ固定', 'Fixed length'),
    fixed_angle: text('角度固定', 'Fixed angle'),
    horizontal: text('水平', 'Horizontal'),
    vertical: text('垂直', 'Vertical'),
    equal_length: text('等長', 'Equal length'),
    parallel: text('平行', 'Parallel'),
    point_on_line: text('点を線上に配置', 'Point on line'),
    mirror_symmetry: text('線対称', 'Mirror symmetry'),
    rotational_symmetry: text('回転対称', 'Rotational symmetry'),
    angle_bisector: text('角の二等分', 'Angle bisector'),
    length_ratio: text('長さの比', 'Length ratio'),
  }),
  scalarLabels: Object.freeze({
    fixed_length: text('長さ (mm)', 'Length (mm)'),
    fixed_angle: text('角度 (度)', 'Angle (degrees)'),
    rotational_symmetry: text('角度 (度)', 'Angle (degrees)'),
    length_ratio: text('長さの比', 'Length ratio'),
  }),
  systemClassifications: Object.freeze({
    under_constrained: text('拘束不足', 'Under-constrained'),
    well_constrained: text('完全拘束', 'Well-constrained'),
    over_constrained: text('過剰拘束', 'Over-constrained'),
  }),
  directConflictLabels: Object.freeze({
    different_fixed_lengths: text(
      '同じ辺 {edge} に異なる長さが指定されています',
      'Different lengths are assigned to the same edge {edge}',
    ),
    different_fixed_angles: text(
      '同じ角に異なる角度が指定されています（頂点 {vertex}）',
      'Different angles are assigned to the same angle (vertex {vertex})',
    ),
    different_length_ratios: text(
      '同じ辺の組に異なる長さ比が指定されています',
      'Different length ratios are assigned to the same pair of edges',
    ),
    horizontal_and_vertical: text(
      '辺 {edge} に水平と垂直が同時に指定されています',
      'Edge {edge} is constrained as both horizontal and vertical',
    ),
    equal_length_with_different_fixed_lengths: text(
      '等長にした辺へ異なる固定長が指定されています',
      'Edges constrained to equal length have different fixed lengths',
    ),
    equal_length_with_non_unit_ratio_and_fixed_length: text(
      '等長な辺に1ではない長さ比と正の固定長が同時に指定されています',
      'Equal-length edges have a non-unit ratio and a positive fixed length',
    ),
    non_reciprocal_length_ratios_with_fixed_length: text(
      '正の固定長を持つ辺の双方向の長さ比が互いに逆数ではありません',
      'Opposite length ratios are not reciprocal for edges with a positive fixed length',
    ),
    length_ratio_with_incompatible_fixed_lengths: text(
      '2辺の固定長が、指定された長さ比と厳密に一致しません',
      'The two fixed lengths do not exactly satisfy the specified length ratio',
    ),
    non_unit_length_ratio_cycle_with_fixed_length: text(
      '正の固定長を含む3辺の長さ比の循環積が1ではありません',
      'The cyclic product of three length ratios is not one for edges with a positive fixed length',
    ),
    inconsistent_length_ratio_graph_with_fixed_length: text(
      '正の固定長につながる長さ比グラフに、厳密に両立しない循環があります',
      'A length-ratio graph connected to a positive fixed length contains an exactly inconsistent cycle',
    ),
    different_fixed_lengths_in_equal_length_component: text(
      '等長制約でつながった辺に、厳密に異なる固定長が指定されています',
      'Edges connected by equal-length constraints have exactly different fixed lengths',
    ),
    perpendicular_orientations_in_parallel_component: text(
      '平行制約でつながった辺に、水平と垂直の向きが同時に指定されています',
      'Edges connected by parallel constraints are constrained to horizontal and vertical orientations',
    ),
    non_parallel_fixed_angle_in_parallel_component: text(
      '平行制約でつながる辺に、平行でない固定角が指定されています',
      'Edges connected by parallel constraints have a fixed angle that is neither 0 nor 180 degrees',
    ),
    parallel_with_fixed_non_parallel_angle: text(
      '平行にした辺へ平行でない固定角が指定されています',
      'Parallel edges have a fixed angle that is not parallel',
    ),
    parallel_with_perpendicular_orientations: text(
      '平行にした辺へ水平と垂直が別々に指定されています',
      'Parallel edges are separately constrained as horizontal and vertical',
    ),
    same_orientation_with_fixed_non_parallel_angle: text(
      '同じ向きに拘束した2辺へ、平行ではない固定角が指定されています',
      'Edges with the same fixed orientation have a non-parallel fixed angle',
    ),
    perpendicular_orientations_with_fixed_non_right_angle: text(
      '水平・垂直に拘束した2辺へ、直角ではない固定角が指定されています',
      'Horizontally and vertically oriented edges have a non-right fixed angle',
    ),
    different_rotational_symmetry_angles_with_fixed_radius: text(
      '同じ回転対称対象へ異なる角度が指定され、正の固定半径と両立しません',
      'Different angles target the same rotational-symmetry relation and conflict with a positive fixed radius',
    ),
  }),
  unknownReasonLabels: Object.freeze({
    work_limit_exceeded: text(
      '診断の処理上限に達したため判定保留です',
      'Indeterminate because the analysis work limit was reached.',
    ),
    solver_required_constraint_kinds: text(
      '完全な制約ソルバーが必要なため判定保留です',
      'Indeterminate because a complete constraint solver is required.',
    ),
    invalid_document_or_geometry: text(
      '制約または展開図を検証できないため判定保留です',
      'Indeterminate because the constraints or crease pattern could not be validated.',
    ),
  }),
}) satisfies GeometricConstraintPanelText
