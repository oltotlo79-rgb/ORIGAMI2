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
    await page.getByTestId('even-cycle-candidate').click(); await page.getByRole('button', { name: 'Generate and prove Kawasaki linkage', exact: true }).click()
    await page.getByText('proof-certified', { exact: true }).waitFor(); await page.getByRole('button', { name: 'apply', exact: true }).click()
    await page.getByRole('button', { name: 'undo', exact: true }).click(); await page.getByRole('button', { name: 'redo', exact: true }).click()
    await page.getByRole('button', { name: 'reopen', exact: true }).click(); await page.getByText(`reopened-${family.toLowerCase()}-candidate-visible`, { exact: true }).waitFor()
  }
  for (const [button, family, profile] of [['Kawasaki 1/2', 'kawasaki-1-2', '1/2'], ['Kawasaki 3/5', 'kawasaki-3-5', '3/5'], ['Kawasaki 5/13', 'kawasaki-5-13', '5/13'], ['Kawasaki 7/25', 'kawasaki-7-25', '7/25']]) {
    await page.getByRole('button', { name: button, exact: true }).click()
    if (await page.getByTestId('kawasaki-endpoint-candidates').getByText('Collision uncertified').count() !== 5) throw new Error('strict endpoint statuses missing')
    await page.getByRole('button', { name: '1/16: Closure certified / Collision uncertified', exact: true }).click()
    await page.getByTestId('even-cycle-candidate').click(); await page.getByRole('button', { name: 'Generate and prove Kawasaki linkage', exact: true }).click()
    await page.getByText('proof-certified-1/16', { exact: true }).waitFor(); await page.getByRole('button', { name: 'apply', exact: true }).click(); await page.getByText(`applied-profile-${profile}`, { exact: true }).waitFor()
    await page.getByRole('button', { name: 'reopen', exact: true }).click(); await page.getByText(`reopened-${family}-profile-${profile}`, { exact: true }).waitFor()
  }
  await page.getByRole('button', { name: 'tamper profile', exact: true }).click(); await page.getByRole('button', { name: 'reopen', exact: true }).click()
  await page.getByText('profile-tamper-rejected', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'none fixture', exact: true }).click(); await page.getByText('none', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'unsupported fixture', exact: true }).click(); await page.getByText('unsupported', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'stale request', exact: true }).click(); await page.getByText('stale-rejected', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'ABA request', exact: true }).click(); await page.getByText('aba-rejected', { exact: true }).waitFor()
  for (const status of ['certified', 'no_path', 'resource_limit', 'cancelled']) { await page.getByRole('button', { name: `dyadic ${status}`, exact: true }).click(); await page.getByText(`${status}; states 9; transitions 24; explored 3; evaluated 8; read-only; certified transitions ${status === 'certified' ? 4 : 0}; binding ${status === 'certified' ? 'a'.repeat(64) : 'unavailable'}; positive thickness not certified; layer transport not certified; Apply disabled`, { exact: true }).waitFor() }
  const evidence = await page.evaluate(() => window.__ORIGAMI2_EVEN_CYCLE_EVIDENCE__)
  if (JSON.stringify(evidence) !== JSON.stringify({ automaticKawasakiProofs: 6, applies: 6, undos: 2, redos: 2, reopens: 7, profileTamperRejects: 1, staleRejects: 1, abaRejects: 1, dyadicReads: 4, dyadicCancels: 1 })) throw new Error(JSON.stringify(evidence))
  console.log('even-cycle candidates browser E2E passed')
} catch (error) {
  const output = process.env.ORIGAMI2_EVEN_CYCLE_BROWSER_ARTIFACT_DIRECTORY
  if (output) { await mkdir(resolve(output), { recursive: true }); await writeFile(join(resolve(output), 'even-cycle-browser-failure.json'), `${JSON.stringify({ schema: 'origami2.even-cycle-browser-failure.v1', message: error instanceof Error ? error.stack ?? error.message : String(error), serverOutput: serverOutput.slice(-16000) }, null, 2)}\n`); try { await page?.screenshot({ path: join(resolve(output), 'even-cycle-browser-failure.png'), fullPage: true }) } catch {} }
  throw error
} finally { await browser?.close(); server.kill('SIGTERM') }
