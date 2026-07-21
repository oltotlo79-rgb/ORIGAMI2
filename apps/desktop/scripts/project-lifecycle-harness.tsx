import { useEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createRecoveryClient, createWindowCloseHandshake, createWindowCloseHandshakeState } from '../src/lib/recoveryClient.ts'
import { createProjectFileClient, ProjectFileClientError } from '../src/lib/projectFileClient.ts'

const INSTANCE = '11111111-1111-4111-8111-111111111111'
const PROJECT = '22222222-2222-4222-8222-222222222222'
const PREPARE = '33333333-3333-4333-8333-333333333333'
const evidence = { saveCalls: 0, maximumActiveSaves: 0, closeRequests: 0, prepareCalls: 0, recoveryCalls: 0,
  strictRejects: 0, concurrentRejects: 0, reopenedExpressionBits: '', reopenedCanUndo: false }
let activeSaves = 0
const projectFile = createProjectFileClient(async (command) => {
  if (command === 'save_project_as') {
    evidence.saveCalls += 1
    activeSaves += 1
    evidence.maximumActiveSaves = Math.max(evidence.maximumActiveSaves, activeSaves)
    await new Promise((resolve) => setTimeout(resolve, 120))
    activeSaves -= 1
    if (evidence.saveCalls === 1) return { canceled: true, project: snapshot(false) }
    if (evidence.saveCalls === 2) throw new Error('redacted disk failure')
    return { canceled: false, project: snapshot(false) }
  }
  if (command === 'open_project') return { canceled: false, project: snapshot(false) }
  return { canceled: false, project: snapshot(false) }
})
const recovery = createRecoveryClient(async () => {
  evidence.recoveryCalls += 1
  return { schema_version: 1, status: 'discarded' }
})
Object.assign(window, { __ORIGAMI2_PROJECT_LIFECYCLE__: evidence })

function Harness() {
  const [notice, setNotice] = useState('dirty')
  const [confirming, setConfirming] = useState(false)
  const [saving, setSaving] = useState(false)
  const opener = useRef<HTMLButtonElement>(null)
  const cancel = useRef<HTMLButtonElement>(null)
  const saveLatch = useRef(false)
  const handshake = useRef<ReturnType<typeof createWindowCloseHandshake> | null>(null)
  useEffect(() => {
    const state = createWindowCloseHandshakeState()
    handshake.current = createWindowCloseHandshake(state, {
      getBlocker: () => null,
      getProjectState: () => ({ project_instance_id: INSTANCE, project_id: PROJECT, revision: 7, is_dirty: true }),
      confirmDiscard: () => true,
      prepare: async (_expected, authorization) => {
        evidence.prepareCalls += 1
        await new Promise((resolve) => setTimeout(resolve, 120))
        return { schema_version: 1, status: 'prepared', close_prepare_id: PREPARE,
          project_instance_id: INSTANCE, project_id: PROJECT, revision: 7, authorization }
      },
      cancel: async (prepared) => ({ ...prepared, status: 'canceled' }),
      requestClose: async () => { evidence.closeRequests += 1 },
      setInteractionLocked: () => {},
      setStatus: setNotice,
      reportFailure: () => setNotice('failed'),
    })
    return () => handshake.current?.dispose()
  }, [])
  useEffect(() => { if (!confirming) opener.current?.focus(); else cancel.current?.focus() }, [confirming])

  const save = async () => {
    if (saveLatch.current) return
    saveLatch.current = true
    setSaving(true)
    try {
      const response = await projectFile.run('save_as')
      setNotice(response.canceled ? 'save-canceled' : 'saved')
    } catch {
      setNotice('save-failed')
    }
    saveLatch.current = false
    setSaving(false)
  }
  const close = () => setConfirming(true)
  const dismiss = () => setConfirming(false)
  const confirm = () => {
    setConfirming(false)
    handshake.current?.handle({ preventDefault() {} })
    handshake.current?.handle({ preventDefault() {} })
  }
  return <main>
    <button ref={opener} onClick={close}>Close project</button>
    <button onClick={() => {
      handshake.current?.handle({ preventDefault() {} })
      handshake.current?.dispose()
    }}>Start stale close</button>
    <button disabled={saving} onClick={() => void save()}>{saving ? 'Saving' : 'Save project as'}</button>
    <button onClick={() => void projectFile.run('open').then((response) => {
      const expression = response.project.numeric_expressions.rectangular_paper_creation
      evidence.reopenedExpressionBits = expression
        ? float64Bits(expression.adopted_width_mm)
        : ''
      evidence.reopenedCanUndo = response.project.can_undo
      setNotice('reopened')
    })}>Reopen project</button>
    <button onClick={() => void testStrictBoundary(setNotice)}>Test invalid responses</button>
    <button onClick={() => void recovery.discard({
      schema_version: 1,
      status: 'available',
      recovery_id: '44444444-4444-4444-8444-444444444444',
      project_id: PROJECT,
      updated_at_unix_ms: 1,
    }).then(() => setNotice('recovery-discarded'))}>Discard recovery</button>
    <output role="status">{notice}</output>
    {confirming && <section role="dialog" aria-label="Discard dirty project?" onKeyDown={(event) => {
      if (event.key === 'Escape') { event.preventDefault(); dismiss() }
      if (event.key === 'Tab') {
        event.preventDefault()
        ;(document.activeElement === cancel.current ? event.currentTarget.querySelectorAll('button')[1] : cancel.current)?.focus()
      }
    }}>
      <p>Unsaved changes will be discarded.</p>
      <button ref={cancel} onClick={dismiss}>Cancel</button><button onClick={confirm}>Discard and close</button>
    </section>}
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)

