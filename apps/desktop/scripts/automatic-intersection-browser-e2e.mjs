import { chromium } from 'playwright'; import { spawn } from 'node:child_process'
const origin = 'http://127.0.0.1:4176'; const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4176', '--strictPort'], { stdio: 'ignore' }); let browser
try { for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise((r) => setTimeout(r, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage(); await page.goto(`${origin}/scripts/automatic-intersection-browser-harness.html`, { waitUntil: 'networkidle' })
  const topology = page.getByTestId('topology'); await page.getByRole('button', { name: 'balloon' }).click()
  if (await topology.textContent() !== 'vertices=7;edges=6') throw new Error('balloon topology mismatch')
  await page.getByRole('button', { name: 'multiple' }).click(); if (await topology.textContent() !== 'vertices=8;edges=7') throw new Error('multiple topology mismatch')
  await page.getByRole('button', { name: 'endpoint' }).click(); if (await topology.textContent() !== 'vertices=4;edges=3') throw new Error('endpoint topology mismatch')
  await page.getByRole('button', { name: 'undo' }).click(); await page.getByRole('button', { name: 'redo' }).click(); await page.getByRole('button', { name: 'save' }).click(); await page.getByRole('button', { name: 'reopen' }).click()
  await page.getByRole('button', { name: 'duplicate' }).click(); await page.getByText('duplicate-rejected', { exact: true }).waitFor()
  const e = await page.evaluate(() => window.__ORIGAMI2_AUTOMATIC_INTERSECTION__); if (e.addCalls !== 4 || e.undoCalls !== 1 || e.redoCalls !== 1 || e.saveCalls !== 1 || e.reopenCalls !== 1 || e.duplicateRejects !== 1) throw new Error(JSON.stringify(e))
  console.log('automatic intersection browser E2E passed')
} finally { await browser?.close(); server.kill('SIGTERM') }
