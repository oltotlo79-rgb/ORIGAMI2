import {
  matchesProjectOccGuard,
  type ProjectSnapshot,
} from './coreClient.ts'
import type {
  LocalizedText,
  MessageVariables,
} from './i18n.ts'

export type BeginnerWorkflowMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

export type BeginnerProjectBinding = Readonly<Pick<
  ProjectSnapshot,
  'project_instance_id' | 'project_id' | 'revision'
>>

export type BeginnerNativeEditRunner = (
  action: (
    projectId: string,
    revision: number,
    projectInstanceId: string,
  ) => Promise<ProjectSnapshot>,
) => Promise<boolean>

export function beginnerWorkflowMessage(
  text: LocalizedText,
  variables?: MessageVariables,
): BeginnerWorkflowMessage {
  return Object.freeze({ text, variables })
}

export function beginnerProjectBinding(
  snapshot: BeginnerProjectBinding,
): BeginnerProjectBinding {
  return Object.freeze({
    project_instance_id: snapshot.project_instance_id,
    project_id: snapshot.project_id,
    revision: snapshot.revision,
  })
}

export function matchesBeginnerProjectBinding(
  expected: BeginnerProjectBinding,
  current: BeginnerProjectBinding | null,
) {
  return current !== null && matchesProjectOccGuard({
    expectedProjectInstanceId: expected.project_instance_id,
    expectedProjectId: expected.project_id,
    expectedRevision: expected.revision,
  }, current)
}
