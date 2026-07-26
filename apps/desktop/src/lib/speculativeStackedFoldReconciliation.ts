import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { ProjectSnapshot } from './coreClient.ts'
import { isSafeCount } from './proofProgressModel.ts'

export type SpeculativeStackedFoldReconciliationAuthorityV1 = Readonly<{
  projectInstanceId: string
  projectId: string
  sourceRevision: number
  targetRevision: number
}>

export type SpeculativeStackedFoldReconciliation =
  | Readonly<{ kind: 'committed'; snapshot: ProjectSnapshot }>
  | Readonly<{ kind: 'unchanged' }>
  | Readonly<{ kind: 'unavailable' }>

/**
 * Resolves an ambiguous one-shot Apply response only from a freshly loaded
 * project binding. It does not retry the mutation command.
 */
export async function reconcileSpeculativeStackedFoldApplyV1(
  refreshSnapshot: () => Promise<ProjectSnapshot>,
  expected: SpeculativeStackedFoldReconciliationAuthorityV1,
): Promise<SpeculativeStackedFoldReconciliation> {
  const expectedBinding = normalizeExpectedAuthority(expected)
  if (!expectedBinding) return Object.freeze({ kind: 'unavailable' })

  let snapshot: ProjectSnapshot
  try {
    snapshot = await refreshSnapshot()
  } catch {
    return Object.freeze({ kind: 'unavailable' })
  }
  const actual = readSnapshotAuthority(snapshot)
  if (
    !actual
    || actual.projectInstanceId !== expectedBinding.projectInstanceId
    || actual.projectId !== expectedBinding.projectId
  ) return Object.freeze({ kind: 'unavailable' })
  if (actual.revision === expectedBinding.targetRevision) {
    return Object.freeze({ kind: 'committed', snapshot })
  }
  if (actual.revision === expectedBinding.sourceRevision) {
    return Object.freeze({ kind: 'unchanged' })
  }
  return Object.freeze({ kind: 'unavailable' })
}

function normalizeExpectedAuthority(
  value: unknown,
): SpeculativeStackedFoldReconciliationAuthorityV1 | null {
  const fields = ownDataFields(value, [
    'projectInstanceId',
    'projectId',
    'sourceRevision',
    'targetRevision',
  ])
  if (!fields) return null
  const [
    projectInstanceId,
    projectId,
    sourceRevision,
    targetRevision,
  ] = fields
  if (
    !isCanonicalNonNilUuid(projectInstanceId)
    || !isCanonicalNonNilUuid(projectId)
    || !isSafeCount(sourceRevision)
    || sourceRevision === Number.MAX_SAFE_INTEGER
    || targetRevision !== sourceRevision + 1
  ) return null
  return Object.freeze({
    projectInstanceId,
    projectId,
    sourceRevision,
    targetRevision,
  })
}

function readSnapshotAuthority(
  value: unknown,
): Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
}> | null {
  const fields = ownDataFields(value, [
    'project_instance_id',
    'project_id',
    'revision',
  ])
  if (!fields) return null
  const [projectInstanceId, projectId, revision] = fields
  if (
    !isCanonicalNonNilUuid(projectInstanceId)
    || !isCanonicalNonNilUuid(projectId)
    || !isSafeCount(revision)
  ) return null
  return Object.freeze({ projectInstanceId, projectId, revision })
}

function ownDataFields(
  value: unknown,
  keys: readonly string[],
): readonly unknown[] | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return null
  }
  try {
    const fields: unknown[] = []
    for (const key of keys) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      if (
        !descriptor
        || !descriptor.enumerable
        || !('value' in descriptor)
      ) return null
      fields.push(descriptor.value)
    }
    return fields
  } catch {
    return null
  }
}
