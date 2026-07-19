import { useSyncExternalStore } from 'react'

import {
  workspaceLayoutStore,
  type WorkspaceLayoutStore,
} from '../lib/workspaceLayout'
import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n'

type WorkspaceLayoutControlProps = Readonly<{
  store?: WorkspaceLayoutStore
  localeStore?: LocaleStore
}>

export function WorkspaceLayoutControl({
  store = workspaceLayoutStore,
  localeStore: localeStore_ = localeStore,
}: WorkspaceLayoutControlProps) {
  const locale = useLocale(localeStore_)
  const layout = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)

  return (
    <details className="workspace-layout-control">
      <summary>{text(WORKSPACE_LAYOUT_TEXT.summary)}</summary>
      <div
        className="workspace-layout-menu"
        role="group"
        aria-label={text(WORKSPACE_LAYOUT_TEXT.groupAriaLabel)}
      >
        <button
          type="button"
          onClick={() => store.setPanelOrder(
            layout.panelOrder === 'two_d_first'
              ? 'three_d_first'
              : 'two_d_first',
          )}
        >
          {text(WORKSPACE_LAYOUT_TEXT.swapPanels)}
        </button>
        <button
          type="button"
          onClick={() => store.setInspectorSide(
            layout.inspectorSide === 'right' ? 'left' : 'right',
          )}
        >
          {layout.inspectorSide === 'right'
            ? text(WORKSPACE_LAYOUT_TEXT.movePropertiesLeft)
            : text(WORKSPACE_LAYOUT_TEXT.movePropertiesRight)}
        </button>
        <button type="button" onClick={store.reset}>
          {text(WORKSPACE_LAYOUT_TEXT.reset)}
        </button>
        <output aria-label={text(WORKSPACE_LAYOUT_TEXT.outputAriaLabel)}>
          2D {formatPercent(layout.editorTwoDPercent)}% ·
          {' '}{text(WORKSPACE_LAYOUT_TEXT.properties)} {layout.inspectorWidthPx}px ·
          {' '}{text(WORKSPACE_LAYOUT_TEXT.timeline)} {layout.timelineHeightPx}px
        </output>
      </div>
    </details>
  )
}

function formatPercent(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(2)
}

const WORKSPACE_LAYOUT_TEXT = Object.freeze({
  summary: Object.freeze({ ja: 'レイアウト', en: 'Layout' }),
  groupAriaLabel: Object.freeze({
    ja: '作業レイアウト',
    en: 'Workspace layout',
  }),
  swapPanels: Object.freeze({
    ja: '2Dと3Dを入れ替え',
    en: 'Swap 2D and 3D',
  }),
  movePropertiesLeft: Object.freeze({
    ja: 'プロパティを左へ',
    en: 'Move properties left',
  }),
  movePropertiesRight: Object.freeze({
    ja: 'プロパティを右へ',
    en: 'Move properties right',
  }),
  reset: Object.freeze({ ja: '初期配置に戻す', en: 'Reset layout' }),
  outputAriaLabel: Object.freeze({
    ja: '現在の作業レイアウト',
    en: 'Current workspace layout',
  }),
  properties: Object.freeze({ ja: 'プロパティ', en: 'Properties' }),
  timeline: Object.freeze({ ja: '手順', en: 'Timeline' }),
})
