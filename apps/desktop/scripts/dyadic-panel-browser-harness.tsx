import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { StackedFoldPanel } from '../src/components/StackedFoldPanel'
import type { ProjectSnapshot } from '../src/lib/coreClient'

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'
const token = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const parameters = new URLSearchParams(location.search)
const requestedHingeCount = Number(parameters.get('hinges'))
const hingeCount = [6, 8, 16].includes(requestedHingeCount) ? requestedHingeCount : 6
const hinges = Array.from({ length: hingeCount }, (_, index) => `018f47a2-4b7a-7cc1-8abc-${String(index + 1).padStart(12, '0')}`)
const hash = 'a'.repeat(64); const positive = 'b'.repeat(64); const layer = 'c'.repeat(64)
const scenario = parameters.get('scenario') ?? 'success'
const evidence = { reads: 0, readHinges: 0, readScheduleHinges: 0, mints: 0, mintHinges: 0, mintScheduleHinges: 0, applyAttempts: 0, mutations: 0, failures: 0, cancels: 0, timelineDtos: 0, undos: 0, redos: 0, reopens: 0 }
let callback = 0
let liveRevision = 1
let consumed = false
let releaseRead: ((value: unknown) => void) | null = null
const scheduleEntryCount = (args?: { request?: Record<string, unknown> }) => {
  const schedule = args?.request?.cycleScheduleV1
  if (typeof schedule !== 'object' || schedule === null || !('entries' in schedule)) return 0
  return Array.isArray(schedule.entries) ? schedule.entries.length : 0
}
Object.assign(window, {
  __ORIGAMI2_DYADIC_PANEL_EVIDENCE__: evidence,
  __TAURI_INTERNALS__: {
    transformCallback: () => ++callback,
    invoke: async (command: string, args?: { request?: Record<string, unknown> }) => {
      if (command === 'plugin:event|listen') return 1
      if (command === 'plugin:event|unlisten') return null
      if (command === 'cancel_current_stacked_fold_read_v1') { evidence.cancels++; releaseRead?.(null); releaseRead = null; return null }
      if (command === 'read_live_hinge_registry_v1') return { version: 1, projectInstanceId: instance, projectId: project, revision: liveRevision, poseGeneration: 1, graphFingerprintSha256: 'd'.repeat(64), entries: hinges.map(edge => ({ edge, initialAngleDegrees: 0 })), authorizesProjectMutation: false }
      if (command === 'read_even_cycle_candidates_v1') return { version: 1, projectInstanceId: instance, projectId: project, revision: liveRevision, status: 'ready', reason: 'one bounded candidate', candidates: [{ version: 1, edges: [hinges[0], hinges[3]], reason: 'same_assignment_geometrically_opposite' }], kawasakiEndpoints: [], authorizesProjectMutation: false }
      if (command === 'read_bounded_dyadic_pose_graph_v1') { evidence.reads++; evidence.readHinges = Array.isArray(args?.request?.targetAngles) ? args.request.targetAngles.length : 0; evidence.readScheduleHinges = scheduleEntryCount(args); if (scenario === 'cancel') return new Promise(resolve => { releaseRead = resolve }); return { version: 1, projectInstanceId: instance, projectId: project, revision: 1, status: 'certified', stateCount: 3, transitionCount: 4, exploredStateCount: 3, evaluatedTransitionCount: 1, certifiedTransitionCount: 1, certificateBindingSha256: hash, positiveThicknessTransitionCount: 1, positiveThicknessCertified: true, positiveThicknessBindingSha256: positive, layerTransportTransitionCount: 1, layerTransportCertified: true, layerTransportBindingSha256: layer, mutationCandidateReady: true, authorizesProjectMutation: false } }
      if (command === 'mint_dyadic_pose_path_preview_v1') { evidence.mints++; evidence.mintHinges = Array.isArray(args?.request?.targetAngles) ? args.request.targetAngles.length : 0; evidence.mintScheduleHinges = scheduleEntryCount(args); return { version: 1, previewToken: token, projectInstanceId: instance, projectId: project, revision: 1, targetBindingSha256: 'e'.repeat(64), pathBindingSha256: hash, positiveThicknessBindingSha256: positive, layerTransportBindingSha256: layer, authorizesProjectMutation: false } }
      if (command === 'apply_dyadic_pose_path_preview_v1') {
        evidence.applyAttempts++
        const request = args?.request
        const invalid = consumed || scenario !== 'success'
          || request?.expectedRevision !== liveRevision || request?.expectedProjectInstanceId !== instance
          || request?.expectedTargetBindingSha256 !== 'e'.repeat(64) || request?.expectedPathBindingSha256 !== hash
        if (invalid) { evidence.failures++; throw new Error(`${scenario} apply rejected`) }
        consumed = true; evidence.mutations++; evidence.timelineDtos = 2; liveRevision = 2; return 2
      }
      throw new Error(`unexpected command ${command}`)
    },
  },
})

function Harness() {
  const [snapshot, setSnapshot] = useState({ project_instance_id: instance, project_id: project, revision: 1 } as ProjectSnapshot)
  const [notice, setNotice] = useState('ready')
  return <main>
    <StackedFoldPanel locale="en" snapshot={snapshot} selectedLine={{ id: hinges[0]!, start: { x: 0, y: 0 }, end: { x: 1, y: 0 } }} disabled={false}
      refreshSnapshot={async () => ({ ...snapshot, revision: 2 }) as ProjectSnapshot}
      onApplied={value => { setSnapshot(value); setNotice('applied-revision-2-timeline-dto-2') }} />
    <button onClick={() => { evidence.undos++; setNotice('undone') }}>undo</button>
    <button onClick={() => { evidence.redos++; setNotice('redone') }}>redo</button>
    <button onClick={() => { evidence.reopens++; setNotice('reopened-timeline-dto-2') }}>reopen</button>
    <output>{notice}</output>
  </main>
}
createRoot(document.getElementById('root')!).render(<Harness />)
