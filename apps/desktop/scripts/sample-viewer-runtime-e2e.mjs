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

const assertAnimationContract = (gltf) => {
  const animation = gltf.animations?.[0]
  const channel = animation?.channels?.[0]
  const sampler = animation?.samplers?.[channel?.sampler]
  const timeAccessor = gltf.accessors?.[sampler?.input]
  const weightAccessor = gltf.accessors?.[sampler?.output]
  if (gltf.animations?.length !== 1
    || animation.channels?.length !== 1
    || animation.samplers?.length !== 1
    || channel.target?.path !== 'weights'
    || sampler.interpolation !== 'STEP'
    || !Number.isInteger(timeAccessor?.count)
    || timeAccessor.count < 2
    || weightAccessor?.count !== timeAccessor.count * timeAccessor.count) {
    throw new Error('animated.glb: invalid STEP morph animation contract')
  }
}

const assertAnimationContractRejectsDrift = () => {
  const valid = {
    animations: [{
      channels: [{ sampler: 0, target: { path: 'weights' } }],
      samplers: [{ input: 0, output: 1, interpolation: 'STEP' }],
    }],
    accessors: [{ count: 3 }, { count: 9 }],
  }
  const invalid = [
    { ...valid, animations: [] },
    {
      ...valid,
      animations: [{
        ...valid.animations[0],
        channels: [{ sampler: 0, target: { path: 'translation' } }],
      }],
    },
    { ...valid, accessors: [{ count: 1 }, { count: 1 }] },
    { ...valid, accessors: [{ count: 3 }, { count: 8 }] },
  ]
  for (const candidate of invalid) {
    let rejected = false
    try {
      assertAnimationContract(candidate)
    } catch {
      rejected = true
    }
    if (!rejected) throw new Error('animation contract admitted a negative fixture')
  }
}

const assertAnimatedGlbContract = (file) => {
  const bytes = readFileSync(file)
  if (bytes.length < 20
    || bytes.toString('ascii', 0, 4) !== 'glTF'
    || bytes.readUInt32LE(4) !== 2
    || bytes.readUInt32LE(8) !== bytes.length
    || bytes.toString('ascii', 16, 20) !== 'JSON') {
    throw new Error('animated.glb: invalid GLB 2.0 envelope')
  }
  const jsonLength = bytes.readUInt32LE(12)
  if (jsonLength > bytes.length - 20) {
    throw new Error('animated.glb: invalid JSON chunk length')
  }
  assertAnimationContract(
    JSON.parse(bytes.toString('utf8', 20, 20 + jsonLength).trimEnd()),
  )
}

const run = (command, args, cwd = workspace) =>
  execFileSync(command, args, {
    cwd,
    stdio: 'inherit',
  })

const retry = async (label, attempts, operation) => {
  let lastError
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await operation(attempt)
    } catch (error) {
      lastError = error
      console.error(`${label}: attempt ${attempt}/${attempts} failed: ${error.message}`)
    }
  }
  throw new Error(`${label}: failed after ${attempts} attempts`, { cause: lastError })
}

let server
let browser
try {
  assertAnimationContractRejectsDrift()
  run('git', ['init', '--quiet', viewer])
  run('git', ['remote', 'add', 'origin',
    'https://github.com/KhronosGroup/glTF-Sample-Viewer-Release.git'], viewer)
  await retry('Sample Viewer fetch', 3, () =>
    run('git', ['fetch', '--quiet', '--depth=1', 'origin', VIEWER_REVISION], viewer))
  run('git', ['checkout', '--quiet', 'FETCH_HEAD'], viewer)
  if (!process.env.ORIGAMI2_GLTF_RUNTIME_ARTIFACTS) {
    run('cargo', ['run', '--quiet', '--locked', '--release', '-p', 'ori-formats',
      '--example', 'generate_gltf_validator_fixtures', '--', artifacts])
  }
  assertAnimatedGlbContract(join(artifacts, 'animated.glb'))

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
  browser = await chromium.launch({
    headless: true,
    args: ['--use-angle=swiftshader', '--enable-webgl', '--ignore-gpu-blocklist'],
  })
  for (const file of ['static.glb', 'textured.glb', 'animated.glb']) {
    await retry(`${file} Sample Viewer runtime`, 3, async () => {
      const page = await browser.newPage({ viewport: { width: 1024, height: 768 } })
      try {
        const runtimeErrors = []
        await page.addInitScript(() => {
          window.__origamiSampleViewerWebglReady = false
          const originalGetContext = HTMLCanvasElement.prototype.getContext
          HTMLCanvasElement.prototype.getContext = function (type, ...args) {
            const context = Reflect.apply(originalGetContext, this, [type, ...args])
            if (context && (type === 'webgl' || type === 'webgl2')) {
              window.__origamiSampleViewerWebglReady = true
            }
            return context
          }
        })
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
          { waitUntil: 'domcontentloaded', timeout: 30_000 },
        )
        const canvas = page.locator('canvas')
        await canvas.waitFor({ state: 'visible', timeout: 30_000 })
        await page.waitForFunction(() => {
          const canvas = document.querySelector('canvas')
          return Boolean(
            window.__origamiSampleViewerWebglReady
            && canvas
            && canvas.width > 0
            && canvas.height > 0,
          )
        }, undefined, { timeout: 30_000 })
        await page.waitForTimeout(3_000)
        if (runtimeErrors.length !== 0) {
          throw new Error(`${file}: runtime errors:\n${runtimeErrors.join('\n')}`)
        }
        const bounds = await canvas.boundingBox()
        if (!bounds) throw new Error(`${file}: rendered canvas has no bounds`)
        const cdp = await page.context().newCDPSession(page)
        const firstFrame = Buffer.from((await cdp.send('Page.captureScreenshot', {
          format: 'png',
          captureBeyondViewport: false,
          clip: { ...bounds, scale: 1 },
        })).data, 'base64')
        if (firstFrame.length < 2_000) {
          throw new Error(`${file}: rendered canvas is unexpectedly empty`)
        }
        console.log(`${file}: Sample Viewer WebGL runtime visible, errors 0`)
      } finally {
        await page.close()
      }
    })
  }
  await browser.close()
  browser = undefined
} finally {
  if (browser) await browser.close()
  if (server) await new Promise((resolveClosed) => server.close(resolveClosed))
  rmSync(scratch, { recursive: true, force: true })
}
