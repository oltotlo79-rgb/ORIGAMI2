import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const origin = 'http://127.0.0.1:4179'
const server = spawn(process.execPath, [
  './node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4179', '--strictPort',
], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] })
let serverOutput = ''
server.stdout.on('data', (chunk) => { serverOutput += chunk })
server.stderr.on('data', (chunk) => { serverOutput += chunk })
let browser
let page
try {
  await waitForServer()
  browser = await chromium.launch({ headless: true })
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  await context.addInitScript(() => localStorage.setItem('origami2.locale', 'en'))
  page = await context.newPage()
  const browserErrors = []
  page.on('console', (message) => {
    if (message.type() === 'error') browserErrors.push(message.text())
  })
  page.on('pageerror', (error) => browserErrors.push(error.message))
  await page.goto(`${origin}/scripts/locale-browser-harness.html`, { waitUntil: 'networkidle' })

  let language = page.getByLabel('Display language')
  await language.waitFor()
  await assertVisibleCopy([
    'Evaluate top 3 of 27 designs',
    'A GLB 2.0 model is a read-only visual reference.',
    'Straight-line stacked fold',
    'Software updates',
  ])
  if (await page.evaluate(() => window.__ORIGAMI2_UPDATE_CHECK_CALLS__) !== 0) {
    throw new Error('update check ran without an explicit user action')
  }
  const checkNow = page.getByRole('button', { name: 'Check now' })
  await checkNow.evaluate((button) => { button.click(); button.click() })
  const updateToggle = page.getByRole('switch', { name: 'Enable update checks' })
  await updateToggle.uncheck()
  await page.waitForTimeout(150)
  if (await page.getByRole('link', { name: /Open release/u }).count() !== 0) {
    throw new Error('late aborted update response became visible')
  }
  await updateToggle.check()
  await checkNow.click()
  await page.getByText(
    'An update is available. Installed 1.0.0; latest release 1.1.0.',
    { exact: true },
  ).waitFor()
  if (await page.evaluate(() => window.__ORIGAMI2_UPDATE_CHECK_CALLS__) !== 2) {
    throw new Error('duplicate update checks were not suppressed')
  }
  const releaseLink = page.getByRole('link', { name: 'Open release 1.1.0 on GitHub' })
  if (await releaseLink.getAttribute('href') !==
    'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.1.0') {
    throw new Error('update release link escaped the canonical repository tag URL')
  }
  await language.selectOption('ja')
  language = page.getByLabel('表示言語')
  await language.waitFor()
  await assertVisibleCopy([
    '27案から上位3案を評価',
    'GLB 2.0モデルは読み取り専用の視覚参照です。',
    '一直線の折り重ね',
    'ソフトウェア更新',
    '更新があります。現在 1.0.0、公開版 1.1.0。',
  ])
  if (await page.getByText('Evaluate top 3 of 27 designs', { exact: true }).count() !== 0) {
    throw new Error('English candidate copy remained after Japanese locale change')
  }
  await assertNoInternalText()
  if (browserErrors.length !== 0) {
    throw new Error(`browser console leaked update failures: ${browserErrors.join(' | ')}`)
  }

  await page.goto(`${origin}/scripts/diagnostics-browser-harness.html`, {
    waitUntil: 'networkidle',
  })
  await page.getByRole('button', { name: 'Open diagnostics' }).click()
  await page.getByRole('dialog', { name: 'Review diagnostics' }).waitFor()
  await page.getByLabel('Diagnostics JSON to review before sharing').waitFor()
  await page.getByLabel('Display language').selectOption('ja')
  await page.getByRole('dialog', { name: '診断情報を確認' }).waitFor()
  await page.getByLabel('共有前に確認する診断JSON').waitFor()
  if (await page.getByRole('dialog', { name: 'Review diagnostics' }).count() !== 0) {
    throw new Error('English diagnostics ARIA name remained after locale change')
  }
  await assertNoInternalText()
  if (await page.locator('html').getAttribute('lang') !== 'ja') {
    throw new Error('document language did not update synchronously')
  }
  console.log('locale browser E2E passed: major feature and diagnostics visible/ARIA copy retranslated without internal identifiers')

  async function assertVisibleCopy(values) {
    for (const value of values) {
      const locator = page.getByText(value, { exact: value === 'Software updates' || value === 'ソフトウェア更新' })
      if (await locator.count() < 1) throw new Error(`missing localized UI contract: ${value}`)
      await locator.first().waitFor()
    }
  }

  async function assertNoInternalText() {
    const visible = await page.locator('body').innerText()
    for (const forbidden of [
      /[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/iu,
      /(?:native_|stacked_fold_)[a-z0-9_]+_v[0-9]+/iu,
      /(?:[A-Z]:\\|\/home\/|\/Users\/)[^\s]*/u,
    ]) if (forbidden.test(visible)) throw new Error('internal identifier or path reached visible UI')
  }
} catch (error) {
  const output = process.env.ORIGAMI2_LOCALE_BROWSER_ARTIFACT_DIRECTORY
  if (output) {
    await mkdir(resolve(output), { recursive: true })
    await writeFile(join(resolve(output), 'locale-browser-failure.json'), `${JSON.stringify({
      schema: 'origami2.locale-browser-failure.v1',
      message: error instanceof Error ? error.stack ?? error.message : String(error),
      serverOutput: serverOutput.slice(-16000),
    }, null, 2)}\n`)
    try { await page?.screenshot({ path: join(resolve(output), 'locale-browser-failure.png'), fullPage: true }) } catch {}
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
