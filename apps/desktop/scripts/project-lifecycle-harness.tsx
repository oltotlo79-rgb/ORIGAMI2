import { useEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createRecoveryClient, createWindowCloseHandshake, createWindowCloseHandshakeState } from '../src/lib/recoveryClient.ts'

const INSTANCE = '11111111-1111-4111-8111-111111111111'
const PROJECT = '22222222-2222-4222-8222-222222222222'
const PREPARE = '33333333-3333-4333-8333-333333333333'
const evidence = { saveCalls: 0, maximumActiveSaves: 0, closeRequests: 0, prepareCalls: 0, recoveryCalls: 0 }
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
    evidence.saveCalls += 1
    evidence.maximumActiveSaves = 1
    await new Promise((resolve) => setTimeout(resolve, 120))
    const call = evidence.saveCalls
    setNotice(call === 1 ? 'save-canceled' : call === 2 ? 'save-failed' : 'saved')
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
