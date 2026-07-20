import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { DiagnosticsDialog } from '../src/components/DiagnosticsDialog.tsx'
import '../src/App.css'

export function Harness() {
  return (
    <main className="app-shell">
      <header className="titlebar" data-a11y-background inert>
        ORIGAMI2
      </header>
      <section className="workspace" data-a11y-background inert>
        Editor
      </section>
      <section className="timeline" data-a11y-background inert>
        Timeline
      </section>
      <footer className="statusbar" data-a11y-background inert>
        Status
      </footer>
      <DiagnosticsDialog open onClose={() => {}} />
    </main>
  )
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Harness />
  </StrictMode>,
)
