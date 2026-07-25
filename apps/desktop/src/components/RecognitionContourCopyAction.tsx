import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  RECOGNITION_CONTOUR_COPY_ACTION_TEXT as TEXT,
} from '../lib/recognitionContourCopyActionText.ts'

export function RecognitionContourCopyAction({ locale, bodyPointCount, localContourCount, onCopy }: {
  locale: Locale; bodyPointCount: number; localContourCount: number; onCopy: () => void
}) {
  if (bodyPointCount === 0 && localContourCount === 0) return null
  return <div>
    <p>{formatLocalizedText(locale, TEXT.summary, {
      bodyPointCount,
      localContourCount,
    })}</p>
    <button type="button" onClick={() => {
      if (window.confirm(selectLocalizedText(locale, TEXT.confirmation))) onCopy()
    }}>{selectLocalizedText(locale, TEXT.copy)}</button>
  </div>
}
