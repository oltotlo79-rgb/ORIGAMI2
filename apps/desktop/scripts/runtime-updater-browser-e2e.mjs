import AxeBuilder from '@axe-core/playwright'
import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4182'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4182', '--strictPort'], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let output = ''; server.stdout.on('data', (chunk) => { output += chunk }); server.stderr.on('data', (chunk) => { output += chunk })
let browser
try {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    try { if ((await fetch(origin)).ok) break } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  browser = await chromium.launch({ headless: true })
  const context = await browser.newContext()
  const page = await context.newPage()
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
} finally {
  await browser?.close(); server.kill('SIGTERM')
}
