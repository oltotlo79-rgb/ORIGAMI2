import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const origin = 'http://127.0.0.1:4174'
const server = spawn(process.execPath, [
  './node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4174', '--strictPort',
], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''
server.stdout.on('data', (chunk) => { serverOutput += chunk })
server.stderr.on('data', (chunk) => { serverOutput += chunk })
let browser
let page
try {
  await waitForServer()
  browser = await chromium.launch({ headless: true })
  page = await browser.newPage()
  await page.goto(`${origin}/scripts/diagnostics-browser-harness.html`, { waitUntil: 'networkidle' })
  const opener = page.getByRole('button', { name: 'Open diagnostics' })
  await opener.click()
  const dialog = page.getByRole('dialog', { name: 'Review diagnostics' })
  await dialog.waitFor()
  const json = page.getByLabel('Diagnostics JSON to review before sharing')
  if (await json.isEditable() || await json.getAttribute('readonly') === null) {
    throw new Error('diagnostics JSON is not readonly')
  }
  JSON.parse(await json.inputValue())

  const dialogClose = dialog.getByRole('button', { name: 'Close' }).last()
  await dialogClose.focus()
  await page.keyboard.press('Tab')
  if (!await dialog.getByLabel('Close').first().evaluate((node) => node === document.activeElement)) {
    throw new Error('forward Tab escaped diagnostics dialog')
  }
  await page.keyboard.press('Shift+Tab')
  if (!await dialogClose.evaluate((node) => node === document.activeElement)) {
    throw new Error('reverse Tab escaped diagnostics dialog')
  }

  const save = dialog.getByRole('button', { name: /Save as JSON file/u })
  await save.evaluate((node) => {
    node.click()
    node.click()
  })
  await page.waitForTimeout(300)
  const cancelText = await dialog.textContent()
  if (!cancelText?.includes('Save was canceled.')) {
    const state = await page.evaluate(() => window.__ORIGAMI2_DIAGNOSTICS_MOCK__)
    throw new Error(`picker cancel was not rendered: ${JSON.stringify({ state, cancelText })}`)
  }
  let mock = await page.evaluate(() => window.__ORIGAMI2_DIAGNOSTICS_MOCK__)
  if (mock.saveCalls !== 1 || mock.maximumActiveSaves !== 1) throw new Error('duplicate save was not suppressed')
  await save.click()
  await dialog.getByText(/Diagnostics JSON could not be saved/u).waitFor()
  await save.click()
  await dialog.getByText('Diagnostics JSON was saved.', { exact: true }).waitFor()
  mock = await page.evaluate(() => window.__ORIGAMI2_DIAGNOSTICS_MOCK__)
  if (mock.saveCalls !== 3 || mock.maximumActiveSaves !== 1) throw new Error('save retry contract failed')

  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'detached' })
  if (!await opener.evaluate((node) => node === document.activeElement)) throw new Error('focus did not return to opener')
  console.log('diagnostics browser E2E passed: readonly, picker cancel/retry, focus trap, Escape, single-flight')
} catch (error) {
  const output = process.env.ORIGAMI2_DIAGNOSTICS_BROWSER_ARTIFACT_DIRECTORY
  if (output) {
    await mkdir(resolve(output), { recursive: true })
    await writeFile(join(resolve(output), 'diagnostics-browser-failure.json'), `${JSON.stringify({
      schema: 'origami2.diagnostics-browser-failure.v1',
      message: error instanceof Error ? error.stack ?? error.message : String(error),
      serverOutput: serverOutput.slice(-16000),
    }, null, 2)}\n`)
    try { await page?.screenshot({ path: join(resolve(output), 'diagnostics-browser-failure.png'), fullPage: true }) } catch {}
  }
  throw error
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}

async function waitForServer() {
  const deadline = Date.now() + 15000
  while (Date.now() < deadline) {
    if (server.exitCode !== null) throw new Error(`Vite exited early: ${serverOutput}`)
    try { if ((await fetch(origin)).ok) return } catch {}
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Vite did not start: ${serverOutput}`)
}
