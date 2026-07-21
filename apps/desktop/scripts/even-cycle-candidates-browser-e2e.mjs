import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const origin = 'http://127.0.0.1:4183'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4183', '--strictPort'], { stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''; server.stdout.on('data', chunk => { serverOutput += chunk }); server.stderr.on('data', chunk => { serverOutput += chunk })
let browser; let page
try {
  for (let index = 0; index < 150; index += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(resolve => setTimeout(resolve, 100)) }
  browser = await chromium.launch({ headless: true }); page = await browser.newPage()
  await page.goto(`${origin}/scripts/even-cycle-candidates-browser-harness.html`, { waitUntil: 'networkidle' })
  for (const family of ['C6', 'C8']) {
    await page.getByRole('button', { name: family, exact: true }).click()
    await page.getByTestId('even-cycle-candidate').click(); await page.getByRole('button', { name: 'proof', exact: true }).click()
    await page.getByText('proof-certified', { exact: true }).waitFor(); await page.getByRole('button', { name: 'apply', exact: true }).click()
    await page.getByRole('button', { name: 'undo', exact: true }).click(); await page.getByRole('button', { name: 'redo', exact: true }).click()
    await page.getByRole('button', { name: 'reopen', exact: true }).click(); await page.getByText(`reopened-${family.toLowerCase()}-candidate-visible`, { exact: true }).waitFor()
  }
  await page.getByRole('button', { name: 'none fixture', exact: true }).click(); await page.getByText('none', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'unsupported fixture', exact: true }).click(); await page.getByText('unsupported', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'stale request', exact: true }).click(); await page.getByText('stale-rejected', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'ABA request', exact: true }).click(); await page.getByText('aba-rejected', { exact: true }).waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_EVEN_CYCLE_EVIDENCE__)
  if (JSON.stringify(evidence) !== JSON.stringify({ proofs: 2, applies: 2, undos: 2, redos: 2, reopens: 2, staleRejects: 1, abaRejects: 1 })) throw new Error(JSON.stringify(evidence))
  console.log('even-cycle candidates browser E2E passed')
} catch (error) {
  const output = process.env.ORIGAMI2_EVEN_CYCLE_BROWSER_ARTIFACT_DIRECTORY
  if (output) { await mkdir(resolve(output), { recursive: true }); await writeFile(join(resolve(output), 'even-cycle-browser-failure.json'), `${JSON.stringify({ schema: 'origami2.even-cycle-browser-failure.v1', message: error instanceof Error ? error.stack ?? error.message : String(error), serverOutput: serverOutput.slice(-16000) }, null, 2)}\n`); try { await page?.screenshot({ path: join(resolve(output), 'even-cycle-browser-failure.png'), fullPage: true }) } catch {} }
  throw error
} finally { await browser?.close(); server.kill('SIGTERM') }
