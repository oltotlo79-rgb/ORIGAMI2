import { useEffect, useRef, useState } from 'react'
import {
  inspectEffectiveCutReadOnlyV1,
  listEffectiveCutCandidatesV1,
  type EffectiveCutCandidateListResponseV1,
  type EffectiveCutReadOnlyResponseV1,
  type ProjectSnapshot,
} from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'
import {
  EFFECTIVE_CUT_DIAGNOSTIC_PANEL_TEXT as TEXT,
} from '../lib/effectiveCutDiagnosticPanelText.ts'

type Props = Readonly<{ snapshot: ProjectSnapshot; localeStore?: LocaleStore }>

export function EffectiveCutDiagnosticPanel({ snapshot, localeStore }: Props) {
  const locale = useLocale(localeStore)
  const generation = useRef(0)
  const [listing, setListing] = useState<EffectiveCutCandidateListResponseV1 | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<EffectiveCutReadOnlyResponseV1 | null>(null)
  const [status, setStatus] = useState<'idle' | 'loading' | 'ready' | 'running' | 'error'>('loading')
  const binding = {
    expectedProjectInstanceId: snapshot.project_instance_id,
    expectedProjectId: snapshot.project_id,
    expectedRevision: snapshot.revision,
    expectedFoldModelFingerprint: snapshot.fold_model_fingerprint,
  }
  const reload = () => {
    const current = ++generation.current
    setStatus('loading')
    setListing(null)
    setSelected(new Set())
    setResult(null)
    void listEffectiveCutCandidatesV1(binding).then((value) => {
      if (generation.current !== current) return
      setListing(value)
      setStatus('ready')
    }, () => {
      if (generation.current === current) setStatus('error')
    })
  }
  useEffect(() => {
    reload()
    return () => { generation.current += 1 }
    // Binding fields intentionally invalidate every in-flight read after edits/project replacement.
  }, [
    snapshot.project_instance_id,
    snapshot.project_id,
    snapshot.revision,
    snapshot.fold_model_fingerprint,
  ])
  const run = () => {
    if (!listing || selected.size === 0 || status !== 'ready') return
    const current = ++generation.current
    setStatus('running')
    const requestedComponentKeys = listing.candidates
      .filter((candidate) => selected.has(candidate.componentKey.join(',')))
      .map((candidate) => [...candidate.componentKey])
    void inspectEffectiveCutReadOnlyV1({ ...binding, requestedComponentKeys }).then((value) => {
      if (generation.current !== current) return
      setResult(value)
      setStatus('ready')
    }, () => {
      if (generation.current === current) setStatus('error')
    })
  }
  return (
    <section aria-label={selectLocalizedText(locale, TEXT.ariaLabel)}>
      <h3>{selectLocalizedText(locale, TEXT.title)}</h3>
      <p>{selectLocalizedText(locale, TEXT.explanation)}</p>
      {status === 'loading' && (
        <p role="status">{selectLocalizedText(locale, TEXT.loading)}</p>
      )}
      {status === 'idle' && (
        <button type="button" onClick={reload}>
          {selectLocalizedText(locale, TEXT.reloadCandidates)}
        </button>
      )}
      {status === 'error' && (
        <p role="alert">
          {selectLocalizedText(locale, TEXT.unavailable)}
          {' '}
          <button type="button" onClick={reload}>
            {selectLocalizedText(locale, TEXT.reload)}
          </button>
        </p>
      )}
      {listing?.candidates.map((candidate, index) => {
        const key = candidate.componentKey.join(',')
        return (
          <label key={key}>
            <input
              type="checkbox"
              checked={selected.has(key)}
              disabled={status !== 'ready'}
              onChange={(event) => {
                const checked = event.currentTarget.checked
                setResult(null)
                setSelected((previous) => {
                  const next = new Set(previous)
                  if (checked) next.add(key)
                  else next.delete(key)
                  return next
                })
              }}
            />
            {formatLocalizedText(locale, TEXT.candidate, {
              index: index + 1,
            })}
            {' · '}
            {formatLocalizedText(locale, TEXT.faceCount, {
              count: candidate.faceCount,
            })}
            {' · '}{candidate.areaSquareMm} mm²
            {' · '}
            {selectLocalizedText(locale, TEXT.removalClosure)}
            {' '}{candidate.closureComponentCount}
            {candidate.nestedDependencyCount > 0
              ? formatLocalizedText(locale, TEXT.dependencies, {
                  count: candidate.nestedDependencyCount,
                })
              : ''}
          </label>
        )
      })}
      <button type="button" disabled={!listing || selected.size === 0 || status !== 'ready'} onClick={run}>
        {selectLocalizedText(
          locale,
          status === 'running' ? TEXT.running : TEXT.diagnoseSelection,
        )}
      </button>
      {(status === 'loading' || status === 'running') && (
        <button
          type="button"
          onClick={() => {
            generation.current += 1
            setResult(null)
            setStatus(listing ? 'ready' : 'idle')
          }}
        >
          {selectLocalizedText(locale, TEXT.cancel)}
        </button>
      )}
      {result && (
        <p role="status">
          {selectLocalizedText(locale, TEXT.sourceFlatPairs)}:{' '}
          {result.sourceFlatPairCount};
          {' '}{selectLocalizedText(locale, TEXT.indeterminate)}:{' '}
          {result.indeterminatePairs};
          {' '}{selectLocalizedText(locale, TEXT.multiHingeCorridorUnproved)}:
          {' '}{result.multiHingeUnionCorridorUnprovedPairs}
        </p>
      )}
    </section>
  )
}
