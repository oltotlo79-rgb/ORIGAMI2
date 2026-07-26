import type { FormEventHandler } from 'react'

import { APP_TEXT } from '../lib/appText.ts'
import { rgbaToHex } from '../lib/appElementMetadata.ts'
import type {
  ElementMetadata,
  ElementMetadataTarget,
} from '../lib/coreClient.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'

export type ElementMetadataFormProps = Readonly<{
  locale: Locale
  target: ElementMetadataTarget
  metadata: ElementMetadata | null
  revision: number
  disabled: boolean
  onSubmit: FormEventHandler<HTMLFormElement>
}>

export function ElementMetadataForm({
  locale,
  target,
  metadata,
  revision,
  disabled,
  onSubmit,
}: ElementMetadataFormProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )

  return (
    <form
      key={`${target.kind}:${target.id}:${revision}`}
      className="element-metadata-form"
      onSubmit={onSubmit}
    >
      <label className="field">
        <span>{text(APP_TEXT.name)}</span>
        <input
          name="element_name"
          type="text"
          maxLength={120}
          defaultValue={metadata?.name ?? ''}
          disabled={disabled}
        />
      </label>
      <label className="field">
        <span>{text(APP_TEXT.memo)}</span>
        <textarea
          name="element_memo"
          maxLength={4_000}
          defaultValue={metadata?.memo ?? ''}
          disabled={disabled}
        />
      </label>
      <label className="check">
        <input
          name="element_use_color"
          type="checkbox"
          defaultChecked={Boolean(metadata?.color)}
          disabled={disabled}
        />{' '}
        {text(APP_TEXT.useCustomColor)}
      </label>
      <label className="paper-color-field">
        <span>{text(APP_TEXT.color)}</span>
        <input
          name="element_color"
          type="color"
          defaultValue={rgbaToHex(metadata?.color ?? undefined, '#4b82c3')}
          disabled={disabled}
        />
      </label>
      <button type="submit" disabled={disabled}>
        {text(APP_TEXT.saveElementDetails)}
      </button>
    </form>
  )
}
