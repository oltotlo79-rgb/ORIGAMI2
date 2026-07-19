import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = read('../src/App.tsx')
const css = read('../src/App.css')
const separator = read('../src/components/WorkspaceLayoutSeparator.tsx')
const timeline = read('../src/components/InstructionTimelinePanel.tsx')

test('App subscribes to one saved workspace layout and exposes bounded CSS values', () => {
  assert.match(app, /useSyncExternalStore\(/u)
  assert.match(
    app,
    /useSyncExternalStore\(\s*workspaceLayoutStore\.subscribe,\s*workspaceLayoutStore\.getSnapshot,\s*workspaceLayoutStore\.getServerSnapshot,\s*\)/su,
  )
  assert.match(
    app,
    /'--workspace-editor-two-d-share':\s*`\$\{workspaceLayout\.editorTwoDPercent\}fr`/u,
  )
  assert.match(
    app,
    /'--workspace-editor-three-d-share':\s*`\$\{100 - workspaceLayout\.editorTwoDPercent\}fr`/u,
  )
  assert.match(
    app,
    /'--workspace-inspector-width': `\$\{workspaceLayout\.inspectorWidthPx\}px`/u,
  )
  assert.match(
    app,
    /'--workspace-timeline-height': `\$\{workspaceLayout\.timelineHeightPx\}px`/u,
  )
  assert.match(
    app,
    /<main className="app-shell" style=\{workspaceLayoutStyle\}>/u,
  )
})

test('all three separators control real identified grid regions', () => {
  for (const id of [
    'workspace-main',
    'workspace-editor-panels',
    'workspace-inspector-panel',
    'crease-editor-panel',
    'fold-preview-panel',
    'instruction-timeline-panel',
  ]) {
    assert.match(`${app}\n${timeline}`, new RegExp(`id="${id}"`, 'u'), id)
    assert.match(separator, new RegExp(`(?:^|[' ])${id}(?: |')`, 'u'), id)
  }

  assert.equal((app.match(/<WorkspaceLayoutSeparator kind="editor" \/>/gu) ?? []).length, 1)
  assert.equal((app.match(/<WorkspaceLayoutSeparator kind="inspector" \/>/gu) ?? []).length, 1)
  assert.equal((app.match(/<WorkspaceLayoutSeparator kind="timeline" \/>/gu) ?? []).length, 1)
  assert.match(
    app,
    /data-panel-order=\{workspaceLayout\.panelOrder\}/u,
  )
  assert.match(
    app,
    /data-inspector-side=\{workspaceLayout\.inspectorSide\}/u,
  )
  assert.match(
    app,
    /className="workspace-timeline-separator" inert=\{modalOpen\}/u,
  )
})

test('the CSS grids consume every saved dimension and both position choices', () => {
  assert.match(
    css,
    /grid-template-rows:[^;]*var\(--workspace-timeline-height,\s*192px\)/su,
  )
  assert.match(
    css,
    /grid-template-columns:[^;]*var\(--workspace-inspector-width,\s*248px\)/su,
  )
  assert.match(
    css,
    /grid-template-columns:[^;]*var\(--workspace-editor-two-d-share,\s*50fr\)[^;]*var\(--workspace-editor-three-d-share,\s*50fr\)/su,
  )
  assert.match(css, /\.workspace\[data-inspector-side='left'\]/u)
  assert.match(css, /\.editor-grid\[data-panel-order='three_d_first'\]/u)
  assert.match(css, /\.workspace-separator:focus-visible/u)
})

test('the saved-layout control is available in the existing inert statusbar', () => {
  const statusbarStart = app.indexOf(
    '<footer className="statusbar" inert={modalOpen}>',
  )
  const statusbarEnd = app.indexOf('</footer>', statusbarStart)
  assert.ok(statusbarStart >= 0)
  assert.ok(statusbarEnd > statusbarStart)
  const statusbar = app.slice(statusbarStart, statusbarEnd)
  assert.match(statusbar, /<WorkspaceLayoutControl \/>/u)
  assert.match(statusbar, /<ThemeControl \/>/u)
})

function read(path: string) {
  return readFileSync(new URL(path, import.meta.url), 'utf8')
}
