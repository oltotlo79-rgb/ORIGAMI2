import { useEffect, useMemo, useRef, useState } from 'react'
import {
  initialSvgImportMapping,
  isValidSvgImportName,
  isSvgImportLineCap,
  localizedSvgImportTargetOptions,
  parseSvgImportScale,
  safeSvgStrokeColor,
  svgImportBoundaryIsValid,
  svgImportPreviewBounds,
  svgImportStyleLabel,
  svgImportWarningText,
  unresolvedSvgImportGroups,
  type SvgBoundaryCandidate,
  type SvgImportMapping,
  type SvgImportPreview,
  type SvgImportSettings,
  type SvgImportSettingsDraft,
  type SvgImportSettingsValidation,
  type SvgImportTarget,
} from '../lib/svgImport'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocalizedText,
} from '../lib/i18n'

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
  const locale = useLocale()
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
            <span className="dialog-eyebrow">
              {selectLocalizedText(locale, TEXT.eyebrow)}
            </span>
            <h2 id="svg-import-title">
              {selectLocalizedText(locale, TEXT.title)}
            </h2>
          </div>
          <button
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label={selectLocalizedText(locale, TEXT.close)}
          >
            ×
          </button>
        </header>

        <div className="svg-import-dialog-body">
          <p id="svg-import-description" className="dialog-note">
            {selectLocalizedText(locale, TEXT.description)}
          </p>

          <div className="svg-import-overview">
            <div className="svg-import-preview">
              {bounds ? (
                <svg
                  viewBox={`${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`}
                  role="img"
                  aria-label={selectLocalizedText(locale, TEXT.previewLabel)}
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
                <p>{selectLocalizedText(locale, TEXT.previewUnavailable)}</p>
              )}
              {preview.preview_truncated && (
                <span>{selectLocalizedText(locale, TEXT.previewTruncated)}</span>
              )}
            </div>
            <dl className="svg-import-metadata">
              <div>
                <dt>{selectLocalizedText(locale, TEXT.fileLabel)}</dt>
                <dd>{formatSvgSourceFileLabel(preview.file_name, locale)}</dd>
              </div>
              <div>
                <dt>{selectLocalizedText(locale, TEXT.segmentsLabel)}</dt>
                <dd>{formatSegmentCount(preview.source_segment_count, locale)}</dd>
              </div>
              <div>
                <dt>{selectLocalizedText(locale, TEXT.styleGroupsLabel)}</dt>
                <dd>{formatStyleGroupCount(preview.style_groups.length, locale)}</dd>
              </div>
              <div>
                <dt>{selectLocalizedText(locale, TEXT.boundaryCandidatesLabel)}</dt>
                <dd>
                  {formatCandidateCount(preview.boundary_candidates.length, locale)}
                </dd>
              </div>
              <div>
                <dt>viewBox</dt>
                <dd>{formatSvgViewBox(preview.root_view_box, locale)}</dd>
              </div>
              <div>
                <dt>{selectLocalizedText(locale, TEXT.physicalSizeLabel)}</dt>
                <dd>{formatSvgPhysicalSize(preview.root_physical_size, locale)}</dd>
              </div>
            </dl>
          </div>

          <div className="svg-import-fields">
            <label className="dialog-field">
              <span>{selectLocalizedText(locale, TEXT.projectName)}</span>
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
                  {selectLocalizedText(locale, TEXT.projectNameHelp)}
                </small>
              )}
            </label>
            <label className="dialog-field">
              <span>{selectLocalizedText(locale, TEXT.scaleLabel)}</span>
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
                  ? selectLocalizedText(locale, TEXT.scaleRequiredHelp)
                  : selectLocalizedText(locale, TEXT.scaleDetectedHelp)}
              </small>
            </label>
          </div>

          <section className="svg-import-boundary" aria-labelledby="svg-import-boundary-title">
            <h3 id="svg-import-boundary-title">
              {selectLocalizedText(locale, TEXT.boundaryTitle)}
            </h3>
            <p>{selectLocalizedText(locale, TEXT.boundaryDescription)}</p>
            <label className="dialog-field">
              <span>{selectLocalizedText(locale, TEXT.boundaryMethod)}</span>
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
                <option value="">
                  {selectLocalizedText(locale, TEXT.selectPrompt)}
                </option>
                <option value="groups">
                  {selectLocalizedText(locale, TEXT.boundaryFromGroups)}
                </option>
                {preview.boundary_candidates.map((candidate, index) => (
                  <option key={candidate.candidate_id} value={String(candidate.candidate_id)}>
                    {boundaryCandidateLabel(candidate, index, locale)}
                  </option>
                ))}
              </select>
            </label>
            {boundarySelection !== undefined && !boundaryIsValid && (
              <p id="svg-import-boundary-error" className="svg-import-attention" role="status">
                {boundarySelection === null
                  ? selectLocalizedText(locale, TEXT.boundaryGroupRequired)
                  : selectLocalizedText(locale, TEXT.boundaryConflict)}
              </p>
            )}
            {validationMatches && (
              <p className="dialog-note">
                {formatLocalizedText(locale, TEXT.validatedDimensions, {
                  width: formatSvgNumber(validation.width_mm, locale),
                  height: formatSvgNumber(validation.height_mm, locale),
                })}
              </p>
            )}
            {boundaryIsValid && !validationMatches && (
              <p className="dialog-note">
                {selectLocalizedText(locale, TEXT.validateGuidance)}
              </p>
            )}
            <button
              type="button"
              disabled={!canValidate}
              onClick={validateSettings}
            >
              {busy
                ? selectLocalizedText(locale, TEXT.validatingBoundary)
                : validationMatches
                  ? selectLocalizedText(locale, TEXT.revalidateBoundary)
                  : selectLocalizedText(locale, TEXT.validateBoundary)}
            </button>
            {validationMatches && (
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={boundaryConfirmed}
                  disabled={busy}
                  onChange={(event) => setBoundaryConfirmed(event.target.checked)}
                />
                {selectLocalizedText(locale, TEXT.confirmBoundary)}
              </label>
            )}
          </section>

          <section className="svg-import-mapping" aria-labelledby="svg-import-mapping-title">
            <h3 id="svg-import-mapping-title">
              {selectLocalizedText(locale, TEXT.mappingTitle)}
            </h3>
            <p>{selectLocalizedText(locale, TEXT.mappingDescription)}</p>
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
                          {formatLocalizedText(locale, TEXT.styleGroupSummary, {
                            index: index + 1,
                            elements: formatElementCount(group.element_count, locale),
                            segments: formatSegmentCount(group.segment_count, locale),
                          })}
                        </b>
                        <small>{svgImportStyleLabel(group, locale)}</small>
                        <small className="svg-import-loss-badge">
                          {selectLocalizedText(locale, TEXT.styleLossBadge)}
                        </small>
                      </span>
                    </span>
                    <select
                      value={mapping[String(group.group_id)] ?? ''}
                      disabled={busy}
                      aria-label={formatLocalizedText(locale, TEXT.mappingLabel, {
                        index: index + 1,
                      })}
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
                      <option value="">
                        {selectLocalizedText(locale, TEXT.selectPrompt)}
                      </option>
                      {localizedSvgImportTargetOptions(
                        typeof boundarySelection === 'number' ? boundarySelection : null,
                        locale,
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
                {formatLocalizedText(locale, TEXT.unresolvedGroups, {
                  groups: unresolved
                    .map((group) => preview.style_groups.indexOf(group) + 1)
                    .join(locale === 'en' ? ', ' : '、'),
                })}
              </p>
            )}
          </section>

          {hasValidatedCuts && (
            <section className="svg-import-cut-confirmation" aria-labelledby="svg-import-cut-title">
              <h3 id="svg-import-cut-title">
                {selectLocalizedText(locale, TEXT.cutTitle)}
              </h3>
              <p>
                {selectLocalizedText(locale, TEXT.cutDescription)}
              </p>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={cuttingAllowedConfirmed}
                  disabled={busy}
                  onChange={(event) => setCuttingAllowedConfirmed(event.target.checked)}
                />
                {selectLocalizedText(locale, TEXT.cutConfirmation)}
              </label>
            </section>
          )}

          {preview.warnings.length > 0 && (
            <section className="svg-import-warnings" aria-labelledby="svg-import-warnings-title">
              <h3 id="svg-import-warnings-title">
                {selectLocalizedText(locale, TEXT.warningsTitle)}
              </h3>
              <ul>
                {preview.warnings.map((warning, index) => (
                  <li key={index}>{svgImportWarningText(warning, locale)}</li>
                ))}
              </ul>
              <label className="dialog-check">
                <input
                  type="checkbox"
                  checked={warningsAcknowledged}
                  disabled={busy}
                  onChange={(event) => setWarningsAcknowledged(event.target.checked)}
                />
                {selectLocalizedText(locale, TEXT.warningsConfirmation)}
              </label>
            </section>
          )}

          {error && <p className="dialog-error" role="alert">{error}</p>}
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>
            {selectLocalizedText(locale, TEXT.cancel)}
          </button>
          <button type="button" className="primary" disabled={!canImport} onClick={submit}>
            {selectLocalizedText(locale, busy ? TEXT.importing : TEXT.importAction)}
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

