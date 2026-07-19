import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = read('../src/App.tsx')
const overlaySource = read('../src/components/RecoveryStartupOverlay.tsx')
const autosaveStatusSource = read(
  '../src/lib/recoveryAutosaveStatusClient.ts',
)
const appCss = read('../src/App.css')

test('native startup single-flights one snapshot with strict recovery discovery', () => {
  assert.match(
    appSource,
    /getProjectSnapshot as requestProjectSnapshot/u,
  )
  assert.match(
    appSource,
    /const initialProjectSnapshotRequestRef =\s*useRef<Promise<ProjectSnapshot> \| null>\(null\)/u,
  )
  const snapshotSingleFlight = section(
    appSource,
    'const getProjectSnapshot = useCallback',
    'const analyzeCurrentGeometricConstraints',
  )
  assert.match(
    snapshotSingleFlight,
    /const pending = initialProjectSnapshotRequestRef\.current\s*if \(pending\) return pending/u,
  )
  assert.match(
    snapshotSingleFlight,
    /Promise\.resolve\(\)\.then\(\(\) => requestProjectSnapshot\(\)\)/u,
  )
  assert.equal(
    snapshotSingleFlight.match(/requestProjectSnapshot\(\)/gu)?.length,
    1,
  )

  const startupEffect = section(
    appSource,
    '  useEffect(() => {\n    if (!isNativeCoreAvailable()) return\n    getProjectSnapshot()',
    '  useEffect(() => {\n    const current = nativeStaticCollisionRequest',
  )
  assert.match(startupEffect, /if \(recoveryStartupStartedRef\.current\) return/u)
  assert.match(startupEffect, /recoveryStartupStartedRef\.current = true/u)
  assert.match(startupEffect, /void checkRecoveryStartup\(false\)/u)

  const discovery = section(
    appSource,
    'const checkRecoveryStartup = useCallback',
    'const restoreStartupRecovery = useCallback',
  )
  assert.match(
    discovery,
    /Promise\.all\(\[\s*getProjectSnapshot\(\),\s*getRecoveryCandidate\(\),\s*\]\)/u,
  )
  assert.match(
    discovery,
    /if \(refreshSnapshot\) initialProjectSnapshotRequestRef\.current = null/u,
  )
})

