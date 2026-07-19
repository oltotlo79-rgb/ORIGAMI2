import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)

test('App routes the strict cross-platform file shortcut resolver', () => {
  assert.match(
    appSource,
    /const fileShortcut = resolveFileKeyboardShortcut\(event\)/u,
  )
  assert.match(
    appSource,
    /if \(fileShortcut === 'new'\)[\s\S]*setNewProjectOpen\(true\)[\s\S]*runShortcutFileOperation\(fileShortcut\)/u,
  )
  assert.match(
    appSource,
    /if \(fileShortcut\) \{[\s\S]*event\.preventDefault\(\)[\s\S]*if \(coreBusy \|\| !nativeSnapshot\) return/u,
  )
})

test('file action buttons expose Windows and macOS mappings to assistive technology', () => {
  for (const shortcut of [
    'Control+N Meta+N',
    'Control+O Meta+O',
    'Control+S Meta+S',
    'Control+Shift+S Meta+Shift+S',
  ]) {
    assert.equal(
      count(appSource, `aria-keyshortcuts="${shortcut}"`),
      1,
      shortcut,
    )
  }
})

function count(source: string, needle: string) {
  return source.split(needle).length - 1
}
