import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
const origin = 'http://127.0.0.1:4191'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4191', '--strictPort'], { stdio: 'ignore' })
let browser
try {
  for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(resolve => setTimeout(resolve, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage(); const errors = []; page.on('pageerror', error => errors.push(String(error)))
  await page.goto(`${origin}/scripts/app-instruction-export-browser-harness.html`, { waitUntil: 'networkidle' })
  try { await page.getByText(/3\. Miura atomic 2/).waitFor({ timeout: 5000 }) } catch (error) { throw new Error(`${error}\npage errors=${errors.join('|')}\nbody=${(await page.locator('body').innerText()).slice(0, 4000)}`) }
  const exportButton = page.locator('button').filter({ hasText: /折り図|謚倥ｊ蝗ｳ/ }).first(); try { await exportButton.click({ timeout: 5000 }) } catch (error) { throw new Error(`${error}\nbody=${(await page.locator('body').innerText()).slice(0, 6000)}\ncommands=${await page.evaluate(() => window.__ORIGAMI2_APP_EXPORT_EVIDENCE__.commands)}`) }
  await page.getByText('miura.pdf', { exact: true }).waitFor()
  const checkbox = page.getByRole('checkbox', { name: /注意事項|荳願ｨ倥/ }); await checkbox.check(); const saveButton = page.locator('button').filter({ hasText: /保存先|菫晏ｭ伜/ }).last(); await saveButton.click(); await page.getByText('miura.pdf', { exact: true }).waitFor({ state: 'detached' })
  await exportButton.click(); await page.getByText('miura.pdf', { exact: true }).waitFor(); await page.getByRole('dialog').locator('select').selectOption('svg_zip'); await page.getByText('miura-svg.zip', { exact: true }).waitFor()
  await page.evaluate(() => window.__ORIGAMI2_APP_EXPORT_EVIDENCE__.setSaveMode('cancel')); await checkbox.check(); await saveButton.click(); await page.getByText('miura-svg.zip', { exact: true }).waitFor()
  await page.locator('button').filter({ hasText: /キャンセル|繧ｭ繝｣繝ｳ繧ｻ繝ｫ/ }).last().click(); await page.getByText('miura-svg.zip', { exact: true }).waitFor({ state: 'detached' })
  await page.evaluate(() => { const evidence = window.__ORIGAMI2_APP_EXPORT_EVIDENCE__; evidence.setPreviewMode('valid'); evidence.setSaveMode('failure') }); await exportButton.click(); await page.getByText('miura.pdf', { exact: true }).waitFor(); await checkbox.check(); await saveButton.click(); await page.getByRole('dialog').getByRole('alert').waitFor(); await page.getByText('miura.pdf', { exact: true }).waitFor(); await page.locator('button').filter({ hasText: /キャンセル|繧ｭ繝｣繝ｳ繧ｻ繝ｫ/ }).last().click()
  for (const mode of ['stale', 'tamper']) { await page.evaluate(value => window.__ORIGAMI2_APP_EXPORT_EVIDENCE__.setPreviewMode(value), mode); await exportButton.click(); await page.getByRole('dialog').getByRole('alert').waitFor(); await page.getByRole('dialog').getByRole('button', { name: /閉じる|髢峨§繧・/ }).click() }
  const commands = await page.evaluate(() => window.__ORIGAMI2_APP_EXPORT_EVIDENCE__.commands)
  for (const expected of ['project_snapshot', 'begin_instruction_export', 'preview_instruction_export:pdf', 'save_instruction_export', 'preview_instruction_export:svg_zip', 'cancel_instruction_export']) if (!commands.includes(expected)) throw new Error(`missing ${expected}: ${commands.join(',')}`)
  if (commands.filter(command => command === 'cancel_instruction_export').length < 4) throw new Error(`stale/tamper/failure cleanup missing: ${commands.join(',')}`)
  if (errors.length) throw new Error(errors.join('\n'))
  console.log('Full App instruction export browser E2E passed')
} finally { await browser?.close(); server.kill('SIGTERM') }
