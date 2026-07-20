import AxeBuilder from '@axe-core/playwright'
import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const host = '127.0.0.1'
const port = 4173
const origin = `http://${host}:${port}`
const server = spawn(
  process.execPath,
  ['./node_modules/vite/bin/vite.js', '--host', host, '--port', String(port), '--strictPort'],
  { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] },
)
let serverOutput = ''
server.stdout.on('data', (chunk) => { serverOutput += chunk })
server.stderr.on('data', (chunk) => { serverOutput += chunk })

let browser
let page
try {
  await waitForServer(origin)
  browser = await chromium.launch({ headless: true })
  const context = await browser.newContext()
  page = await context.newPage()
  const consoleErrors = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  page.on('pageerror', (error) => consoleErrors.push(error.message))

  await page.goto(origin, { waitUntil: 'networkidle' })
  await page.locator('main.app-shell').waitFor()

  for (const selector of [
    'header.titlebar',
    'section.workspace',
    'section.timeline',
    'footer.statusbar',
  ]) {
    const count = await page.locator(selector).count()
    if (count !== 1) throw new Error(`expected one primary region: ${selector}`)
  }
  const viewport = page.locator('.fold-preview-viewport')
  if (await viewport.count() !== 1) throw new Error('3D fold viewport is missing')
  const canvas = page.locator('.crease-canvas')
  if (await canvas.count() !== 1) throw new Error('2D crease canvas is missing')

  let passedRules = 0
  for (const theme of ['light', 'dark']) {
    await page.evaluate((value) => {
      document.documentElement.dataset.theme = value
    }, theme)
    const report = await new AxeBuilder({ page })
      .include('main.app-shell')
      .analyze()
    passedRules += report.passes.length
    const major = report.violations.filter(
      ({ impact }) => impact === 'critical' || impact === 'serious',
    )
    if (major.length > 0) {
      throw new Error(
        `${theme} theme serious accessibility violations:\n`
          + JSON.stringify(major, null, 2),
      )
    }
  }

  await page.goto(`${origin}/scripts/accessibility-harness.html`, {
    waitUntil: 'networkidle',
  })
  const dialog = page.locator('[role="dialog"]:visible').first()
  try {
    await dialog.waitFor({ timeout: 5_000 })
  } catch {
    throw new Error(
      `modal harness did not render:\n${consoleErrors.join('\n')}\n${await page.content()}`,
    )
  }
  const backgroundRegions = page.locator(
    '[data-a11y-background]',
  )
  const inertCount = await backgroundRegions.evaluateAll(
    (elements) => elements.filter((element) => element.inert).length,
  )
  if (inertCount !== 4) {
    throw new Error(`modal background inert contract failed: ${inertCount}/4`)
  }
  await page.keyboard.press('Tab')
  const focusIsInsideDialog = await dialog.evaluate(
    (element) => element.contains(document.activeElement),
  )
  if (!focusIsInsideDialog) throw new Error('keyboard focus escaped the modal dialog')

  if (consoleErrors.length > 0) {
    throw new Error(`browser console errors:\n${consoleErrors.join('\n')}`)
  }
  console.log(
    `browser accessibility smoke passed: ${passedRules} axe rule/theme checks, `
      + '2D/3D/statusbar/modal focus and inert contracts',
  )
} catch (error) {
  const artifactDirectory = process.env.ORIGAMI2_A11Y_ARTIFACT_DIRECTORY
  if (artifactDirectory) {
    const output = resolve(artifactDirectory)
    await mkdir(output, { recursive: true })
    const message = error instanceof Error ? error.stack ?? error.message : String(error)
    await writeFile(
      join(output, 'accessibility-failure.json'),
      `${JSON.stringify({
        schema: 'origami2.accessibility-failure.v1',
        message,
        serverOutput: serverOutput.slice(-16_000),
      }, null, 2)}\n`,
      'utf8',
    )
    try {
      await page?.screenshot({
        path: join(output, 'accessibility-failure.png'),
        fullPage: true,
      })
    } catch {
      // The JSON diagnostic remains available when the page cannot render.
    }
  }
  throw error
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}

async function waitForServer(url) {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    if (server.exitCode !== null) {
      throw new Error(`Vite preview exited early (${server.exitCode}):\n${serverOutput}`)
    }
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {
      // The bounded retry loop owns startup timing.
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Vite preview did not become ready:\n${serverOutput}`)
}
