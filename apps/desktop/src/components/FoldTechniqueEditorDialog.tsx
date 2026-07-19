import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'

import {
  FOLD_TECHNIQUE_LIMITS_V1,
  admitFoldTechniqueDocumentV1,
  createInitialFoldTechniqueOperationV1,
  foldTechniqueDocumentsEqualV1,
  foldTechniqueLocalizedTextV1,
  isFoldTechniqueActionKindV1,
  updateFoldTechniqueDocumentDraftV1,
  validateFoldTechniqueDocumentV1,
  type FoldTechniqueActionKindV1,
  type FoldTechniqueFileDocumentV1,
  type FoldTechniqueOperationV1,
  type FoldTechniqueSourceV1,
} from '../lib/foldTechniqueEditor.ts'
import { useLocale } from '../lib/i18n.ts'
import './FoldTechniqueEditorDialog.css'

export type FoldTechniqueEditorDialogProps = Readonly<{
  mode: 'create' | 'edit'
  initialDocument: unknown
  techniqueIndex?: number
  busy?: boolean
  saveFailed?: boolean
  onConfirm: (document: FoldTechniqueFileDocumentV1) => void
  onCancel: () => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const ACTIONS: readonly FoldTechniqueActionKindV1[] = Object.freeze([
  'instruction_cue',
  'straight_line_stacked_fold',
  'inside_reverse_fold',
  'outside_reverse_fold',
  'sink_fold',
  'layer_selective_manipulation',
])

const COPY = {
  ja: {
    eyebrow: {
      create: '名前付き折り技法の作成',
      edit: '名前付き折り技法の編集',
    },
    title: '説明テンプレートを編集',
    close: '閉じる',
    description:
      '技法名と順序付き手順を、共有可能なV1宣言データとして編集します。この画面から折り操作やプロジェクト変更は実行しません。',
    inertTitle: '安全上の重要事項',
    inert:
      '中割り、かぶせ、沈め、層を選ぶ操作は説明metadataとして保存されるだけで、自動実行されません。必要な未対応物理操作も文書内へ明示されます。',
    invalidInitial:
      '編集元の技法データが厳密なV1契約を満たしていないため、開けません。',
    packageTitle: '共有パッケージ',
    packageId: 'パッケージID',
    authors: '作成者',
    author: '作成者名',
    addAuthor: '作成者を追加',
    removeAuthor: 'この作成者を削除',
    source: '出典区分',
    sourceKinds: {
      user_authored: '利用者が新規作成',
      adapted: '既存資料をもとに改作',
      published_reference: '公開資料を参照',
    },
    citation: '出典の記述（参照されないplain text）',
    license: 'SPDXライセンスID',
    techniqueTitle: '技法',
    techniquePosition: '編集中の技法',
    techniqueId: '技法ID',
    techniqueVersion: '技法の改訂番号',
    nameJa: '技法名（日本語）',
    nameEn: '技法名（英語）',
    descriptionJa: '説明（日本語）',
    descriptionEn: '説明（英語）',
    preserved:
      '既存のparameterとpreconditionは変更せず保持します。この初期UIでは順序付きoperationを編集します。',
    parameters: 'parameter',
    preconditions: 'precondition',
    operationsTitle: '順序付き手順',
    operationsDescription:
      '2〜256件。上下の順序は共有ファイルでもそのまま保持されます。',
    addOperation: '説明手順を追加',
    operation: '手順',
    operationId: '手順ID',
    operationNameJa: '手順名（日本語）',
    operationNameEn: '手順名（英語）',
    action: '動作区分',
    actionLabels: {
      instruction_cue: '文章による案内',
      straight_line_stacked_fold: '一直線の折り重ね',
      inside_reverse_fold: '中割り折り',
      outside_reverse_fold: 'かぶせ折り',
      sink_fold: '沈め折り',
      layer_selective_manipulation: '層を選ぶ操作',
    },
    instructionJa: '案内文（日本語）',
    instructionEn: '案内文（英語）',
    sinkKind: '沈め方',
    openSink: 'オープンシンク',
    closedSink: 'クローズドシンク',
    support: '実行support',
    declarative:
      '宣言のみ。自動実行の許可や物理的な成立証明ではありません。',
    unsupported:
      '未対応物理操作として保存します。現在のsimulatorは自動実行しません。',
    moveUp: '上へ移動',
    moveDown: '下へ移動',
    removeOperation: 'この手順を削除',
    invalid: '入力内容を確認してください。',
    validation: {
      invalid_structure: '文書構造に認識できない値があります。',
      unsupported_schema: '対応していないschemaです。',
      unsupported_version: '対応していないfile versionです。',
      resource_limit: '件数または構造の固定上限を超えています。',
      invalid_field: 'ID、文字、locale、数値範囲のいずれかが不正です。',
      duplicate_identifier: '同じID、locale、作成者または参照が重複しています。',
      missing_reference: 'parameterまたはpreconditionへの参照が見つかりません。',
      parameter_type_mismatch: 'parameterの型、範囲または比較が一致しません。',
      inconsistent_execution_support:
        '動作、必要capability、未対応物理操作metadataが一致しません。',
      encoded_size_limit: '保存後のJSONが1 MiB上限を超えます。',
    },
    saveFailed: '技法データを確定できませんでした。もう一度お試しください。',
    noChanges: '変更はありません。',
    cancel: 'キャンセル',
    saving: '処理中…',
    confirm: {
      create: '技法を作成',
      edit: '変更を確定',
    },
  },
  en: {
    eyebrow: {
      create: 'Create named fold technique',
      edit: 'Edit named fold technique',
    },
    title: 'Edit the instruction template',
    close: 'Close',
    description:
      'Edit the technique name and ordered steps as shareable declarative V1 data. This dialog never performs folds or changes a project.',
    inertTitle: 'Important safety boundary',
    inert:
      'Inside reverse, outside reverse, sink, and layer-selective actions are stored only as descriptive metadata and are never executed automatically. Their unsupported physical operation is recorded explicitly.',
    invalidInitial:
      'The source technique data does not satisfy the strict V1 contract and cannot be opened.',
    packageTitle: 'Shared package',
    packageId: 'Package ID',
    authors: 'Authors',
    author: 'Author name',
    addAuthor: 'Add author',
    removeAuthor: 'Remove this author',
    source: 'Source provenance',
    sourceKinds: {
      user_authored: 'User authored',
      adapted: 'Adapted from a source',
      published_reference: 'Published reference',
    },
    citation: 'Citation text (inert plain text; never fetched)',
    license: 'SPDX license ID',
    techniqueTitle: 'Technique',
    techniquePosition: 'Technique being edited',
    techniqueId: 'Technique ID',
    techniqueVersion: 'Technique revision',
    nameJa: 'Technique name (Japanese)',
    nameEn: 'Technique name (English)',
    descriptionJa: 'Description (Japanese)',
    descriptionEn: 'Description (English)',
    preserved:
      'Existing parameters and preconditions are preserved unchanged. This initial UI edits the ordered operations.',
    parameters: 'parameters',
    preconditions: 'preconditions',
    operationsTitle: 'Ordered steps',
    operationsDescription:
      '2–256 steps. Their order is preserved in the shared file.',
    addOperation: 'Add instruction step',
    operation: 'Step',
    operationId: 'Step ID',
    operationNameJa: 'Step name (Japanese)',
    operationNameEn: 'Step name (English)',
    action: 'Action kind',
    actionLabels: {
      instruction_cue: 'Instruction cue',
      straight_line_stacked_fold: 'Straight-line stacked fold',
      inside_reverse_fold: 'Inside reverse fold',
      outside_reverse_fold: 'Outside reverse fold',
      sink_fold: 'Sink fold',
      layer_selective_manipulation: 'Layer-selective manipulation',
    },
    instructionJa: 'Instruction (Japanese)',
    instructionEn: 'Instruction (English)',
    sinkKind: 'Sink kind',
    openSink: 'Open sink',
    closedSink: 'Closed sink',
    support: 'Execution support',
    declarative:
      'Declarative only. This is not execution permission or proof of physical validity.',
    unsupported:
      'Stored as an unsupported physical operation. The current simulator does not execute it.',
    moveUp: 'Move up',
    moveDown: 'Move down',
    removeOperation: 'Remove this step',
    invalid: 'Review the entered values.',
    validation: {
      invalid_structure: 'The document contains an unrecognized structure.',
      unsupported_schema: 'The schema is not supported.',
      unsupported_version: 'The file version is not supported.',
      resource_limit: 'A fixed collection or structure limit was exceeded.',
      invalid_field: 'An ID, text, locale, or numeric range is invalid.',
      duplicate_identifier: 'An ID, locale, author, or reference is duplicated.',
      missing_reference: 'A parameter or precondition reference is missing.',
      parameter_type_mismatch:
        'A parameter type, range, or comparison does not match.',
      inconsistent_execution_support:
        'The action, required capability, and physical-support metadata disagree.',
      encoded_size_limit: 'The encoded JSON exceeds the 1 MiB limit.',
    },
    saveFailed: 'The technique data could not be committed. Try again.',
    noChanges: 'No changes.',
    cancel: 'Cancel',
    saving: 'Processing…',
    confirm: {
      create: 'Create technique',
      edit: 'Apply changes',
    },
  },
} as const

export function FoldTechniqueEditorDialog({
  mode,
  initialDocument,
  techniqueIndex = 0,
  busy = false,
  saveFailed = false,
  onConfirm,
  onCancel,
}: FoldTechniqueEditorDialogProps) {
  const locale = useLocale()
  const copy = COPY[locale]
  const baseline = useMemo(
    () => admitFoldTechniqueDocumentV1(initialDocument),
    [initialDocument],
  )
  const selectedIndex = Number.isSafeInteger(techniqueIndex)
    && techniqueIndex >= 0
    && baseline
    && techniqueIndex < baseline.techniques.length
    ? techniqueIndex
    : -1
  const [draft, setDraft] = useState<FoldTechniqueFileDocumentV1 | null>(
    () => baseline,
  )
  const initialVersion = selectedIndex >= 0
    ? baseline?.techniques[selectedIndex]?.version
    : null
  const [versionInput, setVersionInput] = useState(
    initialVersion === null || initialVersion === undefined
      ? ''
      : String(initialVersion),
  )
  const dialogRef = useRef<HTMLElement>(null)
  const firstInputRef = useRef<HTMLInputElement>(null)
  const initialCanEditRef = useRef(
    !busy && Boolean(draft) && selectedIndex >= 0,
  )

  useEffect(() => {
    setDraft(baseline)
    const version = selectedIndex >= 0
      ? baseline?.techniques[selectedIndex]?.version
      : null
    setVersionInput(version === null || version === undefined ? '' : String(version))
  }, [baseline, selectedIndex])

  useEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
    const frame = requestAnimationFrame(() => {
      if (initialCanEditRef.current) firstInputRef.current?.focus()
      else dialogRef.current?.focus()
    })
    return () => {
      cancelAnimationFrame(frame)
      if (previouslyFocused?.isConnected) previouslyFocused.focus()
    }
  }, [])

  useEffect(() => {
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      const target = event.target
      if (!dialog || !(target instanceof Node) || dialog.contains(target)) return
      if (busy || !draft || selectedIndex < 0) dialog.focus()
      else firstInputRef.current?.focus()
    }
    document.addEventListener('focusin', handleFocusIn, true)
    return () => document.removeEventListener('focusin', handleFocusIn, true)
  }, [busy, draft, selectedIndex])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing || busy) return
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => document.removeEventListener('keydown', handleKeyDown, true)
  }, [busy, onCancel])

  const validation = draft
    ? validateFoldTechniqueDocumentV1(draft)
    : null
  const versionIsValid = /^(?:[1-9][0-9]{0,6})$/u.test(versionInput)
    && Number(versionInput) <= FOLD_TECHNIQUE_LIMITS_V1.techniqueVersion
  const hasChanges = Boolean(
    baseline
    && validation?.ok
    && !foldTechniqueDocumentsEqualV1(baseline, validation.document),
  )
  const canConfirm = Boolean(
    validation?.ok
    && versionIsValid
    && (mode === 'create' || hasChanges)
    && !busy
    && selectedIndex >= 0,
  )
  const technique = selectedIndex >= 0
    ? draft?.techniques[selectedIndex] ?? null
    : null

  const applyUpdate = (
    update: Parameters<typeof updateFoldTechniqueDocumentDraftV1>[1],
  ) => {
    setDraft((current) =>
      current ? updateFoldTechniqueDocumentDraftV1(current, update) : current)
  }

  const updateVersion = (value: string) => {
    setVersionInput(value)
    if (!/^(?:[1-9][0-9]{0,6})$/u.test(value)) return
    const parsed = Number(value)
    if (
      parsed > FOLD_TECHNIQUE_LIMITS_V1.techniqueVersion
      || selectedIndex < 0
    ) return
    applyUpdate({
      kind: 'technique_version',
      techniqueIndex: selectedIndex,
      value: parsed,
    })
  }

  const updateAuthor = (index: number, value: string) => {
    if (!draft || index < 0 || index >= draft.metadata.authors.length) return
    const authors = [...draft.metadata.authors]
    authors[index] = value
    applyUpdate({ kind: 'authors', value: authors })
  }

  const removeAuthor = (index: number) => {
    if (!draft || draft.metadata.authors.length <= 1) return
    applyUpdate({
      kind: 'authors',
      value: draft.metadata.authors.filter((_, current) => current !== index),
    })
  }

  const changeSource = (kind: FoldTechniqueSourceV1['kind']) => {
    if (!draft) return
    const previous = draft.metadata.source
    const value: FoldTechniqueSourceV1 = kind === 'user_authored'
      ? { kind }
      : {
          kind,
          citation_text: previous.kind === 'user_authored'
            ? ''
            : previous.citation_text,
        }
    applyUpdate({ kind: 'source', value })
  }

  const addOperation = () => {
    if (!technique || selectedIndex < 0) return
    const ids = new Set(technique.operations.map((operation) => operation.id))
    let ordinal = technique.operations.length + 1
    while (ids.has(`step-${ordinal}`) && ordinal <= 1_000_000) ordinal += 1
    const operation = createInitialFoldTechniqueOperationV1(ordinal)
    applyUpdate({
      kind: 'insert_operation',
      techniqueIndex: selectedIndex,
      operationIndex: technique.operations.length,
      operation,
    })
  }

  const trapFocus = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
    )
    if (focusable.length === 0) {
      event.preventDefault()
      dialog.focus()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement
    if (event.shiftKey && (active === first || !dialog.contains(active))) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
      event.preventDefault()
      first?.focus()
    }
  }

  const validationError = !versionIsValid
    ? 'invalid_field' as const
    : validation && !validation.ok
      ? validation.error
      : null

  return (
    <div className="fold-technique-editor-backdrop">
      <section
        ref={dialogRef}
        className="fold-technique-editor-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="fold-technique-editor-title"
        aria-describedby="fold-technique-editor-description"
        aria-busy={busy}
        tabIndex={-1}
        onKeyDown={trapFocus}
      >
        <header className="fold-technique-editor-header">
          <div>
            <span className="fold-technique-editor-eyebrow">
              {copy.eyebrow[mode]}
            </span>
            <h2 id="fold-technique-editor-title">{copy.title}</h2>
          </div>
          <button
            type="button"
            className="fold-technique-editor-close"
            disabled={busy}
            onClick={onCancel}
            aria-label={copy.close}
          >
            ×
          </button>
        </header>

        <div className="fold-technique-editor-body">
          <p id="fold-technique-editor-description">{copy.description}</p>
          <aside
            className="fold-technique-editor-boundary"
            aria-labelledby="fold-technique-editor-boundary-title"
          >
            <strong id="fold-technique-editor-boundary-title">
              {copy.inertTitle}
            </strong>
            <span>{copy.inert}</span>
          </aside>

          {!draft || !technique ? (
            <p className="fold-technique-editor-error" role="alert">
              {copy.invalidInitial}
            </p>
          ) : (
            <fieldset disabled={busy} className="fold-technique-editor-fields">
              <section aria-labelledby="fold-technique-package-title">
                <h3 id="fold-technique-package-title">{copy.packageTitle}</h3>
                <label>
                  <span>{copy.packageId}</span>
                  <input
                    ref={firstInputRef}
                    value={draft.package_id}
                    maxLength={FOLD_TECHNIQUE_LIMITS_V1.packageIdBytes}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(event) => applyUpdate({
                      kind: 'package_id',
                      value: event.currentTarget.value,
                    })}
                  />
                </label>

                <div className="fold-technique-editor-authors">
                  <span>{copy.authors}</span>
                  {draft.metadata.authors.map((author, index) => (
                    <div key={index} className="fold-technique-editor-row">
                      <label>
                        <span className="fold-technique-editor-sr-only">
                          {copy.author} {index + 1}
                        </span>
                        <input
                          value={author}
                          maxLength={FOLD_TECHNIQUE_LIMITS_V1.authorBytes}
                          onChange={(event) =>
                            updateAuthor(index, event.currentTarget.value)}
                        />
                      </label>
                      <button
                        type="button"
                        disabled={draft.metadata.authors.length <= 1}
                        onClick={() => removeAuthor(index)}
                        aria-label={`${copy.removeAuthor} ${index + 1}`}
                      >
                        −
                      </button>
                    </div>
                  ))}
                  <button
                    type="button"
                    disabled={
                      draft.metadata.authors.length
                      >= FOLD_TECHNIQUE_LIMITS_V1.authors
                    }
                    onClick={() => applyUpdate({
                      kind: 'authors',
                      value: [...draft.metadata.authors, 'New author'],
                    })}
                  >
                    {copy.addAuthor}
                  </button>
                </div>

                <label>
                  <span>{copy.source}</span>
                  <select
                    value={draft.metadata.source.kind}
                    onChange={(event) => {
                      const kind = event.currentTarget.value
                      if (
                        kind === 'user_authored'
                        || kind === 'adapted'
                        || kind === 'published_reference'
                      ) changeSource(kind)
                    }}
                  >
                    {(
                      [
                        'user_authored',
                        'adapted',
                        'published_reference',
                      ] as const
                    ).map((kind) => (
                      <option key={kind} value={kind}>
                        {copy.sourceKinds[kind]}
                      </option>
                    ))}
                  </select>
                </label>

                {draft.metadata.source.kind !== 'user_authored' && (
                  <label>
                    <span>{copy.citation}</span>
                    <textarea
                      value={draft.metadata.source.citation_text}
                      maxLength={FOLD_TECHNIQUE_LIMITS_V1.citationBytes}
                      rows={3}
                      onChange={(event) => applyUpdate({
                        kind: 'source',
                        value: {
                          kind: draft.metadata.source.kind === 'adapted'
                            ? 'adapted'
                            : 'published_reference',
                          citation_text: event.currentTarget.value,
                        },
                      })}
                    />
                  </label>
                )}

                <label>
                  <span>{copy.license}</span>
                  <input
                    value={draft.metadata.license_spdx_id}
                    maxLength={FOLD_TECHNIQUE_LIMITS_V1.licenseIdBytes}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(event) => applyUpdate({
                      kind: 'license_spdx_id',
                      value: event.currentTarget.value,
                    })}
                  />
                </label>
              </section>

              <section aria-labelledby="fold-technique-template-title">
                <h3 id="fold-technique-template-title">{copy.techniqueTitle}</h3>
                {draft.techniques.length > 1 && (
                  <p>
                    {copy.techniquePosition}: {selectedIndex + 1}/
                    {draft.techniques.length}
                  </p>
                )}
                <div className="fold-technique-editor-grid">
                  <label>
                    <span>{copy.techniqueId}</span>
                    <input
                      value={technique.id}
                      maxLength={FOLD_TECHNIQUE_LIMITS_V1.identifierBytes}
                      autoComplete="off"
                      spellCheck={false}
                      onChange={(event) => applyUpdate({
                        kind: 'technique_id',
                        techniqueIndex: selectedIndex,
                        value: event.currentTarget.value,
                      })}
                    />
                  </label>
                  <label>
                    <span>{copy.techniqueVersion}</span>
                    <input
                      type="text"
                      inputMode="numeric"
                      value={versionInput}
                      onChange={(event) =>
                        updateVersion(event.currentTarget.value)}
                    />
                  </label>
                  <label>
                    <span>{copy.nameJa}</span>
                    <input
                      value={foldTechniqueLocalizedTextV1(
                        technique.names,
                        'ja',
                      )}
                      maxLength={FOLD_TECHNIQUE_LIMITS_V1.nameBytes}
                      onChange={(event) => applyUpdate({
                        kind: 'technique_name',
                        techniqueIndex: selectedIndex,
                        locale: 'ja',
                        value: event.currentTarget.value,
                      })}
                    />
                  </label>
                  <label>
                    <span>{copy.nameEn}</span>
                    <input
                      value={foldTechniqueLocalizedTextV1(
                        technique.names,
                        'en',
                      )}
                      maxLength={FOLD_TECHNIQUE_LIMITS_V1.nameBytes}
                      onChange={(event) => applyUpdate({
                        kind: 'technique_name',
                        techniqueIndex: selectedIndex,
                        locale: 'en',
                        value: event.currentTarget.value,
                      })}
                    />
                  </label>
                </div>
                <label>
                  <span>{copy.descriptionJa}</span>
                  <textarea
                    value={foldTechniqueLocalizedTextV1(
                      technique.descriptions,
                      'ja',
                    )}
                    maxLength={FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes}
                    rows={3}
                    onChange={(event) => applyUpdate({
                      kind: 'technique_description',
                      techniqueIndex: selectedIndex,
                      locale: 'ja',
                      value: event.currentTarget.value,
                    })}
                  />
                </label>
                <label>
                  <span>{copy.descriptionEn}</span>
                  <textarea
                    value={foldTechniqueLocalizedTextV1(
                      technique.descriptions,
                      'en',
                    )}
                    maxLength={FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes}
                    rows={3}
                    onChange={(event) => applyUpdate({
                      kind: 'technique_description',
                      techniqueIndex: selectedIndex,
                      locale: 'en',
                      value: event.currentTarget.value,
                    })}
                  />
                </label>
                <p className="fold-technique-editor-preserved">
                  {copy.preserved} ({technique.parameters.length}{' '}
                  {copy.parameters} · {technique.preconditions.length}{' '}
                  {copy.preconditions})
                </p>
              </section>

              <section aria-labelledby="fold-technique-operations-title">
                <div className="fold-technique-editor-section-heading">
                  <div>
                    <h3 id="fold-technique-operations-title">
                      {copy.operationsTitle}
                    </h3>
                    <p>{copy.operationsDescription}</p>
                  </div>
                  <button
                    type="button"
                    disabled={
                      technique.operations.length
                      >= FOLD_TECHNIQUE_LIMITS_V1.operations
                    }
                    onClick={addOperation}
                  >
                    {copy.addOperation}
                  </button>
                </div>

                <ol className="fold-technique-editor-operations">
                  {technique.operations.map((operation, operationIndex) => (
                    <li key={operationIndex}>
                      <OperationEditor
                        operation={operation}
                        operationIndex={operationIndex}
                        operationCount={technique.operations.length}
                        copy={copy}
                        update={(update) => applyUpdate(
                          locateOperationUpdate(
                            update,
                            selectedIndex,
                            operationIndex,
                          ),
                        )}
                        remove={() => applyUpdate({
                          kind: 'remove_operation',
                          techniqueIndex: selectedIndex,
                          operationIndex,
                        })}
                        move={(direction) => applyUpdate({
                          kind: 'move_operation',
                          techniqueIndex: selectedIndex,
                          operationIndex,
                          direction,
                        })}
                      />
                    </li>
                  ))}
                </ol>
              </section>
            </fieldset>
          )}

          {validationError && draft && technique && (
            <p className="fold-technique-editor-error" role="alert">
              {copy.invalid} {copy.validation[validationError]}
            </p>
          )}
          {saveFailed && (
            <p className="fold-technique-editor-error" role="alert">
              {copy.saveFailed}
            </p>
          )}
          {mode === 'edit'
            && !validationError
            && !hasChanges
            && draft
            && technique && (
            <p className="fold-technique-editor-status" role="status">
              {copy.noChanges}
            </p>
            )}
        </div>

        <footer className="fold-technique-editor-footer">
          <button type="button" disabled={busy} onClick={onCancel}>
            {copy.cancel}
          </button>
          <button
            type="button"
            className="fold-technique-editor-primary"
            disabled={!canConfirm}
            onClick={() => {
              if (!validation?.ok || !canConfirm) return
              onConfirm(validation.document)
            }}
          >
            {busy ? copy.saving : copy.confirm[mode]}
          </button>
        </footer>
      </section>
    </div>
  )
}

