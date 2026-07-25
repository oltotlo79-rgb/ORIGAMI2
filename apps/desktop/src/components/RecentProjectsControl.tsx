import { useCallback, useEffect, useState } from 'react'
import { selectLocalizedText, type Locale } from '../lib/i18n.ts'
import { createRecentProjectsClient, type RecentProjectItem } from '../lib/recentProjectsClient.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  RECENT_PROJECTS_CONTROL_TEXT as TEXT,
} from '../lib/recentProjectsControlText.ts'

const defaultClient = createRecentProjectsClient()
type StatusTextKey = 'listUnavailable' | 'invalidated' | 'openFailed'

export function RecentProjectsControl({ locale, onOpen, client = defaultClient }: Readonly<{ locale: Locale; onOpen: (project: ProjectSnapshot) => void; client?: ReturnType<typeof createRecentProjectsClient> }>) {
  const [items, setItems] = useState<readonly RecentProjectItem[]>([])
  const [statusKey, setStatusKey] = useState<StatusTextKey | null>(null)
  const [busy, setBusy] = useState(false)
  const t = (key: keyof typeof TEXT) =>
    selectLocalizedText(locale, TEXT[key])
  const refresh = useCallback(async () => {
    try {
      setItems(await client.list())
    } catch {
      setStatusKey('listUnavailable')
    }
  }, [client])
  useEffect(() => { void refresh() }, [locale, refresh])
  const open = async (item: RecentProjectItem) => {
    setBusy(true); setStatusKey(null)
    try {
      const result = await client.open(item)
      if (result.status === 'opened') onOpen(result.file.project)
      else {
        setStatusKey('invalidated')
        await refresh()
      }
    } catch {
      setStatusKey('openFailed')
    }
    finally { setBusy(false) }
  }
  return <section aria-labelledby="recent-projects-title">
    <h2 id="recent-projects-title">{t('title')}</h2>
    {items.length === 0
      ? <p>{t('empty')}</p>
      : <ul>{items.map(item => <li key={item.opaque_id}><button disabled={busy} onClick={() => void open(item)}>{item.display_name}</button></li>)}</ul>}
    <output role="status" aria-live="polite">
      {statusKey === null ? '' : t(statusKey)}
    </output>
  </section>
}