function boundaryCandidateLabel(
  candidate: SvgBoundaryCandidate,
  index: number,
  locale: Locale,
) {
  const source = candidate.kind === 'view_box'
    ? selectLocalizedText(locale, TEXT.viewBoxCandidate)
    : formatLocalizedText(locale, TEXT.indexedCandidate, {
        kind: svgBoundaryCandidateKindLabel(candidate.kind, locale),
        index: index + 1,
      })
  const width = formatSvgNumber(candidate.width, locale)
  const height = formatSvgNumber(candidate.height, locale)
  return formatLocalizedText(locale, TEXT.boundaryCandidateSummary, {
    source,
    edges: formatEdgeCount(candidate.segment_count, locale),
    width,
    height,
  })
}

function svgBoundaryCandidateKindLabel(
  kind: Exclude<SvgBoundaryCandidate['kind'], 'view_box'>,
  locale: Locale,
) {
  switch (kind) {
    case 'polygon':
      return selectLocalizedText(locale, TEXT.polygonCandidate)
    case 'polyline':
      return selectLocalizedText(locale, TEXT.polylineCandidate)
    case 'rectangle':
      return selectLocalizedText(locale, TEXT.rectangleCandidate)
    case 'closed_path':
      return selectLocalizedText(locale, TEXT.pathCandidate)
  }
}

