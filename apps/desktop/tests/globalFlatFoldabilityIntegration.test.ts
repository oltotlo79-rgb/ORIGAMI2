import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = read('../src/App.tsx')
const coordinatorSource = read('../src/lib/globalFlatFoldabilityCoordinator.ts')
const panelSource = read('../src/components/GlobalFlatFoldabilityPanel.tsx')

test('App owns one native coordinator and disposes it with the mounted observer', () => {
  assert.match(
    appSource,
    /const globalFlatFoldabilityCoordinatorRef =\s*useRef<GlobalFlatFoldabilityCoordinator \| null>\(null\)/u,
  )
  const ownership = section(
    appSource,
    'const coordinator = createGlobalFlatFoldabilityCoordinator<number>({',
    '  useEffect(() => {\n    if (!isNativeCoreAvailable()) return\n    getProjectSnapshot()',
  )
  assert.match(
    ownership,
    /transport: createGlobalFlatFoldabilityNativeTransport\(\)/u,
  )
  assert.match(
    ownership,
    /setTimeout: \(callback, delayMs\) => window\.setTimeout\(callback, delayMs\)/u,
  )
  assert.match(
    ownership,
    /clearTimeout: \(handle\) => window\.clearTimeout\(handle\)/u,
  )
  assert.match(
    ownership,
    /onState: \(\{ job \}\) => \{\s*if \(mounted\) setGlobalFlatFoldabilityJob\(job\)/u,
  )
  assert.match(ownership, /mounted = false/u)
  assert.match(
    ownership,
    /globalFlatFoldabilityCoordinatorRef\.current = null/u,
  )
  assert.match(ownership, /coordinator\.dispose\(\)/u)
})

test('snapshot changes invalidate before publishing and replacements are explicit', () => {
  const applySnapshot = section(
    appSource,
    'const applySnapshot = useCallback',
    'const nativeLines = useMemo',
  )
  assert.match(
    applySnapshot,
    /latestSnapshotRef\.current = snapshot\s*globalFlatFoldabilityCoordinatorRef\.current\?\.invalidate\(\{\s*projectId: snapshot\.project_id,\s*revision: snapshot\.revision,\s*foldModelFingerprint: snapshot\.fold_model_fingerprint,\s*\}, forceReplacement\)\s*setNativeSnapshot\(snapshot\)/u,
  )
  assert.match(
    appSource,
    /const snapshot = await action\(current\.project_id, current\.revision\)\s*applySnapshot\(snapshot\)/u,
  )
  assert.match(
    appSource,
    /applySnapshot\(\s*response\.project,\s*operation === 'open' && !response\.canceled,\s*\)/u,
  )
  assert.equal(
    [...appSource.matchAll(/applySnapshot\(snapshot, true\)/gu)].length,
    3,
    'new project, FOLD import and SVG import are snapshot replacements',
  )

  const start = section(
    appSource,
    'const startGlobalFlatFoldability = useCallback',
    'const cancelGlobalFlatFoldability = useCallback',
  )
  assert.match(start, /const current = latestSnapshotRef\.current/u)
  assert.match(
    start,
    /if \(\s*!current\s*\|\| coreOperationRef\.current\s*\|\| benchmarkLoading\s*\|\| benchmarkRun\s*\) return/u,
  )
  assert.match(
    start,
    /globalFlatFoldabilityCoordinatorRef\.current\?\.start\(\s*\{\s*projectId: current\.project_id,\s*revision: current\.revision,\s*foldModelFingerprint: current\.fold_model_fingerprint,\s*\},\s*timeLimitSeconds/u,
  )
})

test('the global panel is controlled by coordinator state and exposes start and cancel', () => {
  assert.match(
    appSource,
    /useState<GlobalFlatFoldabilityTimePreset>\(\s*DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET/u,
  )
  assert.match(
    appSource,
    /<GlobalFlatFoldabilityPanel\s+job=\{globalFlatFoldabilityJob\}\s+timeLimitSeconds=\{globalFlatFoldabilityTimeLimit\}/u,
  )
  assert.match(
    appSource,
    /onTimeLimitChange=\{setGlobalFlatFoldabilityTimeLimit\}\s+onStart=\{startGlobalFlatFoldability\}\s+onCancel=\{cancelGlobalFlatFoldability\}/u,
  )
  assert.doesNotMatch(
    appSource,
    /\{!benchmarkRun && \(\s*<GlobalFlatFoldabilityPanel/u,
  )
  assert.match(
    appSource,
    /startDisabled=\{\s*coreBusy\s*\|\| benchmarkLoading\s*\|\| Boolean\(benchmarkRun\)\s*\|\| !nativeSnapshot\s*\|\| !isNativeCoreAvailable\(\)/u,
  )
  assert.match(
    appSource,
    /const cancelGlobalFlatFoldability = useCallback\(\(\) => \{\s*globalFlatFoldabilityCoordinatorRef\.current\?\.cancel\(\)/u,
  )
  assert.ok(
    appSource.indexOf('局所平坦折り条件')
      < appSource.indexOf('<GlobalFlatFoldabilityPanel'),
  )
  assert.match(panelSource, /role="status"/u)
  assert.match(panelSource, /aria-live="polite"/u)
  assert.match(panelSource, /ref=\{cancelButtonRef\}/u)
})

test('opaque transport identity stays inside the coordinator boundary', () => {
  assert.match(
    coordinatorSource,
    /import \{\s*GlobalFlatFoldabilityNativeError,\s*type GlobalFlatFoldabilityNativeBegin,\s*type GlobalFlatFoldabilityNativeContext,\s*type GlobalFlatFoldabilityNativeTransport/u,
  )
  assert.match(
    coordinatorSource,
    /error instanceof GlobalFlatFoldabilityNativeError/u,
  )
  const publicState = section(
    coordinatorSource,
    'export type GlobalFlatFoldabilityCoordinatorState',
    'export type GlobalFlatFoldabilityCoordinatorOptions',
  )
  assert.deepEqual(
    [...publicState.matchAll(/^\s{2}([A-Za-z]+):/gmu)]
      .map((match) => match[1]),
    ['generation', 'job'],
  )
  assert.doesNotMatch(publicState, /jobId|projectId|revision/u)
  assert.doesNotMatch(
    appSource,
    /begin_global_flat_foldability|get_global_flat_foldability_(progress|result)|cancel_global_flat_foldability/u,
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
