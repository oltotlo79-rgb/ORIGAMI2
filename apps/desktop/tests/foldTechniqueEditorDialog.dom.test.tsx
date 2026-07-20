import { StrictMode } from 'react'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  FoldTechniqueEditorDialog,
  type FoldTechniqueEditorDialogProps,
} from '../src/components/FoldTechniqueEditorDialog.tsx'
import {
  createInitialFoldTechniqueDocumentV1,
  type FoldTechniqueFileDocumentV1,
} from '../src/lib/foldTechniqueEditor.ts'
import { localeStore } from '../src/lib/i18n.ts'

afterEach(() => {
  cleanup()
  localeStore.setLocale('ja')
  localeStore.dispose()
  document.body.replaceChildren()
})

describe('FoldTechniqueEditorDialog', () => {
  it('starts unchanged, validates edits, and returns a frozen V1 document', () => {
    const onConfirm = vi.fn()
    renderDialog({ mode: 'edit', onConfirm })
    const confirm = screen.getByRole('button', { name: '変更を確定' })
    expect((confirm as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByRole('status').textContent).toContain('変更はありません')

    fireEvent.change(screen.getByLabelText('技法名（日本語）'), {
      target: { value: '花の中割り' },
    })
    expect((confirm as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(confirm)
    expect(onConfirm).toHaveBeenCalledTimes(1)
    const document = onConfirm.mock.calls[0]?.[0] as FoldTechniqueFileDocumentV1
    expect(document.schema).toBe('origami2_fold_technique_file')
    expect(document.techniques[0]?.names).toContainEqual({
      locale: 'ja',
      text: '花の中割り',
    })
    expect(Object.isFrozen(document)).toBe(true)
    expect(Object.isFrozen(document.techniques[0]?.operations)).toBe(true)
  })

  it('can create the strict initial template without a synthetic edit', () => {
    const onConfirm = vi.fn()
    renderDialog({ onConfirm })
    const confirm = screen.getByRole('button', { name: '技法を作成' })
    expect((confirm as HTMLButtonElement).disabled).toBe(false)
    expect(screen.queryByText('変更はありません。')).toBeNull()
    fireEvent.click(confirm)
    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(onConfirm.mock.calls[0]?.[0]).toEqual(
      createInitialFoldTechniqueDocumentV1(),
    )
  })

  it('records unsupported physical metadata and never offers execution', () => {
    const onConfirm = vi.fn()
    renderDialog({ mode: 'edit', onConfirm })
    const actions = screen.getAllByLabelText('動作区分')
    fireEvent.change(actions[0] as HTMLSelectElement, {
      target: { value: 'inside_reverse_fold' },
    })
    expect(
      screen.getByText(/未対応物理操作として保存します/u),
    ).toBeTruthy()
    expect(document.body.textContent).not.toMatch(/自動実行する|今すぐ実行/u)

    fireEvent.click(screen.getByRole('button', { name: '変更を確定' }))
    const confirmedDocument =
      onConfirm.mock.calls[0]?.[0] as FoldTechniqueFileDocumentV1
    const operation = confirmedDocument.techniques[0]?.operations[0]
    expect(operation?.action).toEqual({ kind: 'inside_reverse_fold' })
    expect(operation?.required_capabilities).toEqual([
      'inside_reverse_fold_motion_v1',
    ])
    expect(operation?.execution_support).toEqual({
      status: 'unsupported_physical_operation',
      operation: 'inside_reverse_fold_motion_v1',
    })
  })

  it('preserves V1 metadata outside the initial form while editing a name', () => {
    const initial = advancedDocument()
    const onConfirm = vi.fn()
    renderDialog({
      mode: 'edit',
      initialDocument: initial,
      onConfirm,
    })
    fireEvent.change(screen.getByLabelText('技法名（日本語）'), {
      target: { value: '対象外metadata保持' },
    })
    fireEvent.click(screen.getByRole('button', { name: '変更を確定' }))
    const confirmed =
      onConfirm.mock.calls[0]?.[0] as FoldTechniqueFileDocumentV1
    const before = initial.techniques[0]
    const after = confirmed.techniques[0]
    expect(JSON.stringify(after?.parameters)).toBe(
      JSON.stringify(before.parameters),
    )
    expect(JSON.stringify(after?.preconditions)).toBe(
      JSON.stringify(before.preconditions),
    )
    expect(JSON.stringify(after?.operations[0]?.parameter_bindings)).toBe(
      JSON.stringify(before.operations[0].parameter_bindings),
    )
    expect(after?.operations[0]?.required_capabilities).toEqual([
      'human_interpretation_v1',
      'instruction_timeline_v1',
    ])
  })

  it('keeps save disabled for invalid or reverted edits', () => {
    renderDialog({ mode: 'edit' })
    const confirm = screen.getByRole('button', { name: '変更を確定' })
    const packageId = screen.getByLabelText('パッケージID')
    fireEvent.change(packageId, { target: { value: '../execute' } })
    expect(screen.getByRole('alert').textContent).toContain(
      'ID、文字、locale、数値範囲',
    )
    expect(screen.getByRole('alert').textContent).not.toContain('../execute')
    expect((confirm as HTMLButtonElement).disabled).toBe(true)

    fireEvent.change(packageId, { target: { value: 'user.local.techniques' } })
    expect(screen.queryByRole('alert')).toBeNull()
    expect((confirm as HTMLButtonElement).disabled).toBe(true)
  })

  it('adds, reorders, and removes steps without crossing the two-step floor', () => {
    renderDialog()
    const firstId = screen.getAllByLabelText('手順ID')[0] as HTMLInputElement
    firstId.focus()
    fireEvent.change(firstId, { target: { value: 'prepare-paper' } })
    expect(document.activeElement).toBe(firstId)

    const initialRemoveButtons = screen.getAllByRole('button', {
      name: /この手順を削除/u,
    })
    expect(initialRemoveButtons).toHaveLength(2)
    expect(initialRemoveButtons.every(
      (button) => (button as HTMLButtonElement).disabled,
    )).toBe(true)

    fireEvent.click(screen.getByRole('button', { name: '説明手順を追加' }))
    expect(screen.getAllByLabelText('手順ID')).toHaveLength(3)
    fireEvent.click(screen.getByRole('button', { name: '上へ移動 3' }))
    const ids = screen.getAllByLabelText('手順ID') as HTMLInputElement[]
    expect(ids.map(({ value }) => value)).toEqual([
      'prepare-paper',
      'step-3',
      'step-2',
    ])
    fireEvent.click(screen.getByRole('button', { name: 'この手順を削除 2' }))
    expect(screen.getAllByLabelText('手順ID')).toHaveLength(2)
  })

  it('renders English copy and exposes source citation as inert text', () => {
    localeStore.setLocale('en')
    renderDialog({ mode: 'edit' })
    expect(screen.getByRole('heading', {
      name: 'Edit the instruction template',
    })).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Source provenance'), {
      target: { value: 'published_reference' },
    })
    expect(
      screen.getByLabelText('Citation text (inert plain text; never fetched)'),
    ).toBeTruthy()
    expect(document.body.textContent).toContain(
      'stored only as descriptive metadata',
    )
  })

  it('selects and edits every technique in a multi-technique package', () => {
    localeStore.setLocale('en')
    const initial = clone(createInitialFoldTechniqueDocumentV1())
    const second = clone(initial.techniques[0])
    second.id = 'user.second-technique'
    second.names = [
      { locale: 'ja', text: '二つ目の折り技法' },
      { locale: 'en', text: 'Second folding technique' },
    ]
    initial.techniques.push(second)
    const onConfirm = vi.fn()
    renderDialog({ mode: 'edit', initialDocument: initial, onConfirm })

    fireEvent.change(screen.getByLabelText('Technique name (English)'), {
      target: { value: 'First edited technique' },
    })
    fireEvent.change(screen.getByLabelText('Technique to edit'), {
      target: { value: '1' },
    })
    expect(
      (screen.getByLabelText('Technique name (English)') as HTMLInputElement)
        .value,
    ).toBe('Second folding technique')
    fireEvent.change(screen.getByLabelText('Technique name (English)'), {
      target: { value: 'Second edited technique' },
    })
    fireEvent.change(screen.getByLabelText('Technique to edit'), {
      target: { value: '0' },
    })
    expect(
      (screen.getByLabelText('Technique name (English)') as HTMLInputElement)
        .value,
    ).toBe('First edited technique')

    fireEvent.click(screen.getByRole('button', { name: 'Apply changes' }))
    const confirmed =
      onConfirm.mock.calls[0]?.[0] as FoldTechniqueFileDocumentV1
    expect(confirmed.techniques).toHaveLength(2)
    expect(confirmed.techniques[0]?.names).toContainEqual({
      locale: 'en',
      text: 'First edited technique',
    })
    expect(confirmed.techniques[1]?.names).toContainEqual({
      locale: 'en',
      text: 'Second edited technique',
    })
  })

  it('rejects hostile initial values without invoking accessors', () => {
    const hostile = clone(createInitialFoldTechniqueDocumentV1())
    let calls = 0
    Object.defineProperty(hostile, 'metadata', {
      enumerable: true,
      get() {
        calls += 1
        return {}
      },
    })
    renderDialog({ initialDocument: hostile })
    expect(screen.getByRole('alert').textContent).toContain(
      '厳密なV1契約を満たしていない',
    )
    expect(calls).toBe(0)
    const confirm = screen.queryByRole(
      'button',
      { name: '技法を作成' },
    ) as HTMLButtonElement | null
    expect(confirm?.disabled).toBe(true)
  })

  it('traps focus, handles Escape once in StrictMode, and restores focus', async () => {
    const outside = document.createElement('button')
    outside.textContent = 'outside'
    document.body.append(outside)
    outside.focus()
    const onCancel = vi.fn()
    const rendered = render(
      <StrictMode>
        {dialog({ onCancel, mode: 'edit' })}
      </StrictMode>,
    )
    await waitFor(() => {
      expect(document.activeElement).toBe(
        screen.getByLabelText('パッケージID'),
      )
    })

    const close = screen.getByRole('button', { name: '閉じる' })
    const cancel = screen.getByRole('button', { name: 'キャンセル' })
    cancel.focus()
    fireEvent.keyDown(cancel, { key: 'Tab' })
    expect(document.activeElement).toBe(close)
    close.focus()
    fireEvent.keyDown(close, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(cancel)

    fireEvent.keyDown(document, {
      key: 'Escape',
      isComposing: true,
    })
    expect(onCancel).not.toHaveBeenCalled()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
    rendered.unmount()
    expect(document.activeElement).toBe(outside)
  })

  it('blocks keyboard cancellation and confirmation while busy', () => {
    const onCancel = vi.fn()
    const changed = clone(createInitialFoldTechniqueDocumentV1())
    changed.package_id = 'user.changed.techniques'
    renderDialog({
      initialDocument: changed,
      busy: true,
      onCancel,
    })
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).not.toHaveBeenCalled()
    expect((
      screen.getByRole('button', { name: '処理中…' }) as HTMLButtonElement
    ).disabled).toBe(true)
  })
})

function renderDialog(overrides: Partial<FoldTechniqueEditorDialogProps> = {}) {
  return render(dialog(overrides))
}

function dialog(overrides: Partial<FoldTechniqueEditorDialogProps> = {}) {
  return (
    <FoldTechniqueEditorDialog
      mode={overrides.mode ?? 'create'}
      initialDocument={
        overrides.initialDocument ?? createInitialFoldTechniqueDocumentV1()
      }
      techniqueIndex={overrides.techniqueIndex}
      busy={overrides.busy}
      saveFailed={overrides.saveFailed}
      onConfirm={overrides.onConfirm ?? vi.fn()}
      onCancel={overrides.onCancel ?? vi.fn()}
    />
  )
}

function advancedDocument() {
  const document = clone(createInitialFoldTechniqueDocumentV1())
  const technique = document.techniques[0]
  technique.parameters = [
    {
      id: 'confirmed',
      names: [
        { locale: 'en', text: 'Confirmed' },
        { locale: 'ja', text: '確認済み' },
      ],
      descriptions: [
        { locale: 'en', text: 'Whether preparation was confirmed.' },
        { locale: 'ja', text: '準備を確認したか。' },
      ],
      parameter_type: { type: 'boolean', default: false },
    },
  ]
  technique.preconditions = [
    {
      id: 'not-confirmed',
      condition: {
        kind: 'parameter_comparison',
        parameter_id: 'confirmed',
        comparison: 'equal',
        value: { type: 'boolean', value: false },
      },
    },
  ]
  technique.operations[0].parameter_bindings = [
    { role: 'confirmation', parameter_id: 'confirmed' },
  ]
  technique.operations[0].precondition_ids = ['not-confirmed']
  technique.operations[0].required_capabilities = [
    'instruction_timeline_v1',
    'human_interpretation_v1',
  ]
  return document
}

type Mutable<Value> =
  Value extends readonly (infer Item)[]
    ? Mutable<Item>[]
    : Value extends object
      ? { -readonly [Key in keyof Value]: Mutable<Value[Key]> }
      : Value

function clone<Value>(value: Value): Mutable<Value> {
  return JSON.parse(JSON.stringify(value)) as Mutable<Value>
}