function formatSvgViewBox(
  viewBox: SvgImportPreview['root_view_box'],
  locale: Locale,
) {
  if (!viewBox) return selectLocalizedText(locale, TEXT.notSpecified)
  return [
    formatSvgNumber(viewBox.x, locale),
    formatSvgNumber(viewBox.y, locale),
    formatSvgNumber(viewBox.width, locale),
    formatSvgNumber(viewBox.height, locale),
  ].join(' ')
}

function formatSvgPhysicalSize(
  size: SvgImportPreview['root_physical_size'],
  locale: Locale,
) {
  const width = size.width_millimetres
  const height = size.height_millimetres
  if (
    width === null
    && height === null
    && size.width_unit === null
    && size.height_unit === null
  ) return selectLocalizedText(locale, TEXT.notSpecified)
  return `${formatSvgRootLength(width, size.width_unit, locale)} × ${
    formatSvgRootLength(height, size.height_unit, locale)
  }`
}

function formatSvgRootLength(
  millimetres: number | null,
  unit: SvgImportPreview['root_physical_size']['width_unit'],
  locale: Locale,
) {
  const value = millimetres === null
    ? '?'
    : `${formatSvgNumber(millimetres, locale)} mm`
  return unit === null
    ? value
    : formatLocalizedText(locale, TEXT.originalUnit, {
        value,
        unit: svgRootUnitLabel(unit, locale),
      })
}

function svgRootUnitLabel(
  unit: NonNullable<SvgImportPreview['root_physical_size']['width_unit']>,
  locale: Locale,
) {
  return unit === 'unitless'
    ? selectLocalizedText(locale, TEXT.unitless)
    : unit === 'percent'
      ? '%'
      : unit
}

function formatSvgNumber(value: number, locale: Locale) {
  return Number.isFinite(value)
    ? value.toLocaleString(
        locale === 'en' ? 'en-US' : 'ja-JP',
        { maximumSignificantDigits: 12 },
      )
    : '?'
}

function formatSvgSourceFileLabel(value: string, locale: Locale) {
  return locale === 'en' && value === '選択したSVGファイル'
    ? 'Selected SVG file'
    : value
}

