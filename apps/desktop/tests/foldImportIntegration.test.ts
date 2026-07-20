import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const clientSource = readSource('../src/lib/coreClient.ts')
const dialogSource = readSource('../src/components/FoldImportDialog.tsx')
const foldImportSource = readSource('../src/lib/foldImport.ts')
const appMessagesSource = readSource('../src/lib/appMessages.ts')

test('the client exposes only token-based preview, apply, and cancel FOLD invocations', () => {
  const preview = exportedFunction('previewFoldImport')
  const apply = exportedFunction('applyFoldImport')
  const cancel = exportedFunction('cancelFoldImport')

  assert.match(
    preview,
    /invoke<FoldImportPreviewResponse>\('preview_fold_import'\)/u,
  )
  assert.match(
    apply,
    /invoke<ProjectSnapshot>\('apply_fold_import',\s*\{[\s\S]*\}\)/u,
  )
  assert.match(
    cancel,
    /invoke<void>\('cancel_fold_import',\s*\{\s*previewId\s*\}\)/u,
  )

  for (const argument of [
    'previewId: settings.importId',
    'expectedProjectId',
    'expectedRevision',
    'name: settings.name',
    'millimetersPerUnit: settings.mmPerUnit',
    'boundaryCandidateId: settings.boundaryCandidateId',
    'assignmentMappings',
  ]) {
    assert.match(apply, new RegExp(escapeRegExp(argument), 'u'), argument)
  }

  const clientContract = [preview, apply, cancel].join('\n')
  assert.doesNotMatch(
    clientContract,
    /\b(?:path|filePath|file_path)\b/iu,
    'the renderer must not receive or send a filesystem path',
  )
  assert.equal(
    clientContract.match(
      /invoke(?:<[^>]+>)?\('(?:preview|apply|cancel)_fold_import'/gu,
    )?.length,
    3,
  )
})

test('the toolbar performs file selection before opening one explicit mapping modal', () => {
  const begin = appFunction('beginFoldImport', 'closeFoldImportDialog')

  assert.match(
    appSource,
    /ref=\{foldImportButtonRef\}[\s\S]*?onClick=\{\(\) => void beginFoldImport\(\)\}[\s\S]*?aria-haspopup="dialog"[\s\S]*?FOLD取込/u,
  )
  assert.match(begin, /const response = await previewFoldImport\(\)/u)
  assert.match(begin, /if \(response\.canceled\)[\s\S]*?return/u)
  assert.match(begin, /if \(!response\.preview\)[\s\S]*?throw new Error/u)
  assert.match(begin, /setFoldImportPreview\(response\.preview\)/u)
  assert.doesNotMatch(begin, /\bis_dirty\b|window\.confirm|\bpath\b/iu)

  assert.match(
    appSource,
    /\{foldImportPreview && \(\s*<FoldImportDialog[\s\S]*?preview=\{foldImportPreview\}[\s\S]*?onCancel=\{\(\) => void closeFoldImportDialog\(\)\}[\s\S]*?onImport=\{\(settings\) => void confirmFoldImport\(settings\)\}/u,
  )
  assert.match(dialogSource, /role="dialog"/u)
  assert.match(dialogSource, /aria-modal="true"/u)
  assert.match(dialogSource, /initialFoldImportMapping\(preview\.assignments\)/u)
  assert.match(dialogSource, /unresolvedFoldAssignments\(preview\.assignments, mapping\)/u)
  assert.match(
    dialogSource,
    /onImport\(\{\s*importId: preview\.import_id,\s*name: displayedName\.trim\(\),\s*mmPerUnit: scale,\s*mappings: mapping,\s*boundaryCandidateId: selectedBoundary\.id,\s*\}\)/u,
  )
  assert.match(
    dialogSource,
    /foldImportPreviewFileName\(preview\.file_name, locale\)/u,
  )
  assert.match(dialogSource, /foldImportWarningMessage\(warning, locale\)/u)
  assert.match(
    dialogSource,
    /foldImportSuggestedName\(preview\.suggested_name, locale\)/u,
  )
  assert.match(foldImportSource, /export type FoldImportSettings[\s\S]*?importId: string/u)
  assert.doesNotMatch(
    sourceSection(
      foldImportSource,
      'export type FoldImportSettings',
      'export const FOLD_IMPORT_TARGET_OPTIONS',
    ),
    /\b(?:path|filePath|file_path)\b/iu,
  )
})

test('dirty-project confirmation is deferred until immediately before replacement', () => {
  const confirm = appFunction('confirmFoldImport', 'toggleBenchmark')
  const dirtyIndex = confirm.indexOf('current.is_dirty')
  const confirmationIndex = confirm.indexOf('window.confirm')
  const operationLeaseIndex = confirm.indexOf('coreOperationRef.current = true')
  const applyIndex = confirm.indexOf('await applyFoldImport(')

  assert.ok(dirtyIndex >= 0, 'missing dirty-project guard')
  assert.ok(confirmationIndex > dirtyIndex, 'confirmation must be part of the dirty guard')
  assert.ok(
    operationLeaseIndex > confirmationIndex,
    'the import operation must not start before confirmation',
  )
  assert.ok(applyIndex > operationLeaseIndex, 'replacement must happen after confirmation')
  assert.match(
    confirm,
    /current\.is_dirty\s*&&\s*!window\.confirm\(appConfirmationText\(locale, 'replaceWithFold'\)\)\s*\)\s*return/u,
  )
  assert.match(
    appMessagesSource,
    /replaceWithFold:\s*\{\s*ja: '未保存の変更があります。保存せずにFOLD展開図へ置き換えますか？',\s*en: 'There are unsaved changes\. Replace them with the FOLD crease pattern\?'/u,
  )
  assert.match(
    confirm,
    /applyFoldImport\(\s*current\.project_id,\s*current\.revision,\s*settings,\s*\)/u,
  )
})

test('the FOLD modal makes all background regions inert', () => {
  assert.match(
    appSource,
    /const modalOpen = newProjectOpen\s*\|\| diagnosticsDialogOpen\s*\|\| foldTechniqueEditor !== null\s*\|\| foldTechniqueBusy\s*\|\| foldTechniqueTimelinePreview !== null\s*\|\| foldTechniqueTimelineBusy\s*\|\| foldImportPreview !== null\s*\|\| svgImportPreview !== null/u,
  )
  assert.equal(
    appSource.match(/inert=\{modalOpen\}/gu)?.length,
    5,
    'titlebar, workspace, timeline separator, timeline, and statusbar must all be inert',
  )
  assert.match(appSource, /<header className="titlebar" inert=\{modalOpen\}>/u)
  assert.match(
    appSource,
    /<section className="workspace" inert=\{modalOpen\}[^>]*>/u,
  )
  assert.match(
    appSource,
    /<div className="workspace-timeline-separator" inert=\{modalOpen\}>/u,
  )
  assert.match(
    appSource,
    /<InstructionTimelinePanel[\s\S]*?inert=\{modalOpen\}/u,
  )
  assert.match(appSource, /<footer className="statusbar" inert=\{modalOpen\}>/u)
})

test('apply closes and resets editor state only after success while errors keep the dialog', () => {
  const confirm = appFunction('confirmFoldImport', 'toggleBenchmark')
  const tryBody = sourceSection(confirm, 'try {', '} catch {')
  const catchBody = sourceSection(confirm, '} catch {', '} finally {')
  const finallyBody = sourceSection(confirm, '} finally {', '\n    }\n  }')
  const applyIndex = tryBody.indexOf('await applyFoldImport(')
  const snapshotIndex = tryBody.indexOf('applySnapshot(snapshot, true)')
  const closeIndex = tryBody.indexOf('setFoldImportPreview(null)')

  assert.ok(applyIndex >= 0, 'missing FOLD apply call')
  assert.ok(snapshotIndex > applyIndex, 'the returned snapshot must be applied after import')
  assert.ok(closeIndex > snapshotIndex, 'the dialog must close only after snapshot application')
  for (const reset of [
    'setBenchmarkRun(null)',
    'setSelectedLineId(null)',
    'setSelectedVertexId(null)',
    'setPendingEdgeStart(null)',
    'setParallelReferenceEdgeId(null)',
    'setAppliedFoldPose(null)',
    "setActiveTool('select')",
  ]) {
    assert.match(tryBody, new RegExp(escapeRegExp(reset), 'u'), reset)
  }
  assert.match(
    tryBody,
    /setBenchmarkStatus\(appMessage\(\{\s*ja: 'FOLD取込により通常の展開図へ戻りました',\s*en: 'Returned to the normal crease pattern after FOLD import',\s*\}\)\)/u,
  )

  assert.match(
    catchBody,
    /const safeError = appMessage\(\s*appErrorLocalizedText\('fold_import_failed'\),\s*\)[\s\S]*setFoldImportError\(safeError\)[\s\S]*setCoreStatus\(safeError\)/u,
  )
  assert.doesNotMatch(catchBody, /String\(error\)|\{ error:/u)
  assert.doesNotMatch(catchBody, /setFoldImportPreview\(null\)/u)
  assert.doesNotMatch(finallyBody, /setFoldImportPreview\(null\)/u)
  assert.match(finallyBody, /coreOperationRef\.current = false/u)
  assert.match(finallyBody, /setCoreBusy\(false\)/u)
})

test('performance data cannot outlive or obscure a FOLD project replacement', () => {
  const confirm = appFunction('confirmFoldImport', 'toggleBenchmark')
  const tryBody = sourceSection(confirm, 'try {', '} catch {')
  const snapshotIndex = tryBody.indexOf('applySnapshot(snapshot, true)')
  const clearBenchmarkIndex = tryBody.indexOf('setBenchmarkRun(null)')

  assert.ok(snapshotIndex >= 0, 'missing imported snapshot application')
  assert.ok(
    clearBenchmarkIndex > snapshotIndex,
    'benchmark display must be cleared only after a successful replacement',
  )
  assert.match(
    appSource,
    /ref=\{foldImportButtonRef\}[\s\S]*?disabled=\{coreBusy \|\| benchmarkLoading \|\| Boolean\(benchmarkRun\) \|\| !nativeSnapshot\}[\s\S]*?FOLD取込/u,
  )
  assert.doesNotMatch(
    sourceSection(confirm, '} catch {', '} finally {'),
    /setBenchmarkRun\(null\)/u,
  )
})

test('cancel invalidates the native preview token and then closes the dialog', () => {
  const cancel = appFunction('closeFoldImportDialog', 'confirmFoldImport')

  assert.match(cancel, /const preview = foldImportPreview/u)
  assert.match(cancel, /await cancelFoldImport\(preview\.import_id\)/u)
  assert.match(
    cancel,
    /finally \{[\s\S]*?setFoldImportPreview\(null\)[\s\S]*?setFoldImportError\(null\)/u,
  )
  assert.match(
    cancel,
    /requestAnimationFrame\(\(\) => foldImportButtonRef\.current\?\.focus\(\)\)/u,
  )
})

function exportedFunction(name: string) {
  const start = `export function ${name}(`
  const startIndex = clientSource.indexOf(start)
  assert.ok(startIndex >= 0, `missing exported function: ${name}`)
  const nextIndex = clientSource.indexOf('\nexport function ', startIndex + start.length)
  return clientSource.slice(startIndex, nextIndex < 0 ? clientSource.length : nextIndex)
}

function appFunction(name: string, nextName: string) {
  return sourceSection(
    appSource,
    `async function ${name}(`,
    `async function ${nextName}(`,
  )
}

function sourceSection(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')
}

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
