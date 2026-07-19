import { useSyncExternalStore } from 'react'

import {
  workspaceLayoutStore,
  type WorkspaceLayoutStore,
} from '../lib/workspaceLayout'

type WorkspaceLayoutControlProps = Readonly<{
  store?: WorkspaceLayoutStore
}>

export function WorkspaceLayoutControl({
  store = workspaceLayoutStore,
}: WorkspaceLayoutControlProps) {
  const layout = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )

  return (
    <details className="workspace-layout-control">
      <summary>レイアウト</summary>
      <div
        className="workspace-layout-menu"
        role="group"
        aria-label="作業レイアウト"
      >
        <button
          type="button"
          onClick={() => store.setPanelOrder(
            layout.panelOrder === 'two_d_first'
              ? 'three_d_first'
              : 'two_d_first',
          )}
        >
          2Dと3Dを入れ替え
        </button>
        <button
          type="button"
          onClick={() => store.setInspectorSide(
            layout.inspectorSide === 'right' ? 'left' : 'right',
          )}
        >
          {layout.inspectorSide === 'right'
            ? 'プロパティを左へ'
            : 'プロパティを右へ'}
        </button>
        <button type="button" onClick={store.reset}>
          初期配置に戻す
        </button>
        <output aria-label="現在の作業レイアウト">
          2D {formatPercent(layout.editorTwoDPercent)}% ·
          プロパティ {layout.inspectorWidthPx}px ·
          手順 {layout.timelineHeightPx}px
        </output>
      </div>
    </details>
  )
}

function formatPercent(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(2)
}
