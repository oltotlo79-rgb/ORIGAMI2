import type { LocalizedText } from './i18n.ts'

type FoldPreviewComponentTextKey =
  | 'preparingPose'
  | 'fixedFace'
  | 'fixedFaceNote'
  | 'vertexDistance'
  | 'faceNormalAngle'
  | 'measurementUnavailable'
  | 'selectTwoSameKind'
  | 'faceLabel'
  | 'fixedFaceSuffix'
  | 'paperDrag'
  | 'verticalDrag'
  | 'motionTargetBadge'
  | 'motionReadyBadge'
  | 'cutComponentsPlanarOnly'
  | 'cycleConstraintsPlanarOnly'
  | 'treePoseNote'
  | 'staticGraphPoseNote'
  | 'noteSeparator'
  | 'keyboardHingeAndFaceHint'
  | 'keyboardHingeHint'
  | 'keyboardFaceHint'
  | 'singleFoldOperationNoteWithDrag'
  | 'singleFoldOperationNote'
  | 'treeFoldOperationNote'
  | 'sentenceDetail'
  | 'motionViewDescription'
  | 'unverifiedTargetDescription'
  | 'singleFoldPreviewDescription'
  | 'treeFoldPreviewDescription'
  | 'staticGraphPreviewDescription'
  | 'planarPreviewDescription'
  | 'unavailablePreviewDescription'
  | 'selectionHingeAndFaceDescription'
  | 'selectionHingeDescription'
  | 'selectionFaceDescription'
  | 'keyboardSelectionDescription'
  | 'keyboardHingeSelectionDescription'
  | 'keyboardHingeSelected'
  | 'keyboardNoHingeSelected'
  | 'keyboardSelectionBetween'
  | 'keyboardFaceSelectionDescription'
  | 'keyboardFixedFaceSelected'
  | 'keyboardNoFixedFaceSelected'
  | 'singleFoldAngleDragDescription'
  | 'treeAngleDragDescription'
  | 'cameraDescription'
  | 'cameraMouseFoldExclusion'
  | 'cameraTouchFoldExclusion'
  | 'previewGroup'
  | 'measurementGroup'
  | 'measurementMode'
  | 'resetMeasurement'
  | 'view'
  | 'motionPathBadge'
  | 'correctionAnalysisBadge'
  | 'resetCamera'
  | 'resetView'

export type FoldPreviewComponentText = Readonly<
  Record<FoldPreviewComponentTextKey, LocalizedText>
