import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
const origin = 'http://127.0.0.1:4190'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4190', '--strictPort'], { cwd: process.cwd(), stdio: 'ignore' })
let browser
try {
  for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {}; await new Promise((r) => setTimeout(r, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage()
  await page.goto(`${origin}/scripts/generic-target-browser-harness.html`, { waitUntil: 'networkidle' })
  await page.getByRole('button', { name: 'Try oversized target' }).click()
  if (await page.getByRole('list').count()) throw new Error('oversized target reached UI')
  await page.getByRole('button', { name: 'Recognize mixed target image' }).click(); await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate generic target grid' }).click()
  const preview = page.getByRole('region', { name: 'Generic target candidate preview' }); await preview.waitFor()
  await page.getByRole('button', { name: 'Replace recognized target' }).click(); await preview.waitFor({ state: 'detached' })
  await page.getByRole('button', { name: 'Recognize mixed target GLB' }).click(); await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate generic target grid' }).click()
  await page.getByRole('button', { name: 'Cancel generic target grid' }).click(); await preview.waitFor({ state: 'detached' })
  await page.getByRole('button', { name: 'Evaluate generic target grid' }).click()
  await page.getByRole('button', { name: 'Confirm and apply generic target' }).click(); await preview.waitFor({ state: 'detached' })
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate generic target grid')
  for (const [button, status] of [['Undo generic target', 'Generic target undone'], ['Redo generic target', 'Generic target redone'], ['Save and reopen generic target', 'Generic target saved and reopened']]) {
    await page.getByRole('button', { name: button }).click(); await page.getByText(status, { exact: true }).waitFor()
  }
  console.log('generic target browser E2E passed: image/GLB, bounds, stale/cancel, apply history/save')
} finally { await browser?.close(); server.kill('SIGTERM') }
async function assertBindings(page) {
  const list = page.getByRole('list', { name: 'Bounded generic target binding dimensions' }); await list.waitFor()
  if (await list.getByRole('listitem').count() !== 2) throw new Error('generic binding count changed')
}
