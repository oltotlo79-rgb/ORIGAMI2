import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)

test('App subscribes to persisted shortcuts and routes the configured resolver', () => {
  assert.match(
    appSource,
    /const keyboardShortcuts = useSyncExternalStore\(\s*keyboardShortcutStore\.subscribe,\s*keyboardShortcutStore\.getSnapshot,\s*keyboardShortcutStore\.getServerSnapshot,\s*\)/u,
  )
  assert.match(
    appSource,
    /const configuredShortcut = resolveConfiguredKeyboardShortcut\(\s*event,\s*keyboardShortcuts,\s*\)/u,
  )
  assert.match(
    appSource,
    /if \(configuredShortcut\) \{\s*event\.preventDefault\(\)\s*if \(coreBusy \|\| !nativeSnapshot\) return/u,
  )
})

test('new-project Escape ignores repeat and IME composition before closing', () => {
  assert.match(
    appSource,
    /const key = event\.key\.toLowerCase\(\)\s*if \(key === 'escape' && newProjectOpen\) \{\s*if \(event\.repeat \|\| event\.isComposing\) return\s*event\.preventDefault\(\)/u,
  )
})

test('configured commands reach every supported file and history action', () => {
  assert.match(
    appSource,
    /configuredShortcut === 'new'[\s\S]*setNewProjectOpen\(true\)/u,
  )
  assert.match(
    appSource,
    /configuredShortcut === 'open'\s*\|\|\s*configuredShortcut === 'save'\s*\|\|\s*configuredShortcut === 'save_as'[\s\S]*runShortcutFileOperation\(configuredShortcut\)/u,
  )
  assert.match(
    appSource,
    /configuredShortcut === 'undo'\s*&&\s*nativeSnapshot\.can_undo[\s\S]*runNativeEdit\(undo\)/u,
  )
  assert.match(
    appSource,
    /configuredShortcut === 'redo'\s*&&\s*nativeSnapshot\.can_redo[\s\S]*runNativeEdit\(redo\)/u,
  )
})

test('all configured action buttons expose dynamic title and ARIA mappings', () => {
  for (const command of [
    'new',
    'open',
    'save',
    'save_as',
    'undo',
    'redo',
  ]) {
    assert.equal(
      count(
        appSource,
        `aria-keyshortcuts={keyboardShortcutAriaValue('${command}', keyboardShortcuts)}`,
      ),
      1,
      `${command} aria-keyshortcuts`,
    )
    assert.equal(
      count(
        appSource,
        `\${keyboardShortcutDisplayValue('${command}', keyboardShortcuts)}`,
      ),
      1,
      `${command} title`,
    )
  }
})

test('shortcut settings remain inside the modal-inert statusbar', () => {
  const statusbarStart = appSource.indexOf(
    '<footer className="statusbar" inert={modalOpen}>',
  )
  const statusbarEnd = appSource.indexOf('</footer>', statusbarStart)

  assert.ok(statusbarStart >= 0)
  assert.ok(statusbarEnd > statusbarStart)
  assert.match(
    appSource.slice(statusbarStart, statusbarEnd),
    /<KeyboardShortcutControl \/>/u,
  )
})

function count(source: string, needle: string) {
  return source.split(needle).length - 1
}
