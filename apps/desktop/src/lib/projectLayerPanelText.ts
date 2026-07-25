import type { LocalizedText } from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const PROJECT_LAYER_PANEL_TEXT = Object.freeze({
  heading: localized('レイヤー', 'Layers'),
  layerCount: localized('{count}層', '{count} layers'),
  orderNote: localized(
    '一覧の上から下へ描画します。上下ボタンで描画順を変更できます。',
    'Layers are drawn from top to bottom in this list. Use the move buttons to change drawing order.',
  ),
  unsupportedObjects: localized(
    '注釈・下絵レイヤーは空のレイヤーとして作成・改名・並べ替え・削除できます。注釈・下絵オブジェクト自体の編集は初版の今後の対応です。',
    'Annotation and underlay layers can be created empty, renamed, reordered, and deleted. Editing annotation and underlay objects is not yet supported in the first release.',
  ),
  invalidDocument: localized(
    'レイヤー情報を安全に確認できないため、レイヤー操作を無効にしました。',
    'Layer controls are disabled because the layer data could not be validated safely.',
  ),
  busy: localized(
    'レイヤー操作を適用しています…',
    'Applying the layer change…',
  ),
  failed: localized(
    'レイヤー操作を適用できませんでした。プロジェクトが更新された可能性があります。最新の状態を確認して再試行してください。',
    'The layer change was not applied. The project may have changed. Check the latest state and try again.',
  ),
  createLegend: localized('レイヤーを追加', 'Add a layer'),
  nameLabel: localized('名前', 'Name'),
  kindLabel: localized('内容の種類', 'Content type'),
  kindCreasePattern: localized('折り線', 'Crease pattern'),
  kindAnnotation: localized('注釈', 'Annotation'),
  kindUnderlay: localized('下絵', 'Underlay'),
  createAction: localized('追加', 'Add'),
  layerList: localized('プロジェクトのレイヤー一覧', 'Project layer list'),
  defaultLayerName: localized('折り線パターン', 'Crease Pattern'),
  defaultBadge: localized('既定', 'Default'),
  hiddenBadge: localized('非表示', 'Hidden'),
  lockedBadge: localized('ロック中', 'Locked'),
  assignmentCount: localized(
    '明示割当 {count}本',
    '{count} explicitly assigned lines',
  ),
  renameLabel: localized(
    '{name}の新しいレイヤー名',
    'New layer name for {name}',
  ),
  renameAction: localized('名前を保存', 'Save name'),
  presentationLabel: localized(
    '{name}の表示と編集設定',
    'Display and editing settings for {name}',
  ),
  visibleLabel: localized('表示', 'Visible'),
  lockedLabel: localized('編集をロック', 'Lock editing'),
  opacityLabel: localized('不透明度', 'Opacity'),
  opacityInputLabel: localized(
    '{name}の不透明度（パーセント）',
    'Opacity for {name} (percent)',
  ),
  presentationAction: localized('表示設定を適用', 'Apply display settings'),
  moveUp: localized('↑ 上へ', '↑ Up'),
  moveDown: localized('↓ 下へ', '↓ Down'),
  moveUpLabel: localized(
    '{name}を描画順で1つ上へ移動',
    'Move {name} one position up in drawing order',
  ),
  moveDownLabel: localized(
    '{name}を描画順で1つ下へ移動',
    'Move {name} one position down in drawing order',
  ),
  assignAction: localized('選択線を割当', 'Assign selected line'),
  assignedAction: localized('選択線の割当先', 'Selected line layer'),
  assignLabel: localized(
    '選択中の線を{name}へ割り当て',
    'Assign the selected line to {name}',
  ),
  assignedLabel: localized(
    '選択中の線は{name}に割り当て済み',
    'The selected line is assigned to {name}',
  ),
  assignmentUnavailable: localized(
    '折り線は割当不可',
    'Line assignment unavailable',
  ),
  deleteLabel: localized('{name}を削除', 'Delete {name}'),
  defaultDeleteLabel: localized(
    '既定レイヤー{name}は削除できません',
    'Default layer {name} cannot be deleted',
  ),
  defaultDeleteTitle: localized(
    '既定レイヤーは削除できません',
    'The default layer cannot be deleted',
  ),
  deleteAction: localized('削除', 'Delete'),
  deleteConfirmation: localized(
    'レイヤー「{name}」を削除しますか？このレイヤーへ明示割当された折り線{count}本は既定レイヤーへ戻ります。この操作は元に戻せます。',
    'Delete layer “{name}”? Its {count} explicitly assigned lines will return to the default layer. This action can be undone.',
  ),
  selectEdge: localized(
    '折り線レイヤーへ割り当てるには、2D展開図で線を選択してください。',
    'Select a line in the 2D crease pattern before assigning it to a crease-pattern layer.',
  ),
})
