import {
  useEffect,
  useId,
  useRef,
  useState,
} from 'react'
import { LayerOrderViewer } from './LayerOrderViewer.tsx'
import { ProofScopeSummary } from './ProofScopeSummary.tsx'
import {
  getCurrentLayerOrderView,
  type CurrentLayerOrderView,
} from '../lib/currentLayerOrderView.ts'
import type { AssignedLocalSufficiencySummaryResponseV1 } from '../lib/coreClient.ts'

import {
  GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS,
  normalizeGlobalFlatFoldabilityTimePreset,
  type GlobalFlatFoldabilityTimePreset,
} from '../lib/globalFlatFoldability.ts'
import {
  createGlobalFlatFoldabilityPresentation,
  type GlobalFlatFoldabilityPresentationKind,
} from '../lib/globalFlatFoldabilityPresentation.ts'
import { GLOBAL_FLAT_FOLDABILITY_PANEL_TEXT as TEXT } from '../lib/globalFlatFoldabilityPanelText.ts'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'

export type GlobalFlatFoldabilityPanelProps = Readonly<{
  job: unknown
  timeLimitSeconds: GlobalFlatFoldabilityTimePreset
  startDisabled?: boolean
  onTimeLimitChange: (seconds: GlobalFlatFoldabilityTimePreset) => void
  onStart: (seconds: GlobalFlatFoldabilityTimePreset) => void
  onCancel: () => void
  localeStore?: LocaleStore
  authority?: Readonly<{ projectInstanceId: string; projectId: string; revision: number }>
  selectedFaceId?: string | null
  onSelectFace?(faceId: string | null): void
  onHoverFace?(faceId: string | null): void
  localSummary?: AssignedLocalSufficiencySummaryResponseV1 | null
  selectedVertexId?: string | null
  onSelectVertex?(vertexId: string): void
  loadLayerOrderView?(authority: {
    projectInstanceId: string
    projectId: string
    revision: number
  }): Promise<CurrentLayerOrderView>
}>

type LayerViewLoadState = Readonly<{
  terminalResultToken: unknown
  authorityKey: string | null
  status: 'idle' | 'loading' | 'ready' | 'failed'
  view: CurrentLayerOrderView | null
}>

