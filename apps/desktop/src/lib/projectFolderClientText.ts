import type { LocalizedText } from './i18n.ts'
import type { ProjectFolderClientErrorCode } from './projectFolderClient.ts'

export type ProjectFolderClientText = Readonly<
  Record<ProjectFolderClientErrorCode, LocalizedText>
>

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const PROJECT_FOLDER_CLIENT_TEXT = Object.freeze({
  native_unavailable: localized(
    '展開フォルダー操作はデスクトップ版で利用できます。',
    'Expanded-folder operations are available in the desktop app.',
  ),
  busy: localized(
    '別の展開フォルダー操作を処理中です。完了後にもう一度実行してください。',
    'Another expanded-folder operation is running. Try again after it finishes.',
  ),
  invalid_request: localized(
    '展開フォルダー操作の条件を確認できませんでした。もう一度実行してください。',
    'The expanded-folder request could not be verified. Try again.',
  ),
  open_failed: localized(
    '選択した展開フォルダーを安全に開けませんでした。アクセス権を確認してください。',
    'The selected expanded folder could not be opened safely. Check its permissions.',
  ),
  invalid: localized(
    '展開フォルダーのmanifestまたはプロジェクト内容が正しくありません。',
    'The expanded folder has an invalid manifest or project content.',
  ),
  too_large: localized(
    '展開フォルダー内のファイルがサイズ上限を超えています。',
    'A file in the expanded folder exceeds the size limit.',
  ),
  link_or_special_entry: localized(
    '展開フォルダーにリンク、再解析ポイント、ハードリンク、または特殊ファイルが含まれています。通常のファイルだけにしてください。',
    'The expanded folder contains a link, reparse point, hard link, or special file. Use ordinary files only.',
  ),
  changed_during_read: localized(
    '処理中に展開フォルダーが変更されました。変更が止まってからもう一度実行してください。',
    'The expanded folder changed during processing. Try again after changes stop.',
  ),
  save_failed: localized(
    '展開フォルダーを安全に保存できませんでした。保存先のアクセス権と空き容量を確認してください。',
    'The expanded folder could not be saved safely. Check destination permissions and free space.',
  ),
  target_exists: localized(
    '同じ名前の展開フォルダーは別のプロジェクトに属するか、安全な置き換え条件を満たしていません。別の親フォルダーを選んでください。',
    'The same-named expanded folder belongs to another project or cannot be replaced safely. Choose a different parent folder.',
  ),
  project_changed: localized(
    '操作中にプロジェクトが変更されました。現在の内容でもう一度実行してください。',
    'The project changed during the operation. Try again with the current content.',
  ),
  recovery_required: localized(
    '前回の展開フォルダー置き換えを安全に完了する必要があります。保存先が外付けドライブ等にある場合は再接続してから、展開フォルダー操作をもう一度実行してください。',
    'A previous expanded-folder replacement must be recovered safely. If its destination is on an external drive, reconnect it and retry an expanded-folder operation.',
  ),
  replacement_unsupported: localized(
    'この保存先では既存フォルダーの安全な置き換えを保証できません。新しいフォルダー名で保存するか、ローカルのNTFS/ReFS保存先を選んでください。',
    'Safe replacement of an existing folder cannot be guaranteed at this destination. Save with a new folder name or choose a local NTFS/ReFS destination.',
  ),
  invalid_response: localized(
    '展開フォルダー操作の応答を確認できませんでした。もう一度実行してください。',
    'The expanded-folder response could not be verified. Try again.',
  ),
}) satisfies ProjectFolderClientText
