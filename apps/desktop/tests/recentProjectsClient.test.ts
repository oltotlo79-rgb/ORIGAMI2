import assert from 'node:assert/strict'; import test from 'node:test'
import { createRecentProjectsClient, normalizeList, RecentProjectsClientError } from '../src/lib/recentProjectsClient.ts'
const item = { opaque_id: `r1-${'a'.repeat(32)}`, display_name: '鶴' }
test('admits at most ten opaque pathless unique entries', () => {
  const list = normalizeList(Array.from({ length: 10 }, (_, i) => ({ opaque_id: `r1-${i.toString(16).padStart(32, '0')}`, display_name: `Bird ${i}` })))
  assert.equal(list.length, 10); assert.doesNotMatch(JSON.stringify(list), /path|volume|file_index|C:\\/ui)
  assert.throws(() => normalizeList([...list, item]), RecentProjectsClientError)
  assert.throws(() => normalizeList([{ ...item, path: 'secret.ori2' }]), RecentProjectsClientError)
})
test('selection sends only opaque id and admits invalidation', async () => {
  const calls: unknown[] = []; const client = createRecentProjectsClient(async (command, args) => { calls.push([command, args]); return { status: 'invalidated' } })
  assert.deepEqual(await client.open(item), { status: 'invalidated' })
  assert.deepEqual(calls, [['open_recent_project', { opaqueId: item.opaque_id }]])
})
test('unsafe names duplicate ids and response drift fail closed', () => {
  for (const value of [[{ ...item, display_name: '../secret' }], [item, item], [{ ...item, opaque_id: 'C:\\secret' }]]) assert.throws(() => normalizeList(value), RecentProjectsClientError)
})
