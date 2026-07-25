export const MESH_ANIMATION_EXPORT_DIALOG_TEXT = Object.freeze({
  ja: Object.freeze({
    title: '手順アニメーションを書き出す',
    description: '認証済みの手順タイムラインを glTF 2.0 の GLB アニメーションとして保存します。',
    warning: '重要: キーフレーム間は線形補間です。紙厚、テクスチャ、衝突保証、編集可能な手順情報は含まれません。',
    acknowledge: '制限と情報損失を確認しました',
    frames: 'フレーム', duration: '再生時間', geometry: '形状', size: 'サイズ',
    name: '保存名', retry: '現在の手順から再作成', cancel: 'キャンセル',
    save: '保存先を選ぶ', processing: '処理中…',
  }),
  en: Object.freeze({
    title: 'Export instruction animation',
    description: 'Save the authenticated instruction timeline as a glTF 2.0 GLB animation.',
    warning: 'Important: keyframes use linear interpolation. Paper thickness, textures, collision guarantees, and editable instruction semantics are not included.',
    acknowledge: 'I understand the limitations and information loss',
    frames: 'Frames', duration: 'Duration', geometry: 'Geometry', size: 'Size',
    name: 'Suggested name', retry: 'Rebuild from current instructions',
    cancel: 'Cancel', save: 'Choose destination', processing: 'Processing…',
  }),
})
