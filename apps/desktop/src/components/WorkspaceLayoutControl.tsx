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
import { WORKSPACE_LAYOUT_CONTROL_TEXT } from '../lib/workspaceLayoutControlText.ts'

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
      <summary>{text(WORKSPACE_LAYOUT_CONTROL_TEXT.summary)}</summary>
      <div
        className="workspace-layout-menu"
        role="group"
        aria-label={text(WORKSPACE_LAYOUT_CONTROL_TEXT.groupAriaLabel)}
      >
        <button
          type="button"
          onClick={() => store.setPanelOrder(
            layout.panelOrder === 'two_d_first'
              ? 'three_d_first'
              : 'two_d_first',
          )}
        >
          {text(WORKSPACE_LAYOUT_CONTROL_TEXT.swapPanels)}
        </button>
        <button
          type="button"
          onClick={() => store.setInspectorSide(
            layout.inspectorSide === 'right' ? 'left' : 'right',
          )}
        >
          {layout.inspectorSide === 'right'
            ? text(WORKSPACE_LAYOUT_CONTROL_TEXT.movePropertiesLeft)
            : text(WORKSPACE_LAYOUT_CONTROL_TEXT.movePropertiesRight)}
        </button>
        <button type="button" onClick={store.reset}>
          {text(WORKSPACE_LAYOUT_CONTROL_TEXT.reset)}
        </button>
        <output aria-label={text(WORKSPACE_LAYOUT_CONTROL_TEXT.outputAriaLabel)}>
          2D {formatPercent(layout.editorTwoDPercent)}% ·
          {' '}{text(WORKSPACE_LAYOUT_CONTROL_TEXT.properties)} {layout.inspectorWidthPx}px ·
          {' '}{text(WORKSPACE_LAYOUT_CONTROL_TEXT.timeline)} {layout.timelineHeightPx}px
        </output>
      </div>
    </details>
  )
}

function formatPercent(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(2)
}
