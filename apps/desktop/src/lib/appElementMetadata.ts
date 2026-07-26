import type {
  ElementMetadata,
  ElementMetadataTarget,
  ProjectSnapshot,
  RgbaColor,
} from './coreClient'

export function rgbaToCss(color: RgbaColor | undefined) {
  if (!color) return '#fffdf9'
  return `rgba(${color.red}, ${color.green}, ${color.blue}, ${color.alpha / 255})`
}

export function rgbaToHex(color: RgbaColor | undefined, fallback = '#fffdf9') {
  if (!color) return fallback
  const channels = [color.red, color.green, color.blue]
  if (!channels.every(Number.isFinite)) return fallback
  const toHex = (channel: number) => Math.round(Math.min(255, Math.max(0, channel)))
    .toString(16)
    .padStart(2, '0')
  return `#${toHex(color.red)}${toHex(color.green)}${toHex(color.blue)}`
}

export function parseHexColor(value: string): RgbaColor | null {
  if (!/^#[0-9a-f]{6}$/iu.test(value)) return null
  return {
    red: Number.parseInt(value.slice(1, 3), 16),
    green: Number.parseInt(value.slice(3, 5), 16),
    blue: Number.parseInt(value.slice(5, 7), 16),
    alpha: 255,
  }
}

export function findElementMetadata(
  document: ProjectSnapshot['element_metadata'],
  target: ElementMetadataTarget,
): ElementMetadata | null {
  if (target.kind === 'vertex') {
    return document.vertices.find((record) => record.vertex === target.id)?.metadata ?? null
  }
  if (target.kind === 'edge') {
    return document.edges.find((record) => record.edge === target.id)?.metadata ?? null
  }
  return document.faces.find((record) => record.face === target.id)?.metadata ?? null
}

export function hasControlCharacter(value: string) {
  return [...value].some((character) => {
    const codePoint = character.codePointAt(0) ?? 0
    return codePoint <= 31 || (codePoint >= 127 && codePoint <= 159)
  })
}
