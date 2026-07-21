import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { InstructionTimelinePanel } from '../src/components/InstructionTimelinePanel'
import { InstructionExportDialog } from '../src/components/InstructionExportDialog'
import { beginInstructionExportGeneration, cancelInstructionExport, getInstructionExportProgress, previewInstructionExport, saveInstructionExport, type ProjectSnapshot } from '../src/lib/coreClient'
import type { InstructionExportPreview } from '../src/lib/instructionExport'

const model = 'ab'.repeat(32)
const face = '11111111-1111-1111-1111-111111111111'
const edge = '22222222-2222-2222-2222-222222222222'
const binding = Array(32).fill(0x6d)
const poses = [5, 45, 90].map(angle => ({ model: 'absolute_hinge_angles_v1' as const, source_model_fingerprint: model, fixed_face: face, hinge_angles: [{ edge, angle_degrees: angle }] }))
const encoder = new TextEncoder()
const uuid = (value: string) => Uint8Array.from(value.replaceAll('-', '').match(/../gu)!.map(value => Number.parseInt(value, 16)))
async function digest(parts: Uint8Array[]) { const bytes = new Uint8Array(parts.reduce((n, p) => n + p.length, 0)); let offset = 0; for (const part of parts) { bytes.set(part, offset); offset += part.length } return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))] }
async function graphHash(angle: number) { const count = new Uint8Array(8); new DataView(count.buffer).setBigUint64(0, 1n, false); const value = new Uint8Array(8); new DataView(value.buffer).setFloat64(0, angle, false); return digest([encoder.encode('stacked_fold_certified_path_graph_state_v1'), count, uuid(edge), value]) }
const modelBinding = await digest([encoder.encode('path_certificate_source_model_binding_v1'), encoder.encode(model)])
const hashes = await Promise.all([5, 45, 90].map(graphHash))
const visual = { camera: null, arrows: [], focus_points: [], hand_guides: [] }
const steps = poses.map((pose, index) => ({ id: `miura-${index}`, title: index ? `Miura atomic ${index}` : 'Miura開始姿勢', description: index ? `認証済みの連続折り経路で「Miura atomic」を適用します。経路証明 SHA-256: ${'6d'.repeat(32)} / 元モデル SHA-256: ${model}` : '構造化証明の始点姿勢です。', caution: '', duration_ms: 1000, pose, visual: index ? { ...visual, path_certificate_reference_v1: { version: 1 as const, model_id: 'bounded_certified_pose_graph_path_reference_v1' as const, binding_sha256: binding, source_pose_sha256: hashes[index - 1]!, target_pose_sha256: hashes[index]!, source_model_binding_sha256: modelBinding, transition_count: 2 } } : visual }))
const snapshot = { project_instance_id: 'instance-miura', project_id: 'project-miura', name: 'Miura', current_path: null, revision: 2, saved_revision: 2, is_dirty: false, crease_pattern: { vertices: [], edges: [] }, paper: { boundary_vertices: [], thickness_mm: 0.1, length_display_unit: 'mm', cutting_allowed: false, front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null }, back: { color: { red: 240, green: 240, blue: 240, alpha: 255 }, texture_asset: null } }, can_undo: false, can_redo: false, cutting_allowed: false, instruction_timeline: { steps }, fold_model_fingerprint: model } as ProjectSnapshot
const reverseSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '中割り折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '中割り折りの開始姿勢' : `中割り折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '中割り折り'),
    })),
  },
})) as ProjectSnapshot
const sinkSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '沈め折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '沈め折りの開始姿勢' : `沈め折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '沈め折り'),
    })),
  },
})) as ProjectSnapshot
const accordionSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '蛇腹折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '蛇腹折りの開始姿勢' : `蛇腹折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '蛇腹折り'),
    })),
  },
})) as ProjectSnapshot
const layerSelectiveSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '層選択折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '層選択折りの開始姿勢' : `層選択折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '層選択折り'),
    })),
  },
})) as ProjectSnapshot
const bookFoldSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '二つ折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '二つ折りの開始姿勢' : `二つ折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '二つ折り'),
    })),
  },
})) as ProjectSnapshot
const outsideReverseSnapshot = JSON.parse(JSON.stringify({
  ...snapshot,
  name: '外割り折り',
  instruction_timeline: {
    steps: steps.map((step, index) => ({
      ...step,
      title: index === 0 ? '外割り折りの開始姿勢' : `外割り折り ${index}`,
      description: index === 0
        ? step.description
        : step.description.replace('Miura atomic', '外割り折り'),
    })),
  },
})) as ProjectSnapshot
let exports = 0
const ipc: string[] = []
let format: 'pdf' | 'svg_zip' = 'pdf'; let scenario: 'valid' | 'stale' | 'tamper' = 'valid'
const preview: InstructionExportPreview = { export_id: 'miura-export', expected_project_id: snapshot.project_id, expected_revision: snapshot.revision, format: 'pdf', profile: 'instruction_export_v1', projection_profile: 'orthographic_isometric_v1', format_summary: 'PDF 1.7・固定アイソメトリック投影・A4縦', suggested_file_name: 'Miura-折り図.pdf', byte_count: 4096, step_count: 3, page_count: 3, caution_count: 0, warnings: [{ category: 'discrete_step_endpoints_only', message_ja: '構造化証明は離散姿勢の区間を再検証済みです。' }] }
Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (command: string, args?: { format?: string }) => {
  ipc.push(command === 'preview_instruction_export' ? `${command}:${args?.format}` : command)
  if (command === 'begin_instruction_export') return { export_id: 'miura-export', profile: 'instruction_export_v1' }
  if (command === 'preview_instruction_export') {
    if (scenario !== 'valid') throw new Error(scenario)
    return { preview: { export_id: 'miura-export' } }
  }
  if (command === 'cancel_instruction_export') return
  if (command === 'get_instruction_export_progress') return { progress: { export_id: 'miura-export', phase: 'building_document', completed_units: 2, total_units: 3 } }
  if (command === 'save_instruction_export') return { export_id: 'miura-export', saved: true }
  throw new Error(`unexpected ${command}`)
} } })
async function exportThroughProductionIpc() { ipc.length = 0; const generation = await beginInstructionExportGeneration(); let result = 'ready'; try { await previewInstructionExport(generation.export_id, snapshot.project_id, scenario === 'stale' ? snapshot.revision + 1 : snapshot.revision, format); exports += 1 } catch { result = `${scenario}-rejected`; await cancelInstructionExport(generation.export_id) } document.querySelector('[data-testid=exports]')!.textContent = `exports=${exports}; format=${format}; result=${result}; ipc=${ipc.join(',')}` }
function ExportLifecycle() { const [active, setActive] = useState(false); const [open, setOpen] = useState(false); const [event, setEvent] = useState('idle'); return <><output data-testid="lifecycle">{event}</output><button onClick={() => { void (async () => { ipc.length = 0; const generation = await beginInstructionExportGeneration(); setOpen(true); setActive(true); await getInstructionExportProgress(generation.export_id); setEvent(`progress; ipc=${ipc.join(',')}`) })() }}>Start progress lifecycle</button>{open && <InstructionExportDialog format="pdf" preview={preview} busy={false} generationActive={active} phase={active ? 'building_document' : 'ready'} error={null} notice={null} onFormatChange={() => {}} onRetry={() => {}} onSave={(ack) => { void saveInstructionExport('miura-export', snapshot.project_id, snapshot.revision, ack).then(() => setEvent(`saved; ipc=${ipc.join(',')}`)) }} onCancel={() => { void cancelInstructionExport('miura-export').then(() => { setActive(false); setEvent(`cancelled; ipc=${ipc.join(',')}`) }) }} />}</> }
function Harness() { const [technique, setTechnique] = useState<'miura' | 'reverse' | 'sink' | 'accordion' | 'layer-selective' | 'book-fold' | 'outside-reverse'>('miura'); const activeSnapshot = technique === 'miura' ? snapshot : technique === 'reverse' ? reverseSnapshot : technique === 'sink' ? sinkSnapshot : technique === 'accordion' ? accordionSnapshot : technique === 'layer-selective' ? layerSelectiveSnapshot : technique === 'book-fold' ? bookFoldSnapshot : outsideReverseSnapshot; return <><output data-testid="exports">exports={exports}</output><button onClick={() => setTechnique('reverse')}>Inside reverse timeline</button><button onClick={() => setTechnique('outside-reverse')}>Outside reverse timeline</button><button onClick={() => setTechnique('sink')}>Sink timeline</button><button onClick={() => setTechnique('accordion')}>Accordion timeline</button><button onClick={() => setTechnique('layer-selective')}>Layer selective timeline</button><button onClick={() => setTechnique('book-fold')}>Book fold timeline</button><button onClick={() => { format = 'pdf'; scenario = 'valid' }}>PDF mode</button><button onClick={() => { format = 'svg_zip'; scenario = 'valid' }}>SVG mode</button><button onClick={() => { scenario = 'stale' }}>Stale revision</button><button onClick={() => { scenario = 'tamper' }}>Tamper DTO hash</button><InstructionTimelinePanel snapshot={activeSnapshot} appliedPose={null} poseModelKey={technique} manualPoseChangeSequence={0} coreBusy={false} benchmarkActive={false} fileOperationActive={false} exportAvailable exportButtonRef={{ current: null }} animationExportButtonRef={{ current: null }} runNativeEdit={async () => true} applyStepPose={() => true} onExport={() => { void exportThroughProductionIpc() }} onAnimationExport={() => {}} /><ExportLifecycle /></> }
createRoot(document.getElementById('root')!).render(<Harness />)
