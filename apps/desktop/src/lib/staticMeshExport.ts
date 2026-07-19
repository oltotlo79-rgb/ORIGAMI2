import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { Locale } from './i18n.ts'

export type StaticMeshExportFormat = 'obj' | 'stl' | 'glb'

export type StaticMeshExportWarning =
  | 'mid_surface_only'
  | 'no_thickness_solid'
  | 'no_materials_textures_animation'
  | 'no_project_semantics'
  | 'stl_triangle_soup_facet_normals'
  | 'stl_printability_not_guaranteed'

export type StaticMeshExportPreview = Readonly<{
  exportId: string
  projectInstanceId: string
  projectId: string
  revision: number
  sourceFingerprint: string
  poseGeneration: string
  format: StaticMeshExportFormat
  formatSummary: string
  suggestedFileName: string
  byteCount: number
  paperThicknessMm: number
  faceCount: number
  vertexCount: number
  triangleCount: number
  geometryProfile: 'authenticated_mid_surface_triangle_mesh_v1'
  sourceUnit: 'millimeter'
  encodedUnit: 'millimeter' | 'meter'
  sourceAxis: 'right-handed X-right Y-forward Z-up'
  encodedAxis:
    | 'right-handed X-right Y-forward Z-up'
    | 'glTF 2.0 right-handed -X-right Y-up Z-forward'
  warnings: readonly StaticMeshExportWarning[]
}>

export type StaticMeshExportPreviewResponse = Readonly<{
  preview: StaticMeshExportPreview
}>

export type StaticMeshExportSaveResponse = Readonly<{
  canceled: boolean
}>

export const STATIC_MESH_EXPORT_FORMATS:
ReadonlyArray<Readonly<{ value: StaticMeshExportFormat; label: string }>> =
  Object.freeze([
    { value: 'obj', label: 'Wavefront OBJ' },
    { value: 'stl', label: 'Binary STL' },
    { value: 'glb', label: 'glTF 2.0 GLB' },
  ])

const PREVIEW_KEYS = [
  'exportId',
  'projectInstanceId',
  'projectId',
  'revision',
  'sourceFingerprint',
  'poseGeneration',
  'format',
  'formatSummary',
  'suggestedFileName',
  'byteCount',
  'paperThicknessMm',
  'faceCount',
  'vertexCount',
  'triangleCount',
  'geometryProfile',
  'sourceUnit',
  'encodedUnit',
  'sourceAxis',
  'encodedAxis',
  'warnings',
] as const

const BASE_WARNINGS: readonly StaticMeshExportWarning[] = Object.freeze([
  'mid_surface_only',
  'no_thickness_solid',
  'no_materials_textures_animation',
  'no_project_semantics',
])

const SOURCE_AXIS = 'right-handed X-right Y-forward Z-up'
const GLTF_AXIS = 'glTF 2.0 right-handed -X-right Y-up Z-forward'
const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/u
const CANONICAL_U64_PATTERN = /^(?:0|[1-9][0-9]{0,19})$/u
const MAX_OUTPUT_BYTES = 64 * 1024 * 1024
const MAX_VERTICES = 100_000
const MAX_TRIANGLES = 200_000

export function isStaticMeshExportFormat(
  value: unknown,
): value is StaticMeshExportFormat {
  return value === 'obj' || value === 'stl' || value === 'glb'
}

export function staticMeshExportFormatLabel(format: StaticMeshExportFormat) {
  switch (format) {
    case 'obj':
      return 'Wavefront OBJ'
    case 'stl':
      return 'Binary STL'
    case 'glb':
      return 'glTF 2.0 GLB'
  }
}

