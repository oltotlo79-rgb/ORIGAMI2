import { isCanonicalNonNilUuid as isCanonicalUuid } from './canonicalUuid.ts'

/**
 * Accepts a native edit result only when it advances the exact current
 * project instance by one JavaScript-safe revision.
 *
 * This deliberately inspects own data descriptors instead of invoking
 * accessors. It verifies transport identity only; the feature-specific
 * snapshot fields still pass through their own admission boundaries.
 */
export function isExpectedNativeEditSnapshot(
  value: unknown,
  expectedProjectInstanceId: string,
  expectedProjectId: string,
  previousRevision: number,
): boolean {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
      || !isCanonicalUuid(expectedProjectInstanceId)
      || !isCanonicalUuid(expectedProjectId)
      || !Number.isSafeInteger(previousRevision)
      || previousRevision < 0
      || previousRevision >= Number.MAX_SAFE_INTEGER
    ) return false
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return false
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const projectInstanceId = dataValue(descriptors, 'project_instance_id')
    const projectId = dataValue(descriptors, 'project_id')
    const revision = dataValue(descriptors, 'revision')
    return projectInstanceId === expectedProjectInstanceId
      && projectId === expectedProjectId
      && revision === previousRevision + 1
  } catch {
    return false
  }
}

function dataValue(
  descriptors: Readonly<Record<PropertyKey, PropertyDescriptor>>,
  key: string,
) {
  const descriptor = descriptors[key]
  return descriptor && 'value' in descriptor && descriptor.enumerable
    ? descriptor.value
    : undefined
}
