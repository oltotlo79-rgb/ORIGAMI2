const JA_DIAGNOSTICS_COPY = Object.freeze({
  eyebrow: '問題報告の準備',
  title: '診断情報を確認',
  close: '閉じる',
  disclosure: Object.freeze([
    '作品名、作品形状、ファイル内容、ローカルパス、ID、座標、時刻、アプリ版、OS、CPU、GPU情報は含みません。',
    'この情報は自動送信されません。下に表示されたJSONと保存されるJSONは同一です。',
    '保存後、内容を確認したうえで利用者自身がGitHub Issuesへ添付してください。',
  ]),
  loading: '診断情報を準備しています…',
  loadError:
    '診断情報を準備できませんでした。アプリを再起動して、もう一度お試しください。',
  retry: '再試行',
  jsonLabel: '共有前に確認する診断JSON',
  proofScopeLabel: '証明範囲JSON（手動コピー専用）',
  proofScopeDisclosure:
    '全体判定certificateと局所summaryのmodel・version・件数・理由だけを含みます。座標、作品ID、UUID、時刻は含みません。',
  selectProofScope: '証明範囲JSONをすべて選択',
  selectAll: '内容をすべて選択',
  saving: '保存中…',
  save: 'JSONファイルとして保存…',
  notices: Object.freeze({
    selected: '内容をすべて選択しました。Ctrl/Cmd+Cでコピーできます。',
    save_canceled: '保存をキャンセルしました。',
    saved: '診断JSONを保存しました。',
    save_failed:
      '診断JSONを保存できませんでした。保存先を確認して、もう一度お試しください。',
  }),
})

const EN_DIAGNOSTICS_COPY = Object.freeze({
  eyebrow: 'Prepare a problem report',
  title: 'Review diagnostics',
  close: 'Close',
  disclosure: Object.freeze([
    'The report does not include the work name, work geometry, file contents, local paths, IDs, coordinates, timestamps, app version, OS, CPU, or GPU information.',
    'This information is never sent automatically. The JSON shown below is identical to the JSON that will be saved.',
    'After saving and reviewing it, attach the file to GitHub Issues yourself.',
  ]),
  loading: 'Preparing diagnostics…',
  loadError:
    'Diagnostics could not be prepared. Restart the app and try again.',
  retry: 'Retry',
  jsonLabel: 'Diagnostics JSON to review before sharing',
  proofScopeLabel: 'Proof coverage JSON (manual copy only)',
  proofScopeDisclosure:
    'Contains only certificate models, versions, counts, and allowlisted reasons. It excludes coordinates, project IDs, UUIDs, and timestamps.',
  selectProofScope: 'Select all proof coverage JSON',
  selectAll: 'Select all contents',
  saving: 'Saving…',
  save: 'Save as JSON file…',
  notices: Object.freeze({
    selected: 'All contents are selected. Press Ctrl/Cmd+C to copy.',
    save_canceled: 'Save was canceled.',
    saved: 'Diagnostics JSON was saved.',
    save_failed:
      'Diagnostics JSON could not be saved. Check the destination and try again.',
  }),
})

export const DIAGNOSTICS_COPY = Object.freeze({
  ja: JA_DIAGNOSTICS_COPY,
  en: EN_DIAGNOSTICS_COPY,
})