type OperationUpdate = Extract<
  Parameters<typeof updateFoldTechniqueDocumentDraftV1>[1],
  {
    kind:
      | 'operation_id'
      | 'operation_name'
      | 'operation_instruction'
      | 'operation_action'
      | 'operation_sink_kind'
  }
>

type OperationUpdateInput = OperationUpdate extends infer Update
  ? Update extends OperationUpdate
    ? Omit<Update, 'techniqueIndex' | 'operationIndex'>
    : never
  : never

type OperationEditorProps = Readonly<{
  operation: FoldTechniqueOperationV1
  operationIndex: number
  operationCount: number
  copy: (typeof COPY)[keyof typeof COPY]
  update: (update: OperationUpdateInput) => void
  remove: () => void
  move: (direction: -1 | 1) => void
}>

function OperationEditor({
  operation,
  operationIndex,
  operationCount,
  copy,
  update,
  remove,
  move,
}: OperationEditorProps) {
  const hasInstructions = operation.action.kind === 'instruction_cue'
    || operation.action.kind === 'layer_selective_manipulation'
  return (
    <article
      className="fold-technique-editor-operation"
      aria-labelledby={`fold-technique-operation-${operationIndex}`}
    >
      <div className="fold-technique-editor-operation-heading">
        <h4 id={`fold-technique-operation-${operationIndex}`}>
          {copy.operation} {operationIndex + 1}
        </h4>
        <div>
          <button
            type="button"
            disabled={operationIndex === 0}
            onClick={() => move(-1)}
            aria-label={`${copy.moveUp} ${operationIndex + 1}`}
          >
            ↑
          </button>
          <button
            type="button"
            disabled={operationIndex + 1 === operationCount}
            onClick={() => move(1)}
            aria-label={`${copy.moveDown} ${operationIndex + 1}`}
          >
            ↓
          </button>
          <button
            type="button"
            disabled={operationCount <= 2}
            onClick={remove}
            aria-label={`${copy.removeOperation} ${operationIndex + 1}`}
          >
            −
          </button>
        </div>
      </div>

      <div className="fold-technique-editor-grid">
        <label>
          <span>{copy.operationId}</span>
          <input
            value={operation.id}
            maxLength={FOLD_TECHNIQUE_LIMITS_V1.identifierBytes}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => update({
              kind: 'operation_id',
              value: event.currentTarget.value,
            })}
          />
        </label>
        <label>
          <span>{copy.action}</span>
          <select
            value={operation.action.kind}
            onChange={(event) => {
              const action = event.currentTarget.value
              if (isFoldTechniqueActionKindV1(action)) {
                update({ kind: 'operation_action', value: action })
              }
            }}
          >
            {ACTIONS.map((action) => (
              <option key={action} value={action}>
                {copy.actionLabels[action]}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{copy.operationNameJa}</span>
          <input
            value={foldTechniqueLocalizedTextV1(operation.names, 'ja')}
            maxLength={FOLD_TECHNIQUE_LIMITS_V1.nameBytes}
            onChange={(event) => update({
              kind: 'operation_name',
              locale: 'ja',
              value: event.currentTarget.value,
            })}
          />
        </label>
        <label>
          <span>{copy.operationNameEn}</span>
          <input
            value={foldTechniqueLocalizedTextV1(operation.names, 'en')}
            maxLength={FOLD_TECHNIQUE_LIMITS_V1.nameBytes}
            onChange={(event) => update({
              kind: 'operation_name',
              locale: 'en',
              value: event.currentTarget.value,
            })}
          />
        </label>
      </div>

      {hasInstructions && (
        <div className="fold-technique-editor-grid">
          <label>
            <span>{copy.instructionJa}</span>
            <textarea
              rows={3}
              value={foldTechniqueLocalizedTextV1(
                operation.action.instructions,
                'ja',
              )}
              maxLength={FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes}
              onChange={(event) => update({
                kind: 'operation_instruction',
                locale: 'ja',
                value: event.currentTarget.value,
              })}
            />
          </label>
          <label>
            <span>{copy.instructionEn}</span>
            <textarea
              rows={3}
              value={foldTechniqueLocalizedTextV1(
                operation.action.instructions,
                'en',
              )}
              maxLength={FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes}
              onChange={(event) => update({
                kind: 'operation_instruction',
                locale: 'en',
                value: event.currentTarget.value,
              })}
            />
          </label>
        </div>
      )}

      {operation.action.kind === 'sink_fold' && (
        <label>
          <span>{copy.sinkKind}</span>
          <select
            value={operation.action.sink_kind}
            onChange={(event) => {
              const sinkKind = event.currentTarget.value
              if (sinkKind === 'open' || sinkKind === 'closed') {
                update({ kind: 'operation_sink_kind', value: sinkKind })
              }
            }}
          >
            <option value="open">{copy.openSink}</option>
            <option value="closed">{copy.closedSink}</option>
          </select>
        </label>
      )}

      <p className={
        operation.execution_support.status === 'declarative_only'
          ? 'fold-technique-editor-support'
          : 'fold-technique-editor-support is-unsupported'
      }>
        <strong>{copy.support}: </strong>
        {operation.execution_support.status === 'declarative_only'
          ? copy.declarative
          : copy.unsupported}
      </p>
    </article>
  )
}

function locateOperationUpdate(
  update: OperationUpdateInput,
  techniqueIndex: number,
  operationIndex: number,
): OperationUpdate {
  switch (update.kind) {
    case 'operation_id':
    case 'operation_action':
    case 'operation_sink_kind':
      return { ...update, techniqueIndex, operationIndex }
    case 'operation_name':
    case 'operation_instruction':
      return { ...update, techniqueIndex, operationIndex }
  }
}