>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const FOLD_PREVIEW_COMPONENT_TEXT = Object.freeze({
  preparingPose: text('姿勢を準備中', 'Preparing pose'),
  fixedFace: text('固定面 {index}', 'Fixed face {index}'),
  fixedFaceNote: text('・{label}', ' · {label}'),
  vertexDistance: text('2頂点間の距離', 'Vertex distance'),
  faceNormalAngle: text('2面の法線角', 'Face-normal angle'),
  measurementUnavailable: text('計測不能', 'Unavailable'),
  selectTwoSameKind: text(
    '同じ種類を2つ選択（{count}/2）',
    'Select two items of the same kind ({count}/2)',
  ),
  faceLabel: text('面 {index}{fixedSuffix}', 'Face {index}{fixedSuffix}'),
  fixedFaceSuffix: text('（固定）', ' (fixed)'),
  paperDrag: text('紙面ドラッグ', 'Paper drag'),
  verticalDrag: text('上下ドラッグ', 'Vertical drag'),
  motionTargetBadge: text(
    '{action}目標 {target}°・表示 {displayed}° / 離すと検証',
    '{action} target {target}° · displayed {displayed}° / release to verify',
  ),
  motionReadyBadge: text(
    '{action}待機・表示 {displayed}°',
    '{action} ready · displayed {displayed}°',
  ),
  cutComponentsPlanarOnly: text(
    '切断により紙が複数の部品へ分離しているため平面確認のみ',
    'planar inspection only because cuts separated the paper into multiple components',
  ),
  cycleConstraintsPlanarOnly: text(
    '閉路拘束のためここでは平面確認のみ。下の積層折りパネルで閉路姿勢をプレビュー・適用できます',
    'planar inspection only here because of cycle constraints; preview and apply the cycle pose in the stacked-fold panel below',
  ),
  treePoseNote: text(
    '{faces}面・{hinges}ヒンジを{treeAngleNote}{fixedFaceNote}',
    '{faces} faces · {hinges} hinges · {treeAngleNote}{fixedFaceNote}',
  ),
  staticGraphPoseNote: text(
    '{faces}面・{hinges}ヒンジ・{reason}',
    '{faces} faces · {hinges} hinges · {reason}',
  ),
  noteSeparator: text('・', ' · '),
  keyboardHingeAndFaceHint: text(
    '・H/Shift+Hでヒンジ、F/Shift+Fで固定面',
    ' · H/Shift+H: hinge; F/Shift+F: fixed face',
  ),
  keyboardHingeHint: text(
    '・H/Shift+Hでヒンジ',
    ' · H/Shift+H: hinge',
  ),
  keyboardFaceHint: text(
    '・F/Shift+Fで固定面',
    ' · F/Shift+F: fixed face',
  ),
  singleFoldOperationNoteWithDrag: text(
    '移動面ドラッグで物理目標・折り目の上下ドラッグで角度指定・{basePreviewNote}・ドラッグ中の姿勢は未変更・中央面・単一線形経路のみ',
    'Drag the moving face for a physical target · drag the crease vertically to set the angle · {basePreviewNote} · the pose is unchanged while dragging · middle-surface, single linear path only',
  ),
  singleFoldOperationNote: text(
    '{basePreviewNote}・ドラッグ中の姿勢は未変更・中央面・単一線形経路のみ',
    '{basePreviewNote} · the pose is unchanged while dragging · middle-surface, single linear path only',
  ),
  treeFoldOperationNote: text(
    '選択ヒンジの従属面ドラッグで物理目標・{basePreviewNote}・ドラッグ中の姿勢は未変更・選択ヒンジ単一経路のみ',
    'Drag a dependent face of the selected hinge for a physical target · {basePreviewNote} · the pose is unchanged while dragging · selected-hinge single path only',
  ),
  sentenceDetail: text('。{text}', '. {text}'),
  motionViewDescription: text('、{text}', '. {text}'),
  unverifiedTargetDescription: text(
    '、{action}中の未確認目標角 {target}度。この目標角はポインターを離して経路検証が完了するまで3Dへ適用しません',
    '. Unverified target during {action}: {target} degrees. This target is not applied to the 3D view until the pointer is released and path verification completes',
  ),
  singleFoldPreviewDescription: text(
    '実展開図の3D折りプレビュー、表示角 {displayed}度、指定角 {requested}度{unverifiedTarget}{fixedFaceNote}{motionView}{motionDetail}、{collision}、{thickness}',
    '3D fold preview of the actual crease pattern. Displayed angle: {displayed} degrees. Requested angle: {requested} degrees{unverifiedTarget}{fixedFaceNote}{motionView}{motionDetail}. {collision}. {thickness}.',
  ),
  treeFoldPreviewDescription: text(
    '実展開図の木構造複数面3D折りプレビュー、{faces}面・{hinges}ヒンジ、{treeAngleNote}{fixedFaceNote}{motionView}{motionDetail}{correctionAnalysis}、{collision}、{thickness}',
    'Multi-face tree-structure 3D fold preview of the actual crease pattern. {faces} faces and {hinges} hinges. {treeAngleNote}{fixedFaceNote}{motionView}{motionDetail}{correctionAnalysis}. {collision}. {thickness}.',
  ),
  staticGraphPreviewDescription: text(
    '実展開図の複数面3D平面確認、{faces}面・{hinges}ヒンジ、{reason}、{collision}、{thickness}',
    'Multi-face planar 3D inspection of the actual crease pattern. {faces} faces and {hinges} hinges. {reason}. {collision}. {thickness}.',
  ),
  planarPreviewDescription: text(
    '実展開図の平面3Dプレビュー、{collision}、{thickness}',
    'Planar 3D preview of the actual crease pattern. {collision}. {thickness}.',
  ),
  unavailablePreviewDescription: text(
    '3D折りプレビューは利用できません。{message}',
    'The 3D fold preview is unavailable. {message}',
  ),
  selectionHingeAndFaceDescription: text(
    '。3D上のヒンジをクリックして選択し、面をクリックして固定面を変更できます',
    ' Click a hinge in the 3D view to select it, or click a face to change the fixed face.',
  ),
  selectionHingeDescription: text(
    '。3D上のヒンジをクリックして選択できます',
    ' Click a hinge in the 3D view to select it.',
  ),
  selectionFaceDescription: text(
    '。3D上の面をクリックして固定面を変更できます',
    ' Click a face in the 3D view to change the fixed face.',
  ),
  keyboardSelectionDescription: text(
    '。3Dビューにフォーカス中、{hingeDescription}{between}{faceDescription}',
    ' With focus in the 3D view, {hingeDescription}{between}{faceDescription}.',
  ),
  keyboardHingeSelectionDescription: text(
    'Hで次、Shift+Hで前のヒンジを選択し、Escapeで解除できます。現在は{selection}',
    'press H for the next hinge, Shift+H for the previous hinge, or Escape to clear the selection. Current selection: {selection}',
  ),
  keyboardHingeSelected: text(
    'ヒンジ {index}/{total}',
    'hinge {index} of {total}',
  ),
  keyboardNoHingeSelected: text('ヒンジ未選択', 'no hinge selected'),
  keyboardSelectionBetween: text('。', '. '),
  keyboardFaceSelectionDescription: text(
    'Fで次、Shift+Fで前の面を固定面にできます。現在は{selection}',
    'press F for the next fixed face or Shift+F for the previous one. Current selection: {selection}',
  ),
  keyboardFixedFaceSelected: text(
    '固定面 {index}/{total}',
    'fixed face {index} of {total}',
  ),
  keyboardNoFixedFaceSelected: text('固定面未選択', 'no fixed face selected'),
  singleFoldAngleDragDescription: text(
    '。3D上で移動する紙面の表または裏をつかんでドラッグすると、紙の回転軌道から折り角目標を作れます。折り目の上下ドラッグでは、上方向で増加、下方向で減少する角度パラメータ操作ができます。どちらの目標もドラッグ中は未確認で、ポインターを離して連続経路を確認した後にだけ3D表示へ適用されます。Altキーを押したドラッグはカメラ操作になります。キーボードでは下の指定折り量入力を使用できます',
    ' Drag the front or back of the moving paper face to create a fold-angle target from the paper’s rotation path. Drag the crease upward to increase or downward to decrease the angle parameter. Targets remain unverified while dragging and are applied to the 3D view only after release and continuous-path verification. Hold Alt while dragging to control the camera. Keyboard users can use the requested-fold input below.',
  ),
  treeAngleDragDescription: text(
    '。3D上で選択ヒンジから先の紙面の表または裏をつかんでドラッグすると、そのヒンジだけの折り角目標を作れます。目標はドラッグ中は未確認で、ポインターを離して複数面の連続経路を確認した後にだけ3D表示と角度入力へ確定されます',
    ' Drag the front or back of a paper face beyond the selected hinge to create a target for that hinge only. The target remains unverified while dragging and is committed to the 3D view and angle input only after release and multi-face continuous-path verification.',
  ),
  cameraDescription: text(
    '。マウスは{mouseExclusion}左ドラッグで回転、ホイールまたは中ドラッグで拡大縮小、右ドラッグで平行移動できます。タッチは{touchExclusion}1本指で回転、2本指で拡大縮小と平行移動ができます。キーボードは矢印キーで平行移動、Shiftと矢印キーで回転、プラスとマイナスで拡大縮小、Homeまたは0で視点をリセットできます',
    ' Mouse controls: left-drag {mouseExclusion}to rotate, wheel or middle-drag to zoom, and right-drag to pan. Touch controls: one-finger drag {touchExclusion}to rotate, and two fingers to zoom and pan. Keyboard controls: arrow keys to pan, Shift+arrow keys to rotate, plus and minus to zoom, and Home or 0 to reset the view.',
  ),
  cameraMouseFoldExclusion: text(
    '紙面と折り目の折り操作以外の場所を',
    'outside paper and crease fold controls ',
  ),
  cameraTouchFoldExclusion: text(
    '紙面と折り目の折り操作以外を',
    'outside paper and crease fold controls ',
  ),
  previewGroup: text('3D折りプレビュー', '3D fold preview'),
  measurementGroup: text('3D計測', '3D measurement'),
  measurementMode: text('3D計測モード', '3D measurement mode'),
  resetMeasurement: text('計測をリセット', 'Reset measurement'),
  view: text('3Dビュー', '3D view'),
  motionPathBadge: text(
    '移動経路｜{text}',
    'Motion path | {text}',
  ),
  correctionAnalysisBadge: text(
    '補正解析｜{text}',
    'Correction analysis | {text}',
  ),
  resetCamera: text(
    'カメラを初期位置へ戻す',
    'Return the camera to its initial position',
  ),
  resetView: text('視点をリセット', 'Reset view'),
}) satisfies FoldPreviewComponentText
