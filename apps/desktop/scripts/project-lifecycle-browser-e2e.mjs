import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const origin = 'http://127.0.0.1:4175'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4175', '--strictPort'], { stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''; server.stdout.on('data', (x) => { serverOutput += x }); server.stderr.on('data', (x) => { serverOutput += x })
let browser; let page
try {
  for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise((r) => setTimeout(r, 100)) }
  browser = await chromium.launch({ headless: true }); page = await browser.newPage()
  await page.goto(`${origin}/scripts/project-lifecycle-harness.html`, { waitUntil: 'networkidle' })
  const close = page.getByRole('button', { name: 'Close project' }); await close.click()
  const dialog = page.getByRole('dialog'); await dialog.waitFor(); await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
  await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'detached' })
  if (!await close.evaluate((n) => n === document.activeElement)) throw new Error('close focus was not restored')
  const save = page.getByRole('button', { name: 'Save project as' })
  await save.evaluate((n) => { n.click(); n.click() }); await page.getByText('save-canceled', { exact: true }).waitFor()
  await save.click(); await page.getByText('save-failed', { exact: true }).waitFor()
  await save.click(); await page.getByText('saved', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Discard recovery' }).click(); await page.getByText('recovery-discarded', { exact: true }).waitFor()
  await close.click(); await page.getByRole('button', { name: 'Discard and close' }).click(); await page.waitForTimeout(300)
  await close.click(); await page.getByRole('button', { name: 'Discard and close' }).click()
  await page.getByRole('button', { name: 'Start stale close' }).click(); await page.waitForTimeout(300)
  const evidence = await page.evaluate(() => window.__ORIGAMI2_PROJECT_LIFECYCLE__)
  if (evidence.saveCalls !== 3 || evidence.maximumActiveSaves !== 1 || evidence.recoveryCalls !== 1 || evidence.prepareCalls !== 2 || evidence.closeRequests !== 1) throw new Error(`lifecycle evidence mismatch: ${JSON.stringify(evidence)}`)
  console.log('project lifecycle browser E2E passed: dirty close, save retry/single-flight, keyboard focus, close handshake')
} catch (error) {
  const output = process.env.ORIGAMI2_PROJECT_LIFECYCLE_ARTIFACT_DIRECTORY
  if (output) { await mkdir(resolve(output), { recursive: true }); await writeFile(join(resolve(output), 'project-lifecycle-failure.json'), JSON.stringify({ message: String(error), serverOutput: serverOutput.slice(-16000) })); try { await page?.screenshot({ path: join(resolve(output), 'project-lifecycle-failure.png'), fullPage: true }) } catch {} }
  throw error
} finally { await browser?.close(); server.kill('SIGTERM') }
