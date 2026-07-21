import { useEffect, useState } from 'react'
import type { Locale } from '../lib/i18n.ts'
import { createRecentProjectsClient, type RecentProjectItem } from '../lib/recentProjectsClient.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'

const defaultClient = createRecentProjectsClient()

export function RecentProjectsControl({ locale, onOpen, client = defaultClient }: Readonly<{ locale: Locale; onOpen: (project: ProjectSnapshot) => void; client?: ReturnType<typeof createRecentProjectsClient> }>) {
  const [items, setItems] = useState<readonly RecentProjectItem[]>([])
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  const refresh = async () => { try { setItems(await client.list()) } catch { setStatus(locale === 'ja' ? '最近使った作品を確認できません。' : 'Recent projects are unavailable.') } }
  useEffect(() => { void refresh() }, [locale])
  const open = async (item: RecentProjectItem) => {
    setBusy(true); setStatus('')
    try {
      const result = await client.open(item)
      if (result.status === 'opened') onOpen(result.file.project)
      else { setStatus(locale === 'ja' ? '作品が移動または置換されたため一覧から削除しました。' : 'The project moved or was replaced and was removed.'); await refresh() }
    } catch { setStatus(locale === 'ja' ? '作品を安全に開けませんでした。' : 'The project could not be opened safely.') }
    finally { setBusy(false) }
  }
  return <section aria-labelledby="recent-projects-title">
    <h2 id="recent-projects-title">{locale === 'ja' ? '最近使った作品' : 'Recent projects'}</h2>
    {items.length === 0 ? <p>{locale === 'ja' ? '履歴はありません。' : 'No recent projects.'}</p> : <ul>{items.map(item => <li key={item.opaque_id}><button disabled={busy} onClick={() => void open(item)}>{item.display_name}</button></li>)}</ul>}
    <output role="status" aria-live="polite">{status}</output>
  </section>
}
