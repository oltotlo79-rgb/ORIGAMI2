import { useEffect, useMemo, useRef, useState } from 'react'
import {
  initialSvgImportMapping,
  isValidSvgImportName,
  isSvgImportLineCap,
  parseSvgImportScale,
  safeSvgStrokeColor,
  svgImportBoundaryIsValid,
  svgImportPreviewBounds,
  svgImportStyleLabel,
  svgImportTargetOptions,
  unresolvedSvgImportGroups,
  type SvgBoundaryCandidate,
  type SvgImportMapping,
  type SvgImportPreview,
  type SvgImportSettings,
  type SvgImportSettingsDraft,
  type SvgImportSettingsValidation,
  type SvgImportTarget,
} from '../lib/svgImport'

type SvgImportDialogProps = Readonly<{
  preview: SvgImportPreview
  validation: SvgImportSettingsValidation | null
  busy: boolean
  error: string | null
  onInvalidateValidation: () => void
  onValidate: (settings: SvgImportSettingsDraft) => void
  onCancel: () => void
  onImport: (settings: SvgImportSettings) => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function SvgImportDialog({
  preview,
  validation,
  busy,
  error,
  onInvalidateValidation,
  onValidate,
  onCancel,
  onImport,
}: SvgImportDialogProps) {
  const [name, setName] = useState(preview.suggested_name)
  const [scaleInput, setScaleInput] = useState(
    preview.default_mm_per_unit === null ? '' : String(preview.default_mm_per_unit),
  )
  const [mapping, setMapping] = useState<SvgImportMapping>(
    () => initialSvgImportMapping(preview.style_groups),
  )
  const [boundarySelection, setBoundarySelection] = useState<
    number | null | undefined
  >(undefined)
  const [boundaryConfirmed, setBoundaryConfirmed] = useState(false)
  const [warningsAcknowledged, setWarningsAcknowledged] = useState(
    preview.warnings.length === 0,
  )
  const [cuttingAllowedConfirmed, setCuttingAllowedConfirmed] = useState(false)
  const dialogRef = useRef<HTMLElement>(null)
  const nameInputRef = useRef<HTMLInputElement>(null)
  const scale = parseSvgImportScale(scaleInput)
  const unresolved = unresolvedSvgImportGroups(preview.style_groups, mapping)
  const nameIsValid = isValidSvgImportName(name)
  const boundaryIsValid = boundarySelection !== undefined
    && svgImportBoundaryIsValid(preview, boundarySelection, mapping)
  const selectedCandidate = boundarySelection === null || boundarySelection === undefined
    ? null
    : preview.boundary_candidates.find(
      (candidate) => candidate.candidate_id === boundarySelection,
    ) ?? null
  const validationMatches = validation !== null
    && scale !== null
    && validation.preview_id === preview.import_id
    && Object.is(validation.millimeters_per_unit, scale)
    && validation.boundary_candidate_id === boundarySelection
    && Number.isFinite(validation.width_mm)
    && Number.isFinite(validation.height_mm)
    && validation.width_mm > 0
    && validation.height_mm > 0
  const hasValidatedCuts = validationMatches && validation.has_cuts
  const canValidate = !busy
    && scale !== null
    && unresolved.length === 0
    && boundaryIsValid
  const canImport = !busy
    && nameIsValid
    && scale !== null
    && unresolved.length === 0
    && boundaryIsValid
    && validationMatches
    && boundaryConfirmed
    && warningsAcknowledged
    && (!hasValidatedCuts || cuttingAllowedConfirmed)
  const bounds = useMemo(
    () => svgImportPreviewBounds([
      ...preview.preview_vertices,
      ...preview.boundary_candidates.flatMap((candidate) => candidate.vertices),
    ]),
    [preview.boundary_candidates, preview.preview_vertices],
  )
  const groupsById = useMemo(
    () => new Map(preview.style_groups.map((group) => [group.group_id, group])),
    [preview.style_groups],
  )
  useEffect(() => {
    nameInputRef.current?.focus()
  }, [])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.isComposing && !busy) {
        event.preventDefault()
        onCancel()
        return
      }
      if (event.key !== 'Tab') return
      const dialog = dialogRef.current
      if (!dialog) return
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
      if (focusable.length === 0) {
        event.preventDefault()
        dialog.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      if (
        event.shiftKey
        && (active === first || active === dialog || !dialog.contains(active))
      ) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      if (
        dialog
        && event.target instanceof Node
        && !dialog.contains(event.target)
      ) {
        dialog.focus()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    window.addEventListener('focusin', handleFocusIn)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
      window.removeEventListener('focusin', handleFocusIn)
    }
  }, [busy, onCancel])

  const invalidateValidation = () => {
    setBoundaryConfirmed(false)
    setCuttingAllowedConfirmed(false)
    onInvalidateValidation()
  }

  const selectBoundary = (value: string) => {
    invalidateValidation()
    if (value === '') {
      setBoundarySelection(undefined)
      return
    }
    if (value === 'groups') {
      setBoundarySelection(null)
      return
    }
    const candidateId = Number(value)
    if (
      !Number.isSafeInteger(candidateId)
      || !preview.boundary_candidates.some(
        (candidate) => candidate.candidate_id === candidateId,
      )
    ) {
      setBoundarySelection(undefined)
      return
    }
    setBoundarySelection(candidateId)
    setMapping((current) => {
      const next = { ...current }
      for (const group of preview.style_groups) {
        if (next[String(group.group_id)] === 'boundary') {
          delete next[String(group.group_id)]
        }
      }
      return next
    })
  }

  const validateSettings = () => {
    if (!canValidate || scale === null || boundarySelection === undefined) return
    invalidateValidation()
    onValidate({
      importId: preview.import_id,
      mmPerUnit: scale,
      boundaryCandidateId: boundarySelection,
      mappings: mapping,
    })
  }

  const submit = () => {
    if (
      !canImport
      || scale === null
      || boundarySelection === undefined
      || !validationMatches
    ) return
    onImport({
      importId: preview.import_id,
      validationId: validation.validation_id,
      name: name.trim(),
      mmPerUnit: scale,
      boundaryCandidateId: boundarySelection,
      boundaryConfirmed,
      mappings: mapping,
      warningsAcknowledged,
      cuttingAllowedConfirmed,
    })
  }

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="svg-import-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="svg-import-title"
        aria-describedby="svg-import-description"
        tabIndex={-1}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">SVG 1.1 / 2 静的線図取込</span>
            <h2 id="svg-import-title">外周・線種・縮尺を確認</h2>
          </div>
          <button
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label="閉じる"
          >
            ×
          </button>
        </header>

        <div className="svg-import-dialog-body">
          <p id="svg-import-description" className="dialog-note">
            元のSVGは変更しません。直線を交点で分割し、編集可能な未保存プロジェクトとして取り込みます。
          </p>

          <div className="svg-import-overview">
            <div className="svg-import-preview">
              {bounds ? (
                <svg
                  viewBox={`${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`}
                  role="img"
                  aria-label="取り込むSVG線図のプレビュー"
                  preserveAspectRatio="xMidYMid meet"
                >
                  {preview.preview_edges.map((edge, index) => {
                    const start = preview.preview_vertices[edge.start]
                    const end = preview.preview_vertices[edge.end]
                    if (!start || !end) return null
                    const target = mapping[String(edge.group_id)]
                    const group = groupsById.get(edge.group_id)
                    const sourceColor = safeSvgStrokeColor(group?.stroke_color ?? null)
                    return (
                      <line
                        key={`${edge.start}:${edge.end}:${edge.group_id}:${index}`}
                        className={`svg-preview-edge target-${target ?? 'unresolved'}`}
                        style={target ? undefined : sourceColor ? { stroke: sourceColor } : undefined}
                        x1={start.x}
                        y1={start.y}
                        x2={end.x}
                        y2={end.y}
                        vectorEffect="non-scaling-stroke"
                      />
                    )
                  })}
                  {selectedCandidate && selectedCandidate.vertices.length >= 2 && (
                    <polygon
                      className="svg-preview-boundary-candidate"
                      points={selectedCandidate.vertices
                        .map((vertex) => `${vertex.x},${vertex.y}`)
                        .join(' ')}
                      vectorEffect="non-scaling-stroke"
                    />
                  )}
                </svg>
              ) : (
                <p>プレビューを表示できません。</p>
              )}
              {preview.preview_truncated && (
                <span>表示用に一部の線だけを描画しています。</span>
              )}
            </div>
            <dl className="svg-import-metadata">
              <div><dt>ファイル</dt><dd>{preview.file_name}</dd></div>
              <div>
                <dt>線分</dt>
                <dd>{preview.source_segment_count.toLocaleString()}本</dd>
              </div>
              <div>
                <dt>線種候補</dt>
                <dd>{preview.style_groups.length.toLocaleString()}組</dd>
              </div>
              <div>
                <dt>外周候補</dt>
                <dd>{preview.boundary_candidates.length.toLocaleString()}件</dd>
              </div>
              <div>
                <dt>viewBox</dt>
                <dd>{formatSvgViewBox(preview.root_view_box)}</dd>
              </div>
              <div>
                <dt>SVG記載の実寸</dt>
                <dd>{formatSvgPhysicalSize(preview.root_physical_size)}</dd>
              </div>
            </dl>
          </div>

          <div className="svg-import-fields">
            <label className="dialog-field">
              <span>作品名</span>
              <input
                ref={nameInputRef}
                value={name}
                maxLength={240}
                disabled={busy}
                aria-invalid={!nameIsValid}
                aria-describedby={!nameIsValid ? 'svg-import-name-help' : undefined}
                onChange={(event) => setName(event.target.value)}
              />
              {!nameIsValid && (
                <small id="svg-import-name-help">
                  制御文字を含まない120文字以内の名前が必要です。
                </small>
              )}
            </label>
            <label className="dialog-field">
              <span>1 SVG単位の長さ</span>
              <span className="number-with-unit">
                <input
                  value={scaleInput}
                  type="number"
                  min="0"
                  max="1000000000"
                  step="any"
                  disabled={busy}
                  aria-invalid={scale === null}
                  aria-describedby="svg-import-scale-help"
                  onChange={(event) => {
                    setScaleInput(event.target.value)
                    invalidateValidation()
                  }}
                />
                mm
              </span>
              <small id="svg-import-scale-help">
                {preview.default_mm_per_unit === null
                  ? '物理単位を一意に決められないため、実寸への換算値を指定してください。'
                  : 'SVGの単位とviewBoxから算出した値です。必要なら変更できます。'}
              </small>
            </label>
          </div>

          <section className="svg-import-boundary" aria-labelledby="svg-import-boundary-title">
            <h3 id="svg-import-boundary-title">用紙外周</h3>
            <p>最大の輪郭を自動採用せず、紙として使う外周を明示してください。</p>
            <label className="dialog-field">
              <span>外周の指定方法</span>
              <select
                value={boundarySelectionValue(boundarySelection)}
                disabled={busy}
                aria-invalid={!boundaryIsValid}
                aria-describedby={
                  boundarySelection !== undefined && !boundaryIsValid
                    ? 'svg-import-boundary-error'
                    : undefined
                }
                onChange={(event) => selectBoundary(event.target.value)}
              >
                <option value="">選択してください</option>
                <option value="groups">下の線種割当で「用紙境界」を指定</option>
                {preview.boundary_candidates.map((candidate, index) => (
                  <option key={candidate.candidate_id} value={String(candidate.candidate_id)}>
                    {boundaryCandidateLabel(candidate, index)}
                  </option>
                ))}
              </select>
            </label>
            {boundarySelection !== undefined && !boundaryIsValid && (
              <p id="svg-import-boundary-error" className="svg-import-attention" role="status">
                {boundarySelection === null
                  ? '少なくとも1組の線を「用紙境界」へ割り当ててください。'
                  : '閉じた輪郭を使う場合、線種側へ用紙境界を重ねて指定できません。'}
              </p>
            )}
            {validationMatches && (
              <p className="dialog-note">
                Rust検証済みの用紙寸法: {formatSvgNumber(validation.width_mm)} ×{' '}
                {formatSvgNumber(validation.height_mm)} mm
              </p>
            )}
            {boundaryIsValid && !validationMatches && (
              <p className="dialog-note">
                現在の線種割当と縮尺で外周を検証し、取込後の用紙寸法を確認してください。
              </p>
            )}
            <button
              type="button"
              disabled={!canValidate}
              onClick={validateSettings}
            >
              {busy
                ? '外周を検証中…'
                : validationMatches
                  ? '外周と寸法を再検証'
                  : '外周と寸法を検証'}
            </button>
            {validationMatches && (
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={boundaryConfirmed}
                  disabled={busy}
                  onChange={(event) => setBoundaryConfirmed(event.target.checked)}
                />
                Rustで検証済みの境界と寸法を、この作品の用紙外周として使用する
              </label>
            )}
          </section>

          <section className="svg-import-mapping" aria-labelledby="svg-import-mapping-title">
            <h3 id="svg-import-mapping-title">色・線種・属性の割当</h3>
            <p>色だけに頼らず、破線、class、レイヤー、属性を併記します。</p>
            <div className="svg-import-mapping-list">
              {preview.style_groups.map((group, index) => {
                const sourceColor = safeSvgStrokeColor(group.stroke_color)
                const sourceLineCap = isSvgImportLineCap(group.line_cap)
                  ? group.line_cap
                  : undefined
                return (
                  <label key={group.group_id}>
                    <span className="svg-import-style-description">
                      <span className="svg-import-style-samples" aria-hidden="true">
                        <span
                          className="svg-import-style-swatch"
                          style={sourceColor ? { backgroundColor: sourceColor } : undefined}
                        />
                        <svg className="svg-import-dash-swatch" viewBox="0 0 40 8">
                          <line
                            x1="1"
                            y1="4"
                            x2="39"
                            y2="4"
                            style={{
                              ...(sourceColor ? { stroke: sourceColor } : {}),
                              ...(sourceLineCap ? { strokeLinecap: sourceLineCap } : {}),
                            }}
                            strokeDasharray={group.dash_array ?? undefined}
                            strokeLinecap={sourceLineCap}
                          />
                        </svg>
                      </span>
                      <span>
                        <b>
                          線種候補 {index + 1} · {group.element_count.toLocaleString()}要素 /{' '}
                          {group.segment_count.toLocaleString()}線分
                        </b>
                        <small>{svgImportStyleLabel(group)}</small>
                        <small className="svg-import-loss-badge">表示属性は取込後に保存しません</small>
                      </span>
                    </span>
                    <select
                      value={mapping[String(group.group_id)] ?? ''}
                      disabled={busy}
                      aria-label={`線種候補 ${index + 1} の割当`}
                      aria-invalid={!mapping[String(group.group_id)]}
                      aria-describedby={
                        !mapping[String(group.group_id)] ? 'svg-import-mapping-error' : undefined
                      }
                  onChange={(event) => {
                    const value = event.target.value as SvgImportTarget | ''
                    invalidateValidation()
                    setMapping((current) => ({
                          ...current,
                          [String(group.group_id)]: value || undefined,
                        }))
                      }}
                    >
                      <option value="">選択してください</option>
                      {svgImportTargetOptions(
                        typeof boundarySelection === 'number' ? boundarySelection : null,
                      ).map((option) => (
                        <option key={option.value} value={option.value}>{option.label}</option>
                      ))}
                    </select>
                  </label>
                )
              })}
            </div>
            {unresolved.length > 0 && (
              <p id="svg-import-mapping-error" className="svg-import-attention" role="status">
                未選択の線種候補: {unresolved
                  .map((group) => preview.style_groups.indexOf(group) + 1)
                  .join('、')}
              </p>
            )}
          </section>

          {hasValidatedCuts && (
            <section className="svg-import-cut-confirmation" aria-labelledby="svg-import-cut-title">
              <h3 id="svg-import-cut-title">切断を許可</h3>
              <p>
                「切断線」として割り当てた線を残すため、取込後の作品では切断を許可します。
              </p>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={cuttingAllowedConfirmed}
                  disabled={busy}
                  onChange={(event) => setCuttingAllowedConfirmed(event.target.checked)}
                />
                この作品で切断を許可する
              </label>
            </section>
          )}

          {preview.warnings.length > 0 && (
            <section className="svg-import-warnings" aria-labelledby="svg-import-warnings-title">
              <h3 id="svg-import-warnings-title">取り込まれない・変更される情報</h3>
              <ul>
                {preview.warnings.map((warning, index) => <li key={index}>{warning}</li>)}
              </ul>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={warningsAcknowledged}
                  disabled={busy}
                  onChange={(event) => setWarningsAcknowledged(event.target.checked)}
                />
                上記を確認し、直線の展開図として取り込む
              </label>
            </section>
          )}

          {error && <p className="dialog-error" role="alert">{error}</p>}
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>キャンセル</button>
          <button type="button" className="primary" disabled={!canImport} onClick={submit}>
            {busy ? '取込中…' : '取り込む'}
          </button>
        </footer>
      </section>
    </div>
  )
}

