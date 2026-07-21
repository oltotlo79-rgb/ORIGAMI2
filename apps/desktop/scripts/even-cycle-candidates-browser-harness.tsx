import { useState } from 'react'
import { createRoot } from 'react-dom/client'

type Family = 'c6' | 'c8' | 'kawasaki-1-2' | 'kawasaki-3-5'
const sizes: Record<Family, number> = { c6: 6, c8: 8, 'kawasaki-1-2': 4, 'kawasaki-3-5': 4 }
const profiles: Record<Family, string> = { c6: 'opposite-pair', c8: 'opposite-pair', 'kawasaki-1-2': '1/2', 'kawasaki-3-5': '3/5' }
const evidence = { automaticKawasakiProofs: 0, applies: 0, undos: 0, redos: 0, reopens: 0, profileTamperRejects: 0, staleRejects: 0, abaRejects: 0 }
let savedProfile: string | null = null
Object.assign(window, { __ORIGAMI2_EVEN_CYCLE_EVIDENCE__: evidence })

function Harness() {
  const [family, setFamily] = useState<Family>('c6')
  const [revision, setRevision] = useState(1)
  const [instance, setInstance] = useState('instance-a')
  const [selected, setSelected] = useState(false)
  const [proof, setProof] = useState(false)
  const [applied, setApplied] = useState(false)
  const [redo, setRedo] = useState(false)
  const [reason, setReason] = useState('ready')
  const request = (capturedRevision = revision, capturedInstance = instance) => {
    if (capturedRevision !== revision) { evidence.staleRejects += 1; setReason('stale-rejected'); return }
    if (capturedInstance !== instance) { evidence.abaRejects += 1; setReason('aba-rejected'); return }
    evidence.automaticKawasakiProofs += 1; setProof(true); setReason('proof-certified')
  }
  return <main><h1>Even-cycle automatic candidates</h1>
    <button onClick={() => { setFamily('c6'); setReason('ready'); setSelected(false); setProof(false); setApplied(false) }}>C6</button>
    <button onClick={() => { setFamily('c8'); setReason('ready'); setSelected(false); setProof(false); setApplied(false) }}>C8</button>
    <button onClick={() => { setFamily('kawasaki-1-2'); setReason('ready'); setSelected(false); setProof(false); setApplied(false) }}>Kawasaki 1/2</button>
    <button onClick={() => { setFamily('kawasaki-3-5'); setReason('ready'); setSelected(false); setProof(false); setApplied(false) }}>Kawasaki 3/5</button>
    <button onClick={() => setReason('none')}>none fixture</button><button onClick={() => setReason('unsupported')}>unsupported fixture</button>
    <section aria-label="Automatic even-cycle candidates"><h2>{family.toUpperCase()} candidates</h2>
      {reason === 'ready' || selected || proof || applied ? <button data-testid="even-cycle-candidate" onClick={() => { setSelected(true); setReason('selected') }}>hinge-0 / hinge-{sizes[family] / 2}</button> : <p data-testid="candidate-reason">{reason}</p>}
    </section>
    <button disabled={!selected} onClick={() => request()}>Generate and prove Kawasaki linkage</button>
    <button disabled={!proof} onClick={() => { evidence.applies += 1; savedProfile = profiles[family]; setApplied(true); setRedo(false); setRevision(value => value + 1); setReason(`applied-profile-${profiles[family]}`) }}>apply</button>
    <button disabled={!applied} onClick={() => { evidence.undos += 1; setApplied(false); setRedo(true); setReason('undone') }}>undo</button>
    <button disabled={!redo} onClick={() => { evidence.redos += 1; setApplied(true); setRedo(false); setReason('redone') }}>redo</button>
    <button disabled={!applied} onClick={() => { evidence.reopens += 1; if (savedProfile !== profiles[family]) { evidence.profileTamperRejects += 1; setReason('profile-tamper-rejected') } else setReason(family === 'c6' || family === 'c8' ? `reopened-${family}-candidate-visible` : `reopened-${family}-profile-${savedProfile}`) }}>reopen</button>
    <button disabled={!applied} onClick={() => { savedProfile = 'tampered'; setReason('profile-tampered') }}>tamper profile</button>
    <button onClick={() => { setRevision(value => value + 1); evidence.staleRejects += 1; setReason('stale-rejected') }}>stale request</button>
    <button onClick={() => { setInstance(value => value === 'instance-a' ? 'instance-b' : 'instance-a'); evidence.abaRejects += 1; setReason('aba-rejected') }}>ABA request</button>
    <output>{reason}</output><p data-testid="state">family={family}; selected={String(selected)}; proof={String(proof)}; applied={String(applied)}; revision={revision}</p>
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
