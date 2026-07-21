import { createRoot } from 'react-dom/client'
import { useRef, useState } from 'react'
import { CompleteInsectBindingList } from '../src/components/CompleteInsectBindingList.tsx'
import { BeginnerGridProgressStatus } from '../src/components/BeginnerGridProgressStatus.tsx'
import { finishBeginnerGridCancellation, runBeginnerGridApplyWorkflow } from '../src/lib/beginnerGridWorkflow.ts'
import '../src/App.css'

const target = (id: number, direction: [number, number, number], y: number) => ({
  id, count: 2, length_tenths_mm: id * 90, thickness_tenths_mm: id * 9,
  position_tenths_mm: [0, y, 0] as [number, number, number], direction_milli: direction,
  symmetry: 'bilateral' as const, curvature_degrees: 0, joint: 'fixed' as const,
  motion_degrees: [0, 0] as [number, number], side: 'either' as const, priority: 50,
})
const bindings = [target(1, [1000, 0, 0], 0), target(2, [0, -1000, 0], 0),
  target(3, [1000, 0, 0], -30), target(4, [1000, 0, 0], 0), target(5, [1000, 0, 0], 30)]

function Harness() {
  const [recognized, setRecognized] = useState(false)
  const [preview, setPreview] = useState(false)
  const [busy, setBusy] = useState(false)
  const [applied, setApplied] = useState(false)
  const [status, setStatus] = useState('Waiting for an insect image or GLB')
  const evaluateRef = useRef<HTMLButtonElement>(null)
  const focus = () => requestAnimationFrame(() => evaluateRef.current?.focus())
  const recognize = (source: string) => {
    setRecognized(true); setPreview(false); setApplied(false)
    setStatus(`${source} recognized with five canonical pair bindings`)
  }
  return <main>
    <h1>Complete insect candidate</h1>
    <button onClick={() => recognize('Image')}>Recognize complete insect image</button>
    <button onClick={() => recognize('GLB')}>Recognize complete insect GLB</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected asymmetric insect pair') }}>
      Try asymmetric insect pair
    </button>
    <p role="status">{status}</p>
    {recognized && <CompleteInsectBindingList locale="en" protrusions={bindings} />}
    <button ref={evaluateRef} onClick={() => {
      if (recognized) { setBusy(true); setPreview(true); setStatus('Complete insect grid ready') }
    }}>Evaluate complete insect grid</button>
    <BeginnerGridProgressStatus locale="en" busy={busy} enumerated={27} checked={3}
      onCancel={() => {
        setBusy(false); setStatus('Complete insect grid canceled')
        finishBeginnerGridCancellation(() => setPreview(false), focus)
      }} />
    {preview && <section aria-label="Complete insect candidate preview">
      <p>Global flat-foldability proven</p>
      <button onClick={() => { setPreview(false); setBusy(false); setStatus('Stale insect candidate replaced') }}>
        Replace insect reference
      </button>
      <button onClick={() => void runBeginnerGridApplyWorkflow({
        confirm: () => true, apply: async () => true,
        clearPreview: () => setPreview(false), restoreFocus: focus,
      }).then((ok) => { if (ok) { setBusy(false); setApplied(true); setStatus('Complete insect applied') } })}>
        Confirm and apply complete insect
      </button>
    </section>}
    {applied && <section aria-label="Complete insect history">
      <button onClick={() => setStatus('Complete insect apply undone')}>Undo complete insect</button>
      <button onClick={() => setStatus('Complete insect apply redone')}>Redo complete insect</button>
      <button onClick={() => setStatus('Complete insect saved and reopened')}>Save and reopen complete insect</button>
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
