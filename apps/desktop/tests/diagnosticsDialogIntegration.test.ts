import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const dialogSource = readSource('../src/components/DiagnosticsDialog.tsx')
const cssSource = readSource('../src/App.css')

test('the native-only status action opens one modal that makes every background region inert', () => {
  assert.match(
    appSource,
    /isDiagnosticsShareAvailable\(\)\s*&&\s*\(/u,
  )
  assert.match(appSource, /aria-haspopup="dialog"/u)
  assert.match(
    appSource,
    /\{text\(\{\s*ja: '診断情報',\s*en: 'Diagnostics'\s*\}\)\}/u,
  )
  assert.equal(
    appSource.match(/inert=\{modalOpen\}/gu)?.length,
    5,
    'titlebar, workspace, timeline separator, timeline, and statusbar must all be inert',
  )
  assert.match(
    appSource,
    /<div className="workspace-timeline-separator" inert=\{modalOpen\}>/u,
  )
  assert.match(
    appSource,
    /const modalOpen = newProjectOpen\s*\|\| diagnosticsDialogOpen\s*\|\| foldTechniqueEditor !== null\s*\|\| foldTechniqueBusy\s*\|\| foldImportPreview !== null\s*\|\| svgImportPreview !== null/u,
  )
  assert.match(
    appSource,
    /requestAnimationFrame\(\(\) => diagnosticsButtonRef\.current\?\.focus\(\)\)/u,
  )
})

test('the dialog exposes exact read-only JSON and explicit manual actions', () => {
  assert.match(dialogSource, /role="dialog"/u)
  assert.match(dialogSource, /aria-modal="true"/u)
  assert.match(dialogSource, /aria-labelledby="diagnostics-dialog-title"/u)
  assert.match(dialogSource, /aria-describedby="diagnostics-dialog-description"/u)
  assert.match(dialogSource, /aria-busy=\{state\.kind === 'loading' \|\| saving\}/u)
  assert.match(dialogSource, /readOnly\s+value=\{state\.preview\.json\}/u)
  assert.match(dialogSource, /wrap="off"/u)
  assert.match(dialogSource, /spellCheck=\{false\}/u)
  assert.match(dialogSource, /jsonRef\.current\?\.focus\(\)/u)
  assert.match(dialogSource, /jsonRef\.current\?\.select\(\)/u)
  assert.match(dialogSource, /JSONファイルとして保存…/u)
  assert.match(dialogSource, /この情報は自動送信されません/u)
  assert.match(dialogSource, /表示されたJSONと保存されるJSONは同一/u)
})

test('dialog errors remain fixed and the UI has no automatic sharing capability', () => {
  assert.match(dialogSource, /\.catch\(\(\) => \{/u)
  assert.match(dialogSource, /\}\s*catch\s*\{/u)
  assert.doesNotMatch(dialogSource, /catch\s*\(\s*\w+\s*\)/u)
  assert.doesNotMatch(
    dialogSource,
    /navigator\.clipboard|fetch\(|window\.open|JSON\.stringify|reportUnexpected/u,
  )
  assert.match(dialogSource, /診断情報を準備できませんでした/u)
  assert.match(dialogSource, /診断JSONを保存できませんでした/u)
  assert.match(dialogSource, /notice: DiagnosticsNotice \| null/u)
  assert.match(
    dialogSource,
    /role=\{state\.notice === 'save_failed' \? 'alert' : 'status'\}/u,
  )
  assert.match(
    dialogSource,
    /aria-live=\{state\.notice === 'save_failed' \? 'assertive' : 'polite'\}/u,
  )
  assert.match(dialogSource, /Diagnostics JSON could not be saved/u)
})

test('focus, Escape, stale requests, and responsive overflow are explicitly bounded', () => {
  assert.match(dialogSource, /document\.addEventListener\('keydown', handleKeyDown, true\)/u)
  assert.match(dialogSource, /event\.key !== 'Escape'/u)
  assert.match(dialogSource, /if \(savingRef\.current\) return/u)
  assert.match(dialogSource, /event\.key !== 'Tab'/u)
  assert.match(dialogSource, /requestId !== requestSequenceRef\.current/gu)
  assert.match(cssSource, /\.diagnostics-dialog\s*\{\s*width: min\(760px/u)
  assert.match(cssSource, /\.diagnostics-json\s*\{[\s\S]*overflow: auto/u)
  assert.match(cssSource, /@media \(max-width: 800px\)/u)
})

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
