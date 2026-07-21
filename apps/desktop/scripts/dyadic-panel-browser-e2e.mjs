import { chromium } from 'playwright'; import { spawn } from 'node:child_process'
const origin='http://127.0.0.1:4184'; const server=spawn(process.execPath,['./node_modules/vite/bin/vite.js','--host','127.0.0.1','--port','4184','--strictPort'],{stdio:'ignore'}); let browser
try { for(let i=0;i<100;i++){try{if((await fetch(origin)).ok)break}catch{} await new Promise(r=>setTimeout(r,100))} browser=await chromium.launch({headless:true}); const page=await browser.newPage(); page.on('pageerror', error => console.error(error)); await page.goto(`${origin}/scripts/dyadic-panel-browser-harness.html`,{waitUntil:'networkidle'})
  if (await page.getByRole('button',{name:'Apply authenticated path'}).count()) throw new Error('Apply exposed before preview mint')
  await page.getByRole('button',{name:'Search bounded dyadic paths'}).click(); await page.getByText(/mutation candidate ready/).waitFor()
  const status=await page.getByTestId('dyadic-pose-graph-status').innerText(); for(const expected of ['states 3','transitions 4','certified transitions 1',`binding ${'a'.repeat(64)}`,'positive thickness certified 1/1','layer transport certified 1/1']) if(!status.includes(expected)) throw new Error(`missing status: ${expected}`)
  if (await page.getByRole('button',{name:'Apply authenticated path'}).count()) throw new Error('Apply exposed before authenticated preview')
  await page.getByRole('button',{name:'Issue read-only preview'}).click(); await page.getByText(/authenticated one-shot/).waitFor()
  await page.getByRole('button',{name:'Apply authenticated path'}).click(); await page.getByText('applied-revision-2-timeline-dto-2').waitFor()
  for(const name of ['undo','redo','reopen']){await page.getByRole('button',{name}).click()} await page.getByText('reopened-timeline-dto-2').waitFor()
  const evidence=await page.evaluate(()=>window.__ORIGAMI2_DYADIC_PANEL_EVIDENCE__); if(JSON.stringify(evidence)!==JSON.stringify({reads:1,mints:1,applies:1,timelineDtos:2,undos:1,redos:1,reopens:1}))throw new Error(JSON.stringify(evidence)); console.log('dyadic production panel browser E2E passed')
} finally { await browser?.close(); server.kill('SIGTERM') }
