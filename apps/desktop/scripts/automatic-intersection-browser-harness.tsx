import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { addEdge, redo, undo } from '../src/lib/coreClient.ts'

const INSTANCE = '11111111-1111-4111-8111-111111111111'
const PROJECT = '22222222-2222-4222-8222-222222222222'
const ids = Array.from({ length: 20 }, (_, index) => `00000000-0000-4000-8000-${String(index + 10).padStart(12, '0')}`)
const evidence = { addCalls: 0, undoCalls: 0, redoCalls: 0, saveCalls: 0, reopenCalls: 0, duplicateRejects: 0 }
let scenario = 'balloon'; let revision = 7; let saved: ReturnType<typeof snapshot> | null = null
Object.assign(window, { __ORIGAMI2_AUTOMATIC_INTERSECTION__: evidence, __TAURI_INTERNALS__: { invoke: async (command: string) => {
  if (command === 'add_edge') { evidence.addCalls += 1; if (scenario === 'duplicate') { evidence.duplicateRejects += 1; throw new Error('duplicate edge') } revision += 1; return snapshot(scenario) }
  if (command === 'undo') { evidence.undoCalls += 1; revision += 1; return snapshot('original') }
  if (command === 'redo') { evidence.redoCalls += 1; revision += 1; return snapshot(scenario) }
  throw new Error(`unexpected command ${command}`)
} } })

function snapshot(kind: string) {
  const counts: Record<string, [number, number]> = { balloon: [7, 6], multiple: [8, 7], endpoint: [4, 3], original: [6, 2] }
  const [vertexCount, edgeCount] = counts[kind] ?? [6, 2]
  const vertices = ids.slice(0, vertexCount).map((id, index) => ({ id, position: { x: index, y: index % 3 } }))
  const edges = Array.from({ length: edgeCount }, (_, index) => ({ id: ids[index + 10], start: vertices[index % vertexCount].id, end: vertices[(index + 1) % vertexCount].id, kind: index % 2 ? 'valley' : 'mountain' }))
  return { project_instance_id: INSTANCE, project_id: PROJECT, name: '交差回帰', memo: '', current_path: null, revision, saved_revision: revision - 1, is_dirty: true,
    paper: { boundary_vertices: [], thickness_mm: 0.1, length_display_unit: 'mm', cutting_allowed: false, front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null }, back: { color: { red: 248, green: 248, blue: 245, alpha: 255 }, texture_asset: null } },
    crease_pattern: { vertices, edges }, instruction_timeline: { steps: [] }, numeric_expressions: { rectangular_paper_creation: null, undo_stack: [], redo_stack: [] }, geometric_constraints: { schema_version: 1, constraints: [] },
    beginner_design_profile: { schema_version: 1, preset: 'balanced', shape_fidelity_weight: 35, foldability_weight: 35, step_count_weight: 15, paper_efficiency_weight: 15, generation_constraints: { schema_version: 1, maximum_steps: 60, detail_level: 'standard', target_category: null, target_parts: [], skeleton_segments: [], protrusions: [], bulge_targets: [], target_asset: null, allowed_techniques: ['valley_fold', 'mountain_fold'] } },
    project_layers: { schema_version: 1, layers: [{ id: '00000000-0000-4000-8000-000000000001', name: 'Crease Pattern', content_kind: 'crease_pattern', visible: true, locked: false, opacity: 1 }], edge_assignments: [] }, element_metadata: { vertices: [], edges: [], faces: [] }, annotations: { schema_version: 1, annotations: [] }, underlays: { schema_version: 1, underlays: [] }, fold_model_fingerprint: 'a'.repeat(64), can_undo: true, can_redo: false, cutting_allowed: false }
}

function Harness() {
  const [current, setCurrent] = useState(snapshot('original')); const [notice, setNotice] = useState('ready')
  const run = async (next: string) => { scenario = next; try { setCurrent(await addEdge(PROJECT, revision, INSTANCE, ids[0], ids[1], 'mountain')); setNotice(`${next}-ok`) } catch { setNotice('duplicate-rejected') } }
  return <main><h1>新規折り線の自動交差分割</h1>
    {['balloon', 'multiple', 'endpoint', 'duplicate'].map((name) => <button key={name} onClick={() => void run(name)}>{name}</button>)}
    <button onClick={() => void undo(PROJECT, revision, INSTANCE).then(setCurrent)}>undo</button>
    <button onClick={() => void redo(PROJECT, revision, INSTANCE).then(setCurrent)}>redo</button>
    <button onClick={() => { evidence.saveCalls += 1; saved = structuredClone(current); setNotice('saved') }}>save</button>
    <button onClick={() => { evidence.reopenCalls += 1; if (saved) setCurrent(structuredClone(saved)); setNotice('reopened') }}>reopen</button>
    <output>{notice}</output><p data-testid="topology">vertices={current.crease_pattern.vertices.length};edges={current.crease_pattern.edges.length}</p>
    <svg aria-label="展開図" width="300" height="160">{current.crease_pattern.edges.map((edge, index) => <line key={edge.id} x1={10} y1={10 + index * 15} x2={280} y2={10 + index * 15} stroke="black" />)}</svg>
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