export function GlobalFlatFoldabilityPanel({
  job,
  timeLimitSeconds,
  startDisabled = false,
  onTimeLimitChange,
  onStart,
  onCancel,
  localeStore: localeStore_ = localeStore,
  authority,
  selectedFaceId = null,
  onSelectFace,
  onHoverFace,
  localSummary = null,
  selectedVertexId = null,
  onSelectVertex,
  loadLayerOrderView = getCurrentLayerOrderView,
}: GlobalFlatFoldabilityPanelProps) {
  const locale = useLocale(localeStore_)
  const titleId = useId()
  const cautionId = useId()
  const resultLabelId = useId()
  const presentation = createGlobalFlatFoldabilityPresentation(job, locale)
  const selectedTimeLimit = normalizeGlobalFlatFoldabilityTimePreset(
    timeLimitSeconds,
  )
  const startButtonRef = useRef<HTMLButtonElement>(null)
  const cancelButtonRef = useRef<HTMLButtonElement>(null)
  const previousKindRef = useRef(presentation.kind)
  const [layerViewLoad, setLayerViewLoad] = useState<LayerViewLoadState>({
    terminalResultToken: null,
    authorityKey: null,
    status: 'idle',
    view: null,
  })
  const [selectedCell, setSelectedCell] = useState<string | null>(null)
  const [hoveredFace, setHoveredFace] = useState<string | null>(null)
  const selectedViewerFaceRef = useRef<string | null>(null)
  const selectedFaceIdRef = useRef(selectedFaceId)
  const onSelectFaceRef = useRef(onSelectFace)
  const onHoverFaceRef = useRef(onHoverFace)
  onSelectFaceRef.current = onSelectFace
  onHoverFaceRef.current = onHoverFace
  selectedFaceIdRef.current = selectedFaceId
  const authorityInstanceId = authority?.projectInstanceId
  const authorityProjectId = authority?.projectId
  const authorityRevision = authority?.revision
  const terminalResultToken = presentation.kind === 'possible'
    && presentation.layerViewAvailable
    ? job
    : null
  const authorityKey = authorityInstanceId === undefined
    || authorityProjectId === undefined
    || authorityRevision === undefined
    ? null
    : `${authorityInstanceId}:${authorityProjectId}:${authorityRevision}`
  const canLoadLayerView =
    terminalResultToken !== null
    && presentation.layerViewAvailable
    && authorityKey !== null
  const layerLoadMatchesCurrentResult = canLoadLayerView
    && layerViewLoad.terminalResultToken === terminalResultToken
    && layerViewLoad.authorityKey === authorityKey
  const layerView = layerLoadMatchesCurrentResult
    ? layerViewLoad.view
    : null
  const layerViewStatus = presentation.kind !== 'possible'
    ? 'idle'
    : !canLoadLayerView
      ? 'failed'
      : layerLoadMatchesCurrentResult
        ? layerViewLoad.status
        : 'loading'
  const hasTerminalResult = !presentation.active
    && presentation.kind !== 'idle'

  useEffect(() => {
    const previousKind = previousKindRef.current
    const wasActive = isActiveKind(previousKind)
    if (
      !wasActive
      && presentation.active
      && document.activeElement === startButtonRef.current
    ) {
      cancelButtonRef.current?.focus({ preventScroll: true })
    }
    previousKindRef.current = presentation.kind
  }, [presentation.active, presentation.kind])

  useEffect(() => {
    let current = true
    setSelectedCell(null)
    setHoveredFace(null)
    onHoverFaceRef.current?.(null)
    if (selectedViewerFaceRef.current !== null) {
      const viewerFace = selectedViewerFaceRef.current
      selectedViewerFaceRef.current = null
      if (selectedFaceIdRef.current === viewerFace) {
        onSelectFaceRef.current?.(null)
      }
    }
    if (
      terminalResultToken === null
      || authorityKey === null
      || authorityInstanceId === undefined
      || authorityProjectId === undefined
      || authorityRevision === undefined
    ) {
      setLayerViewLoad({
        terminalResultToken: null,
        authorityKey: null,
        status: 'idle',
        view: null,
      })
      return () => { current = false }
    }
    const expected = {
      projectInstanceId: authorityInstanceId,
      projectId: authorityProjectId,
      revision: authorityRevision,
    }
    setLayerViewLoad({
      terminalResultToken,
      authorityKey,
      status: 'loading',
      view: null,
    })
    void loadLayerOrderView(expected).then((result) => {
      if (current
        && result.projectInstanceId === expected.projectInstanceId
        && result.projectId === expected.projectId
        && result.revision === expected.revision) {
        setLayerViewLoad({
          terminalResultToken,
          authorityKey,
          status: 'ready',
          view: result,
        })
      } else if (current) {
        setLayerViewLoad({
          terminalResultToken,
          authorityKey,
          status: 'failed',
          view: null,
        })
      }
    }).catch(() => {
      if (current) {
        setLayerViewLoad({
          terminalResultToken,
          authorityKey,
          status: 'failed',
          view: null,
        })
      }
    })
    return () => { current = false }
  }, [
    authorityInstanceId,
    authorityKey,
    authorityProjectId,
    authorityRevision,
    loadLayerOrderView,
    terminalResultToken,
  ])

  return (
    <section
      className="global-flat-foldability-panel"
      aria-labelledby={titleId}
      aria-describedby={cautionId}
    >
      <header className="global-flat-foldability-header">
        <div>
          <span className="global-flat-foldability-eyebrow">
            {selectLocalizedText(locale, TEXT.eyebrow)}
          </span>
          <h3 id={titleId}>
            {selectLocalizedText(locale, TEXT.title)}
          </h3>
        </div>
      </header>

      <div className="global-flat-foldability-controls">
        <label>
          <span>{selectLocalizedText(locale, TEXT.timeLimit)}</span>
          <select
            value={selectedTimeLimit}
            disabled={presentation.active}
            onChange={(event) => {
              const next = Number(event.currentTarget.value)
              const normalized = normalizeGlobalFlatFoldabilityTimePreset(next)
              if (next === normalized) onTimeLimitChange(normalized)
            }}
          >
            {GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS.map((seconds) => (
              <option key={seconds} value={seconds}>
                {formatLocalizedText(locale, TEXT.seconds, { seconds })}
              </option>
            ))}
          </select>
        </label>
        <button
          ref={startButtonRef}
          type="button"
          className="global-flat-foldability-start"
          disabled={presentation.active || startDisabled}
          onClick={() => onStart(selectedTimeLimit)}
        >
          {presentation.active
            ? selectLocalizedText(locale, TEXT.checking)
            : hasTerminalResult
              ? selectLocalizedText(locale, TEXT.runAgain)
              : selectLocalizedText(locale, TEXT.start)}
        </button>
      </div>

      <div
        className={`global-flat-foldability-status is-${presentation.kind}`}
        role="group"
        aria-labelledby={resultLabelId}
        aria-busy={presentation.active}
        data-result-kind={presentation.kind}
      >
        <div className="global-flat-foldability-status-heading">
          <span
            className="global-flat-foldability-status-icon"
            aria-hidden="true"
          >
            {presentation.icon}
          </span>
          <strong id={resultLabelId}>{presentation.label}</strong>
        </div>
        <p>{presentation.detail}</p>

        {presentation.active && (
          <div className="global-flat-foldability-running">
            <p className="global-flat-foldability-phase">
              <strong>{presentation.phaseText}</strong>
              <span>{presentation.workText}</span>
            </p>
            <button
              ref={cancelButtonRef}
              type="button"
              className="global-flat-foldability-cancel"
              onClick={onCancel}
            >
              {presentation.cancelRequested
                ? selectLocalizedText(locale, TEXT.cancelRequested)
                : selectLocalizedText(locale, TEXT.cancel)}
            </button>
          </div>
        )}
      </div>

      <dl className="global-flat-foldability-summary">
        {presentation.summaryEntries.map((entry) => (
          <div key={entry.label}>
            <dt>{entry.label}</dt>
            <dd>{entry.value}</dd>
          </div>
        ))}
      </dl>

      {presentation.resultEntries.length > 0 && (
        <dl className="global-flat-foldability-result-details">
          {presentation.resultEntries.map((entry) => (
            <div key={entry.label}>
              <dt>{entry.label}</dt>
              <dd>{entry.value}</dd>
            </div>
          ))}
        </dl>
      )}

      <ProofScopeSummary
        globalJob={job}
        localSummary={localSummary}
        localeStore={localeStore_}
        selectedVertexId={selectedVertexId}
        onSelectVertex={onSelectVertex}
      />

      {layerView && layerView.cells.length > 0 && (
        <LayerOrderViewer
          locale={locale}
          scope="global-flat-result"
          cells={layerView.cells}
          selectedCell={selectedCell}
          selectedFace={selectedFaceId}
          hoveredFace={hoveredFace}
          onSelectCell={setSelectedCell}
          onSelectFace={(face) => {
            selectedViewerFaceRef.current = face
            onSelectFace?.(face)
          }}
          onHoverFace={(face) => {
            setHoveredFace(face)
            onHoverFace?.(face)
          }}
        />
      )}
      {presentation.kind === 'possible' && layerViewStatus === 'loading' && (
        <p role="status" className="global-flat-foldability-layer-loading">
          {selectLocalizedText(locale, TEXT.layerLoading)}
        </p>
      )}
      {presentation.kind === 'possible'
        && layerViewStatus === 'ready'
        && layerView?.cells.length === 0 && (
        <p role="status" className="global-flat-foldability-layer-empty">
          {selectLocalizedText(locale, TEXT.layerEmpty)}
        </p>
      )}
      {presentation.kind === 'possible' && layerViewStatus === 'failed' && (
        <p role="alert" className="global-flat-foldability-layer-unavailable">
          {selectLocalizedText(locale, TEXT.layerUnavailable)}
        </p>
      )}

      <aside
        id={cautionId}
        className="global-flat-foldability-caution"
        aria-label={selectLocalizedText(locale, TEXT.limitationsLabel)}
      >
        <strong>
          {selectLocalizedText(locale, TEXT.limitationsTitle)}
        </strong>
        <p>
          {selectLocalizedText(locale, TEXT.limitationsDetail)}
        </p>
      </aside>

      <p
        className="visually-hidden"
        role="status"
        aria-live="polite"
        aria-atomic="true"
        aria-relevant="text"
      >
        {presentation.liveText}
      </p>
    </section>
  )
}

function isActiveKind(kind: GlobalFlatFoldabilityPresentationKind) {
  return kind === 'queued' || kind === 'running'
}
