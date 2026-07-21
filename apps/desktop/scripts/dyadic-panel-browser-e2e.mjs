import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4184'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4184', '--strictPort'], { stdio: 'ignore' })
let browser
const expectedStatus = ['states 3', 'transitions 4', 'certified transitions 1', `binding ${'a'.repeat(64)}`, 'positive thickness certified 1/1', 'layer transport certified 1/1']

async function openScenario(scenario) {
  const page = await browser.newPage()
  await page.goto(`${origin}/scripts/dyadic-panel-browser-harness.html?scenario=${scenario}`, { waitUntil: 'networkidle' })
  return page
}

async function mintPreview(page) {
  if (await page.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error('Apply exposed before preview mint')
  await page.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await page.getByText(/mutation candidate ready/).waitFor()
  const status = await page.getByTestId('dyadic-pose-graph-status').innerText()
  for (const expected of expectedStatus) if (!status.includes(expected)) throw new Error(`missing status: ${expected}`)
  if (await page.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error('Apply exposed before authenticated preview')
  await page.getByRole('button', { name: 'Issue read-only preview' }).click()
  await page.getByText(/authenticated one-shot/).waitFor()
}

try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(resolve => setTimeout(resolve, 100)) }
  browser = await chromium.launch({ headless: true })

  const success = await openScenario('success')
  await mintPreview(success)
  await success.getByRole('button', { name: 'Apply authenticated path' }).click()
  await success.getByText('applied-revision-2-timeline-dto-2').waitFor()
  if (await success.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error('one-shot Apply remained visible')
  const replayRejected = await success.evaluate(async () => {
    const request = { previewToken: '018f47a2-4b7a-7cc1-8abc-778899aabbcc', expectedProjectInstanceId: '018f47a2-4b7a-7cc1-8abc-112233445566', expectedProjectId: '018f47a2-4b7a-7cc1-8abc-665544332211', expectedRevision: 1, expectedTargetBindingSha256: 'e'.repeat(64), expectedPathBindingSha256: 'a'.repeat(64), expectedPositiveThicknessBindingSha256: 'b'.repeat(64), expectedLayerTransportBindingSha256: 'c'.repeat(64) }
    try { await window.__TAURI_INTERNALS__.invoke('apply_dyadic_pose_path_preview_v1', { request }); return false } catch { return true }
  })
  if (!replayRejected) throw new Error('consumed preview replay succeeded')
  for (const name of ['undo', 'redo', 'reopen']) await success.getByRole('button', { name }).click()
  const successEvidence = await success.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (JSON.stringify(successEvidence) !== JSON.stringify({ reads: 1, readHinges: 6, mints: 1, mintHinges: 6, applyAttempts: 2, mutations: 1, failures: 1, cancels: 0, timelineDtos: 2, undos: 1, redos: 1, reopens: 1 })) throw new Error(JSON.stringify(successEvidence))
  await success.close()

  for (const scenario of ['stale', 'aba', 'tamper']) {
    const page = await openScenario(scenario)
    await mintPreview(page)
    await page.getByRole('button', { name: 'Apply authenticated path' }).click()
    await page.getByRole('button', { name: 'Apply authenticated path' }).waitFor({ state: 'detached' })
    const evidence = await page.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
    if (evidence.readHinges !== 6 || evidence.mintHinges !== 6 || evidence.applyAttempts !== 1 || evidence.failures !== 1 || evidence.mutations !== 0 || evidence.timelineDtos !== 0) throw new Error(`${scenario}: ${JSON.stringify(evidence)}`)
    if ((await page.locator('output').innerText()) !== 'ready') throw new Error(`${scenario} changed UI snapshot`)
    await page.close()
  }

  const cancel = await openScenario('cancel')
  await cancel.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await cancel.getByRole('button', { name: 'Cancel search' }).click()
  const cancelEvidence = await cancel.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (cancelEvidence.reads !== 1 || cancelEvidence.readHinges !== 6 || cancelEvidence.cancels !== 1 || cancelEvidence.mutations !== 0) throw new Error(`cancel: ${JSON.stringify(cancelEvidence)}`)
  if (await cancel.getByTestId('dyadic-pose-graph-status').count()) throw new Error('cancel published stale read')
  await cancel.close()
  console.log('dyadic production panel lifecycle browser E2E passed')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
