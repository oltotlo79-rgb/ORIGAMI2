import { createRoot } from 'react-dom/client'
import { LanguageControl } from '../src/components/LanguageControl.tsx'
import { LayerOrderViewer } from '../src/components/StackedFoldPanel.tsx'
import { UpdateCheckControl } from '../src/components/UpdateCheckControl.tsx'
import { localeStore, selectLocalizedText, useLocale } from '../src/lib/i18n.ts'
import { createUpdateCheckSettingsStore } from '../src/lib/updateCheckSettings.ts'
import '../src/App.css'

localeStore.initialize()
localeStore.setLocale('en')

declare global {
  interface Window { __ORIGAMI2_UPDATE_CHECK_CALLS__: number }
}
window.__ORIGAMI2_UPDATE_CHECK_CALLS__ = 0
const updateSettings = createUpdateCheckSettingsStore({
  readStoredSettings: () => null,
  writeStoredSettings: () => undefined,
})
const updateClient = {
  async checkNow() {
    window.__ORIGAMI2_UPDATE_CHECK_CALLS__ += 1
    await new Promise((resolve) => setTimeout(resolve, 100))
    return {
      kind: 'update_available' as const,
      currentVersion: '1.0.0',
      latestVersion: '1.1.0',
      releasePageUrl: 'https://github.com/oltotlo79-rgb/ORIGAMI2/releases/tag/v1.1.0',
    }
  },
}
const installedVersionProvider = { getVersion: () => '1.0.0' }

const faces = [
  '018f47a2-4b7a-7cc1-8abc-778899aabbcc',
  '018f47a2-4b7a-7cc1-8abc-778899aabbdd',
]
const cells = [{
  cellKeySha256: 'a'.repeat(64),
  boundaryWorld: [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
  bottomToTopFaces: faces,
}] as const

function Harness() {
  const locale = useLocale(localeStore)
  const t = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
  return <main>
    <LanguageControl store={localeStore} />
    <section aria-label={t('27案探索', '27-design search')}>
      <button>{t('27案から上位3案を評価', 'Evaluate top 3 of 27 designs')}</button>
      <p>{t(
        'GLB 2.0モデルは読み取り専用の視覚参照です。形状の自動認識や折り設計の生成権限は与えません。',
        'A GLB 2.0 model is a read-only visual reference. It grants no automatic recognition or fold-generation authority.',
      )}</p>
    </section>
    <h2>{t('一直線の折り重ね', 'Straight-line stacked fold')}</h2>
    <p role="status">{t('経路証明を待機中', 'Waiting for path certificate')}</p>
    <LayerOrderViewer
      locale={locale} cells={cells} selectedCell={null} selectedFace={null}
      hoveredFace={null} onSelectCell={() => undefined} onSelectFace={() => undefined}
      onHoverFace={() => undefined}
    />
    <UpdateCheckControl
      localeStore={localeStore}
      settingsStore={updateSettings}
      client={updateClient}
      versionProvider={installedVersionProvider}
    />
  </main>
}

createRoot(document.getElementById('root')!).render(<Harness />)
