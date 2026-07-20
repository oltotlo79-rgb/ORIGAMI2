import { execFileSync } from 'node:child_process'
import { createServer } from 'node:http'
import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { extname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright'

const VIEWER_REVISION = 'd4eabef31e6eb70cbefb939767637539c37c7a33'
const desktop = resolve(fileURLToPath(new URL('..', import.meta.url)))
const workspace = resolve(desktop, '..', '..')
const scratch = mkdtempSync(join(tmpdir(), 'origami2-sample-viewer-'))
const viewer = join(scratch, 'viewer')
const artifacts = process.env.ORIGAMI2_GLTF_RUNTIME_ARTIFACTS
  ? resolve(process.env.ORIGAMI2_GLTF_RUNTIME_ARTIFACTS)
  : join(scratch, 'artifacts')
const missingReleaseUiIcons = new Set([
  'Animation 30X30.svg',
  'Capture 30X30.svg',
  'Developer 30X30.svg',
  'Display 30X30.svg',
  'Model 30X30.svg',
  'XMP 30X30.svg',
])
const transparentSvg = '<svg xmlns="http://www.w3.org/2000/svg" width="30" height="30"/>'

const run = (command, args, cwd = workspace) =>
  execFileSync(command, args, {
    cwd,
    stdio: 'inherit',
  })

let server
let browser
try {
  run('git', ['init', '--quiet', viewer])
  run('git', ['remote', 'add', 'origin',
    'https://github.com/KhronosGroup/glTF-Sample-Viewer-Release.git'], viewer)
  run('git', ['fetch', '--quiet', '--depth=1', 'origin', VIEWER_REVISION], viewer)
  run('git', ['checkout', '--quiet', 'FETCH_HEAD'], viewer)
  if (!process.env.ORIGAMI2_GLTF_RUNTIME_ARTIFACTS) {
    run('cargo', ['run', '--quiet', '--locked', '--release', '-p', 'ori-formats',
      '--example', 'generate_gltf_validator_fixtures', '--', artifacts])
  }

  const roots = { '/artifacts/': artifacts, '/': join(viewer, 'docs') }
  server = createServer((request, response) => {
    const pathname = new URL(request.url ?? '/', 'http://localhost').pathname
    const missingIcon = decodeURIComponent(pathname).replace('/assets/ui/', '')
    if (pathname.startsWith('/assets/ui/') && missingReleaseUiIcons.has(missingIcon)) {
      response.writeHead(200, { 'content-type': 'image/svg+xml' })
      response.end(transparentSvg)
      return
    }
    const prefix = Object.keys(roots).find((candidate) => pathname.startsWith(candidate))
    if (!prefix) { response.writeHead(404).end(); return }
    const relative = pathname.slice(prefix.length) || 'index.html'
    if (relative.includes('..')) { response.writeHead(400).end(); return }
    const file = join(roots[prefix], relative)
    try {
      if (!statSync(file).isFile()) throw new Error('not a file')
      response.setHeader('content-type', {
        '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css',
        '.glb': 'model/gltf-binary', '.wasm': 'application/wasm',
        '.png': 'image/png', '.jpg': 'image/jpeg', '.hdr': 'application/octet-stream',
      }[extname(file)] ?? 'application/octet-stream')
      response.end(readFileSync(file))
    } catch { response.writeHead(404).end() }
  })
  await new Promise((resolveReady) => server.listen(0, '127.0.0.1', resolveReady))
  const { port } = server.address()
  for (const file of ['static.glb', 'textured.glb', 'animated.glb']) {
    browser = await chromium.launch({
      headless: true,
      args: ['--use-angle=swiftshader', '--enable-webgl', '--ignore-gpu-blocklist'],
    })
    const page = await browser.newPage({ viewport: { width: 1024, height: 768 } })
    const runtimeErrors = []
    page.on('console', (message) => {
      if (message.type() === 'error'
        && !message.text().startsWith('Failed to load resource:')) {
        runtimeErrors.push(`console: ${message.text()}`)
      }
    })
    page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
    page.on('response', (response) => {
      if (response.status() >= 400) {
        runtimeErrors.push(`http ${response.status()}: ${response.url()}`)
      }
    })
    const asset = `http://127.0.0.1:${port}/artifacts/${file}`
    await page.goto(
      `http://127.0.0.1:${port}/?model=${encodeURIComponent(asset)}&noUI`,
      { waitUntil: 'networkidle' },
    )
    const canvas = page.locator('canvas')
    await canvas.waitFor({ state: 'visible', timeout: 30_000 })
    await page.waitForTimeout(3_000)
    if (runtimeErrors.length !== 0) {
      throw new Error(`${file}: runtime errors:\n${runtimeErrors.join('\n')}`)
    }
    const bounds = await canvas.boundingBox()
    if (!bounds) throw new Error(`${file}: rendered canvas has no bounds`)
    const cdp = await page.context().newCDPSession(page)
    const capture = async () => Buffer.from((await cdp.send('Page.captureScreenshot', {
      format: 'png',
      captureBeyondViewport: false,
      clip: { ...bounds, scale: 1 },
    })).data, 'base64')
    const firstFrame = await capture()
    if (firstFrame.length < 2_000) {
      throw new Error(`${file}: rendered canvas is unexpectedly empty`)
    }
    await page.waitForTimeout(file === 'animated.glb' ? 650 : 100)
    const secondFrame = await capture()
    if (file === 'animated.glb' && firstFrame.equals(secondFrame)) {
      throw new Error('animated.glb: rendered frame did not change during playback')
    }
    console.log(`${file}: Sample Viewer WebGL runtime visible, errors 0`)
    await browser.close()
    browser = undefined
  }
} finally {
  if (browser) await browser.close()
  if (server) await new Promise((resolveClosed) => server.close(resolveClosed))
  rmSync(scratch, { recursive: true, force: true })
}
