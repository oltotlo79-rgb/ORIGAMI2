import AxeBuilder from '@axe-core/playwright'
import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const origin = 'http://127.0.0.1:4182'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4182', '--strictPort'], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let output = ''; server.stdout.on('data', (chunk) => { output += chunk }); server.stderr.on('data', (chunk) => { output += chunk })
let browser
let page
try {
  await waitForServer()
  browser = await chromium.launch({ headless: true })
  const context = await browser.newContext()
  page = await context.newPage()
  await page.goto(`${origin}/scripts/runtime-updater-browser-harness.html`, { waitUntil: 'networkidle' })
  const region = page.getByRole('region', { name: 'App update' })
  await region.getByText('Check for updates manually').waitFor()
  await region.getByRole('button', { name: 'Check for updates' }).click()
  await region.getByRole('button', { name: 'Cancel' }).click()
  await region.getByText('Operation cancelled').waitFor()
  await region.getByRole('button', { name: 'Check for updates' }).click()
  await region.getByText('Security update').waitFor()
  await region.getByRole('button', { name: 'Download and verify' }).click()
  await region.getByRole('button', { name: 'Restart and apply' }).click()
  await region.getByText('Update application confirmed').waitFor()
  await region.getByRole('button').focus().catch(() => undefined)
  const axe = await new AxeBuilder({ page }).include('.runtime-updater-control').analyze()
  const major = axe.violations.filter(({ impact }) => impact === 'critical' || impact === 'serious')
  if (major.length > 0) throw new Error(JSON.stringify(major))
  console.log('runtime updater browser E2E passed: recovery, cancel/retry, explicit verify/apply, accessibility')
} catch (error) {
  const artifactDirectory = process.env.ORIGAMI2_RUNTIME_UPDATER_BROWSER_ARTIFACT_DIRECTORY
  if (artifactDirectory) {
    const directory = resolve(artifactDirectory)
    await mkdir(directory, { recursive: true })
    await writeFile(join(directory, 'runtime-updater-browser-failure.json'), `${JSON.stringify({
      schema: 'origami2.runtime-updater-browser-failure.v1',
      message: error instanceof Error ? error.stack ?? error.message : String(error),
      serverOutput: output.slice(-16000),
    }, null, 2)}\n`)
    try { await page?.screenshot({ path: join(directory, 'runtime-updater-browser-failure.png'), fullPage: true }) } catch {}
  }
  throw error
} finally {
  await browser?.close(); server.kill('SIGTERM')
}

async function waitForServer() {
  const deadline = Date.now() + 15000
  while (Date.now() < deadline) {
    if (server.exitCode !== null) throw new Error(`Vite exited early: ${output}`)
    try { if ((await fetch(origin)).ok) return } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Vite did not start: ${output}`)
}
