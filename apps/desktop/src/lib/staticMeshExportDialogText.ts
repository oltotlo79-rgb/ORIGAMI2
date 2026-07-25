const JA_STATIC_MESH_EXPORT_COPY = Object.freeze({
  eyebrow: '現在姿勢の3D書き出し',
  title: '形式と中央面メッシュの制約を確認',
  close: '閉じる',
  description:
    '3Dプレビューに現在表示されている認証済みの完成姿勢を、静的な三角形メッシュとして書き出します。編集履歴や保存状態は変わりません。',
  format: '出力形式',
  optionDetails: Object.freeze({
    obj: 'Blenderなどで扱いやすいテキスト形式・mm・Z-up',
    stl: 'スライサーで広く読めるバイナリ形式・mm・Z-up',
    glb: 'glTF 2.0の単一バイナリ・m・Y-up',
  }),
  generating: '現在姿勢を検証・生成しています…',
  retry: '同じ形式で再試行',
  rebuild: '現在姿勢から作り直す',
  midSurface:
    '重要: 出力は紙の「中央面」だけです。紙厚を持つソリッド、閉じた多様体、3Dプリント可能な模型ではありません。',
  faceSolids:
    '重要: 紙厚を面ごとの閉じた立体として出力します。折り目での和集合や3Dプリント可能性は保証しません。',
  metadata: Object.freeze({
    format: '形式',
    specification: '出力仕様',
    suggestedName: '保存名候補',
    size: 'サイズ',
    geometry: '形状',
    source: '固定元',
    thickness: '設定紙厚',
    units: '単位',
    axes: '座標軸',
  }),
  faces: '面',
  vertices: '頂点',
  triangles: '三角形',
  sourceUnit: '生成元',
  encodedUnit: 'ファイル',
  lossTitle: '出力に含まれない情報・保証されない性質',
  printabilityTitle: 'プリント適性・マニフォールド検査',
  printabilityStatus: Object.freeze({
    manifold_verified: '限定条件内でマニフォールドを確認',
    not_verified: 'マニフォールドを確認できません',
    not_applicable: '対象外（正厚のSTL/GLBのみ）',
  }),
  printabilityChecks: '水密・向き・体積・重複・縮退・交差の保守検査',
  printabilityCounts: '連結成分 / 検査辺 / 検査三角形ペア',
  printabilityDisclaimer:
    '限定的な幾何検査です。最小肉厚、支持材、プリンターやスライサーとの互換性は保証しません。',
  acknowledge: '上記の情報損失と制約を確認しました',
  cancel: 'キャンセル',
  processing: '処理中…',
  save: '保存先を選んで書き出す…',
  formatSummaries: Object.freeze({
    obj: 'Wavefront OBJ・mm・右手系Z-up・静的三角形',
    stl: 'Binary STL・mm・右手系Z-up・静的三角形',
    glb: 'glTF 2.0 GLB・m・右手系Y-up・静的三角形',
  }),
})

const EN_STATIC_MESH_EXPORT_COPY = Object.freeze({
  eyebrow: 'Export current 3D pose',
  title: 'Review format and mid-surface limitations',
  close: 'Close',
  description:
    'Export the authenticated completed pose currently shown in the 3D preview as a static triangle mesh. Project history and save state are unchanged.',
  format: 'Export format',
  optionDetails: Object.freeze({
    obj: 'Text format for Blender and similar tools · mm · Z-up',
    stl: 'Widely supported binary slicer format · mm · Z-up',
    glb: 'Single-file glTF 2.0 binary · m · Y-up',
  }),
  generating: ' current pose is being validated and generated…',
  retry: 'Retry the same format',
  rebuild: 'Rebuild from the current pose',
  midSurface:
    'Important: this exports only the paper mid-surface. It is not a paper-thickness solid, a closed manifold, or a guaranteed printable model.',
  faceSolids:
    'Important: exactly coplanar adjacent faces are welded. A strictly two-triangle, one-hinge pose is also joined only when the native exact thickness corridor issues and revalidates a boundary capability. Other hinge solids remain separate; general unions and 3D printability are not guaranteed.',
  metadata: Object.freeze({
    format: 'Format',
    specification: 'Specification',
    suggestedName: 'Suggested file name',
    size: 'Size',
    geometry: 'Geometry',
    source: 'Source',
    thickness: 'Paper setting',
    units: 'Units',
    axes: 'Axes',
  }),
  faces: 'faces',
  vertices: 'vertices',
  triangles: 'triangles',
  sourceUnit: 'Source',
  encodedUnit: 'File',
  lossTitle: 'Information omitted and properties not guaranteed',
  printabilityTitle: 'Printability and manifold report',
  printabilityStatus: Object.freeze({
    manifold_verified: 'Manifold verified within the bounded checks',
    not_verified: 'Manifold not verified',
    not_applicable: 'Not applicable (positive-thickness STL/GLB only)',
  }),
  printabilityChecks:
    'Watertightness, orientation, volume, duplicates, degeneracy, conservative intersection',
  printabilityCounts: 'components / checked edges / checked triangle pairs',
  printabilityDisclaimer:
    'This limited geometry report does not guarantee wall thickness, supports, or printer/slicer compatibility.',
  acknowledge: 'I understand the information loss and limitations above',
  cancel: 'Cancel',
  processing: 'Processing…',
  save: 'Choose destination and export…',
  formatSummaries: Object.freeze({
    obj: 'Wavefront OBJ · mm · right-handed Z-up · static triangles',
    stl: 'Binary STL · mm · right-handed Z-up · static triangles',
    glb: 'glTF 2.0 GLB · m · right-handed Y-up · static triangles',
  }),
})

export const STATIC_MESH_EXPORT_COPY = Object.freeze({
  ja: JA_STATIC_MESH_EXPORT_COPY,
  en: EN_STATIC_MESH_EXPORT_COPY,
})
