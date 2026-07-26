import { APP_TEXT } from './appText.ts'
import type { LocalizedText } from './i18n.ts'
import type { SnapSettings } from './snap.ts'

export const SNAP_INSPECTOR_OPTIONS: ReadonlyArray<{
  kind: keyof SnapSettings
  label: LocalizedText
}> = [
  { kind: 'grid', label: APP_TEXT.grid },
  { kind: 'vertex', label: APP_TEXT.vertex },
  { kind: 'intersection', label: APP_TEXT.intersection },
  { kind: 'edge', label: APP_TEXT.edge },
  { kind: 'midpoint', label: APP_TEXT.midpoint },
  { kind: 'horizontal', label: APP_TEXT.horizontal },
  { kind: 'vertical', label: APP_TEXT.vertical },
  { kind: 'parallel', label: APP_TEXT.parallel },
  { kind: 'angle', label: APP_TEXT.angle },
]
