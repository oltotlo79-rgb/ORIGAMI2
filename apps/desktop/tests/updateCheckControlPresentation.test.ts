import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const controlCss = await readFile(
  new URL('../src/components/UpdateCheckControl.css', import.meta.url),
  'utf8',
)
const appCss = await readFile(
  new URL('../src/App.css', import.meta.url),
  'utf8',
)

test('update actions use the contrast-safe action token in every theme', () => {
  const actionRule = ruleBody('.update-check-actions button')
  assert.match(
    actionRule,
    /background:\s*var\(--accent-action,\s*#176b87\)/u,
  )
  assert.match(
    actionRule,
    /border:\s*1px solid var\(--accent-action,\s*#176b87\)/u,
  )
  assert.match(actionRule, /color:\s*#fff/u)

  const darkTokens = themeTokens("html[data-theme='dark']")
  assert.ok(
    contrast('#ffffff', darkTokens['accent-action']) >= 4.5,
    'white action text must keep at least 4.5:1 contrast in dark mode',
  )
})

test('failure text has explicit readable light and dark colours', () => {
  const lightPanel = themeTokens(':root').panel
  assert.ok(
    contrast('#b42318', lightPanel) >= 4.5,
    'light-theme failure text must keep at least 4.5:1 contrast',
  )

  const darkPanel = themeTokens("html[data-theme='dark']").panel
  assert.match(
    controlCss,
    /html\[data-theme='dark'\] \.update-check-status-unavailable,\s*html\[data-theme='dark'\] \.update-check-persistence-error\s*\{\s*color:\s*#ffb4ab;/su,
  )
  assert.ok(
    contrast('#ffb4ab', darkPanel) >= 4.5,
    'dark-theme failure text must keep at least 4.5:1 contrast',
  )
})

function ruleBody(selector: string): string {
  let body: string | null = null
  for (const match of controlCss.matchAll(/([^{}]+)\{([^{}]*)\}/gu)) {
    const selectors = match[1]?.split(',').map((value) => value.trim())
    if (selectors?.includes(selector)) body = match[2] ?? ''
  }
  if (body !== null) return body
  return assert.fail(`missing CSS rule for ${selector}`)
}

function themeTokens(selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')
  const block = appCss.match(
    new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, 'su'),
  )?.[1]
  assert.ok(block, `missing theme block: ${selector}`)
  return Object.fromEntries(
    [...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/giu)]
      .map((match) => [match[1], match[2]]),
  )
}

function contrast(
  foreground: string,
  background: string | undefined,
): number {
  assert.match(background ?? '', /^#[0-9a-f]{6}$/iu)
  const first = luminance(foreground)
  const second = luminance(background ?? '')
  return (
    (Math.max(first, second) + 0.05)
    / (Math.min(first, second) + 0.05)
  )
}

function luminance(hex: string): number {
  assert.match(hex, /^#[0-9a-f]{6}$/iu)
  const channels = [1, 3, 5].map(
    (offset) => Number.parseInt(hex.slice(offset, offset + 2), 16) / 255,
  )
  const linear = channels.map((channel) =>
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4)
  return (
    0.2126 * (linear[0] ?? 0)
    + 0.7152 * (linear[1] ?? 0)
    + 0.0722 * (linear[2] ?? 0)
  )
}
