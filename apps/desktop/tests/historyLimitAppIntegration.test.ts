import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)

test('App loads the history limit only for the exact current project binding', () => {
  const effect = section(
    appSource,
    '  useEffect(() => {\n    if (!isNativeCoreAvailable()) {\n      setHistoryLimitLoadState',
    '  useEffect(() => {\n    const current = nativeStaticCollisionRequest',
  )

  assert.match(effect, /historyLimitClient\.get\(expected\)/u)
  assert.match(effect, /const requestId = \+\+historyLimitRequestSequenceRef\.current/u)
  assert.match(effect, /disposed\s*\|\| requestId !== historyLimitRequestSequenceRef\.current/u)
  assert.match(effect, /current\.project_instance_id !== settings\.projectInstanceId/u)
  assert.match(effect, /current\.project_id !== settings\.projectId/u)
  assert.match(effect, /current\.revision !== settings\.revision/u)
  assert.match(effect, /\.catch\(\(\) => \{/u)
  assert.doesNotMatch(effect, /catch\s*\(\s*\w+|String\(|console\./u)
})

test('App refreshes Undo/Redo availability after applying a limit', () => {
  const apply = section(
    appSource,
    'const acceptAppliedHistoryLimit = useCallback',
    'const resetRecoveredProjectUi = useCallback',
  )

  assert.match(apply, /const current = latestSnapshotRef\.current/u)
  assert.match(apply, /current\.project_instance_id !== settings\.projectInstanceId/u)
  assert.match(apply, /current\.project_id !== settings\.projectId/u)
  assert.match(apply, /current\.revision !== settings\.revision/u)
  assert.match(apply, /const refreshed = await requestProjectSnapshot\(\)/u)
  assert.match(apply, /latest !== current/u)
  assert.match(apply, /applySnapshot\(refreshed\)/u)
  assert.match(apply, /setHistoryLimitLoadState\(\{ kind: 'ready', settings \}\)/u)
})

test('App exposes explicit loading, retry, desktop-only, and bound control states', () => {
  const panel = section(
    appSource,
    "<h2>{text({ ja: '編集履歴', en: 'Edit history' })}</h2>",
    "<h2>{text({ ja: 'スナップ', en: 'Snap' })}</h2>",
  )

  assert.match(panel, /ja: '編集履歴', en: 'Edit history'/u)
  assert.match(panel, /<HistoryLimitControl/u)
  assert.match(panel, /settings=\{boundHistoryLimitSettings\}/u)
  assert.match(panel, /expectedProjectInstanceId=\{nativeSnapshot\.project_instance_id\}/u)
  assert.match(panel, /expectedProjectId=\{nativeSnapshot\.project_id\}/u)
  assert.match(panel, /expectedRevision=\{nativeSnapshot\.revision\}/u)
  assert.match(panel, /onApplied=\{acceptAppliedHistoryLimit\}/u)
  assert.match(panel, /historyLimitLoadState\.kind === 'failed'/u)
  assert.match(panel, /setHistoryLimitRetrySequence/u)
  assert.match(panel, /historyLimitLoadState\.kind === 'unavailable'/u)
  assert.match(panel, /role="status" aria-live="polite"/u)
})

function section(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}