export function normalizeStaticMeshExportPreviewResponse(
  value: unknown,
): StaticMeshExportPreviewResponse | null {
  const response = exactRecord(value, ['preview'])
  const record = response ? exactRecord(response.preview, PREVIEW_KEYS) : null
  if (!record) return null
  const format = record.format
  if (
    !isCanonicalNonNilUuid(record.exportId)
    || !isCanonicalNonNilUuid(record.projectInstanceId)
    || !isCanonicalNonNilUuid(record.projectId)
    || !isSafeNonNegativeInteger(record.revision)
    || typeof record.sourceFingerprint !== 'string'
    || !FINGERPRINT_PATTERN.test(record.sourceFingerprint)
    || typeof record.poseGeneration !== 'string'
    || !isCanonicalU64(record.poseGeneration)
    || record.poseGeneration === '0'
    || !isStaticMeshExportFormat(format)
    || typeof record.formatSummary !== 'string'
    || record.formatSummary !== expectedFormatSummary(format)
    || !isSafeFileName(record.suggestedFileName, format)
    || !isSafePositiveInteger(record.byteCount)
    || record.byteCount > MAX_OUTPUT_BYTES
    || typeof record.paperThicknessMm !== 'number'
    || !Number.isFinite(record.paperThicknessMm)
    || record.paperThicknessMm < 0
    || !isSafePositiveInteger(record.faceCount)
    || record.faceCount > MAX_VERTICES
    || !isSafePositiveInteger(record.vertexCount)
    || record.vertexCount > MAX_VERTICES
    || !isSafePositiveInteger(record.triangleCount)
    || record.triangleCount > MAX_TRIANGLES
    || record.faceCount > record.triangleCount
    || record.faceCount * 3 > record.vertexCount
    || record.geometryProfile !== 'authenticated_mid_surface_triangle_mesh_v1'
    || record.sourceUnit !== 'millimeter'
    || record.encodedUnit !== (format === 'glb' ? 'meter' : 'millimeter')
    || record.sourceAxis !== SOURCE_AXIS
    || record.encodedAxis !== (format === 'glb' ? GLTF_AXIS : SOURCE_AXIS)
  ) return null
  const warnings = normalizeWarnings(record.warnings, format)
  if (!warnings) return null
  return Object.freeze({
    preview: Object.freeze({
      exportId: record.exportId,
      projectInstanceId: record.projectInstanceId,
      projectId: record.projectId,
      revision: record.revision,
      sourceFingerprint: record.sourceFingerprint,
      poseGeneration: record.poseGeneration,
      format,
      formatSummary: record.formatSummary,
      suggestedFileName: record.suggestedFileName,
      byteCount: record.byteCount,
      paperThicknessMm: normalizeZero(record.paperThicknessMm),
      faceCount: record.faceCount,
      vertexCount: record.vertexCount,
      triangleCount: record.triangleCount,
      geometryProfile: 'authenticated_mid_surface_triangle_mesh_v1',
      sourceUnit: 'millimeter',
      encodedUnit: format === 'glb' ? 'meter' : 'millimeter',
      sourceAxis: SOURCE_AXIS,
      encodedAxis: format === 'glb' ? GLTF_AXIS : SOURCE_AXIS,
      warnings,
    }),
  })
}

export function normalizeStaticMeshExportSaveResponse(
  value: unknown,
): StaticMeshExportSaveResponse | null {
  const response = exactRecord(value, ['canceled'])
  return response && typeof response.canceled === 'boolean'
    ? Object.freeze({ canceled: response.canceled })
    : null
}

export function formatStaticMeshExportBytes(
  bytes: number,
  locale: Locale,
) {
  if (!isSafeNonNegativeInteger(bytes)) {
    return locale === 'ja' ? '不明' : 'Unknown'
  }
  if (bytes < 1_000) {
    return `${bytes.toLocaleString(locale === 'ja' ? 'ja-JP' : 'en-US')} B`
  }
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}

export function staticMeshExportWarningMessage(
  warning: StaticMeshExportWarning,
  locale: Locale,
) {
  const copy = {
    ja: {
      mid_surface_only:
        '出力は現在姿勢の紙の中央面だけです。紙の表面・裏面を持つ立体ではありません。',
      no_thickness_solid:
        '設定した紙厚は形状へ反映されません。層ずらし、厚み付きソリッド、閉じた多様体は含みません。',
      no_materials_textures_animation:
        '表裏色、材質、テクスチャ、カメラ、折りアニメーションは含みません。',
      no_project_semantics:
        '折り線、山谷、面ID、編集履歴、折り手順などORIGAMI2固有情報は含みません。',
      stl_triangle_soup_facet_normals:
        'STLは頂点indexと頂点法線を保持しません。各三角形が独立したtriangle soupになり、法線は面ごとのfacet normalへ置き換わります。',
      stl_printability_not_guaranteed:
        'STL出力は3Dプリント可能性を保証しません。厚みのない中央面であり、スライサーで別途確認が必要です。',
    },
    en: {
      mid_surface_only:
        'The export contains only the paper mid-surface in the current pose. It is not a solid with front and back surfaces.',
      no_thickness_solid:
        'Configured paper thickness is not applied to geometry. Layer offsets, a thickness solid, and a closed manifold are not included.',
      no_materials_textures_animation:
        'Front/back colors, materials, textures, camera, and folding animation are not included.',
      no_project_semantics:
        'Creases, mountain/valley assignments, face IDs, edit history, folding steps, and other ORIGAMI2 semantics are not included.',
      stl_triangle_soup_facet_normals:
        'STL does not preserve vertex indices or vertex normals. It stores independent triangle soup with one facet normal per triangle.',
      stl_printability_not_guaranteed:
        'STL export does not guarantee 3D printability. This is a zero-thickness mid-surface and must be checked separately in a slicer.',
    },
  } as const
  return copy[locale][warning]
}

