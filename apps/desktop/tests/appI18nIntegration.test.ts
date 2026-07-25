import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = [
  readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8'),
  readFileSync(new URL('../src/lib/appText.ts', import.meta.url), 'utf8'),
].join('\n')
const cssSource = readFileSync(
  new URL('../src/App.css', import.meta.url),
  'utf8',
)

test('App subscribes to locale and connects the language control in status controls', () => {
  assert.match(
    appSource,
    /import \{ LanguageControl \} from '\.\/components\/LanguageControl'/u,
  )
  assert.match(appSource, /const locale = useLocale\(\)/u)

  const statusbar = appSource.slice(appSource.indexOf('<footer className="statusbar"'))
  const theme = statusbar.indexOf('<ThemeControl />')
  const language = statusbar.indexOf('<LanguageControl />')
  assert.ok(theme >= 0)
  assert.ok(language > theme)
})

test('App fixed and variable messages use the strict localized text APIs', () => {
  assert.match(
    appSource,
    /selectLocalizedText\(locale, localized\)/u,
  )
  assert.match(
    appSource,
    /formatLocalizedText\(locale, localized, variables\)/u,
  )
  assert.match(
    appSource,
    /createdNameASaveLocationHasNotBeenSetYet: localized\([\s\S]*?'Created “\{name\}”/u,
  )
  assert.doesNotMatch(appSource, /`「\$\{snapshot\.name\}/u)
  assert.match(appSource, /projectActions: localized\('プロジェクト操作', 'Project actions'\)/u)
  assert.match(appSource, /newShortcut: localized\('新規 \(\{shortcut\}\)', 'New \(\{shortcut\}\)'\)/u)
  assert.match(appSource, /toolTool: localized\('ツール: \{tool\}', 'Tool: \{tool\}'\)/u)
})

test('language control has bounded statusbar styling and visible focus', () => {
  assert.match(cssSource, /\.language-control\s*\{/u)
  assert.match(cssSource, /\.language-control select\s*\{/u)
  assert.match(cssSource, /\.language-control select:focus-visible\s*\{/u)
  assert.match(
    cssSource,
    /\.language-control select:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--accent\)/su,
  )
})
