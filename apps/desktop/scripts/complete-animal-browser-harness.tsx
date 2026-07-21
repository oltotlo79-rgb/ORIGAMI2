import { createRoot } from 'react-dom/client'
import { useRef, useState } from 'react'
import { BeginnerGridProgressStatus } from '../src/components/BeginnerGridProgressStatus.tsx'
import { CompleteAnimalBindingList } from '../src/components/CompleteAnimalBindingList.tsx'
import {
  finishBeginnerGridCancellation,
  runBeginnerGridApplyWorkflow,
} from '../src/lib/beginnerGridWorkflow.ts'
import '../src/App.css'

const protrusions = [
  { id: 1, count: 1, symmetry: 'none', direction_milli: [0, -1000], length_tenths_mm: 120, thickness_tenths_mm: 18 },
  { id: 2, count: 1, symmetry: 'none', direction_milli: [1000, 0], length_tenths_mm: 180, thickness_tenths_mm: 20 },
  { id: 3, count: 2, symmetry: 'bilateral', direction_milli: [700, -700], length_tenths_mm: 80, thickness_tenths_mm: 12 },
  { id: 4, count: 4, symmetry: 'bilateral', direction_milli: [700, 700], length_tenths_mm: 240, thickness_tenths_mm: 28 },
  { id: 5, count: 2, symmetry: 'bilateral', direction_milli: [1000, 0], length_tenths_mm: 160, thickness_tenths_mm: 16 },
] as const

function Harness() {
  const [recognized, setRecognized] = useState(false)
  const [preview, setPreview] = useState(false)
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState('Waiting for an image or GLB reference')
  const [applied, setApplied] = useState(false)
  const evaluateRef = useRef<HTMLButtonElement>(null)
  const restoreFocus = () => requestAnimationFrame(() => evaluateRef.current?.focus())
  const apply = (confirmed: boolean, applied: boolean) => runBeginnerGridApplyWorkflow({
    confirm: () => confirmed,
    apply: async () => applied,
    clearPreview: () => setPreview(false),
    restoreFocus,
  })
  const recognize = (source: 'image' | 'GLB') => {
    setRecognized(true); setPreview(false); setBusy(false); setApplied(false)
    setStatus(`${source} recognized: wing pair is canonical binding 5`)
  }
  const reset = () => {
    if (!recognized) return
    setPreview(true); setBusy(true); setStatus('27 designs evaluated; top candidate ready')
  }
  return <main>
    <h1>Complete winged animal candidate</h1>
    <button onClick={() => recognize('image')}>Recognize winged animal image</button>
    <button onClick={() => recognize('GLB')}>Recognize winged animal GLB</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: wing binding is missing') }}>
      Try missing wing binding
    </button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: wing pair is asymmetric') }}>
      Try asymmetric wing pair
    </button>
    <p role="status">{status}</p>
    {recognized && <CompleteAnimalBindingList locale="en" protrusions={[...protrusions]} />}
    <button ref={evaluateRef} onClick={reset}>Evaluate complete animal grid</button>
    <BeginnerGridProgressStatus locale="en" busy={busy} enumerated={27} checked={3} refined={18}
      onCancel={() => {
        setBusy(false)
        setStatus('Winged animal grid evaluation canceled')
        finishBeginnerGridCancellation(() => setPreview(false), restoreFocus)
      }} />
    {preview && <section aria-label="Complete animal candidate preview">
      <p>Global flat-foldability proven</p>
      <button onClick={() => void apply(false, true)}>Reject confirmation</button>
      <button onClick={() => void apply(true, false)}>Simulate failed apply</button>
      <button onClick={() => void apply(true, true).then((success) => {
        if (success) { setApplied(true); setBusy(false); setStatus('Winged animal applied') }
      })}>Confirm and apply</button>
      <button onClick={() => { setPreview(false); setBusy(false); setStatus('Stale candidate replaced by a newer reference') }}>
        Replace reference while preview is open
      </button>
    </section>}
    {applied && <section aria-label="Applied winged animal history">
      <button onClick={() => setStatus('Winged animal apply undone')}>Undo winged animal</button>
      <button onClick={() => setStatus('Winged animal apply redone')}>Redo winged animal</button>
      <button onClick={() => setStatus('Winged animal project saved and reopened')}>Save and reopen project</button>
    </section>}
  </main>
}

createRoot(document.getElementById('root')!).render(<Harness />)
