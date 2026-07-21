import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const origin = 'http://127.0.0.1:4176'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4176', '--strictPort'], { stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''
server.stdout.on('data', (chunk) => { serverOutput += chunk })
server.stderr.on('data', (chunk) => { serverOutput += chunk })
let browser
let page
try {
  for (let i = 0; i < 150; i += 1) {
    try { if ((await fetch(origin)).ok) break } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  browser = await chromium.launch({ headless: true })
  page = await browser.newPage()
  await page.goto(`${origin}/scripts/automatic-intersection-browser-harness.html`, { waitUntil: 'networkidle' })
  const topology = page.getByTestId('topology')
  await page.getByRole('button', { name: 'balloon' }).click()
  if (await topology.textContent() !== 'vertices=7;edges=6') throw new Error('balloon topology mismatch')
  await page.getByRole('button', { name: 'multiple' }).click()
  if (await topology.textContent() !== 'vertices=8;edges=7') throw new Error('multiple topology mismatch')
  await page.getByRole('button', { name: 'endpoint' }).click()
  if (await topology.textContent() !== 'vertices=4;edges=3') throw new Error('endpoint topology mismatch')
  await page.getByRole('button', { name: 'undo' }).click()
  await page.getByRole('button', { name: 'redo' }).click()
  await page.getByRole('button', { name: 'save' }).click()
  await page.getByRole('button', { name: 'reopen' }).click()
  await page.getByRole('button', { name: 'duplicate' }).click()
  await page.getByText('duplicate-rejected', { exact: true }).waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_AUTOMATIC_INTERSECTION__)
  if (evidence.addCalls !== 4 || evidence.undoCalls !== 1 || evidence.redoCalls !== 1 || evidence.saveCalls !== 1 || evidence.reopenCalls !== 1 || evidence.duplicateRejects !== 1) throw new Error(JSON.stringify(evidence))
  console.log('automatic intersection browser E2E passed')
} catch (error) {
  const output = process.env.ORIGAMI2_AUTOMATIC_INTERSECTION_ARTIFACT_DIRECTORY
  if (output) {
    const directory = resolve(output)
    await mkdir(directory, { recursive: true })
    await writeFile(join(directory, 'automatic-intersection-failure.json'), JSON.stringify({ message: String(error), serverOutput: serverOutput.slice(-16_000) }))
    try { await page?.screenshot({ path: join(directory, 'automatic-intersection-failure.png'), fullPage: true }) } catch {}
  }
  throw error
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