test('none, candidate, failure, and retry are closed startup transitions', () => {
  assert.match(
    appSource,
    /useState<RecoveryStartupState>\(\s*\(\) => isNativeCoreAvailable\(\)\s*\? \{ kind: 'checking' \}\s*: \{ kind: 'ready' \}/u,
  )
  const discovery = section(
    appSource,
    'const checkRecoveryStartup = useCallback',
    'const restoreStartupRecovery = useCallback',
  )
  assert.match(
    discovery,
    /applySnapshot\(snapshot\)\s*if \(candidate\.status === 'none'\) \{\s*setRecoveryStartup\(\{ kind: 'ready' \}\)/u,
  )
  assert.match(
    discovery,
    /setRecoveryStartup\(\{ kind: 'candidate', candidate \}\)/u,
  )
  assert.match(
    discovery,
    /\} catch \{\s*if \([\s\S]*?setRecoveryStartup\(\{ kind: 'failed' \}\)/u,
  )
  assert.doesNotMatch(discovery, /catch\s*\(\s*\w+/u)
  assert.doesNotMatch(discovery, /String\(|recovery_id:/u)

  assert.match(
    appSource,
    /const retryRecoveryStartup = useCallback\(\(\) => \{\s*return checkRecoveryStartup\(true\)/u,
  )
  assert.doesNotMatch(
    appSource,
    /recovery_id:\s*['"`]|project_id:\s*['"`][0-9a-f]{8}-/u,
  )
})

test('every editor region stays inert behind checking, failure, or a candidate', () => {
  const modal = section(
    appSource,
    'const modalOpen = newProjectOpen',
    'const closeDiagnosticsDialog',
  )
  assert.match(modal, /\|\| recoveryBlocking/u)
  assert.equal(
    appSource.match(/inert=\{modalOpen\}/gu)?.length,
    5,
  )
  assert.match(
    appSource,
    /\(recoveryStartup\.kind === 'checking'\s*\|\| recoveryStartup\.kind === 'failed'\) && \(\s*<RecoveryStartupOverlay/u,
  )
  assert.match(
    appSource,
    /recoveryStartup\.kind === 'candidate' && \(\s*<RecoveryDialog/u,
  )
  assert.match(
    appSource,
    /candidate=\{recoveryStartup\.candidate\}\s+busy=\{recoveryActionBusy\}\s+error=\{recoveryActionError\}/u,
  )
  assert.match(overlaySource, /There is intentionally no close/u)
  assert.doesNotMatch(overlaySource, /onClose|rawError|error:/u)
})

test('restore binds the latest snapshot and performs a force replacement reset', () => {
  const restore = section(
    appSource,
    'const restoreStartupRecovery = useCallback',
    'const discardStartupRecovery = useCallback',
  )
  assert.match(restore, /const current = latestSnapshotRef\.current/u)
  assert.match(
    restore,
    /restoreRecoveryCandidate\(candidate, \{\s*project_instance_id: current\.project_instance_id,\s*project_id: current\.project_id,\s*revision: current\.revision,\s*\}\)/u,
  )
  assert.match(restore, /latestSnapshotRef\.current !== current/u)
  assert.match(restore, /sameRecoveryCandidate\(recoveryStartupRef\.current, candidate\)/u)
  assert.match(restore, /applySnapshot\(recoveredSnapshot, true\)/u)
  assert.match(restore, /resetRecoveredProjectUi\(\)/u)
  assert.match(restore, /setRecoveryStartup\(\{ kind: 'ready' \}\)/u)
  assert.doesNotMatch(restore, /catch\s*\(\s*\w+|String\(/u)

  const reset = section(
    appSource,
    'const resetRecoveredProjectUi = useCallback',
    'const checkRecoveryStartup = useCallback',
  )
  for (const required of [
    'benchmarkRequestIdRef.current += 1',
    'setBenchmarkRun(null)',
    'setSelectedLineId(null)',
    'setSelectedVertexId(null)',
    'setPendingEdgeStart(null)',
    'setParallelReferenceEdgeId(null)',
    'setAppliedFoldPose(null)',
    'setFoldAngleOverrides({ projectId: null, values: new Map() })',
    'setFixedFaceChoice({ projectId: null, faceId: null })',
    "setActiveTool('select')",
    'setCancelInteractionToken((token) => token + 1)',
  ]) {
    assert.ok(reset.includes(required), `missing recovery reset: ${required}`)
  }
})

test('discard, retry, stale responses, and StrictMode have explicit ownership', () => {
  const discard = section(
    appSource,
    'const discardStartupRecovery = useCallback',
    'const retryRecoveryStartup = useCallback',
  )
  assert.match(discard, /await discardRecoveryCandidate\(candidate\)/u)
  assert.match(discard, /setRecoveryStartup\(\{ kind: 'ready' \}\)/u)
  assert.match(discard, /setRecoveryActionError\(true\)/u)
  assert.doesNotMatch(discard, /catch\s*\(\s*\w+|String\(/u)

  assert.match(
    appSource,
    /recoveryMountedRef\.current = true\s*return \(\) => \{\s*recoveryMountedRef\.current = false/u,
  )
  assert.match(
    appSource,
    /const requestId = \+\+recoveryRequestSequenceRef\.current/gu,
  )
  assert.match(
    appSource,
    /!recoveryMountedRef\.current\s*\|\| requestId !== recoveryRequestSequenceRef\.current/gu,
  )
  assert.match(
    appSource,
    /recoveryOperationRef\.current\s*\|\| !sameRecoveryCandidate/u,
  )
  assert.match(
    appSource,
    /function sameRecoveryCandidate\(/u,
  )
})

test('modal, keyboard, file, edit, and close guards all include recovery', () => {
  const keyboard = section(
    appSource,
    'function handleKeyboardShortcut(event: KeyboardEvent)',
    "window.addEventListener('keydown', handleKeyboardShortcut)",
  )
  assert.match(
    keyboard,
    /if \(recoveryBlocking\) \{\s*if \(key === 'escape'\) event\.preventDefault\(\)\s*return/u,
  )

  const fileOperation = section(
    appSource,
    "async function runFileOperation(operation: 'open' | 'save' | 'save_as')",
    'async function beginFoldImport',
  )
  assert.match(fileOperation, /\|\| recoveryBlockingRef\.current/u)

  const editOperation = section(
    appSource,
    'const runNativeEdit = useCallback',
    'const addSelectedEdgeOrientationConstraint',
  )
  assert.match(editOperation, /\|\| recoveryBlockingRef\.current/u)

  const closeGuard = section(
    appSource,
    'const closeHandshake = createWindowCloseHandshake',
    '}).then((stopListening)',
  )
  assert.match(
    closeGuard,
    /recoveryBlockingRef\.current\s*\|\| recoveryOperationRef\.current/u,
  )
  const recoveryCloseBranch = section(
    closeGuard,
    'getBlocker: () => {',
    'getProjectState: () => {',
  )
  assert.doesNotMatch(recoveryCloseBranch, /window\.confirm/u)
  assert.match(closeGuard, /prepare: prepareWindowClose/u)
  assert.match(closeGuard, /cancel: cancelWindowClosePrepare/u)
  assert.match(closeGuard, /requestClose: \(\) => appWindow\.close\(\)/u)
  assert.match(
    closeGuard,
    /setInteractionLocked: \(locked\) => \{\s*coreOperationRef\.current = locked\s*if \(recoveryMountedRef\.current\) setCoreBusy\(locked\)/u,
  )
  assert.match(
    closeGuard,
    /coreOperationRef\.current\s*&& !windowCloseHandshakeStateRef\.current\.interaction_locked/u,
  )
  assert.match(closeGuard, /closeHandshake\.handle\(event\)/u)
})

test('autosave persistence health uses a guarded five-second poll and an independent persistent warning', () => {
  assert.match(
    appSource,
    /createRecoveryAutosaveStatusPoller,\s*type RecoveryAutosaveMonitorView/u,
  )
  assert.match(
    appSource,
    /useState<RecoveryAutosaveMonitorView>\(\(\) => \(\s*isNativeCoreAvailable\(\)\s*\? \{ kind: 'checking' \}\s*: \{ kind: 'inactive' \}/u,
  )
  const polling = section(
    appSource,
    '  useEffect(() => {\n    const nativeAvailable = isNativeCoreAvailable()',
    "  useEffect(() => {\n    if (!isNativeCoreAvailable()) {\n      setHistoryLimitLoadState",
  )
  assert.match(
    polling,
    /if \(!nativeAvailable \|\| recoveryStartup\.kind !== 'ready'\) return/u,
  )
  assert.match(polling, /onChange: setRecoveryAutosaveMonitor/u)
  assert.match(polling, /document\.visibilityState === 'visible'/u)
  assert.match(polling, /window\.addEventListener\('focus'/u)
  assert.match(polling, /poller\.dispose\(\)/u)
  assert.doesNotMatch(polling, /setCoreStatus|String\(|catch\s*\(\s*\w+/u)

  assert.match(
    appSource,
    /<RecoveryAutosaveStatusBanner view=\{recoveryAutosaveMonitor\} \/>/u,
  )
  assert.match(
    autosaveStatusSource,
    /RECOVERY_AUTOSAVE_STATUS_POLL_INTERVAL_MS = 5_000/u,
  )
  assert.match(
    autosaveStatusSource,
    /nativeInvoke\('get_recovery_autosave_status'\)/u,
  )
  assert.doesNotMatch(
    autosaveStatusSource,
    /@tauri-apps\/api\/event|(?:raw_)?error:|current_path|project_id/u,
  )
  assert.match(appCss, /\.recovery-autosave-warning \{\s*position: fixed;/u)
  assert.match(
    appCss,
    /\.recovery-autosave-warning\.is-persistence-failed/u,
  )
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function section(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}
