import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from 'react'

import {
  DEFAULT_PROJECT_LAYER_ID,
  DEFAULT_PROJECT_LAYER_NAME,
  MAX_LAYER_NAME_CHARS,
  type LayerContentKindV1,
  type LayerRecordV1,
  type ProjectLayerDocumentV1,
} from '../lib/projectLayers.ts'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'

type ProjectLayerPanelProps = {
  document: ProjectLayerDocumentV1
  bindingKey: string
  selectedEdgeId: string | null
  disabled: boolean
  documentInvalid?: boolean
  onCreate(
    name: string,
    contentKind: LayerContentKindV1,
  ): Promise<boolean>
  onRename(layerId: string, name: string): Promise<boolean>
  onMove(layerId: string, targetIndex: number): Promise<boolean>
  onDelete(layerId: string): Promise<boolean>
  onAssignSelectedEdge(layerId: string): Promise<boolean>
  localeStore?: LocaleStore
}

type OperationState = 'idle' | 'busy' | 'error'

export function ProjectLayerPanel({
  document,
  bindingKey,
  selectedEdgeId,
  disabled,
  documentInvalid = false,
  onCreate,
  onRename,
  onMove,
  onDelete,
  onAssignSelectedEdge,
  localeStore: localeStore_ = localeStore,
}: ProjectLayerPanelProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const [operationState, setOperationState] =
    useState<OperationState>('idle')
  const operationActiveRef = useRef(false)
  const requestSequenceRef = useRef(0)
  const bindingKeyRef = useRef(bindingKey)
  bindingKeyRef.current = bindingKey

  useEffect(() => {
    requestSequenceRef.current += 1
    operationActiveRef.current = false
    setOperationState('idle')
  }, [bindingKey])

  useEffect(() => () => {
    requestSequenceRef.current += 1
    operationActiveRef.current = false
  }, [])

  const controlsDisabled = disabled
    || documentInvalid
    || operationState === 'busy'
  const selectedEdgeLayerId = selectedEdgeId === null
    ? null
    : document.edge_assignments.find(
      (assignment) => assignment.edge === selectedEdgeId,
    )?.layer ?? DEFAULT_PROJECT_LAYER_ID
  const assignmentCounts = new Map<string, number>()
  for (const assignment of document.edge_assignments) {
    assignmentCounts.set(
      assignment.layer,
      (assignmentCounts.get(assignment.layer) ?? 0) + 1,
    )
  }

  async function runMutation(action: () => Promise<boolean>) {
    if (controlsDisabled || operationActiveRef.current) return
    operationActiveRef.current = true
    const requestSequence = ++requestSequenceRef.current
    const expectedBindingKey = bindingKey
    setOperationState('busy')
    try {
      const applied = await Promise.resolve().then(action)
      if (
        requestSequenceRef.current !== requestSequence
        || bindingKeyRef.current !== expectedBindingKey
      ) return
      setOperationState(applied ? 'idle' : 'error')
    } catch {
      if (
        requestSequenceRef.current === requestSequence
        && bindingKeyRef.current === expectedBindingKey
      ) setOperationState('error')
    } finally {
      if (
        requestSequenceRef.current === requestSequence
        && bindingKeyRef.current === expectedBindingKey
      ) {
        operationActiveRef.current = false
        setOperationState((current) => (
          current === 'busy' ? 'error' : current
        ))
      }
    }
  }

  function submitCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (controlsDisabled) return
    const form = event.currentTarget
    const values = new FormData(form)
    const name = String(values.get('project_layer_name') ?? '').trim()
    const rawContentKind = values.get('project_layer_content_kind')
    if (!isContentKind(rawContentKind)) {
      setOperationState('error')
      return
    }
    void runMutation(async () => {
      const applied = await onCreate(name, rawContentKind)
      if (applied && form.isConnected) form.reset()
      return applied
    })
  }

  function submitRename(
    event: FormEvent<HTMLFormElement>,
    layerId: string,
    previousName: string,
    displayedPreviousName: string,
  ) {
    event.preventDefault()
    if (controlsDisabled) return
    const values = new FormData(event.currentTarget)
    const name = String(values.get('project_layer_rename') ?? '').trim()
    if (name === previousName || name === displayedPreviousName) return
    void runMutation(() => onRename(layerId, name))
  }

  function confirmDelete(
    layerId: string,
    layerName: string,
    assignmentCount: number,
  ) {
    if (
      controlsDisabled
      || layerId === DEFAULT_PROJECT_LAYER_ID
      || !window.confirm(formatLocalizedText(
        locale,
        TEXT.deleteConfirmation,
        {
          name: layerName,
          count: assignmentCount,
        },
      ))
    ) return
    void runMutation(() => onDelete(layerId))
  }

  return (
    <section
      className="project-layers"
      aria-labelledby="project-layers-title"
      aria-busy={operationState === 'busy'}
    >
      <div className="project-layers-heading">
        <h2 id="project-layers-title">{text(TEXT.heading)}</h2>
        <span>
          {formatLocalizedText(locale, TEXT.layerCount, {
            count: document.layers.length,
          })}
        </span>
      </div>

      <p className="project-layer-note">{text(TEXT.orderNote)}</p>
      <p id="project-layer-kind-note" className="project-layer-note">
        {text(TEXT.unsupportedObjects)}
      </p>

      {documentInvalid && (
        <p
          className="project-layer-status is-error"
          role="alert"
          aria-live="assertive"
          aria-atomic="true"
        >
          {text(TEXT.invalidDocument)}
        </p>
      )}
      {!documentInvalid && operationState === 'busy' && (
        <p
          className="project-layer-status is-busy"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {text(TEXT.busy)}
        </p>
      )}
      {!documentInvalid && operationState === 'error' && (
        <p
          className="project-layer-status is-error"
          role="alert"
          aria-live="assertive"
          aria-atomic="true"
        >
          {text(TEXT.failed)}
        </p>
      )}

      <form className="project-layer-create" onSubmit={submitCreate}>
        <fieldset disabled={controlsDisabled}>
          <legend>{text(TEXT.createLegend)}</legend>
          <label>
            <span>{text(TEXT.nameLabel)}</span>
            <input
              name="project_layer_name"
              type="text"
              required
              maxLength={MAX_LAYER_NAME_CHARS * 2}
              aria-describedby="project-layer-kind-note"
              disabled={controlsDisabled}
            />
          </label>
          <label>
            <span>{text(TEXT.kindLabel)}</span>
            <select
              name="project_layer_content_kind"
              defaultValue="crease_pattern"
              aria-describedby="project-layer-kind-note"
              disabled={controlsDisabled}
            >
              <option value="crease_pattern">
                {text(TEXT.kindCreasePattern)}
              </option>
              <option value="annotation">{text(TEXT.kindAnnotation)}</option>
              <option value="underlay">{text(TEXT.kindUnderlay)}</option>
            </select>
          </label>
          <button type="submit" disabled={controlsDisabled}>
            {text(TEXT.createAction)}
          </button>
        </fieldset>
      </form>

      <ol
        className="project-layer-list"
        aria-label={text(TEXT.layerList)}
      >
        {document.layers.map((layer, index) => {
          const isDefault = layer.id === DEFAULT_PROJECT_LAYER_ID
          const displayName = projectLayerDisplayName(layer, locale)
          const assignmentCount = assignmentCounts.get(layer.id) ?? 0
          const selectedEdgeAssigned = selectedEdgeLayerId === layer.id
          const nameId = `project-layer-name-${layer.id}`
          return (
            <li
              key={layer.id}
              aria-labelledby={nameId}
              data-layer-content-kind={layer.content_kind}
            >
              <div className="project-layer-summary">
                <strong id={nameId}>{displayName}</strong>
                <span className="project-layer-kind">
                  {contentKindLabel(layer.content_kind, locale)}
                </span>
                {isDefault && (
                  <span className="project-layer-default">
                    {text(TEXT.defaultBadge)}
                  </span>
                )}
                {layer.content_kind === 'crease_pattern' && (
                  <span className="project-layer-assignment-count">
                    {formatLocalizedText(locale, TEXT.assignmentCount, {
                      count: assignmentCount,
                    })}
                  </span>
                )}
              </div>

              <form
                className="project-layer-rename"
                onSubmit={(event) => submitRename(
                  event,
                  layer.id,
                  layer.name,
                  displayName,
                )}
              >
                <label>
                  <span className="visually-hidden">
                    {formatLocalizedText(locale, TEXT.renameLabel, {
                      name: displayName,
                    })}
                  </span>
                  <input
                    key={`${layer.id}:${layer.name}:${locale}`}
                    name="project_layer_rename"
                    type="text"
                    required
                    defaultValue={displayName}
                    maxLength={MAX_LAYER_NAME_CHARS * 2}
                    disabled={controlsDisabled}
                  />
                </label>
                <button type="submit" disabled={controlsDisabled}>
                  {text(TEXT.renameAction)}
                </button>
              </form>

              <div className="project-layer-row-actions">
                <button
                  type="button"
                  disabled={controlsDisabled || index === 0}
                  aria-label={formatLocalizedText(locale, TEXT.moveUpLabel, {
                    name: displayName,
                  })}
                  onClick={() => void runMutation(
                    () => onMove(layer.id, index - 1),
                  )}
                >
                  {text(TEXT.moveUp)}
                </button>
                <button
                  type="button"
                  disabled={
                    controlsDisabled
                    || index === document.layers.length - 1
                  }
                  aria-label={formatLocalizedText(locale, TEXT.moveDownLabel, {
                    name: displayName,
                  })}
                  onClick={() => void runMutation(
                    () => onMove(layer.id, index + 1),
                  )}
                >
                  {text(TEXT.moveDown)}
                </button>
                {layer.content_kind === 'crease_pattern' ? (
                  <button
                    type="button"
                    disabled={
                      controlsDisabled
                      || selectedEdgeId === null
                      || selectedEdgeAssigned
                    }
                    aria-pressed={selectedEdgeAssigned}
                    aria-label={formatLocalizedText(
                      locale,
                      selectedEdgeAssigned
                        ? TEXT.assignedLabel
                        : TEXT.assignLabel,
                      { name: displayName },
                    )}
                    onClick={() => void runMutation(
                      () => onAssignSelectedEdge(layer.id),
                    )}
                  >
                    {selectedEdgeAssigned
                      ? text(TEXT.assignedAction)
                      : text(TEXT.assignAction)}
                  </button>
                ) : (
                  <span className="project-layer-unavailable">
                    {text(TEXT.assignmentUnavailable)}
                  </span>
                )}
                <button
                  type="button"
                  className="danger"
                  disabled={controlsDisabled || isDefault}
                  aria-label={formatLocalizedText(
                    locale,
                    isDefault ? TEXT.defaultDeleteLabel : TEXT.deleteLabel,
                    { name: displayName },
                  )}
                  title={isDefault ? text(TEXT.defaultDeleteTitle) : undefined}
                  onClick={() => confirmDelete(
                    layer.id,
                    displayName,
                    assignmentCount,
                  )}
                >
                  {text(TEXT.deleteAction)}
                </button>
              </div>
            </li>
          )
        })}
      </ol>

      {selectedEdgeId === null && (
        <p className="project-layer-note">{text(TEXT.selectEdge)}</p>
      )}
    </section>
  )
}

