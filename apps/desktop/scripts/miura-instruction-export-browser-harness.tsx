import { createRoot } from 'react-dom/client'
import { InstructionTimelinePanel } from '../src/components/InstructionTimelinePanel'
import type { ProjectSnapshot } from '../src/lib/coreClient'

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
let exports = 0
function Harness() { return <><output data-testid="exports">exports={exports}</output><InstructionTimelinePanel snapshot={snapshot} appliedPose={null} poseModelKey="miura" manualPoseChangeSequence={0} coreBusy={false} benchmarkActive={false} fileOperationActive={false} exportAvailable exportButtonRef={{ current: null }} animationExportButtonRef={{ current: null }} runNativeEdit={async () => true} applyStepPose={() => true} onExport={() => { exports += 1; document.querySelector('[data-testid=exports]')!.textContent = `exports=${exports}` }} onAnimationExport={() => {}} /></> }
createRoot(document.getElementById('root')!).render(<Harness />)
