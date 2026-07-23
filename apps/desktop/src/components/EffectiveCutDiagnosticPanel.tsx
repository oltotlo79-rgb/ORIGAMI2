import { useEffect, useRef, useState } from 'react'
import {
  inspectEffectiveCutReadOnlyV1,
  listEffectiveCutCandidatesV1,
  type EffectiveCutCandidateListResponseV1,
  type EffectiveCutReadOnlyResponseV1,
  type ProjectSnapshot,
} from '../lib/coreClient.ts'
import { useLocale, type LocaleStore } from '../lib/i18n.ts'

type Props = Readonly<{ snapshot: ProjectSnapshot; localeStore?: LocaleStore }>

export function EffectiveCutDiagnosticPanel({ snapshot, localeStore }: Props) {
  const locale = useLocale(localeStore)
  const ja = locale === 'ja'
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
    <section aria-label={ja ? '有効カット診断' : 'Effective-cut diagnostic'}>
      <h3>{ja ? '有効カット診断（読み取り専用）' : 'Effective-cut diagnostic (read-only)'}</h3>
      <p>{ja
        ? '候補の面積はその成分単体です。依存成分は閉包数として別に表示します。'
        : 'Area is for the candidate component alone; dependent components are reported as closure counts.'}</p>
      {status === 'loading' && <p role="status">{ja ? '候補を取得中…' : 'Loading candidates…'}</p>}
      {status === 'idle' && (
        <button type="button" onClick={reload}>{ja ? '候補を再取得' : 'Reload candidates'}</button>
      )}
      {status === 'error' && (
        <p role="alert">
          {ja ? '現在の編集内容では診断できません。' : 'Diagnostics are unavailable for the current edit.'}
          {' '}<button type="button" onClick={reload}>{ja ? '再取得' : 'Reload'}</button>
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
                setSelected((previous) => {
                  const next = new Set(previous)
                  if (checked) next.add(key)
                  else next.delete(key)
                  setResult(null)
                  return next
                })
              }}
            />
            {ja ? `候補 ${index + 1}` : `Candidate ${index + 1}`}
            {' · '}{candidate.faceCount} {ja ? '面' : 'faces'}
            {' · '}{candidate.areaSquareMm} mm²
            {' · '}{ja ? '除去範囲' : 'removal closure'} {candidate.closureComponentCount}
            {candidate.nestedDependencyCount > 0
              ? ` (+${candidate.nestedDependencyCount} ${ja ? '依存成分' : 'dependencies'})`
              : ''}
          </label>
        )
      })}
      <button type="button" disabled={!listing || selected.size === 0 || status !== 'ready'} onClick={run}>
        {status === 'running' ? (ja ? '診断中…' : 'Running…') : (ja ? '選択を診断' : 'Diagnose selection')}
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
          {ja ? 'キャンセル' : 'Cancel'}
        </button>
      )}
      {result && (
        <p role="status">
          {ja ? '平面ペア' : 'Source-flat pairs'}: {result.sourceFlatPairCount};
          {' '}{ja ? '未確定' : 'indeterminate'}: {result.indeterminatePairs};
          {' '}{ja ? '複数ヒンジ経路未証明' : 'multi-hinge corridor unproved'}:
          {' '}{result.multiHingeUnionCorridorUnprovedPairs}
        </p>
      )}
    </section>
  )
}
