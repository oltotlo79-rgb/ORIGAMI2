import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)
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
    /ja: '「\{name\}」を作成しました[^']*'[\s\S]*en: 'Created “\{name\}”/u,
  )
  assert.doesNotMatch(appSource, /`「\$\{snapshot\.name\}/u)
  assert.match(appSource, /ja: 'プロジェクト操作'[\s\S]*en: 'Project actions'/u)
  assert.match(appSource, /ja: '新規 \(\{shortcut\}\)'[\s\S]*en: 'New \(\{shortcut\}\)'/u)
  assert.match(appSource, /ja: 'ツール: \{tool\}'[\s\S]*en: 'Tool: \{tool\}'/u)
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
