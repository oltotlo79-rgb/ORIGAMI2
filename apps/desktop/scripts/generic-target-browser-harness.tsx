import { createRoot } from 'react-dom/client'
import { useRef, useState } from 'react'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList.tsx'
import { finishBeginnerGridCancellation, runBeginnerGridApplyWorkflow } from '../src/lib/beginnerGridWorkflow.ts'
import '../src/App.css'

const bindings = [
  { id: 1, count: 4, symmetry: 'bilateral', length_tenths_mm: 270, thickness_tenths_mm: 50,
    position_tenths_mm: [0, 0, 0], direction_milli: [0, 1000, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 50 },
  { id: 2, count: 2, symmetry: 'bilateral', length_tenths_mm: 220, thickness_tenths_mm: 40,
    position_tenths_mm: [0, 0, 0], direction_milli: [1000, 0, 0], curvature_degrees: 0,
    joint: 'fixed', motion_degrees: [0, 0], side: 'either', priority: 60 },
] as const
function Harness() {
  const [recognized, setRecognized] = useState(false), [preview, setPreview] = useState(false)
  const [status, setStatus] = useState('Waiting for image or GLB'), [applied, setApplied] = useState(false)
  const evaluate = useRef<HTMLButtonElement>(null)
  const focus = () => requestAnimationFrame(() => evaluate.current?.focus())
  const recognize = (source: string) => { setRecognized(true); setPreview(false); setStatus(`${source} recognized two bounded bindings`) }
  return <main><h1>Bounded generic target</h1>
    <button onClick={() => recognize('Image')}>Recognize mixed target image</button>
    <button onClick={() => recognize('GLB')}>Recognize mixed target GLB</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: target exceeds eight bindings') }}>Try oversized target</button>
    <p role="status">{status}</p>
    {recognized && <GenericTargetBindingList locale="en" protrusions={[...bindings]} />}
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
