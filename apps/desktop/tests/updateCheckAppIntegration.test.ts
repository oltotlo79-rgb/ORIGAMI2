import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const appSource = await readFile(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)
const controlSource = await readFile(
  new URL('../src/components/UpdateCheckControl.tsx', import.meta.url),
  'utf8',
)
const controlCss = await readFile(
  new URL('../src/components/UpdateCheckControl.css', import.meta.url),
  'utf8',
)

test('the statusbar mounts the localized native update popover beside display settings', () => {
  assert.match(
    appSource,
    /import \{ UpdateCheckPopover \} from '\.\/components\/UpdateCheckControl'/u,
  )
  const footer = appSource.match(
    /<footer className="statusbar" inert=\{modalOpen\}>([\s\S]*?)<\/footer>/u,
  )?.[1]
  assert.ok(footer)
  assert.match(
    footer,
    /<WorkspaceLayoutControl \/>[\s\S]*<UpdateCheckPopover \/>[\s\S]*<ThemeControl \/>[\s\S]*<LanguageControl \/>/u,
  )
  assert.doesNotMatch(footer, /checkNow|getVersion|requestLatestRelease/u)

  assert.match(
    controlSource,
    /<details className="update-check-popover">[\s\S]*?<summary>[\s\S]*?popoverSummary[\s\S]*?<UpdateCheckControl \{\.\.\.props\} \/>/u,
  )
  assert.match(
    controlSource,
    /popoverSummary:\s*Object\.freeze\(\{\s*ja: '更新',\s*en: 'Updates'/u,
  )
})

test('the upward popover is bounded to the viewport and remains below modal dialogs', () => {
  const rule = cssRuleBody(
    '.update-check-popover > .update-check-control',
  )
  assert.match(rule, /position:\s*fixed/u)
  assert.match(rule, /right:\s*12px/u)
  assert.match(rule, /bottom:\s*36px/u)
  assert.match(rule, /width:\s*min\(420px,\s*calc\(100vw - 24px\)\)/u)
  assert.match(rule, /max-height:\s*calc\(100vh - 52px\)/u)
  assert.match(rule, /overflow:\s*auto/u)
  const zIndex = Number(rule.match(/z-index:\s*(\d+)/u)?.[1])
  assert.ok(Number.isInteger(zIndex) && zIndex < 20)

  const summary = cssRuleBody('.update-check-popover > summary')
  assert.match(summary, /min-height:\s*23px/u)
  assert.match(
    controlCss,
    /\.update-check-popover > summary:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--accent/su,
  )
})

function cssRuleBody(selector: string): string {
  for (const match of controlCss.matchAll(/([^{}]+)\{([^{}]*)\}/gu)) {
    const selectors = match[1]?.split(',').map((value) => value.trim())
    if (selectors?.includes(selector)) return match[2] ?? ''
  }
  return assert.fail(`missing CSS rule for ${selector}`)
}
