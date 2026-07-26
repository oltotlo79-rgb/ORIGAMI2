import { APP_TEXT } from '../lib/appText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'

type EditorState = ReturnType<typeof useBeginnerEditorState>

export function BeginnerSkeletonEditor({
  locale,
  coreBusy,
  recoveryBlocking,
  editor,
}: Readonly<{
  locale: Locale
  coreBusy: boolean
  recoveryBlocking: boolean
  editor: EditorState
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerSkeletonSegments,
    setBeginnerSkeletonSegments,
    beginnerSkeletonTree,
    addBeginnerSkeletonSegment,
  } = editor

  return (
    <>
      <fieldset aria-describedby="beginner-skeleton-help">
        <legend>{text(APP_TEXT.stickSkeleton)}</legend>
        {([
          ['skeleton_start_x_mm', APP_TEXT.startXMm, 0, -10000, 10000],
          ['skeleton_start_y_mm', APP_TEXT.startYMm, 0, -10000, 10000],
          ['skeleton_length_mm', APP_TEXT.lengthMm, 10, 0.1, 10000],
          ['skeleton_angle_degrees', APP_TEXT.angleDegrees, 0, -360, 360],
          ['skeleton_thickness_mm', APP_TEXT.thicknessMm, 1, 0.1, 1000],
        ] as const).map(([name, label, initial, min, max]) => (
          <label className="field" key={name}>
            <span>{text(label)}</span>
            <input
              name={name}
              type="number"
              min={min}
              max={max}
              step={0.1}
              defaultValue={initial}
              required={name !== 'skeleton_start_x_mm'
                && name !== 'skeleton_start_y_mm'}
            />
          </label>
        ))}
        <button
          type="button"
          disabled={
            beginnerSkeletonSegments.length >= 64
            || coreBusy
            || recoveryBlocking
          }
          onClick={(event) => {
            if (event.currentTarget.form) {
              addBeginnerSkeletonSegment(event.currentTarget.form)
            }
          }}
        >
          {text(APP_TEXT.addSkeletonBar)}
        </button>
        <svg
          viewBox="-110 -110 220 220"
          role="img"
          aria-label={text(APP_TEXT.stickSkeletonPreview)}
        >
          {beginnerSkeletonSegments.map((segment) => (
            <line
              key={segment.id}
              x1={segment.start.x_tenths_mm / 10}
              y1={segment.start.y_tenths_mm / 10}
              x2={segment.end.x_tenths_mm / 10}
              y2={segment.end.y_tenths_mm / 10}
              stroke="currentColor"
              strokeWidth={Math.max(
                0.5,
                segment.thickness_tenths_mm / 10,
              )}
            />
          ))}
        </svg>
        <ul aria-label={text(APP_TEXT.skeletonBarList)}>
          {beginnerSkeletonSegments.map((segment) => (
            <li key={segment.id}>
              #{segment.id}: {formattedText(APP_TEXT.thicknessThicknessMm, {
                thickness: segment.thickness_tenths_mm / 10,
              })}
              {([
                ['start.x_tenths_mm', APP_TEXT.startX,
                  segment.start.x_tenths_mm],
                ['start.y_tenths_mm', APP_TEXT.startY,
                  segment.start.y_tenths_mm],
                ['end.x_tenths_mm', APP_TEXT.endX,
                  segment.end.x_tenths_mm],
                ['end.y_tenths_mm', APP_TEXT.endY,
                  segment.end.y_tenths_mm],
                ['thickness_tenths_mm', APP_TEXT.thickness,
                  segment.thickness_tenths_mm],
              ] as const).map(([field, labelText, tenths]) => {
                const label = text(labelText)
                return (
                  <label key={field}>
                    <span>{label} (mm)</span>
                    <input
                      type="number"
                      step="0.1"
                      defaultValue={tenths / 10}
                      min={field === 'thickness_tenths_mm' ? 0.1 : -10000}
                      max={field === 'thickness_tenths_mm' ? 1000 : 10000}
                      aria-label={formattedText(
                        APP_TEXT.skeletonBarSegmentIdLabelMm,
                        { segmentId: segment.id, label },
                      )}
                      onBlur={(event) => {
                        const next = Math.round(
                          Number(event.currentTarget.value) * 10,
                        )
                        const valid = Number.isSafeInteger(next)
                          && (field === 'thickness_tenths_mm'
                            ? next >= 1 && next <= 10_000
                            : Math.abs(next) <= 100_000)
                        if (!valid) {
                          event.currentTarget.value = String(tenths / 10)
                          return
                        }
                        setBeginnerSkeletonSegments((segments) =>
                          segments.map((item) => {
                            if (item.id !== segment.id) return item
                            if (field === 'thickness_tenths_mm') {
                              return {
                                ...item,
                                thickness_tenths_mm: next,
                              }
                            }
                            const [endpoint, axis] = field.split('.') as [
                              'start' | 'end',
                              'x_tenths_mm' | 'y_tenths_mm',
                            ]
                            const changed = {
                              ...item,
                              [endpoint]: {
                                ...item[endpoint],
                                [axis]: next,
                              },
                            }
                            return changed.start.x_tenths_mm
                                === changed.end.x_tenths_mm
                              && changed.start.y_tenths_mm
                                === changed.end.y_tenths_mm
                              ? item
                              : changed
                          }))
                      }}
                    />
                  </label>
                )
              })}
              <button
                type="button"
                onClick={() => setBeginnerSkeletonSegments(
                  (segments) => segments.filter(
                    (item) => item.id !== segment.id,
                  ),
                )}
              >
                {text(APP_TEXT.remove)}
              </button>
            </li>
          ))}
        </ul>
      </fieldset>
      <p id="beginner-skeleton-help" className="muted">
        {text(APP_TEXT.upTo64BarsAreStoredAt01Mm)}
      </p>
      <p role="status">
        {beginnerSkeletonTree.status === 'tree'
          ? formattedText(
              APP_TEXT.skeletonTreeConfirmedPointsJointsAndEdgesBranchesCandidateGeneration,
              {
                points: beginnerSkeletonTree.pointCount,
                edges: beginnerSkeletonTree.edgeCount,
              },
            )
          : formattedText(
              APP_TEXT.skeletonTreeUnconfirmedReasonCyclesDuplicateEdgesAndDisconnectedSkeleton,
              { reason: beginnerSkeletonTree.status },
            )}
      </p>
    </>
  )
}
