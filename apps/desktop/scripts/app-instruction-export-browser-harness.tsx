import { createRoot } from 'react-dom/client'
import '../src/index.css'
import '../src/App.css'

const projectInstanceId = '10000000-0000-4000-8000-000000000001'
const projectId = '10000000-0000-4000-8000-000000000002'
const edgeId = '20000000-0000-4000-8000-000000000005'
const faceId = '30000000-0000-4000-8000-000000000001'
const model = 'ab'.repeat(32)
const commands: string[] = []
let saveMode: 'success' | 'cancel' | 'failure' = 'success'
const evidence = { commands, setSaveMode: (mode: typeof saveMode) => { saveMode = mode } }
Object.assign(window, { __ORIGAMI2_APP_EXPORT_EVIDENCE__: evidence })

const visual = { camera: null, arrows: [], focus_points: [], hand_guides: [] }
const steps = [5, 45, 90].map((angle, index) => ({
  id: `miura-${index}`, title: index ? `Miura atomic ${index}` : 'Miura start',
  description: index ? 'Certified Miura atomic transition.' : 'Certified initial pose.', caution: '', duration_ms: 1000,
  pose: { model: 'absolute_hinge_angles_v1', source_model_fingerprint: model, fixed_face: faceId, hinge_angles: [{ edge: edgeId, angle_degrees: angle }] }, visual,
}))
const vertices = [
  ['20000000-0000-4000-8000-000000000001', 0, 0], ['20000000-0000-4000-8000-000000000002', 400, 0],
  ['20000000-0000-4000-8000-000000000003', 400, 400], ['20000000-0000-4000-8000-000000000004', 0, 400],
].map(([id, x, y]) => ({ id, position: { x, y } }))
const snapshot = {
  project_instance_id: projectInstanceId, project_id: projectId, name: 'Miura App E2E', memo: '', current_path: null,
  revision: 7, saved_revision: 7, is_dirty: false,
  paper: { boundary_vertices: vertices.map(v => v.id), thickness_mm: 0.1, length_display_unit: 'mm', cutting_allowed: false,
    front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null }, back: { color: { red: 248, green: 248, blue: 245, alpha: 255 }, texture_asset: null } },
  crease_pattern: { vertices, edges: [
    { id: '20000000-0000-4000-8000-000000000011', start: vertices[0].id, end: vertices[1].id, kind: 'boundary' },
    { id: '20000000-0000-4000-8000-000000000012', start: vertices[1].id, end: vertices[2].id, kind: 'boundary' },
    { id: '20000000-0000-4000-8000-000000000013', start: vertices[2].id, end: vertices[3].id, kind: 'boundary' },
    { id: '20000000-0000-4000-8000-000000000014', start: vertices[3].id, end: vertices[0].id, kind: 'boundary' },
    { id: edgeId, start: vertices[0].id, end: vertices[2].id, kind: 'mountain' },
  ] }, instruction_timeline: { steps }, numeric_expressions: { rectangular_paper_creation: null, undo_stack: [], redo_stack: [] },
  geometric_constraints: { schema_version: 1, constraints: [] }, beginner_design_profile: { schema_version: 1, preset: 'balanced', shape_fidelity_weight: 35, foldability_weight: 35, step_count_weight: 15, paper_efficiency_weight: 15, generation_constraints: { schema_version: 1, maximum_steps: 60, detail_level: 'standard', target_category: null, target_parts: [], skeleton_segments: [], protrusions: [], bulge_targets: [], target_asset: null, allowed_techniques: ['valley_fold', 'mountain_fold'] } },
  project_layers: { schema_version: 1, layers: [{ id: '00000000-0000-4000-8000-000000000001', name: 'Crease Pattern', content_kind: 'crease_pattern', visible: true, locked: false, opacity: 1 }], edge_assignments: [] },
  element_metadata: { vertices: [], edges: [], faces: [] }, annotations: { schema_version: 1, annotations: [] }, underlays: { schema_version: 1, underlays: [] }, fold_model_fingerprint: model, can_undo: false, can_redo: false, cutting_allowed: false,
}
const preview = (format: 'pdf' | 'svg_zip') => ({ export_id: 'app-miura-export', expected_project_id: projectId, expected_revision: 7, format, profile: 'instruction_export_v1', projection_profile: 'orthographic_isometric_v1', format_summary: format === 'pdf' ? 'PDF 1.7 / A4 portrait' : 'SVG ZIP', suggested_file_name: format === 'pdf' ? 'miura.pdf' : 'miura-svg.zip', byte_count: 4096, step_count: 3, page_count: 3, caution_count: 0, warnings: [{ category: 'discrete_step_endpoints_only', message_ja: '離散姿勢のみです。' }] })
const otherFaceId = '30000000-0000-4000-8000-000000000002'
const half = (edge: string, origin: number, destination: number) => ({ edge, origin: vertices[origin].id, destination: vertices[destination].id })
const topology = { project_id: projectId, revision: 7, simulation_ready: true, issues: [], snapshot: { source_revision: 7,
  faces: [
    { id: faceId, key: Array(32).fill(1), outer: { half_edges: [half('20000000-0000-4000-8000-000000000011', 0, 1), half('20000000-0000-4000-8000-000000000012', 1, 2), half(edgeId, 2, 0)], signed_double_area: 160000 }, area: 80000 },
    { id: otherFaceId, key: Array(32).fill(2), outer: { half_edges: [half(edgeId, 0, 2), half('20000000-0000-4000-8000-000000000013', 2, 3), half('20000000-0000-4000-8000-000000000014', 3, 0)], signed_double_area: 160000 }, area: 80000 },
  ], edge_incidence: [
    ['20000000-0000-4000-8000-000000000011', { kind: 'boundary', material: faceId }], ['20000000-0000-4000-8000-000000000012', { kind: 'boundary', material: faceId }],
    ['20000000-0000-4000-8000-000000000013', { kind: 'boundary', material: otherFaceId }], ['20000000-0000-4000-8000-000000000014', { kind: 'boundary', material: otherFaceId }],
    [edgeId, { kind: 'hinge', left: otherFaceId, right: faceId, assignment: 'mountain' }],
  ], hinge_adjacency: [{ edge: edgeId, first: faceId, second: otherFaceId, assignment: 'mountain' }], material_components: [{ key: Array(32).fill(3), sheet_origin: projectId, faces: [faceId, otherFaceId] }],
} }
Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (command: string, args?: Record<string, unknown>) => {
  commands.push(command === 'preview_instruction_export' ? `${command}:${args?.format}` : command)
  if (command === 'project_snapshot') return snapshot
  if (command === 'get_recovery_candidate') return { schema_version: 1, status: 'none' }
  if (command === 'analyze_project_topology') return topology
  if (command === 'begin_instruction_export') return { export_id: 'app-miura-export', profile: 'instruction_export_v1' }
  if (command === 'preview_instruction_export') return { preview: preview(args?.format as 'pdf' | 'svg_zip') }
  if (command === 'get_instruction_export_progress') return { progress: { export_id: 'app-miura-export', phase: 'ready', completed_units: 3, total_units: 3 } }
  if (command === 'save_instruction_export') { if (saveMode === 'failure') throw new Error('atomic-save-failed'); return { canceled: saveMode === 'cancel' } }
  if (command === 'cancel_instruction_export') return null
  if (command.startsWith('plugin:')) return null
  throw new Error(`unexpected ${command}`)
}, transformCallback: () => 1, unregisterCallback: () => undefined, metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } } } })

const { default: App } = await import('../src/App')
createRoot(document.getElementById('root')!).render(<App />)