function formatSegmentCount(count: number, locale: Locale) {
  return formatLocalizedCount(count, locale, TEXT.segmentCount, TEXT.segmentCountOne)
}

function formatStyleGroupCount(count: number, locale: Locale) {
  return formatLocalizedCount(count, locale, TEXT.styleGroupCount, TEXT.styleGroupCountOne)
}

function formatCandidateCount(count: number, locale: Locale) {
  return formatLocalizedCount(count, locale, TEXT.candidateCount, TEXT.candidateCountOne)
}

function formatElementCount(count: number, locale: Locale) {
  return formatLocalizedCount(count, locale, TEXT.elementCount, TEXT.elementCountOne)
}

function formatEdgeCount(count: number, locale: Locale) {
  return formatLocalizedCount(count, locale, TEXT.edgeCount, TEXT.edgeCountOne)
}

function formatLocalizedCount(
  count: number,
  locale: Locale,
  many: LocalizedText,
  one: LocalizedText,
) {
  return formatLocalizedText(
    locale,
    locale === 'en' && count === 1 ? one : many,
    {
      count: count.toLocaleString(locale === 'en' ? 'en-US' : 'ja-JP'),
    },
  )
}

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const TEXT = Object.freeze({
  eyebrow: localized('SVG 1.1 / 2 静的線図取込', 'SVG 1.1 / 2 static line import'),
  title: localized(
    '外周・線種・縮尺を確認',
    'Review boundary, line types, and scale',
  ),
  close: localized('閉じる', 'Close'),
  description: localized(
    '元のSVGは変更しません。直線を交点で分割し、編集可能な未保存プロジェクトとして取り込みます。',
    'The source SVG is not changed. Straight lines are split at intersections and imported as an editable, unsaved project.',
  ),
  previewLabel: localized(
    '取り込むSVG線図のプレビュー',
    'Preview of the SVG line drawing to import',
  ),
  previewUnavailable: localized(
    'プレビューを表示できません。',
    'The preview cannot be displayed.',
  ),
  previewTruncated: localized(
    '表示用に一部の線だけを描画しています。',
    'Only some lines are drawn in the preview.',
  ),
  fileLabel: localized('ファイル', 'File'),
  segmentsLabel: localized('線分', 'Segments'),
  styleGroupsLabel: localized('線種候補', 'Line type candidates'),
  boundaryCandidatesLabel: localized('外周候補', 'Boundary candidates'),
  physicalSizeLabel: localized('SVG記載の実寸', 'Physical size in SVG'),
  projectName: localized('作品名', 'Project name'),
  projectNameHelp: localized(
    '制御文字を含まない120文字以内の名前が必要です。',
    'Enter a name of at most 120 characters without control characters.',
  ),
  scaleLabel: localized('1 SVG単位の長さ', 'Length of one SVG unit'),
  scaleRequiredHelp: localized(
    '物理単位を一意に決められないため、実寸への換算値を指定してください。',
    'The physical unit is ambiguous. Enter a conversion to the actual size.',
  ),
  scaleDetectedHelp: localized(
    'SVGの単位とviewBoxから算出した値です。必要なら変更できます。',
    'Calculated from the SVG unit and viewBox. You can change it if needed.',
  ),
  boundaryTitle: localized('用紙外周', 'Paper boundary'),
  boundaryDescription: localized(
    '最大の輪郭を自動採用せず、紙として使う外周を明示してください。',
    'Explicitly choose the paper boundary; the largest outline is not selected automatically.',
  ),
  boundaryMethod: localized('外周の指定方法', 'Boundary selection method'),
  selectPrompt: localized('選択してください', 'Select an option'),
  boundaryFromGroups: localized(
    '下の線種割当で「用紙境界」を指定',
    'Assign “Paper boundary” in the line types below',
  ),
  boundaryGroupRequired: localized(
    '少なくとも1組の線を「用紙境界」へ割り当ててください。',
    'Assign at least one line group to “Paper boundary”.',
  ),
  boundaryConflict: localized(
    '閉じた輪郭を使う場合、線種側へ用紙境界を重ねて指定できません。',
    'When using a closed outline, a paper boundary cannot also be assigned in the line groups.',
  ),
  validatedDimensions: localized(
    'Rust検証済みの用紙寸法: {width} × {height} mm',
    'Rust-validated paper size: {width} × {height} mm',
  ),
  validateGuidance: localized(
    '現在の線種割当と縮尺で外周を検証し、取込後の用紙寸法を確認してください。',
    'Validate the boundary with the current line assignments and scale, then review the imported paper size.',
  ),
  validatingBoundary: localized('外周を検証中…', 'Validating boundary…'),
  revalidateBoundary: localized(
    '外周と寸法を再検証',
    'Revalidate boundary and size',
  ),
  validateBoundary: localized('外周と寸法を検証', 'Validate boundary and size'),
  confirmBoundary: localized(
    'Rustで検証済みの境界と寸法を、この作品の用紙外周として使用する',
    'Use the Rust-validated boundary and dimensions as this project’s paper boundary',
  ),
  mappingTitle: localized(
    '色・線種・属性の割当',
    'Assign colors, line types, and attributes',
  ),
  mappingDescription: localized(
    '色だけに頼らず、破線、class、レイヤー、属性を併記します。',
    'Dash patterns, classes, layers, and attributes are shown so the mapping does not rely on color alone.',
  ),
  styleGroupSummary: localized(
    '線種候補 {index} · {elements} / {segments}',
    'Line type candidate {index} · {elements} / {segments}',
  ),
  styleLossBadge: localized(
    '表示属性は取込後に保存しません',
    'Display attributes will not be saved after import',
  ),
  mappingLabel: localized(
    '線種候補 {index} の割当',
    'Assignment for line type candidate {index}',
  ),
  unresolvedGroups: localized(
    '未選択の線種候補: {groups}',
    'Unassigned line type candidates: {groups}',
  ),
  cutTitle: localized('切断を許可', 'Allow cutting'),
  cutDescription: localized(
    '「切断線」として割り当てた線を残すため、取込後の作品では切断を許可します。',
    'Cutting will be allowed in the imported project so lines assigned as “Cut line” can be retained.',
  ),
  cutConfirmation: localized(
    'この作品で切断を許可する',
    'Allow cutting in this project',
  ),
  warningsTitle: localized(
    '取り込まれない・変更される情報',
    'Information that will not be imported or will be changed',
  ),
  warningsConfirmation: localized(
    '上記を確認し、直線の展開図として取り込む',
    'I reviewed the above and want to import it as a straight-line crease pattern',
  ),
  cancel: localized('キャンセル', 'Cancel'),
  importing: localized('取込中…', 'Importing…'),
  importAction: localized('取り込む', 'Import'),
  viewBoxCandidate: localized('SVGページ矩形を生成', 'Generate SVG page rectangle'),
  indexedCandidate: localized('{kind} {index}', '{kind} {index}'),
  polygonCandidate: localized(
    'polygon由来の閉じた輪郭',
    'Closed outline from polygon',
  ),
  polylineCandidate: localized(
    'polyline由来の閉じた輪郭',
    'Closed outline from polyline',
  ),
  rectangleCandidate: localized(
    'rect由来の閉じた輪郭',
    'Closed outline from rect',
  ),
  pathCandidate: localized(
    'path由来の閉じた輪郭',
    'Closed outline from path',
  ),
  boundaryCandidateSummary: localized(
    '{source} · {edges} · {width} × {height}単位',
    '{source} · {edges} · {width} × {height} units',
  ),
  notSpecified: localized('記載なし', 'Not specified'),
  originalUnit: localized('{value}（元: {unit}）', '{value} (source: {unit})'),
  unitless: localized('単位なし', 'unitless'),
  segmentCount: localized('{count}本', '{count} segments'),
  segmentCountOne: localized('{count}本', '{count} segment'),
  styleGroupCount: localized('{count}組', '{count} groups'),
  styleGroupCountOne: localized('{count}組', '{count} group'),
  candidateCount: localized('{count}件', '{count} candidates'),
  candidateCountOne: localized('{count}件', '{count} candidate'),
  elementCount: localized('{count}要素', '{count} elements'),
  elementCountOne: localized('{count}要素', '{count} element'),
  edgeCount: localized('{count}辺', '{count} edges'),
  edgeCountOne: localized('{count}辺', '{count} edge'),
})
