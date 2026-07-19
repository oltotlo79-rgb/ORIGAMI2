import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const mainSource = readFileSync(
  new URL('../src/main.tsx', import.meta.url),
  'utf8',
)
const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)
const cssSource = readFileSync(
  new URL('../src/App.css', import.meta.url),
  'utf8',
)

test('theme is initialized before React mounts and the control is connected', () => {
  const initialization = mainSource.indexOf('initializeTheme()')
  const mount = mainSource.indexOf('createRoot(')

  assert.ok(initialization >= 0)
  assert.ok(mount >= 0)
  assert.ok(initialization < mount)
  assert.match(appSource, /import \{ ThemeControl \}/u)
  assert.match(appSource, /<ThemeControl \/>/u)
})

test('dark styling is explicitly data-theme scoped without OS-only media CSS', () => {
  assert.doesNotMatch(cssSource, /prefers-color-scheme\s*:\s*dark/u)
  assert.match(cssSource, /html\[data-theme='light'\]\s*\{[^}]*color-scheme:\s*light/su)
  assert.match(cssSource, /html\[data-theme='dark'\]\s*\{[^}]*color-scheme:\s*dark/su)
  assert.match(
    cssSource,
    /html\[data-theme='dark'\]\s+\.svg-import-loss-badge/u,
  )
  assert.match(
    cssSource,
    /html\[data-theme='dark'\]\s+\.global-flat-foldability-cancel/u,
  )
})

test('dark core tokens meet WCAG text and non-text contrast floors', () => {
  const block = cssSource.match(/html\[data-theme='dark'\]\s*\{([^}]*)\}/su)?.[1]
  assert.ok(block, 'dark theme token block must exist')
  const tokens = Object.fromEntries(
    [...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/giu)]
      .map((match) => [match[1], match[2]]),
  )

  for (const foreground of ['text', 'muted', 'accent']) {
    for (const background of ['bg', 'panel', 'panel-strong']) {
      assert.ok(
        contrast(tokens[foreground], tokens[background]) >= 4.5,
        `${foreground} on ${background} must be at least 4.5:1`,
      )
    }
  }
  for (const background of ['panel', 'panel-strong']) {
    assert.ok(
      contrast(tokens.border, tokens[background]) >= 3,
      `border on ${background} must be at least 3:1`,
    )
  }
  assert.ok(
    contrast('#ffffff', tokens['accent-action']) >= 4.5,
    'white action text must be at least 4.5:1',
  )
  assert.ok(
    contrast(tokens.mountain, tokens['panel-strong']) >= 3,
    'mountain lines must be at least 3:1',
  )
  assert.ok(
    contrast(tokens.valley, tokens['panel-strong']) >= 3,
    'valley lines must be at least 3:1',
  )
})

function contrast(first: string | undefined, second: string | undefined) {
  assert.match(first ?? '', /^#[0-9a-f]{6}$/iu)
  assert.match(second ?? '', /^#[0-9a-f]{6}$/iu)
  const firstLuminance = luminance(first ?? '')
  const secondLuminance = luminance(second ?? '')
  return (
    Math.max(firstLuminance, secondLuminance) + 0.05
  ) / (
    Math.min(firstLuminance, secondLuminance) + 0.05
  )
}

function luminance(hex: string) {
  const channels = [1, 3, 5].map((offset) => (
    Number.parseInt(hex.slice(offset, offset + 2), 16) / 255
  ))
  const linear = channels.map((channel) => (
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  ))
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}
