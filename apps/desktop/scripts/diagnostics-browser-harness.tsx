import { useEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { DiagnosticsDialog } from '../src/components/DiagnosticsDialog.tsx'
import { LanguageControl } from '../src/components/LanguageControl.tsx'
import { DIAGNOSTIC_SCOPES } from '../src/lib/diagnostics.ts'
import { localeStore, selectLocalizedText, useLocale } from '../src/lib/i18n.ts'
import '../src/App.css'

declare global {
  interface Window {
    __ORIGAMI2_DIAGNOSTICS_MOCK__: {
      saveCalls: number
      activeSaves: number
      maximumActiveSaves: number
    }
  }
}

const json = JSON.stringify({
  schema: 'origami2.redacted-diagnostics.v1',
  unexpected: DIAGNOSTIC_SCOPES.map((scope) => ({ scope, count: '0' })),
})
const byteLength = new TextEncoder().encode(json).byteLength
const mock = { saveCalls: 0, activeSaves: 0, maximumActiveSaves: 0 }
window.__ORIGAMI2_DIAGNOSTICS_MOCK__ = mock
Object.assign(window, {
  __TAURI_INTERNALS__: {
    invoke: async (command: string) => {
      if (command === 'prepare_diagnostics_share_preview') {
        return { preview_generation: 7, json, byte_length: byteLength }
      }
      if (command !== 'save_diagnostics_share_preview') throw new Error('unexpected mock command')
      mock.saveCalls += 1
      mock.activeSaves += 1
      mock.maximumActiveSaves = Math.max(mock.maximumActiveSaves, mock.activeSaves)
      await new Promise((resolve) => setTimeout(resolve, 120))
      mock.activeSaves -= 1
      if (mock.saveCalls === 1) {
        return { preview_generation: 7, byte_length: byteLength, canceled: true }
      }
      if (mock.saveCalls === 2) throw new Error('mock native picker failure')
      return { preview_generation: 7, byte_length: byteLength, canceled: false }
    },
  },
})

localeStore.initialize()
localeStore.setLocale('en')

function Harness() {
  const locale = useLocale(localeStore)
  const [open, setOpen] = useState(false)
  const opener = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (!open) opener.current?.focus()
  }, [open])
  return <main>
    <LanguageControl store={localeStore} />
    <button ref={opener} type="button" onClick={() => setOpen(true)}>
      {selectLocalizedText(locale, { ja: '診断情報を開く', en: 'Open diagnostics' })}
    </button>
    <DiagnosticsDialog open={open} onClose={() => setOpen(false)} />
  </main>
}

createRoot(document.getElementById('root')!).render(<Harness />)
