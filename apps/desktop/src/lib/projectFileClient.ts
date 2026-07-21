import { invoke } from '@tauri-apps/api/core'

import type { ProjectFileResponse } from './coreClient.ts'
import { parsePathlessProjectSnapshot } from './recoveryClient.ts'

export type ProjectFileOperation = 'open' | 'save' | 'save_as'

export type ProjectFileNativeInvoke = (
  command: 'open_project' | 'save_project' | 'save_project_as',
) => Promise<unknown>

export class ProjectFileClientError extends Error {
  readonly code: 'busy' | 'invalid_response'

  constructor(code: 'busy' | 'invalid_response') {
    super(code)
    this.name = 'ProjectFileClientError'
    this.code = code
  }
}

export type ProjectFileClient = Readonly<{
  run: (operation: ProjectFileOperation) => Promise<ProjectFileResponse>
}>

const COMMANDS = Object.freeze({
  open: 'open_project',
  save: 'save_project',
  save_as: 'save_project_as',
} as const)

/**
 * Owns the normal project-file IPC boundary. Responses are admitted as exact,
 * pathless snapshots before UI state can be replaced. A client also permits
 * only one dialog/save transaction at a time, so an older completion cannot
 * overtake a newer project operation.
 */
export function createProjectFileClient(
  nativeInvoke: ProjectFileNativeInvoke = command => invoke<unknown>(command),
): ProjectFileClient {
  let active = false
  return Object.freeze({
    async run(operation: ProjectFileOperation): Promise<ProjectFileResponse> {
      if (!(operation in COMMANDS)) throw new ProjectFileClientError('invalid_response')
      if (active) throw new ProjectFileClientError('busy')
      active = true
      try {
        return normalizeProjectFileResponse(await nativeInvoke(COMMANDS[operation]))
      } finally {
        active = false
      }
    },
  })
}

export function normalizeProjectFileResponse(value: unknown): ProjectFileResponse {
  if (!isRecord(value) || !hasExactKeys(value, ['canceled', 'project'])) {
    throw new ProjectFileClientError('invalid_response')
  }
  if (typeof value.canceled !== 'boolean') {
    throw new ProjectFileClientError('invalid_response')
  }
  const project = parsePathlessProjectSnapshot(value.project)
  if (!project) throw new ProjectFileClientError('invalid_response')
  return Object.freeze({ canceled: value.canceled, project })
}

const defaultClient = createProjectFileClient()

export function runProjectFileOperation(
  operation: ProjectFileOperation,
): Promise<ProjectFileResponse> {
  return defaultClient.run(operation)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort()
  const sortedExpected = [...expected].sort()
  return actual.length === sortedExpected.length
    && actual.every((key, index) => key === sortedExpected[index])
}
