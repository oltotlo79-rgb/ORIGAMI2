import { chromium } from 'playwright'; import { spawn } from 'node:child_process'; import { mkdir } from 'node:fs/promises'; import { join, resolve } from 'node:path'
const origin = 'http://127.0.0.1:4176'; const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4176', '--strictPort'], { stdio: 'ignore' }); let browser
try {
  for (let i = 0; i < 100; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(r => setTimeout(r, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage(); await page.goto(`${origin}/scripts/recent-projects-harness.html`)
  await page.getByRole('heading', { name: '最近使った作品' }).waitFor(); await page.getByRole('button', { name: '折り鶴' }).waitFor()
  await page.reload({ waitUntil: 'networkidle' }); await page.getByRole('button', { name: '折り鶴' }).waitFor()
  await page.getByRole('button', { name: '折り鶴' }).click()
  await page.getByText('作品が移動または置換されたため一覧から削除しました。').waitFor(); await page.getByText('履歴はありません。').waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_RECENT_PROJECTS__)
  if (evidence.opened !== 0 || evidence.invalidated !== 1 || evidence.pathExposed) throw new Error(JSON.stringify(evidence))
  const violations = await page.locator('button:not([disabled]), a[href], input, select, textarea').evaluateAll(nodes => nodes.filter(node => !node.matches(':focus-visible')).length)
  if (violations !== 0) throw new Error('unexpected remaining interactive controls')
  console.log('recent projects browser E2E passed: cross-session pathless list and native invalidation')
} catch (error) {
  const output = process.env.ORIGAMI2_RECENT_PROJECTS_ARTIFACT_DIRECTORY
  if (output) {
    await mkdir(resolve(output), { recursive: true })
    const pages = browser?.contexts().flatMap(context => context.pages()) ?? []
    await pages[0]?.screenshot({ path: join(resolve(output), 'recent-projects-failure.png'), fullPage: true }).catch(() => {})
  }
  throw error
} finally { await browser?.close(); server.kill('SIGTERM') }
