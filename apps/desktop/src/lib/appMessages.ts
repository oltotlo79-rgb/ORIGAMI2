import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

const APP_CONFIRMATIONS = Object.freeze({
  quitDiscard: {
    ja: '未保存の変更があります。変更を破棄して終了しますか？\nキャンセルすると編集画面に戻ります。',
    en: 'There are unsaved changes. Discard them and quit?\nChoose Cancel to return to the editor.',
  },
  newProject: {
    ja: '未保存の変更があります。保存せずに新しいプロジェクトを作成しますか？',
    en: 'There are unsaved changes. Create a new project without saving?',
  },
  openProject: {
    ja: '未保存の変更があります。保存せずに別のプロジェクトを開きますか？',
    en: 'There are unsaved changes. Open another project without saving?',
  },
  replaceWithFold: {
    ja: '未保存の変更があります。保存せずにFOLD展開図へ置き換えますか？',
    en: 'There are unsaved changes. Replace them with the FOLD crease pattern?',
  },
  replaceWithSvg: {
    ja: '未保存の変更があります。保存せずにSVG展開図へ置き換えますか？',
    en: 'There are unsaved changes. Replace them with the SVG crease pattern?',
  },
} satisfies Readonly<Record<string, LocalizedText>>)

export type AppConfirmation = keyof typeof APP_CONFIRMATIONS

export function appConfirmationText(
  locale: Locale,
  confirmation: AppConfirmation,
) {
  return selectLocalizedText(locale, APP_CONFIRMATIONS[confirmation])
}
