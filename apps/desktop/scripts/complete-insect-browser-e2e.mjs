import { runBrowserE2E } from './browser-e2e-runtime.mjs'

await runBrowserE2E({
  name: 'complete insect browser E2E',
  port: 4189,
  harnessPath: '/scripts/complete-insect-browser-harness.html',
  readyButtonName: 'Try asymmetric insect pair',
}, async (page) => {
  await page.getByRole('button', { name: 'Try asymmetric insect pair' }).click()
  if (await page.getByRole('list').count()) throw new Error('asymmetric pair reached binding UI')
  await page.getByRole('button', { name: 'Recognize complete insect image' }).click()
  await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  const preview = page.getByRole('region', { name: 'Complete insect candidate preview' })
  await preview.waitFor()
  await page.getByRole('button', { name: 'Replace insect reference' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.getByText('Stale insect candidate replaced', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Recognize complete insect GLB' }).click()
  await assertBindings(page)
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  await page.getByRole('button', { name: 'Confirm and apply complete insect' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete insect grid')
  for (const [button, status] of [
    ['Undo complete insect', 'Complete insect apply undone'],
    ['Redo complete insect', 'Complete insect apply redone'],
    ['Save and reopen complete insect', 'Complete insect saved and reopened'],
  ]) {
    await page.getByRole('button', { name: button }).click()
    await page.getByText(status, { exact: true }).waitFor()
  }
  await page.getByRole('button', { name: 'Recognize complete insect image' }).click()
  await page.getByRole('button', { name: 'Evaluate complete insect grid' }).click()
  await page.getByRole('button', { name: 'Cancel candidate generation' }).click()
  await preview.waitFor({ state: 'detached' })
  await page.waitForFunction(() => document.activeElement?.textContent === 'Evaluate complete insect grid')
})
console.log('complete insect browser E2E passed: image/GLB, five bindings, stale/cancel, apply history/save')

async function assertBindings(page) {
  const list = page.getByRole('list', { name: 'Five complete-insect binding dimensions' })
  await list.waitFor()
  if (await list.getByRole('listitem').count() !== 5) throw new Error('complete insect binding count changed')
}
