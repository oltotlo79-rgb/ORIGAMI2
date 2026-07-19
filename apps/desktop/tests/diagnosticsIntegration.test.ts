import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  DIAGNOSTIC_SCOPES,
  type DiagnosticScope,
} from '../src/lib/diagnostics.ts'

const appSource = readSource('../src/App.tsx')
const foldPreviewSource = readSource('../src/components/FoldPreview.tsx')
const mainSource = readSource('../src/main.tsx')

test('only allowlisted scope codes cross the frontend diagnostics boundary', () => {
  const reports = [
    ...collectReports(appSource),
    ...collectReports(foldPreviewSource),
    ...collectReports(mainSource),
  ]
  const expected = new Map<DiagnosticScope, number>([
    ['app.unhandled_error', 1],
    ['app.unhandled_rejection', 1],
    ['app.project_snapshot', 2],
    ['app.topology_analysis', 1],
    ['app.close_guard', 1],
    ['app.validation', 3],
    ['app.benchmark', 1],
    ['fold_preview.geometry', 1],
    ['fold_preview.render', 2],
    ['fold_preview.scene_initialization', 1],
    ['fold_preview.pose_application', 1],
    ['fold_preview.pose_schedule', 1],
    ['fold_preview.selection_render', 1],
    ['fold_preview.camera', 2],
    ['fold_preview.resize', 1],
  ])

  assert.deepEqual(new Set(reports), new Set(DIAGNOSTIC_SCOPES))
  for (const [scope, count] of expected) {
    assert.equal(
      reports.filter((reportedScope) => reportedScope === scope).length,
      count,
      scope,
    )
  }
})

test('integrations never pass an error or arbitrary context to diagnostics', () => {
  for (const source of [appSource, foldPreviewSource, mainSource]) {
    assert.doesNotMatch(
      source,
      /reportUnexpected\(\s*['"][^'"]+['"]\s*,/,
    )
  }
  assert.match(mainSource, /const reportUnhandledError = \(\) => \{/)
  assert.match(mainSource, /const reportUnhandledRejection = \(\) => \{/)
})

test('every application reporter uses the native-aware diagnostics runtime', () => {
  assert.match(appSource, /from '\.\/lib\/diagnosticsRuntime'/)
  assert.match(
    foldPreviewSource,
    /from '\.\.\/lib\/diagnosticsRuntime'/,
  )
  assert.match(mainSource, /from '\.\/lib\/diagnosticsRuntime'/)
  for (const source of [appSource, foldPreviewSource, mainSource]) {
    assert.doesNotMatch(source, /from ['"][^'"]*\/diagnostics['"]/)
  }
})

function collectReports(source: string) {
  return Array.from(
    source.matchAll(/reportUnexpected\('([^']+)'\)/g),
    (match) => match[1] as DiagnosticScope,
  )
}

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
