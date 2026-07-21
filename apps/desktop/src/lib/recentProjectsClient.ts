import { invoke } from '@tauri-apps/api/core'

import { normalizeProjectFileResponse } from './projectFileClient.ts'
import type { ProjectFileResponse } from './coreClient.ts'

export type RecentProjectItem = Readonly<{ opaque_id: string; display_name: string }>
export type RecentProjectOpenResult =
  | Readonly<{ status: 'opened'; file: ProjectFileResponse }>
  | Readonly<{ status: 'invalidated' }>

export class RecentProjectsClientError extends Error {
  readonly code: 'busy' | 'invalid_response'
  constructor(code: 'busy' | 'invalid_response') { super(code); this.name = 'RecentProjectsClientError'; this.code = code }
}

type NativeInvoke = (command: string, args?: Readonly<Record<string, unknown>>) => Promise<unknown>

export function createRecentProjectsClient(nativeInvoke: NativeInvoke = (command, args) => invoke(command, args)) {
  let active = false
  return Object.freeze({
    async list(): Promise<readonly RecentProjectItem[]> {
      return normalizeList(await nativeInvoke('list_recent_projects'))
    },
    async open(item: RecentProjectItem): Promise<RecentProjectOpenResult> {
      if (active) throw new RecentProjectsClientError('busy')
      if (!validItem(item)) throw new RecentProjectsClientError('invalid_response')
      active = true
      try {
        const value = await nativeInvoke('open_recent_project', Object.freeze({ opaqueId: item.opaque_id }))
        if (!record(value) || !exact(value, value.status === 'opened' ? ['status', 'file'] : ['status'])) {
          throw new RecentProjectsClientError('invalid_response')
        }
        if (value.status === 'invalidated') return Object.freeze({ status: 'invalidated' })
        if (value.status !== 'opened') throw new RecentProjectsClientError('invalid_response')
        return Object.freeze({ status: 'opened', file: normalizeProjectFileResponse(value.file) })
      } finally { active = false }
    },
  })
}

export function normalizeList(value: unknown): readonly RecentProjectItem[] {
  if (!Array.isArray(value) || value.length > 10) throw new RecentProjectsClientError('invalid_response')
  const ids = new Set<string>(); const result: RecentProjectItem[] = []
  for (const item of value) {
    if (!validItem(item) || ids.has(item.opaque_id)) throw new RecentProjectsClientError('invalid_response')
    ids.add(item.opaque_id); result.push(Object.freeze({ opaque_id: item.opaque_id, display_name: item.display_name }))
  }
  return Object.freeze(result)
}

function validItem(value: unknown): value is RecentProjectItem {
  return record(value) && exact(value, ['opaque_id', 'display_name'])
    && typeof value.opaque_id === 'string' && /^r1-[0-9a-f]{32}$/u.test(value.opaque_id)
    && typeof value.display_name === 'string' && value.display_name === value.display_name.trim()
    && value.display_name.length > 0 && new TextEncoder().encode(value.display_name).length <= 160
    && !/[\p{Cc}/\\]/u.test(value.display_name)
}
function record(value: unknown): value is Record<string, unknown> { return typeof value === 'object' && value !== null && !Array.isArray(value) }
function exact(value: Record<string, unknown>, keys: readonly string[]) { return Object.keys(value).sort().join('\0') === [...keys].sort().join('\0') }
