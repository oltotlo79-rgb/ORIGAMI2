export function RecognitionContourCopyAction({ locale, bodyPointCount, localContourCount, onCopy }: {
  locale: 'ja' | 'en'; bodyPointCount: number; localContourCount: number; onCopy: () => void
}) {
  if (bodyPointCount === 0 && localContourCount === 0) return null
  return <div>
    <p>{locale === 'ja'
      ? `編集可能な胴体輪郭 ${bodyPointCount} 点・局所輪郭 ${localContourCount} 件`
      : `Editable body contour: ${bodyPointCount} points; local contours: ${localContourCount}`}</p>
    <button type="button" onClick={() => {
      if (window.confirm(locale === 'ja'
        ? '認識候補の輪郭を編集欄へコピーしますか？保存するまでprojectは変更されません。'
        : 'Copy the proposed contours into the editor? The project stays unchanged until saved.')) onCopy()
    }}>{locale === 'ja' ? '確認して輪郭を編集欄へコピー' : 'Review and copy contours to editor'}</button>
  </div>
}
