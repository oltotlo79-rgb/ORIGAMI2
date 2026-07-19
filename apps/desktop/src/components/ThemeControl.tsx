import { useSyncExternalStore } from 'react'

import {
  isThemePreference,
  themeStore,
  type ThemeStore,
} from '../lib/theme'

type ThemeControlProps = Readonly<{
  store?: ThemeStore
}>

export function ThemeControl({
  store = themeStore,
}: ThemeControlProps) {
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )

  return (
    <label className="theme-control">
      <span className="theme-control-label">テーマ</span>
      <select
        aria-label="表示テーマ"
        value={snapshot.preference}
        onChange={(event) => {
          const preference = event.currentTarget.value
          if (isThemePreference(preference)) {
            store.setPreference(preference)
          }
        }}
      >
        <option value="system">OS設定に合わせる</option>
        <option value="light">ライト</option>
        <option value="dark">ダーク</option>
      </select>
      <output
        className="theme-effective"
        role="status"
        aria-label="現在の実効テーマ"
        aria-live="polite"
      >
        現在: {snapshot.effectiveTheme === 'dark' ? 'ダーク' : 'ライト'}
      </output>
    </label>
  )
}
