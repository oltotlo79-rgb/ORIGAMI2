import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const clientSource = readSource('../src/lib/coreClient.ts')
const dialogSource = readSource('../src/components/CreaseExportDialog.tsx')
const nativeSource = readSource('../src-tauri/src/lib.rs')
const nativeExportSource = readSource('../src-tauri/src/crease_export.rs')

test('the toolbar opens one background-blocking export confirmation dialog', () => {
  assert.match(appSource, /ref=\{creaseExportButtonRef\}/u)
  assert.match(appSource, />\s*\{fileOperation === 'crease_export' \? '生成中…' : '書出し'\}\s*</u)
  assert.match(appSource, /\|\| creaseExportOpen/u)
  assert.match(appSource, /\{creaseExportOpen && \(\s*<CreaseExportDialog/u)
  assert.match(appSource, /requestAnimationFrame\(\(\) => creaseExportButtonRef\.current\?\.focus\(\)\)/u)
})

test('the native IPC contract exposes metadata and opaque identity but no export bytes or path', () => {
  assert.match(clientSource, /preview_crease_pattern_export/u)
  assert.match(clientSource, /save_crease_pattern_export/u)
  assert.match(clientSource, /cancel_crease_pattern_export/u)
  assert.match(nativeSource, /\.manage\(CreaseExportState::default\(\)\)/u)
  assert.match(
    nativeSource,
    /preview_crease_pattern_export,\s*save_crease_pattern_export,\s*cancel_crease_pattern_export/u,
  )
  assert.match(
    clientSource,
    /expectedProjectId,\s*expectedRevision,\s*format/u,
  )
  assert.match(
    clientSource,
    /exportId,\s*expectedProjectId,\s*expectedRevision,\s*warningsAcknowledged/u,
  )
  const previewType = sliceBetween(
    clientSource,
    'export type CreasePatternExportPreviewResponse',
    'export type EdgeIntersectionResponse',
  )
  assert.doesNotMatch(previewType, /\bbytes?\b|\bpath\b|content|json|xml/iu)
  const previewSnapshot = sliceBetween(
    nativeExportSource,
    'struct CreaseExportPreviewSnapshot',
    'pub(super) struct CreaseExportSaveResponse',
  )
  assert.doesNotMatch(previewSnapshot, /\bbytes?\b|\bpath\b|content|json|xml/iu)
})

test('save is bound to the preview project and revision and native cancel remains retryable', () => {
  assert.match(
    appSource,
    /current\.project_id !== preview\.expected_project_id\s*\|\|\s*current\.revision !== preview\.expected_revision/u,
  )
  assert.match(appSource, /saveCreasePatternExport\(\s*preview\.export_id/u)
  assert.match(appSource, /if \(response\.canceled\) \{[\s\S]*確認画面から再試行できます/u)
  assert.match(appSource, /cancelCreasePatternExport\(preview\.export_id\)/u)
})

test('the dialog requires explicit loss acknowledgement and handles focus and IME safely', () => {
  assert.match(dialogSource, /preview\.warnings\.length === 0 \|\| warningsAcknowledged/u)
  assert.match(dialogSource, /上記の情報が出力に含まれないことを確認しました/u)
  assert.match(dialogSource, /event\.key !== 'Escape' \|\| event\.isComposing \|\| busy/u)
  assert.match(dialogSource, /event\.key !== 'Tab'/u)
  assert.match(dialogSource, /aria-modal="true"/u)
  assert.match(dialogSource, /aria-busy=\{busy\}/u)
})

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function sliceBetween(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.notEqual(startIndex, -1)
  assert.notEqual(endIndex, -1)
  return source.slice(startIndex, endIndex)
}