function isContentKind(value: FormDataEntryValue | null):
value is LayerContentKindV1 {
  return value === 'crease_pattern'
    || value === 'annotation'
    || value === 'underlay'
}

function contentKindLabel(kind: LayerContentKindV1, locale: Locale) {
  switch (kind) {
    case 'crease_pattern':
      return selectLocalizedText(locale, TEXT.kindCreasePattern)
    case 'annotation':
      return selectLocalizedText(locale, TEXT.kindAnnotation)
    case 'underlay':
      return selectLocalizedText(locale, TEXT.kindUnderlay)
  }
}

function projectLayerDisplayName(
  layer: Pick<LayerRecordV1, 'id' | 'name'>,
  locale: Locale,
) {
  return layer.id === DEFAULT_PROJECT_LAYER_ID
    && layer.name === DEFAULT_PROJECT_LAYER_NAME
    ? selectLocalizedText(locale, TEXT.defaultLayerName)
    : layer.name
}

function localized(ja: string, en: string): LocalizedText {
  return { ja, en }
}

const TEXT = {
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
  assignmentCount: localized(
    '明示割当 {count}本',
    '{count} explicitly assigned lines',
  ),
  renameLabel: localized(
    '{name}の新しいレイヤー名',
    'New layer name for {name}',
  ),
  renameAction: localized('名前を保存', 'Save name'),
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
} as const satisfies Readonly<Record<string, LocalizedText>>
