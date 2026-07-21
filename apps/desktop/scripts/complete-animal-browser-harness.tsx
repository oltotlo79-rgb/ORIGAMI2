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
] as const

function Harness() {
  const [preview, setPreview] = useState(true)
  const [busy, setBusy] = useState(true)
  const evaluateRef = useRef<HTMLButtonElement>(null)
  const restoreFocus = () => requestAnimationFrame(() => evaluateRef.current?.focus())
  const apply = (confirmed: boolean, applied: boolean) => runBeginnerGridApplyWorkflow({
    confirm: () => confirmed,
    apply: async () => applied,
    clearPreview: () => setPreview(false),
    restoreFocus,
  })
  const reset = () => { setPreview(true); setBusy(true) }
  return <main>
    <h1>Complete animal candidate</h1>
    <button ref={evaluateRef} onClick={reset}>Evaluate complete animal grid</button>
    <BeginnerGridProgressStatus locale="en" busy={busy} enumerated={27} checked={3}
      onCancel={() => {
        setBusy(false)
        finishBeginnerGridCancellation(() => setPreview(false), restoreFocus)
      }} />
    {preview && <section aria-label="Complete animal candidate preview">
      <CompleteAnimalBindingList locale="en" protrusions={[...protrusions]} />
      <p>Global flat-foldability proven</p>
      <button onClick={() => void apply(false, true)}>Reject confirmation</button>
      <button onClick={() => void apply(true, false)}>Simulate failed apply</button>
      <button onClick={() => void apply(true, true)}>Confirm and apply</button>
    </section>}
  </main>
}

createRoot(document.getElementById('root')!).render(<Harness />)
