import { runBrowserE2E } from './browser-e2e-runtime.mjs'

await runBrowserE2E({
  name: 'complete animal browser E2E',
  port: 4188,
  harnessPath: '/scripts/complete-animal-browser-harness.html',
  readyButtonName: 'Try missing wing binding',
}, async (page) => {
  const preview = page.getByRole('region', { name: 'Complete animal candidate preview' })
  await page.getByRole('button', { name: 'Try missing wing binding' }).click()
  await page.getByText('Rejected: wing binding is missing', { exact: true }).waitFor()
  if (await page.getByRole('list').count()) throw new Error('missing wing reached the binding UI')
  await page.getByRole('button', { name: 'Try asymmetric wing pair' }).click()
  await page.getByText('Rejected: wing pair is asymmetric', { exact: true }).waitFor()
  if (await page.getByRole('list').count()) throw new Error('asymmetric wing pair reached the binding UI')

  await page.getByRole('button', { name: 'Recognize winged animal image' }).click()
  await assertFiveBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete animal grid' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Replace reference while preview is open' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.getByText('Stale candidate replaced by a newer reference', { exact: true }).waitFor()

  await page.getByRole('button', { name: 'Recognize winged animal GLB' }).click()
  await assertFiveBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete animal grid' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Reject confirmation' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Simulate failed apply' }).click()
  await preview.waitFor()
  await page.getByRole('button', { name: 'Confirm and apply' }).click()
  await preview.waitFor({ state: 'detached' })
  await assertEvaluateFocus(page)
  await page.getByRole('button', { name: 'Undo winged animal' }).click()
  await page.getByText('Winged animal apply undone', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Redo winged animal' }).click()
  await page.getByText('Winged animal apply redone', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Save and reopen project' }).click()
  await page.getByText('Winged animal project saved and reopened', { exact: true }).waitFor()

  await page.getByRole('button', { name: 'Recognize winged animal image' }).click()
  await page.getByRole('button', { name: 'Evaluate complete animal grid' }).click()
  await page.getByRole('button', { name: 'Cancel candidate generation' }).click()
  await preview.waitFor({ state: 'detached' })
  await assertEvaluateFocus(page)
})
console.log('complete winged animal browser E2E passed: image/GLB, five bindings, stale/cancel, apply history/save')

async function assertEvaluateFocus(page) {
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete animal grid')
}

async function assertFiveBindings(page) {
  const bindings = page.getByRole('list', { name: 'Five complete-animal binding dimensions' })
  await bindings.waitFor()
  if (await bindings.getByRole('listitem').count() !== 5) {
    throw new Error('winged animal did not expose exactly five semantic bindings')
  }
}
