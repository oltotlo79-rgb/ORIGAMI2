import type { InstructionStepPresentation } from './instructionTimeline'

const encoder = new TextEncoder()

export async function pathCertificateEndpointsMatch(
  previous: InstructionStepPresentation | undefined,
  step: InstructionStepPresentation,
): Promise<boolean> {
  const reference = step.visual.path_certificate_reference_v1
  const fixedFace = step.pose.fixed_face
  if (!reference || !previous || !fixedFace
    || previous.pose.source_model_fingerprint !== step.pose.source_model_fingerprint) return false
  const model = step.pose.source_model_fingerprint
  const [modelBinding, source, target] = await Promise.all([
    digest([encoder.encode('path_certificate_source_model_binding_v1'), encoder.encode(model)]),
    poseDigest(model, fixedFace, previous.pose.hinge_angles),
    poseDigest(model, fixedFace, step.pose.hinge_angles),
  ])
  return equal(modelBinding, reference.source_model_binding_sha256)
    && equal(source, reference.source_pose_sha256)
    && equal(target, reference.target_pose_sha256)
}

async function poseDigest(
  model: string,
  fixedFace: string,
  hinges: InstructionStepPresentation['pose']['hinge_angles'],
): Promise<Uint8Array> {
  const face = uuidBytes(fixedFace)
  if (!face) return new Uint8Array()
  const fields: Uint8Array[] = [
    encoder.encode('origami2_instruction_pose_fingerprint_v1'),
    encoder.encode(model),
    face,
  ]
  const sorted = [...hinges].sort((left, right) => left.edge.localeCompare(right.edge))
  for (const hinge of sorted) {
    const edge = uuidBytes(hinge.edge)
    if (!edge) return new Uint8Array()
    const angle = new Uint8Array(8)
    new DataView(angle.buffer).setFloat64(0, hinge.angle_degrees, false)
    fields.push(edge, angle)
  }
  return digest(fields)
}

async function digest(fields: readonly Uint8Array[]): Promise<Uint8Array> {
  const size = fields.reduce((total, field) => total + field.byteLength, 0)
  const bytes = new Uint8Array(size)
  let offset = 0
  for (const field of fields) {
    bytes.set(field, offset)
    offset += field.byteLength
  }
  return new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))
}

function uuidBytes(value: string): Uint8Array | null {
  const hex = value.replaceAll('-', '')
  if (!/^[0-9a-f]{32}$/iu.test(hex)) return null
  return Uint8Array.from({ length: 16 }, (_, index) => Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16))
}

function equal(left: Uint8Array, right: readonly number[]): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index])
}
