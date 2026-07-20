import type { ProjectSnapshot } from './coreClient.ts'
import type { FoldPreviewAppliedPoseSnapshot } from './foldPreviewAppliedPose.ts'
import {
  createInstructionPoseDraft,
  type InstructionPoseDraft,
} from './instructionTimeline.ts'
import type { Locale } from './i18n.ts'

export type InstructionAutoRecordPlan = Readonly<{
  sequence: number
  title: string
  pose: InstructionPoseDraft
}>

export function planInstructionAutoRecord(input: Readonly<{
  enabled: boolean
  sequence: number
  lastRecordedSequence: number
  snapshot: ProjectSnapshot | null
  appliedPose: FoldPreviewAppliedPoseSnapshot | null
  locale: Locale
}>): InstructionAutoRecordPlan | null {
  if (
    !input.enabled
    || !Number.isSafeInteger(input.sequence)
    || !Number.isSafeInteger(input.lastRecordedSequence)
    || input.sequence <= input.lastRecordedSequence
  ) return null
  const { snapshot, appliedPose } = input
  if (
    !snapshot
    || !appliedPose
    || appliedPose.projectId !== snapshot.project_id
    || appliedPose.revision !== snapshot.revision
    || appliedPose.state === 'running'
  ) return null
  const pose = createInstructionPoseDraft(
    appliedPose,
    snapshot.fold_model_fingerprint,
  )
  if (!pose) return null
  const stepNumber = snapshot.instruction_timeline.steps.length + 1
  return Object.freeze({
    sequence: input.sequence,
    title: input.locale === 'en'
      ? `Auto-recorded step ${stepNumber}`
      : `自動記録 手順 ${stepNumber}`,
    pose,
  })
}
