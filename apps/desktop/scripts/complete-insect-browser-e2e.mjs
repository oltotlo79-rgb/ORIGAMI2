import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4189'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4189', '--strictPort'],
  { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let browser
try {
  await waitForServer()
  browser = await chromium.launch({ headless: true })
  const page = await browser.newPage()
  await page.goto(`${origin}/scripts/complete-insect-browser-harness.html`, { waitUntil: 'networkidle' })
  await page.getByRole('button', { name: 'Try asymmetric insect pair' }).click()
  if (await page.getByRole('list').count()) throw new Error('asymmetric pair reached binding UI')
  await page.getByRole('button', { name: 'Recognize complete insect image' }).click()
  await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  const preview = page.getByRole('region', { name: 'Complete insect candidate preview' })
  await preview.waitFor()
  await page.getByRole('button', { name: 'Replace insect reference' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.getByText('Stale insect candidate replaced', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Recognize complete insect GLB' }).click()
  await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  await page.getByRole('button', { name: 'Confirm and apply complete insect' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete insect grid')
  for (const [button, status] of [
    ['Undo complete insect', 'Complete insect apply undone'],
    ['Redo complete insect', 'Complete insect apply redone'],
    ['Save and reopen complete insect', 'Complete insect saved and reopened'],
  ]) {
    await page.getByRole('button', { name: button }).click()
    await page.getByText(status, { exact: true }).waitFor()
  }
  await page.getByRole('button', { name: 'Recognize complete insect image' }).click()
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  await page.getByRole('button', { name: 'Cancel 27-design evaluation' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete insect grid')
  console.log('complete insect browser E2E passed: image/GLB, five bindings, stale/cancel, apply history/save')
} finally {
  await browser?.close(); server.kill('SIGTERM')
}

async function assertBindings(page) {
  const list = page.getByRole('list', { name: 'Five complete-insect binding dimensions' })
  await list.waitFor()
  if (await list.getByRole('listitem').count() !== 5) throw new Error('complete insect binding count changed')
}
async function waitForServer() {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    try { if ((await fetch(origin)).ok) return } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error('complete insect browser harness did not start')
}
