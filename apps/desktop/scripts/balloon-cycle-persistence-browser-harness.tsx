import { useState } from 'react'
import { createRoot } from 'react-dom/client'

const hinges = Array.from({ length: 6 }, (_, index) =>
  `018f47a2-4b7a-7cc1-8abc-00000000000${index}`)
const proof = {
  version: 1,
  model_id: 'native_continuous_layer_transport_certificate_v1',
  target_order_sha256: Array(32).fill(0xab),
  transition_count: 5,
  pairs: [{ lower_face: hinges[0], upper_face: hinges[3] }],
}
const appliedStep = {
  id: '018f47a2-4b7a-7cc1-8abc-778899aabbcc',
  title: 'C6 balloon opposite-axis fold',
  pose: {
    model: 'absolute_hinge_angles_v1',
    hinge_angles: hinges.map((edge, index) => ({ edge, angle_degrees: index === 0 || index === 3 ? 10 : 0 })),
  },
  visual: { cycle_layer_order_proof_v1: proof },
}
type Document = { instruction_timeline: { steps: typeof appliedStep[] } }
const evidence = { saves: 0, reopens: 0, undos: 0, redos: 0, tamperRejects: 0 }
let saved: Document | null = null
let redoStep: typeof appliedStep | null = null

function validate(document: Document) {
  const step = document.instruction_timeline.steps[0]
  if (!step) return
  const angles = step.pose.hinge_angles
  if (angles.length !== 6 || angles[0]?.angle_degrees !== angles[3]?.angle_degrees) {
    throw new Error('persisted cycle pose is not cycle-closing')
  }
}

Object.assign(window, {
  __ORIGAMI2_BALLOON_CYCLE_PERSISTENCE__: evidence,
  __TAURI_INTERNALS__: {
    invoke: async (command: string, args?: { document?: Document }) => {
      if (command === 'save_project') {
        evidence.saves += 1
        saved = structuredClone(args?.document ?? { instruction_timeline: { steps: [] } })
        return null
      }
      if (command === 'reopen_project') {
        evidence.reopens += 1
        if (!saved) throw new Error('no saved project')
        validate(saved)
        return structuredClone(saved)
      }
      throw new Error(`unexpected command ${command}`)
    },
  },
})

function Harness() {
  const [document, setDocument] = useState<Document>({ instruction_timeline: { steps: [appliedStep] } })
  const [notice, setNotice] = useState('applied')
  const invoke = (window as any).__TAURI_INTERNALS__.invoke as (command: string, args?: any) => Promise<any>
  const save = async () => { await invoke('save_project', { document }); setNotice('saved') }
  const reopen = async () => {
    try { setDocument(await invoke('reopen_project')); setNotice('reopened') }
    catch { evidence.tamperRejects += 1; setNotice('tamper-rejected') }
  }
  const undo = () => {
    evidence.undos += 1
    redoStep = document.instruction_timeline.steps[0] ?? null
    setDocument({ instruction_timeline: { steps: [] } }); setNotice('undone')
  }
  const redo = () => {
    evidence.redos += 1
    setDocument({ instruction_timeline: { steps: redoStep ? [redoStep] : [] } }); setNotice('redone')
  }
  const tamper = () => {
    if (!saved) return
    saved.instruction_timeline.steps[0]!.pose.hinge_angles[0]!.angle_degrees += 0.01
    setNotice('tampered')
  }
  const step = document.instruction_timeline.steps[0]
  return <main>
    <h1>C6 balloon persistence</h1>
    <button onClick={() => void save()}>save</button><button onClick={() => void reopen()}>reopen</button>
    <button onClick={undo}>undo</button><button onClick={redo}>redo</button><button onClick={tamper}>tamper saved pose</button>
    <output>{notice}</output>
    <p data-testid="step-count">steps={document.instruction_timeline.steps.length}</p>
    {step && <section aria-label="Persisted cycle proof"><h2>{step.title}</h2>
      <p>hinges={step.pose.hinge_angles.length}</p><p>transitions={step.visual.cycle_layer_order_proof_v1.transition_count}</p>
      <p>proof={step.visual.cycle_layer_order_proof_v1.target_order_sha256.map(value => value.toString(16).padStart(2, '0')).join('')}</p>
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
