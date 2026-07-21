import { createRoot } from 'react-dom/client'
import { useRef, useState } from 'react'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList.tsx'
import { ProtrusionDimensionEditor } from '../src/components/ProtrusionDimensionEditor.tsx'
import { GenericBodyOutlineEditor } from '../src/components/GenericBodyOutlineEditor.tsx'
import { finishBeginnerGridCancellation, runBeginnerGridApplyWorkflow } from '../src/lib/beginnerGridWorkflow.ts'
import type { BeginnerGenerationConstraintsV1 } from '../src/lib/coreClient.ts'
import '../src/App.css'

const initialBindings: NonNullable<BeginnerGenerationConstraintsV1['protrusions']> = [
  { id: 1, count: 1, symmetry: 'none', length_tenths_mm: 270, thickness_tenths_mm: 50,
    position_tenths_mm: [0, 0, 0], direction_milli: [0, 1000, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 50 },
  { id: 2, count: 2, symmetry: 'bilateral', length_tenths_mm: 220, thickness_tenths_mm: 40,
    position_tenths_mm: [0, 0, 0], direction_milli: [1000, 0, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 60 },
]
function Harness() {
  const [recognized, setRecognized] = useState(false), [preview, setPreview] = useState(false)
  const [status, setStatus] = useState('Waiting for image or GLB'), [applied, setApplied] = useState(false)
  const [bindings, setBindings] = useState([...initialBindings])
  const [kinds, setKinds] = useState<Array<'leg' | 'horn' | 'ear' | 'wing' | 'fin' | 'antenna' | 'tail'>>(['tail', 'fin'])
  const [outline, setOutline] = useState<Array<[number, number]>>([])
  const evaluate = useRef<HTMLButtonElement>(null)
  const focus = () => requestAnimationFrame(() => evaluate.current?.focus())
  const canonicalize = (targets: typeof bindings) => targets.map(
    (target, index) => ({ ...target, id: index + 1 }),
  )
  const move = (index: number, offset: -1 | 1) => setBindings((current) => {
    const destination = index + offset
    if (destination < 0 || destination >= current.length) return current
    const moved = [...current]
    ;[moved[index], moved[destination]] = [moved[destination]!, moved[index]!]
    setKinds((currentKinds) => {
      const movedKinds = [...currentKinds]
      ;[movedKinds[index], movedKinds[destination]] = [movedKinds[destination]!, movedKinds[index]!]
      return movedKinds
    })
    return canonicalize(moved)
  })
  const recognize = (source: string) => {
    setBindings(initialBindings.map((target) => ({ ...target })))
    setKinds(['tail', 'fin'])
    setRecognized(true); setPreview(false); setStatus(`${source} recognized two bounded bindings`)
  }
  return <main><h1>Bounded generic target</h1>
    <button onClick={() => recognize('Empty generic target')}>Create empty generic target</button>
    <button onClick={() => recognize('Image')}>Recognize mixed target image</button>
    <button onClick={() => recognize('GLB')}>Recognize mixed target GLB</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: target exceeds eight bindings') }}>Try oversized target</button>
    <p role="status">{status}</p>
    {recognized && <GenericBodyOutlineEditor locale="en" points={outline} onChange={setOutline} />}
    {recognized && <button disabled={bindings.length >= 8} onClick={() => {
      setBindings((current) => canonicalize([...current, { ...initialBindings[0]!, id: current.length + 1 }]))
      setKinds((current) => [...current, 'tail'])
    }}>Add binding</button>}
    {recognized && <GenericTargetBindingList locale="en" protrusions={[...bindings]} />}
    {recognized && <ul aria-label="Editable generic target dimensions">{bindings.map((target, index) =>
      <ProtrusionDimensionEditor key={target.id} locale="en" target={target}
        kind={kinds[index] ?? 'tail'} onKindChange={(kind) => setKinds((current) =>
          current.map((item, kindIndex) => kindIndex === index ? kind : item))}
        onChange={(changed) => setBindings((current) => current.map((item) => item.id === changed.id ? changed : item))}
        onRemove={() => setBindings((current) => {
          if (current.length <= 2) return current
          setKinds((currentKinds) => currentKinds.filter((_, kindIndex) => kindIndex !== index))
          return canonicalize(current.filter((item) => item.id !== target.id))
        })}
        canRemove={bindings.length > 2}
        canMoveUp={index > 0} canMoveDown={index + 1 < bindings.length}
        onMoveUp={() => move(index, -1)} onMoveDown={() => move(index, 1)} />
    )}</ul>}
    <button ref={evaluate} onClick={() => { if (recognized) { setPreview(true); setStatus('Generic target grid ready') } }}>Evaluate generic target grid</button>
    {preview && <section aria-label="Generic target candidate preview"><p>Global flat-foldability proven</p>
      <button onClick={() => { setPreview(false); setStatus('Stale generic target replaced') }}>Replace recognized target</button>
      <button onClick={() => { finishBeginnerGridCancellation(() => setPreview(false), focus); setStatus('Generic target grid canceled') }}>Cancel generic target grid</button>
      <button onClick={() => void runBeginnerGridApplyWorkflow({ confirm: () => true, apply: async () => true,
        clearPreview: () => setPreview(false), restoreFocus: focus }).then((ok) => { if (ok) { setApplied(true); setStatus('Generic target applied') } })}>Confirm and apply generic target</button>
    </section>}
    {applied && <section aria-label="Generic target history">
      <button onClick={() => setStatus('Generic target undone')}>Undo generic target</button>
      <button onClick={() => setStatus('Generic target redone')}>Redo generic target</button>
      <button onClick={() => setStatus('Generic target saved and reopened')}>Save and reopen generic target</button>
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
