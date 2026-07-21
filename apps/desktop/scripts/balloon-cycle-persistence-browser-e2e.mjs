import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4177'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4177', '--strictPort'], { stdio: 'inherit' })
let browser
try {
  for (let index = 0; index < 150; index += 1) {
    try { if ((await fetch(origin)).ok) break } catch {}
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  browser = await chromium.launch({ headless: true })
  const page = await browser.newPage()
  await page.goto(`${origin}/scripts/balloon-cycle-persistence-browser-harness.html`, { waitUntil: 'networkidle' })
  await page.getByRole('button', { name: 'save', exact: true }).click()
  await page.getByRole('button', { name: 'reopen', exact: true }).click()
  const proof = page.getByRole('region', { name: 'Persisted cycle proof' })
  if (!(await proof.textContent()).includes('hinges=6transitions=5')) throw new Error('persisted C6 proof missing')
  await page.getByRole('button', { name: 'undo', exact: true }).click()
  if (await page.getByTestId('step-count').textContent() !== 'steps=0') throw new Error('undo did not remove step')
  await page.getByRole('button', { name: 'redo', exact: true }).click()
  if (await page.getByTestId('step-count').textContent() !== 'steps=1') throw new Error('redo did not restore step')
  await page.getByRole('button', { name: 'save', exact: true }).click()
  await page.getByRole('button', { name: 'tamper saved pose', exact: true }).click()
  await page.getByRole('button', { name: 'reopen', exact: true }).click()
  await page.getByText('tamper-rejected', { exact: true }).waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_BALLOON_CYCLE_PERSISTENCE__)
  if (evidence.saves !== 2 || evidence.reopens !== 2 || evidence.undos !== 1 || evidence.redos !== 1 || evidence.tamperRejects !== 1) throw new Error(JSON.stringify(evidence))
  console.log('balloon cycle persistence browser E2E passed')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
