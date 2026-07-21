import { createRoot } from 'react-dom/client'
import { RuntimeUpdaterControl } from '../src/components/RuntimeUpdaterControl.tsx'
import { createLocaleStore } from '../src/lib/i18n.ts'
import '../src/App.css'

const locale = createLocaleStore({ readStoredLocale: () => 'en', writeStoredLocale() {}, applyDocumentLanguage() {} })
locale.initialize()
let checks = 0
const controller = {
  async recoverPending() { await new Promise((resolve) => setTimeout(resolve, 50)); return 'ready' as const },
  async check(signal: AbortSignal) {
    checks += 1
    if (checks === 1) await new Promise<void>((resolve) => signal.addEventListener('abort', () => resolve(), { once: true }))
    return { version: '2.0.0', releaseNotes: 'Security update', platform: 'windows-x64' as const, byteLength: 25 * 1024 * 1024 }
  },
  async downloadAndVerify() { return 'verified' as const },
  async restartAndApply() { return 'applied' as const },
}
createRoot(document.getElementById('root')!).render(<RuntimeUpdaterControl controller={controller} localeStore={locale} />)
