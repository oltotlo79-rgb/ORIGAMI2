import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4188'
const server = spawn(process.execPath, [
  './node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4188', '--strictPort',
], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''
server.stdout.on('data', (chunk) => { serverOutput += chunk })
server.stderr.on('data', (chunk) => { serverOutput += chunk })
let browser
try {
  await waitForServer()
  browser = await chromium.launch({ headless: true })
  const page = await browser.newPage()
  const errors = []
  page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()) })
  page.on('pageerror', (error) => errors.push(error.message))
  await page.goto(`${origin}/scripts/complete-animal-browser-harness.html`, { waitUntil: 'networkidle' })

  const preview = page.getByRole('region', { name: 'Complete animal candidate preview' })
  await preview.waitFor()
  if (await page.getByRole('list', { name: 'Four complete-animal binding dimensions' }).getByRole('listitem').count() !== 4) {
    throw new Error('complete animal did not expose exactly four semantic bindings')
  }
  await page.getByRole('button', { name: 'Reject confirmation' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Simulate failed apply' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Confirm and apply' }).click()
  await preview.waitFor({ state: 'detached' })
  await assertEvaluateFocus(page)

  await page.getByRole('button', { name: 'Evaluate complete animal grid' }).click()
  await page.getByRole('button', { name: 'Cancel 27-design evaluation' }).click()
  await preview.waitFor({ state: 'detached' })
  await assertEvaluateFocus(page)
  if (errors.length) throw new Error(`browser errors: ${errors.join(' | ')}`)
  console.log('complete animal browser E2E passed: bindings, reject/failure retention, apply/cancel focus')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}

async function assertEvaluateFocus(page) {
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete animal grid')
}

async function waitForServer() {
  const deadline = Date.now() + 15000
  while (Date.now() < deadline) {
    if (server.exitCode !== null) throw new Error(`Vite exited early: ${serverOutput}`)
    try { if ((await fetch(origin)).ok) return } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Vite did not start: ${serverOutput}`)
}
