import { chromium } from 'playwright'
import { spawn } from 'node:child_process'
const origin = 'http://127.0.0.1:4187'
const namedTechniqueCoverage = [
  'miura', 'inside_reverse_fold', 'outside_reverse_fold', 'sink_fold', 'accordion_fold',
  'layer_selective', 'book_fold', 'squash_fold', 'petal_fold', 'crimp_fold',
  'mountain_fold', 'valley_fold',
]
const server = spawn(process.execPath, ['./node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '4187', '--strictPort'], { stdio: 'ignore' })
let browser
try {
  for (let i = 0; i < 150; i += 1) { try { if ((await fetch(origin)).ok) break } catch {} await new Promise(resolve => setTimeout(resolve, 100)) }
  browser = await chromium.launch({ headless: true }); const page = await browser.newPage(); page.on('pageerror', error => console.error(error))
  await page.goto(`${origin}/scripts/miura-instruction-export-browser-harness.html`, { waitUntil: 'networkidle' })
  await page.getByText('3. Miura atomic 2 · 完成形サムネイル', { exact: true }).click()
  const details = page.getByLabel('構造化経路証明', { exact: true }); await details.waitFor()
  for (const text of ['出力前確認（読み取り専用）', '証明指紋: 6d6d6d6d6d6d…', '検証区間: 2', '始点姿勢:', '終点姿勢:', '元モデル束縛:']) if (!(await details.textContent()).includes(text)) throw new Error(`missing ${text}`)
  if (await details.locator('input, textarea, button').count()) throw new Error('proof details are editable')
  const exportButton = page.getByRole('button', { name: '折り図を書き出す', exact: true }); if (!(await exportButton.isEnabled())) throw new Error('export disabled')
  await exportButton.click(); await page.getByText('exports=1; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=2; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Stale revision', exact: true }).click(); await exportButton.click(); await page.getByText('exports=2; format=svg_zip; result=stale-rejected; ipc=begin_instruction_export,preview_instruction_export:svg_zip,cancel_instruction_export', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Tamper DTO hash', exact: true }).click(); await exportButton.click(); await page.getByText('exports=2; format=svg_zip; result=tamper-rejected; ipc=begin_instruction_export,preview_instruction_export:svg_zip,cancel_instruction_export', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Inside reverse timeline', exact: true }).click(); await page.getByText('3. 中割り折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=3; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Sink timeline', exact: true }).click(); await page.getByText('3. 沈め折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=4; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Accordion timeline', exact: true }).click(); await page.getByText('3. 蛇腹折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=5; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Layer selective timeline', exact: true }).click(); await page.getByText('3. 層選択折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=6; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Book fold timeline', exact: true }).click(); await page.getByText('3. 二つ折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=7; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Outside reverse timeline', exact: true }).click(); await page.getByText('3. 外割り折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=8; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Squash fold timeline', exact: true }).click(); await page.getByText('3. つぶし折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=9; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Petal fold timeline', exact: true }).click(); await page.getByText('3. 花弁折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=10; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Crimp fold timeline', exact: true }).click(); await page.getByText('3. 段折り 2 · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=11; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Mountain fold timeline', exact: true }).click(); await page.getByText('2. 山折り · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await exportButton.click(); await page.getByText('exports=12; format=pdf; result=ready; ipc=begin_instruction_export,preview_instruction_export:pdf', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Valley fold timeline', exact: true }).click(); await page.getByText('2. 谷折り · 完成形サムネイル', { exact: true }).click(); await page.getByLabel('構造化経路証明', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG mode', exact: true }).click(); await exportButton.click(); await page.getByText('exports=13; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Uncertified timelines', exact: true }).click()
  const uncertifiedTechniques = [
    ['Miura timeline', 'Miura atomic 2'],
    ['Inside reverse timeline', '中割り折り 2'],
    ['Outside reverse timeline', '外割り折り 2'],
    ['Sink timeline', '沈め折り 2'],
    ['Accordion timeline', '蛇腹折り 2'],
    ['Layer selective timeline', '層選択折り 2'],
    ['Book fold timeline', '二つ折り 2'],
    ['Squash fold timeline', 'つぶし折り 2'],
    ['Petal fold timeline', '花弁折り 2'],
    ['Crimp fold timeline', '段折り 2'],
    ['Mountain fold timeline', '山折り'],
    ['Valley fold timeline', '谷折り'],
  ]
  for (const [button, title] of uncertifiedTechniques) {
    await page.getByRole('button', { name: button, exact: true }).click()
    const ordinal = button === 'Mountain fold timeline' || button === 'Valley fold timeline' ? 2 : 3
    await page.getByText(`${ordinal}. ${title} · 完成形サムネイル`, { exact: true }).click()
    const description = page.getByRole('textbox', { name: '説明', exact: true }); await description.waitFor()
    if (!(await description.inputValue()).includes('連続折り経路は未証明です。')) throw new Error(`${title} omits the uncertified explanation`)
    if (await page.getByLabel('構造化経路証明', { exact: true }).count()) throw new Error(`${title} exposes structured proof without a certificate`)
    if (!(await exportButton.isEnabled())) throw new Error(`${title} uncertified export is disabled`)
    await exportButton.click()
  }
  await page.getByText('exports=25; format=svg_zip; result=ready; ipc=begin_instruction_export,preview_instruction_export:svg_zip', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Start progress lifecycle', exact: true }).click(); await page.getByText('progress; ipc=begin_instruction_export,get_instruction_export_progress', { exact: true }).waitFor()
  await page.getByRole('button', { name: '生成を中止', exact: true }).click(); await page.getByText('cancelled; successes=0; cleanup=cleared; ipc=begin_instruction_export,get_instruction_export_progress,cancel_instruction_export', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'PDF save success', exact: true }).click(); await page.getByRole('checkbox', { name: '上記の注意事項を確認しました', exact: true }).check(); await page.getByRole('button', { name: '保存先を選んで書き出す…', exact: true }).click(); await page.getByText('saved; successes=1; cleanup=cleared; format=pdf; ipc=save_instruction_export', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'SVG save success', exact: true }).click(); await page.getByRole('checkbox', { name: '上記の注意事項を確認しました', exact: true }).check(); await page.getByRole('button', { name: '保存先を選んで書き出す…', exact: true }).click(); await page.getByText('saved; successes=2; cleanup=cleared; format=svg_zip; ipc=save_instruction_export', { exact: true }).waitFor()
  await page.getByRole('button', { name: 'Save picker cancel', exact: true }).click(); await page.getByRole('checkbox', { name: '上記の注意事項を確認しました', exact: true }).check(); await page.getByRole('button', { name: '保存先を選んで書き出す…', exact: true }).click(); await page.getByText('save-cancelled; successes=2; cleanup=retained; format=pdf; ipc=save_instruction_export', { exact: true }).waitFor(); await page.getByText('保存先の選択をキャンセルしました。', { exact: true }).waitFor(); await page.getByRole('button', { name: 'キャンセル', exact: true }).click()
  await page.getByRole('button', { name: 'Atomic save failure', exact: true }).click(); await page.getByRole('checkbox', { name: '上記の注意事項を確認しました', exact: true }).check(); await page.getByRole('button', { name: '保存先を選んで書き出す…', exact: true }).click(); await page.getByText('save-failed; successes=2; cleanup=retained; format=svg_zip; ipc=save_instruction_export', { exact: true }).waitFor(); await page.getByRole('alert').getByText('折り図を原子的に保存できませんでした。', { exact: true }).waitFor(); await page.getByRole('button', { name: 'キャンセル', exact: true }).click()
  if (namedTechniqueCoverage.length !== 12) throw new Error('named technique coverage is incomplete')
  console.log('Named technique instruction export browser E2E passed')
} finally { await browser?.close(); server.kill('SIGTERM') }