async function testStrictBoundary(setNotice: (value: string) => void) {
  const invalids = [
    { canceled: false, project: snapshot(true), path: 'C:\\private\\bird.ori2' },
    { canceled: false, project: { ...snapshot(true), revision: -1 } },
  ]
  for (const invalid of invalids) {
    const client = createProjectFileClient(async () => invalid)
    try { await client.run('open') } catch (error) {
      if (error instanceof ProjectFileClientError && error.code === 'invalid_response') evidence.strictRejects += 1
    }
  }
  let release!: () => void
  const held = new Promise<void>((resolve) => { release = resolve })
  const concurrent = createProjectFileClient(async () => { await held; return { canceled: false, project: snapshot(true) } })
  const first = concurrent.run('save')
  try { await concurrent.run('open') } catch (error) {
    if (error instanceof ProjectFileClientError && error.code === 'busy') evidence.concurrentRejects += 1
  }
  release(); await first
  setNotice('strict-rejected')
}

function snapshot(dirty: boolean) {
  return {
    project_instance_id: INSTANCE, project_id: PROJECT, name: 'Lifecycle', memo: '', current_path: null,
    revision: 7, saved_revision: dirty ? 6 : 7, is_dirty: dirty,
    paper: { boundary_vertices: [], thickness_mm: 0.1, length_display_unit: 'mm', cutting_allowed: false,
      front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null },
      back: { color: { red: 248, green: 248, blue: 245, alpha: 255 }, texture_asset: null } },
    crease_pattern: { vertices: [], edges: [] }, instruction_timeline: { steps: [] },
    numeric_expressions: { rectangular_paper_creation: { schema_version: 1, width_source: 'sqrt(2)', height_source: '1', adopted_width_mm: Math.SQRT2, adopted_height_mm: 1 }, undo_stack: [null], redo_stack: [] },
    geometric_constraints: { schema_version: 1, constraints: [] },
    beginner_design_profile: { schema_version: 1, preset: 'balanced', shape_fidelity_weight: 35, foldability_weight: 35,
      step_count_weight: 15, paper_efficiency_weight: 15, generation_constraints: { schema_version: 1, maximum_steps: 60,
        detail_level: 'standard', target_category: null, target_parts: [], skeleton_segments: [], protrusions: [], bulge_targets: [],
        target_asset: null, allowed_techniques: ['valley_fold', 'mountain_fold'] } },
    project_layers: { schema_version: 1, layers: [{ id: '00000000-0000-4000-8000-000000000001', name: 'Crease Pattern',
      content_kind: 'crease_pattern', visible: true, locked: false, opacity: 1 }], edge_assignments: [] },
    element_metadata: { vertices: [], edges: [], faces: [] }, annotations: { schema_version: 1, annotations: [] },
    underlays: { schema_version: 1, underlays: [] }, fold_model_fingerprint: 'a'.repeat(64), can_undo: true, can_redo: false,
    cutting_allowed: false,
  }
}

function float64Bits(value: number): string {
  const bytes = new ArrayBuffer(8)
  const view = new DataView(bytes)
  view.setFloat64(0, value, false)
  return view.getBigUint64(0, false).toString()
}