function normalizeWarnings(
  value: unknown,
  format: StaticMeshExportFormat,
): readonly StaticMeshExportWarning[] | null {
  const warnings = exactArray(value)
  if (!warnings) return null
  const expected = format === 'stl'
    ? [
        ...BASE_WARNINGS,
        'stl_triangle_soup_facet_normals',
        'stl_printability_not_guaranteed',
      ] as const
    : BASE_WARNINGS
  if (
    warnings.length !== expected.length
    || warnings.some((warning, index) => warning !== expected[index])
  ) return null
  return Object.freeze([...expected])
}

function expectedFormatSummary(format: StaticMeshExportFormat) {
  switch (format) {
    case 'obj':
      return 'Wavefront OBJ・mm・右手系Z-up・静的三角形'
    case 'stl':
      return 'Binary STL・mm・右手系Z-up・静的三角形'
    case 'glb':
      return 'glTF 2.0 GLB・m・右手系Y-up・静的三角形'
  }
}

function isSafeFileName(
  value: unknown,
  format: StaticMeshExportFormat,
): value is string {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > 512
    || value !== value.trim()
    || !value.toLowerCase().endsWith(`.${format}`)
    || !value.slice(0, -(`.${format}`.length)).endsWith('-pose')
    || value.includes('/')
    || value.includes('\\')
    || /[<>:"|?*]/u.test(value)
  ) return false
  for (const character of value) {
    const code = character.codePointAt(0)
    if (code === undefined || code <= 0x1f || (code >= 0x7f && code <= 0x9f)) {
      return false
    }
  }
  const stem = value.slice(0, -(`.${format}`.length))
  if (
    stem === '.'
    || stem === '..'
    || stem.endsWith('.')
    || stem.endsWith(' ')
    || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/iu.test(stem)
  ) return false
  return true
}

function isCanonicalU64(value: string) {
  if (!CANONICAL_U64_PATTERN.test(value)) return false
  try {
    return BigInt(value) <= 18_446_744_073_709_551_615n
  } catch {
    return false
  }
}

function exactRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Record<Keys[number], unknown> | null {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const actual = Reflect.ownKeys(descriptors)
    if (
      actual.length !== keys.length
      || actual.some((key) => typeof key !== 'string' || !(keys as readonly string[]).includes(key))
      || keys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of keys) {
      const descriptor = descriptors[key]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot[key] = descriptor.value
    }
    return snapshot as Record<Keys[number], unknown>
  } catch {
    return null
  }
}

function exactArray(value: unknown): readonly unknown[] | null {
  try {
    if (!Array.isArray(value)) return null
    const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
      Record<PropertyKey, PropertyDescriptor | undefined>
    const ownKeys = Reflect.ownKeys(descriptors)
    const lengthDescriptor = descriptors.length
    const length = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value as unknown
      : null
    if (
      !lengthDescriptor
      || !('value' in lengthDescriptor)
      || typeof length !== 'number'
      || !Number.isSafeInteger(length)
      || length < 0
      || ownKeys.length !== length + 1
    ) return null
    const snapshot: unknown[] = []
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[String(index)]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot.push(descriptor.value)
    }
    if (
      ownKeys.some((key) =>
        key !== 'length'
        && (typeof key !== 'string' || !/^(?:0|[1-9][0-9]*)$/u.test(key)))
    ) return null
    return snapshot
  } catch {
    return null
  }
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function isSafePositiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
