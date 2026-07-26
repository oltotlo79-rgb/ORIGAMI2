import type { LocalizedText } from './i18n.ts'
import type { StaticMeshExportWarning } from './staticMeshExport.ts'

type StaticMeshExportPresentationTextKey =
  | 'unknownByteCount'
  | 'numberLocale'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const STATIC_MESH_EXPORT_PRESENTATION_TEXT: Readonly<
  Record<StaticMeshExportPresentationTextKey, LocalizedText>
> = Object.freeze({
  unknownByteCount: localized('不明', 'Unknown'),
  numberLocale: localized('ja-JP', 'en-US'),
})

export const STATIC_MESH_EXPORT_WARNING_TEXT: Readonly<
  Record<StaticMeshExportWarning, LocalizedText>
> = Object.freeze({
  mid_surface_only: localized(
    '出力は現在姿勢の紙の中央面だけです。紙の表面・裏面を持つ立体ではありません。',
    'The export contains only the paper mid-surface in the current pose. It is not a solid with front and back surfaces.',
  ),
  no_thickness_solid: localized(
    '設定した紙厚は形状へ反映されません。層ずらし、厚み付きソリッド、閉じた多様体は含みません。',
    'Configured paper thickness is not applied to geometry. Layer offsets, a thickness solid, and a closed manifold are not included.',
  ),
  independent_face_solids: localized(
    '紙厚は面ごとの閉じた立体として出力します。折り目で隣接する立体の和集合や隙間・重なりの除去は保証しません。',
    'Paper thickness is exported as one closed solid per face. The solids are not unioned, and hinge gaps or overlaps are not removed.',
  ),
  no_textures_animation: localized(
    'GLBには紙色を含め、紙厚付き形状では表裏色を分けます。テクスチャ、カメラ、折りアニメーションは含みません。',
    'GLB includes paper colors and distinguishes front and back on thickness geometry. Textures, camera, and folding animation are not included.',
  ),
  no_project_semantics: localized(
    '折り線、山谷、面ID、編集履歴、折り手順などORIGAMI2固有情報は含みません。',
    'Creases, mountain/valley assignments, face IDs, edit history, folding steps, and other ORIGAMI2 semantics are not included.',
  ),
  stl_triangle_soup_facet_normals: localized(
    'STLは頂点indexと頂点法線を保持しません。各三角形が独立したtriangle soupになり、法線は面ごとのfacet normalへ置き換わります。',
    'STL does not preserve vertex indices or vertex normals. It stores independent triangle soup with one facet normal per triangle.',
  ),
  stl_printability_not_guaranteed: localized(
    'STL出力は3Dプリント可能性を保証しません。面ごとの立体は折り目で重なりや隙間が生じるため、スライサーで確認してください。',
    'STL export does not guarantee 3D printability. Per-face solids may overlap or leave hinge gaps and must be checked in a slicer.',
  ),
})
