import { createRoot } from 'react-dom/client'
import { RecentProjectsControl } from '../src/components/RecentProjectsControl.tsx'
import { createRecentProjectsClient } from '../src/lib/recentProjectsClient.ts'
const id = `r1-${'a'.repeat(32)}`; let listCalls = 0
const evidence = { opened: 0, invalidated: 0, pathExposed: false }
const client = createRecentProjectsClient(async (command, args) => {
  evidence.pathExposed ||= JSON.stringify(args ?? {}).includes('path')
  if (command === 'list_recent_projects') { listCalls += 1; return listCalls === 1 ? [{ opaque_id: id, display_name: '折り鶴' }] : [] }
  evidence.invalidated += 1; return { status: 'invalidated' }
})
Object.assign(window, { __ORIGAMI2_RECENT_PROJECTS__: evidence })
createRoot(document.getElementById('root')!).render(<RecentProjectsControl locale="ja" client={client} onOpen={() => { evidence.opened += 1 }} />)