function boundarySelectionValue(selection: number | null | undefined) {
  if (selection === undefined) return ''
  return selection === null ? 'groups' : String(selection)
}

function boundaryCandidateLabel(candidate: SvgBoundaryCandidate, index: number) {
  const source = candidate.kind === 'view_box'
    ? 'SVGページ矩形を生成'
    : `${svgBoundaryCandidateKindLabel(candidate.kind)} ${index + 1}`
  const width = Number.isFinite(candidate.width) ? candidate.width.toLocaleString() : '?'
  const height = Number.isFinite(candidate.height) ? candidate.height.toLocaleString() : '?'
  return `${source} · ${candidate.segment_count.toLocaleString()}辺 · ${width} × ${height}単位`
}

function svgBoundaryCandidateKindLabel(
  kind: Exclude<SvgBoundaryCandidate['kind'], 'view_box'>,
) {
  switch (kind) {
    case 'polygon':
      return 'polygon由来の閉じた輪郭'
    case 'polyline':
      return 'polyline由来の閉じた輪郭'
    case 'rectangle':
      return 'rect由来の閉じた輪郭'
    case 'closed_path':
      return 'path由来の閉じた輪郭'
  }
}

function formatSvgViewBox(viewBox: SvgImportPreview['root_view_box']) {
  if (!viewBox) return '記載なし'
  return [
    formatSvgNumber(viewBox.x),
    formatSvgNumber(viewBox.y),
    formatSvgNumber(viewBox.width),
    formatSvgNumber(viewBox.height),
  ].join(' ')
}

function formatSvgPhysicalSize(size: SvgImportPreview['root_physical_size']) {
  const width = size.width_millimetres
  const height = size.height_millimetres
  if (
    width === null
    && height === null
    && size.width_unit === null
    && size.height_unit === null
  ) return '記載なし'
  return `${formatSvgRootLength(width, size.width_unit)} × ${
    formatSvgRootLength(height, size.height_unit)
  }`
}

function formatSvgRootLength(
  millimetres: number | null,
  unit: SvgImportPreview['root_physical_size']['width_unit'],
) {
  const value = millimetres === null ? '?' : `${formatSvgNumber(millimetres)} mm`
  return unit === null ? value : `${value}（元: ${svgRootUnitLabel(unit)}）`
}

function svgRootUnitLabel(unit: NonNullable<SvgImportPreview['root_physical_size']['width_unit']>) {
  return unit === 'unitless' ? '単位なし' : unit === 'percent' ? '%' : unit
}

function formatSvgNumber(value: number) {
  return Number.isFinite(value)
    ? value.toLocaleString(undefined, { maximumSignificantDigits: 12 })
    : '?'
}
