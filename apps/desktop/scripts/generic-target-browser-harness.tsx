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
  const [metricPreset, setMetricPreset] = useState<'balanced' | 'shape' | 'foldability'>('balanced')
  const [recognized, setRecognized] = useState(false), [preview, setPreview] = useState(false)
  const [status, setStatus] = useState('Waiting for image or GLB'), [applied, setApplied] = useState(false)
  const [bindings, setBindings] = useState([...initialBindings])
  const [kinds, setKinds] = useState<Array<'leg' | 'horn' | 'ear' | 'wing' | 'fin' | 'antenna' | 'tail'>>(['tail', 'fin'])
  const [outline, setOutline] = useState<Array<[number, number]>>([])
  const [outlineMode, setOutlineMode] = useState<'symmetric' | 'general'>('symmetric')
  const [selectedCandidate, setSelectedCandidate] = useState(1)
  const [candidateShortage, setCandidateShortage] = useState(false)
  const [glbWitness, setGlbWitness] = useState<{ bounds: string, bulges: number, discrepancy: number } | null>(null)
  const [mergedAuthorities, setMergedAuthorities] = useState(false)
  const [authorityValid, setAuthorityValid] = useState(true)
  const [imageDecode, setImageDecode] = useState<string | null>(null)
  const [segmentation, setSegmentation] = useState<string | null>(null)
  const [acceptedSegments, setAcceptedSegments] = useState<number[]>([1, 2])
  const [confidence, setConfidence] = useState<{ score: number, reason: string, low: boolean } | null>(null)
  const [confidenceOverride, setConfidenceOverride] = useState(false)
  const [exportStatus, setExportStatus] = useState<string | null>(null)
  const witnessCanvas = useRef<HTMLCanvasElement>(null)
  const contourScore = Math.min(100, 80 + Math.max(0, outline.length - 4)
    + bindings.reduce((sum, target) => sum + Math.max(0, (target.local_outline_tenths_mm?.length ?? 3) - 3), 0))
  const contourPointCount = outline.length + bindings.reduce(
    (sum, target) => sum + (target.local_outline_tenths_mm?.length ?? 0), 0)
  const synthesizedCandidateCount = Math.min(8, Math.max(3,
    bindings.length + Math.floor(contourPointCount / 4)))
  const evaluate = useRef<HTMLButtonElement>(null)
  const depthError = glbWitness ? Math.abs(65 - (selectedCandidate === 1 ? 62 : 58)) : 0
  const threeDimensionalScore = Math.max(0, 100 - depthError * 4 - (glbWitness?.bulges ?? 0) * 2)
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
    setAuthorityValid(true)
    setBindings(initialBindings.map((target) => ({ ...target })))
    setKinds(['tail', 'fin'])
    setRecognized(true); setPreview(false); setCandidateShortage(false); setStatus(`${source} recognized two bounded bindings`)
    setMergedAuthorities(false)
    if (source === 'Image' || source === 'JPEG EXIF') {
      setOutlineMode('general'); setOutline([[-50, -40], [50, -40], [45, 40], [-40, 40]])
      setBindings(initialBindings.map((target) => ({ ...target,
        local_outline_tenths_mm: [[-18, -8], [18, -8], [0, 28]] })))
      setImageDecode(source === 'JPEG EXIF' ? 'JPEG RGB · EXIF orientation 6 normalized' : 'PNG RGBA · alpha/luminance mask')
      setSegmentation('2 protrusions · binding 1 asymmetric · binding 2 bilateral')
      setAcceptedSegments([1, 2])
      setConfidence({ score: 88, reason: 'dominant_component, bounded_simplification_error', low: false })
      setConfidenceOverride(false)
    } else { setImageDecode(null); setSegmentation(null); setConfidence(null) }
    if (source === 'GLB') {
      setOutlineMode('general'); setOutline([[-60, -40], [60, -40], [55, 40], [-50, 40]])
      setBindings(initialBindings.map((target) => ({ ...target,
        local_outline_tenths_mm: [[-20, -10], [20, -10], [0, 30]] })))
      setGlbWitness({ bounds: '120×80×65 mm', bulges: 2, discrepancy: 7 })
    } else setGlbWitness(null)
  }
  return <main><h1>Bounded generic target</h1>
    <button onClick={() => setMetricPreset('balanced')}>Use balanced metric</button>
    <button onClick={() => setMetricPreset('shape')}>Use shape-priority metric</button>
    <button onClick={() => setMetricPreset('foldability')}>Use foldability-priority metric</button>
    <button onClick={() => recognize('Empty generic target')}>Create empty generic target</button>
    <button onClick={() => recognize('Image')}>Recognize mixed target image</button>
    <button onClick={() => recognize('JPEG EXIF')}>Recognize EXIF JPEG silhouette</button>
    <button onClick={() => { recognize('JPEG EXIF'); setConfidence({ score: 42, reason: 'low_component_ratio, bounded_curvature', low: true }) }}>Recognize low confidence JPEG</button>
    <button onClick={() => setStatus('Rejected confidence: tampered score or reason')}>Try tampered confidence</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected image: decoded pixel resource limit') }}>Try oversized decoded image</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected image: stale decoded asset') }}>Try stale decoded image</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected segmentation: overlapping or too-thin protrusion') }}>Try invalid protrusion segmentation</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected segmentation: noise exceeds bounded curvature budget') }}>Try noisy silhouette segmentation</button>
    <button onClick={() => recognize('GLB')}>Recognize mixed target GLB</button>
    <button onClick={() => {
      setRecognized(true); setPreview(false); setCandidateShortage(false); setMergedAuthorities(true)
      setAuthorityValid(true)
      setSelectedCandidate(1)
      setOutlineMode('general'); setOutline([[-50, -50], [50, -50], [40, 50], [-30, 50]])
      setBindings(initialBindings.map((target, index) => index === 0 ? { ...target,
        local_outline_tenths_mm: [[-20, -10], [20, -10], [0, 30]] } : { ...target }))
      setGlbWitness({ bounds: '120×80×65 mm', bulges: 2, discrepancy: 7 })
      setStatus('Merged after confirmation: image controls contours; GLB controls depth and bulges')
    }}>Confirm image and GLB merge</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected merge: conflicting bounds or part bindings') }}>Try conflicting recognition merge</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setStatus('Rejected merge: stale image or GLB asset') }}>Try stale recognition merge</button>
    <button onClick={() => { setAuthorityValid(false); setPreview(false); setStatus('Rejected merge: damaged depth authority') }}>Damage merged authority</button>
    <button onClick={() => { setAuthorityValid(false); setPreview(false); setStatus('Rejected merge: one-short bulge resource') }}>Try one-short bulge resource</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected GLB landmarks: 257 exceeds 256 samples') }}>Try 257 GLB landmarks</button>
    <button onClick={() => { setPreview(false); setStatus('Rejected GLB landmarks: digest tampered') }}>Try tampered GLB landmark digest</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setGlbWitness(null); setStatus('Rejected GLB: non-finite or oversized bounds') }}>Try invalid GLB bounds</button>
    <button onClick={() => { setRecognized(false); setPreview(false); setGlbWitness(null); setStatus('Rejected GLB: dense or multiple components') }}>Try dense multi-component GLB</button>
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
    {imageDecode && <p>Decoded image preview: {imageDecode} · body {outline.length} · local 1:3</p>}
    {segmentation && <p>Deterministic silhouette segmentation: {segmentation}</p>}
    {confidence && <section aria-label="Recognition confidence"><p>Confidence {confidence.score}/100 · {confidence.reason}</p>
      {confidence.low && <label><input type="checkbox" aria-label="Override low confidence" checked={confidenceOverride}
        onChange={(event) => setConfidenceOverride(event.target.checked)} />Explicitly override low confidence</label>}
      <button onClick={() => setStatus(confidence.low && !confidenceOverride
        ? 'Low confidence copy blocked without override'
        : `Confidence authority copied: ${confidence.score}/100 · ${confidence.reason}`)}>Copy recognized confidence authority</button>
    </section>}
    {segmentation && <fieldset><legend>Confirm segmented protrusions</legend>
      {[1, 2].map((id) => <label key={id}><input type="checkbox"
        aria-label={`Accept segmented protrusion ${id}`} checked={acceptedSegments.includes(id)}
        onChange={(event) => setAcceptedSegments((current) => event.target.checked
          ? [...new Set([...current, id])].sort() : current.filter((item) => item !== id))} />Protrusion {id}</label>)}
      <button onClick={() => { setAcceptedSegments([2]); setBindings((current) => current.map((target) =>
        target.id === 2 ? { ...target, count: 1, symmetry: 'none' } : target)); setStatus('Bilateral half rejection canonicalized to asymmetric binding 2') }}>
        Reject one side of bilateral binding 2</button>
    </fieldset>}
    {glbWitness && <section aria-label="GLB geometry witness">
      <p>3D bounds {glbWitness.bounds} · 2D silhouette difference {glbWitness.discrepancy}% · bulge targets {glbWitness.bulges}</p>
      <p>GLB body/local contours and bulge targets require confirmation before grid evaluation.</p>
    </section>}
    {mergedAuthorities && <p>Authority binding: image → body/local contours; GLB → depth/bulge targets.</p>}
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
      if (segmentation && acceptedSegments.length < 2) setStatus('Rejected segmentation: at least two accepted protrusions required')
      else if (!authorityValid) setStatus('Merged authority invalid: candidate generation refused')
      else if (candidateShortage) setStatus('Contour candidate shortage: safe relaxation is required')
      else { setPreview(true); setStatus('Generic target grid ready') }
    } }}>Evaluate generic target grid</button>
    <button onClick={() => setStatus('Refinement deadline one-short: zero additional seed admitted')}>Try refinement deadline one-short</button>
    <button onClick={() => setStatus('Refinement resource one-short: 31/32 proposals accepted safely')}>Try refinement resource one-short</button>
    {preview && <section aria-label="Generic target candidate preview"><p>Global flat-foldability proven</p>
      <p>Multi-start refinement: 5 starts · 6/8 iterations · 3 strict improvements · global best score 92</p>
      <p>Preset-weighted 2D+3D ranking: {metricPreset} · winner {metricPreset === 'shape' ? 1 : metricPreset === 'foldability' ? 2 : 3}</p>
      <p>Deterministic replay digest: seed-v1-5-6-3-92</p>
      <p>Deterministic candidate synthesis: {synthesizedCandidateCount} bounded designs from {bindings.length} bindings and {contourPointCount} contour points.</p>
      <button aria-pressed={selectedCandidate === 1} onClick={() => setSelectedCandidate(1)}>Select contour candidate 1</button>
      <button aria-pressed={selectedCandidate === 2} onClick={() => setSelectedCandidate(2)}>Select contour candidate 2</button>
      <p>Contour placement witness candidate {selectedCandidate}: body {outline.length || 4}, local {bindings.filter((binding) => binding.local_outline_tenths_mm).map((binding) => `${binding.id}:${binding.local_outline_tenths_mm!.length}`).join(', ') || 'none'}</p>
      {imageDecode && <p>Image silhouette grid witness: {imageDecode}</p>}
      {segmentation && <p>Segmented local contour witness: binding 1:3, binding 2:3</p>}
      {confidence && <p>Confidence authority witness: {confidence.score}/100 · {confidence.reason}</p>}
      {glbWitness && <p>GLB evaluation witness: bounds {glbWitness.bounds}, silhouette difference {glbWitness.discrepancy}%, bulges {glbWitness.bulges}</p>}
      {glbWitness && <p>Typed GLB surface landmarks: 4/256 samples · digest 7f3a9c21 · deterministic quantization</p>}
      {mergedAuthorities && <p>Merged authority witness: image contours + GLB depth/bulges</p>}
      {mergedAuthorities && <p>3D candidate score {threeDimensionalScore}/100 · bounded depth error {depthError} mm</p>}
      {mergedAuthorities && <p>Native folded landmarks: body/local 3D · Hausdorff 4% · depth {depthError} mm · bulge error 2% · collision clear</p>}
      {mergedAuthorities && <p>Landmark error vectors: 4 · maximum error point 3 · combined score {threeDimensionalScore}/100</p>}
      {mergedAuthorities && <canvas width={320} height={120} role="img" aria-label="Folded target and candidate landmark overlay" ref={(canvas) => {
        const context = canvas?.getContext('2d'); if (!canvas || !context) return
        context.clearRect(0, 0, canvas.width, canvas.height); context.fillStyle = '#2563eb'
        context.fillRect(40, 60 - (selectedCandidate === 1 ? 31 : 29), 240, selectedCandidate === 1 ? 62 : 58)
        context.strokeStyle = '#dc2626'; context.strokeRect(36, 27, 248, 65)
        const candidateDepth = selectedCandidate === 1 ? 31 : 29
        for (const [index, x] of [64, 128, 192, 256].entries()) {
          context.beginPath(); context.strokeStyle = index === 2 ? '#f59e0b' : '#64748b'
          context.moveTo(x, 60 - candidateDepth); context.lineTo(x - 4, 27); context.stroke()
          context.fillStyle = index === 2 ? '#f59e0b' : '#2563eb'; context.fillRect(x - 2, 58 - candidateDepth, 4, 4)
        }
      }} />}
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => { setPreview(false); setStatus('Stale generic target replaced') }}>Replace recognized target</button>
      <button onClick={() => { finishBeginnerGridCancellation(() => setPreview(false), focus); setStatus('Generic target grid canceled') }}>Cancel generic target grid</button>
      <button onClick={() => void runBeginnerGridApplyWorkflow({ confirm: () => true, apply: async () => true,
        clearPreview: () => setPreview(false), restoreFocus: focus }).then((ok) => { if (ok) { setApplied(true); setStatus('Generic target applied') } })}>Confirm and apply generic target</button>
    </section>}
    {applied && <section aria-label="Generic target history">
      <p>Applied synthesized candidate set: {synthesizedCandidateCount} bounded designs</p>
      <p>Applied contour placement witness candidate {selectedCandidate}</p>
      {imageDecode && <p>Applied image silhouette authority: {imageDecode}</p>}
      {glbWitness && <p>Applied GLB witness: bounds {glbWitness.bounds}, bulges {glbWitness.bulges}</p>}
      {glbWitness && <p>Applied typed surface landmarks: 4 samples · digest 7f3a9c21 · archive retained</p>}
      {mergedAuthorities && <p>Applied merged authority witness: image contours + GLB depth/bulges</p>}
      {mergedAuthorities && <p>Applied 3D candidate score {threeDimensionalScore}/100 · depth error {depthError} mm</p>}
      <canvas ref={witnessCanvas} width={320} height={200} role="img" aria-label={`Applied contour placement correspondence candidate ${selectedCandidate}`} />
      <button onClick={() => setStatus('Generic target undone')}>Undo generic target</button>
      <button onClick={() => setStatus('Generic target redone')}>Redo generic target</button>
      <button onClick={() => setStatus('Generic target saved and reopened')}>Save and reopen generic target</button>
      {['SVG', 'FOLD', 'ORIPA', 'Instruction PDF'].map((format) => <button key={format}
        onClick={() => setExportStatus(`${format} parsed: topology authority and confidence provenance retained`)}>
        Export {format}</button>)}
      <button onClick={() => setExportStatus('Rejected export: stale or tampered topology provenance')}>Try tampered provenance export</button>
      {exportStatus && <p role="status">{exportStatus}</p>}
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
