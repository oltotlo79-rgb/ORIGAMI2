import { createRoot } from 'react-dom/client'
import { useEffect, useRef, useState } from 'react'
import { GenericTargetBindingList } from '../src/components/GenericTargetBindingList.tsx'
import { ProtrusionDimensionEditor } from '../src/components/ProtrusionDimensionEditor.tsx'
import { GenericBodyOutlineEditor } from '../src/components/GenericBodyOutlineEditor.tsx'
import { BeginnerShapeCanvasPreview } from '../src/components/BeginnerShapeCanvasPreview.tsx'
import { RecognitionContourCopyAction } from '../src/components/RecognitionContourCopyAction.tsx'
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
  const [outlineMode, setOutlineMode] = useState<'symmetric' | 'general'>('symmetric')
  const [selectedCandidate, setSelectedCandidate] = useState(1)
  const [candidateShortage, setCandidateShortage] = useState(false)
  const witnessCanvas = useRef<HTMLCanvasElement>(null)
  const contourScore = Math.min(100, 80 + Math.max(0, outline.length - 4)
    + bindings.reduce((sum, target) => sum + Math.max(0, (target.local_outline_tenths_mm?.length ?? 3) - 3), 0))
  const evaluate = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (!preview && !applied) return
    const canvas = witnessCanvas.current, context = canvas?.getContext('2d')
    if (!canvas || !context) return
    context.clearRect(0, 0, canvas.width, canvas.height)
    const points = outline.length >= 4 ? outline : [[-50, -50], [50, -50], [50, 50], [-50, 50]]
    const scale = selectedCandidate === 1 ? 1 : 0.82
    context.strokeStyle = '#2563eb'; context.lineWidth = 3; context.beginPath()
    points.forEach(([x, y], index) => { const px = 160 + x * scale, py = 100 + y * scale
      if (index === 0) context.moveTo(px, py); else context.lineTo(px, py) })
    context.closePath(); context.stroke()
    context.strokeStyle = '#dc2626'; context.lineWidth = 1
    points.forEach(([x, y], index) => { const angle = index * Math.PI * 2 / points.length
      context.beginPath(); context.moveTo(160 + x * scale, 100 + y * scale)
      context.lineTo(160 + Math.cos(angle) * 78, 100 + Math.sin(angle) * 78); context.stroke() })
  }, [preview, applied, outline, selectedCandidate])
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
    setRecognized(true); setPreview(false); setCandidateShortage(false); setStatus(`${source} recognized two bounded bindings`)
  }
  return <main><h1>Bounded generic target</h1>
    <button onClick={() => recognize('Empty generic target')}>Create empty generic target</button>
    <button onClick={() => recognize('Image')}>Recognize mixed target image</button>
    <button onClick={() => recognize('GLB')}>Recognize mixed target GLB</button>
    <RecognitionContourCopyAction locale="en" bodyPointCount={4} localContourCount={1}
      onCopy={() => {
        recognize('Image contour proposal')
        setBindings(initialBindings.map((target, index) => index === 0
          ? { ...target, local_outline_tenths_mm: [[-20, -10], [0, -20], [20, -10], [10, 20], [-10, 20]] }
          : { ...target }))
        setOutlineMode('general'); setOutline([[-50, -50], [50, -50], [40, 50], [-30, 50]])
      }} />
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected: target exceeds eight bindings') }}>Try oversized target</button>
    <button onClick={() => {
      setRecognized(true); setPreview(false); setCandidateShortage(true); setOutlineMode('general')
      setOutline(Array.from({ length: 16 }, (_, index) => {
        const angle = Math.PI * 2 * index / 16
        return [Math.round(Math.cos(angle) * 50), Math.round(Math.sin(angle) * 50)] as [number, number]
      }))
      setStatus('Contour candidate shortage: no three witnessed designs at the strict 16-point threshold')
    }}>Try strict dense contour</button>
    <p role="status">{status}</p>
    {recognized && <p>Contour approximation score: {contourScore}</p>}
    {recognized && <GenericBodyOutlineEditor locale="en" points={outline} onChange={setOutline}
      mode={outlineMode} onModeChange={setOutlineMode} />}
    {recognized && <BeginnerShapeCanvasPreview locale="en" bodySize={[400, 300]}
      bodyOutline={outline} bodyMode={outlineMode} protrusions={bindings}
      onBodyOutlineChange={setOutline}
      onProtrusionChange={(changed) => setBindings((current) => current.map(
        (target) => target.id === changed.id ? changed : target,
      ))} />}
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
    {candidateShortage && <section aria-label="Contour candidate recovery">
      <p>Candidate shortage: strict contour placement produced fewer than three safe designs.</p>
      <button onClick={() => {
        setOutline((current) => Array.from({ length: 12 }, (_, index) => current[Math.floor(index * current.length / 12)]!))
        setCandidateShortage(false); setPreview(true); setStatus('Contour relaxed safely to 12 points; alternative grid ready')
      }}>Relax contour to 12 points and regenerate</button>
    </section>}
    <button ref={evaluate} onClick={() => { if (recognized) {
      if (candidateShortage) setStatus('Contour candidate shortage: safe relaxation is required')
      else { setPreview(true); setStatus('Generic target grid ready') }
    } }}>Evaluate generic target grid</button>
    {preview && <section aria-label="Generic target candidate preview"><p>Global flat-foldability proven</p>
      <button aria-pressed={selectedCandidate === 1} onClick={() => setSelectedCandidate(1)}>Select contour candidate 1</button>
      <button aria-pressed={selectedCandidate === 2} onClick={() => setSelectedCandidate(2)}>Select contour candidate 2</button>
      <p>Contour placement witness candidate {selectedCandidate}: body {outline.length || 4}, local {bindings.filter((binding) => binding.local_outline_tenths_mm).map((binding) => `${binding.id}:${binding.local_outline_tenths_mm!.length}`).join(', ') || 'none'}</p>
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => { setPreview(false); setStatus('Stale generic target replaced') }}>Replace recognized target</button>
      <button onClick={() => { finishBeginnerGridCancellation(() => setPreview(false), focus); setStatus('Generic target grid canceled') }}>Cancel generic target grid</button>
      <button onClick={() => void runBeginnerGridApplyWorkflow({ confirm: () => true, apply: async () => true,
        clearPreview: () => setPreview(false), restoreFocus: focus }).then((ok) => { if (ok) { setApplied(true); setStatus('Generic target applied') } })}>Confirm and apply generic target</button>
    </section>}
    {applied && <section aria-label="Generic target history">
      <p>Applied contour placement witness candidate {selectedCandidate}</p>
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Applied contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => setStatus('Generic target undone')}>Undo generic target</button>
      <button onClick={() => setStatus('Generic target redone')}>Redo generic target</button>
      <button onClick={() => setStatus('Generic target saved and reopened')}>Save and reopen generic target</button>
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
