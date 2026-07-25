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
import { SVG_IMPORT_DIALOG_TEXT as TEXT } from '../lib/svgImportDialogText.ts'
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
                <dt>{selectLocalizedText(locale, TEXT.viewBoxLabel)}</dt>
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
                {selectLocalizedText(locale, TEXT.millimetresUnit)}
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
                    .join(selectLocalizedText(locale, TEXT.listSeparator)),
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
    : `${formatSvgNumber(millimetres, locale)} ${
      selectLocalizedText(locale, TEXT.millimetresUnit)
    }`
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
  return locale === 'en' && value === TEXT.selectedFileFallback.ja
    ? TEXT.selectedFileFallback.en
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
