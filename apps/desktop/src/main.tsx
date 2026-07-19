import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { reportUnexpected } from './lib/diagnosticsRuntime'
import { initializeLocaleStore, localeStore } from './lib/i18n'
import { initializeTheme, themeStore } from './lib/theme'

initializeLocaleStore()
initializeTheme()

const reportUnhandledError = () => {
  reportUnexpected('app.unhandled_error')
}
const reportUnhandledRejection = () => {
  reportUnexpected('app.unhandled_rejection')
}

window.addEventListener('error', reportUnhandledError)
window.addEventListener('unhandledrejection', reportUnhandledRejection)

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    window.removeEventListener('error', reportUnhandledError)
    window.removeEventListener('unhandledrejection', reportUnhandledRejection)
    localeStore.dispose()
    themeStore.dispose()
  })
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
