import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const origin = 'http://127.0.0.1:4184'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4184', '--strictPort'], { stdio: 'ignore' })
let browser
const expectedStatus = ['reason proof_complete', 'states 3', 'transitions 4', 'certified transitions 1', `binding ${'a'.repeat(64)}`, 'positive thickness certified 1/1', 'layer transport certified 1/1']
const exactSchedule = count => JSON.stringify({ version: 1, entries: Array.from({ length: count }, (_, index) => `018f47a2-4b7a-7cc1-8abc-${String(index + 1).padStart(12, '0')}`).map(edge => ({ edge, uDomain: [{ numerator: 0, denominator: 1 }, { numerator: 1, denominator: 1 }], numeratorPowerCoefficients: [{ numerator: 0, denominator: 1 }], denominatorPowerCoefficients: [{ numerator: 1, denominator: 1 }], requestedAngleDegrees: 0 })) })

async function openScenario(scenario, hinges = 6) {
  const page = await browser.newPage()
  await page.goto(`${origin}/scripts/dyadic-panel-browser-harness.html?scenario=${scenario}&hinges=${hinges}`, { waitUntil: 'networkidle' })
  return page
}

async function mintPreview(page, hinges = 6) {
  if (await page.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error('Apply exposed before preview mint')
  await page.getByLabel(/Cycle path definition/).fill(exactSchedule(hinges))
  await page.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await page.getByText(/mutation candidate ready/).waitFor()
  const status = await page.getByTestId('dyadic-pose-graph-status').innerText()
  for (const expected of expectedStatus) if (!status.includes(expected)) throw new Error(`missing status: ${expected}`)
  if (await page.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error('Apply exposed before authenticated preview')
  await page.getByRole('button', { name: 'Issue read-only preview' }).click()
  await page.getByText(/authenticated one-shot/).waitFor()
}

async function verifyHigherDegreeLifecycle(hinges, label) {
  const page = await openScenario('success', hinges)
  await mintPreview(page, hinges)
  await page.getByRole('button', { name: 'Apply authenticated path' }).click()
  await page.getByText('applied-revision-2-timeline-dto-2').waitFor()
  for (const name of ['undo', 'redo', 'reopen']) await page.getByRole('button', { name }).click()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  const expected = { reads: 1, readHinges: hinges, readScheduleHinges: hinges, mints: 1, mintHinges: hinges, mintScheduleHinges: hinges, applyAttempts: 1, mutations: 1, failures: 0, cancels: 0, timelineDtos: 2, undos: 1, redos: 1, reopens: 1 }
  if (JSON.stringify(evidence) !== JSON.stringify(expected)) throw new Error(`${label}: ${JSON.stringify(evidence)}`)
  await page.close()
}

async function verifyAutomaticOppositePair(hinges, label) {
  const page = await openScenario('success', hinges)
  await page.getByLabel('Angle (degrees)').fill(String(2 * Math.atan2(1, 100) * 180 / Math.PI))
  await page.getByTestId('even-cycle-candidate').click()
  await page.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await page.getByText(/mutation candidate ready/).waitFor()
  await page.getByRole('button', { name: 'Issue read-only preview' }).click()
  await page.getByText(/authenticated one-shot/).waitFor()
  await page.getByRole('button', { name: 'Apply authenticated path' }).click()
  await page.getByText('applied-revision-2-timeline-dto-2').waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (evidence.readHinges !== hinges || evidence.readScheduleHinges !== 0 || evidence.mintHinges !== hinges || evidence.mintScheduleHinges !== 0 || evidence.mutations !== 1) throw new Error(`${label}: ${JSON.stringify(evidence)}`)
  await page.close()
}

async function verifyDetectedCycleBasis(hinges, label) {
  const page = await openScenario('success', hinges)
  for (const input of await page.getByLabel(/^Requested angle /).all()) await input.fill('1')
  await page.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await page.getByText(/mutation candidate ready/).waitFor()
  await page.getByRole('button', { name: 'Issue read-only preview' }).click()
  await page.getByText(/authenticated one-shot/).waitFor()
  await page.getByRole('button', { name: 'Apply authenticated path' }).click()
  await page.getByText('applied-revision-2-timeline-dto-2').waitFor()
  const evidence = await page.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (evidence.readHinges !== hinges || evidence.readScheduleHinges !== 0 || evidence.mintHinges !== hinges || evidence.mintScheduleHinges !== 0 || evidence.mutations !== 1) throw new Error(`${label}: ${JSON.stringify(evidence)}`)
  await page.close()
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
  if (JSON.stringify(successEvidence) !== JSON.stringify({ reads: 1, readHinges: 6, readScheduleHinges: 6, mints: 1, mintHinges: 6, mintScheduleHinges: 6, applyAttempts: 2, mutations: 1, failures: 1, cancels: 0, timelineDtos: 2, undos: 1, redos: 1, reopens: 1 })) throw new Error(JSON.stringify(successEvidence))
  await success.close()

  await verifyAutomaticOppositePair(6, 'automatic C6')
  await verifyAutomaticOppositePair(8, 'automatic C8')
  await verifyAutomaticOppositePair(16, 'automatic C16')
  await verifyDetectedCycleBasis(32, 'detected C32')
  await verifyDetectedCycleBasis(64, 'detected C64')

  await verifyHigherDegreeLifecycle(8, 'C8')
  await verifyHigherDegreeLifecycle(16, 'C16')
  await verifyHigherDegreeLifecycle(32, 'C32')
  await verifyHigherDegreeLifecycle(64, 'C64')

  const overLimit = await openScenario('success', 65)
  await overLimit.getByLabel(/Cycle path definition/).fill(exactSchedule(65))
  if (await overLimit.getByRole('button', { name: 'Search bounded dyadic paths' }).isEnabled()) throw new Error('C65 search must be disabled before IPC')
  const overLimitEvidence = await overLimit.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (overLimitEvidence.reads !== 0 || await overLimit.getByTestId('dyadic-pose-graph-status').count() || await overLimit.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error(`C65 must fail before IPC: ${JSON.stringify(overLimitEvidence)}`)
  await overLimit.close()

  for (const scenario of ['concave', 'cut', 'hole']) {
    const unsupportedGeometry = await openScenario(scenario, 6)
    await unsupportedGeometry.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
    await unsupportedGeometry.getByText(/reason no_certified_path/).waitFor()
    if (await unsupportedGeometry.getByRole('button', { name: 'Issue read-only preview' }).count() || await unsupportedGeometry.getByRole('button', { name: 'Apply authenticated path' }).count()) throw new Error(`${scenario} no-path exposed mutation controls`)
    const evidence = await unsupportedGeometry.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
    if (evidence.reads !== 1 || evidence.mints !== 0 || evidence.mutations !== 0 || evidence.timelineDtos !== 0) throw new Error(`${scenario} no-op: ${JSON.stringify(evidence)}`)
    await unsupportedGeometry.close()
  }

  for (const scenario of ['stale', 'aba', 'tamper']) {
    const page = await openScenario(scenario)
    await mintPreview(page)
    await page.getByRole('button', { name: 'Apply authenticated path' }).click()
    await page.getByRole('button', { name: 'Apply authenticated path' }).waitFor({ state: 'detached' })
    const evidence = await page.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
    if (evidence.readHinges !== 6 || evidence.readScheduleHinges !== 6 || evidence.mintHinges !== 6 || evidence.mintScheduleHinges !== 6 || evidence.applyAttempts !== 1 || evidence.failures !== 1 || evidence.mutations !== 0 || evidence.timelineDtos !== 0) throw new Error(`${scenario}: ${JSON.stringify(evidence)}`)
    if ((await page.locator('output').innerText()) !== 'ready') throw new Error(`${scenario} changed UI snapshot`)
    await page.close()
  }

  const cancel = await openScenario('cancel')
  await cancel.getByLabel(/Cycle path definition/).fill(exactSchedule(6))
  await cancel.getByRole('button', { name: 'Search bounded dyadic paths' }).click()
  await cancel.getByRole('button', { name: 'Cancel search' }).click()
  const cancelEvidence = await cancel.evaluate(() => window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__)
  if (cancelEvidence.reads !== 1 || cancelEvidence.readHinges !== 6 || cancelEvidence.readScheduleHinges !== 6 || cancelEvidence.cancels !== 1 || cancelEvidence.mutations !== 0) throw new Error(`cancel: ${JSON.stringify(cancelEvidence)}`)
  if (await cancel.getByTestId('dyadic-pose-graph-status').count()) throw new Error('cancel published stale read')
  await cancel.close()
  console.log('dyadic production panel lifecycle browser E2E passed')
} finally {
  await browser?.close()
  server.kill('SIGTERM')
}
