import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
const origin = 'http://127.0.0.1:4187'
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4187', '--strictPort'], { stdio: 'ignore' })
let browser
try {
  for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(resolve => setTimeout(resolve, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage()
  await page.goto(`${origin}/scripts/miura-instruction-export-browser-harness.html`, { waitUntil: 'networkidle' })
  await page.getByText('3. Miura atomic 2 · 完成形サムネイル', { exact: true }).click()
  const details = page.getByLabel('構造化経路証明', { exact: true }); await details.waitFor()
  for (const text of ['出力前確認（読み取り専用）', '証明指紋: 6d6d6d6d6d6d…', '検証区間: 2', '始点姿勢:', '終点姿勢:', '元モデル束縛:']) if (!(await details.textContent()).includes(text)) throw new Error(`missing ${text}`)
  if (await details.locator('input, textarea, button').count()) throw new Error('proof details are editable')
  const exportButton = page.getByRole('button', { name: '折り図を書き出す', exact: true }); if (!(await exportButton.isEnabled())) throw new Error('export disabled')
  await exportButton.click(); await page.getByText('exports=1; ipc=begin_instruction_export,preview_instruction_export', { exact: true }).waitFor()
  console.log('Miura instruction export browser E2E passed')
} finally { await browser?.close(); server.kill('SIGTERM') }
