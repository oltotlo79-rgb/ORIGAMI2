# Windows正式版 GitHub Releases 配布契約

状態: 初版実装済み。対象はWindows 10/11 x64のNSISインストーラーだけとし、
macOSは従来どおり自動ビルド・テスト・`.app`生成のCI検証だけを維持する。

## 1. 目的と費用

`.github/workflows/release-windows.yml`は、リポジトリのソース、GitHub Actions、
GitHub Releases、リポジトリごとの`GITHUB_TOKEN`だけで正式配布物を作る。
外部サーバー、更新配信サービス、署名用secret、常駐処理を必要としない。
GitHub Actionsの利用可能時間・保存量はリポジトリの公開範囲とGitHubの利用条件に
従うが、ORIGAMI2固有の継続課金サービスは導入しない。

## 2. 正式成果物

1回のReleaseへ公開するファイルは次の2個に限定する。

- `ORIGAMI2-vMAJOR.MINOR.PATCH-windows-x64-unsigned-setup.exe`
- `SHA256SUMS.txt`

ビルドjobから公開jobへ渡す非公開の一時artifactには、上記2個に加えて固定の
`release-notes.md`だけを含める。macOS `.app`、開発用binary、任意のglobに一致した
追加ファイルは公開しない。NSIS内の日本語font・ライセンス・アプリ本体は、既存の
`scripts/verify_windows_bundle.ps1`で公開候補ごとに再検査する。

## 3. 起動条件

### 3.1 タグpush

`v*`タグのpushを入口とするが、workflow内のrelease gateで次をすべて満たさなければ
失敗へ閉じる。

- タグ名が`vMAJOR.MINOR.PATCH`のcanonicalな安定版SemVerである。
- checkoutした`HEAD`、タグをcommitへpeeledした値、イベントの完全な40桁SHAが一致する。
- `apps/desktop/src-tauri/tauri.conf.json`の`version`がタグと一致する。
- root `Cargo.toml`の`workspace.package.version`がタグと一致する。
- `Cargo.lock`内の`origami2-desktop`版がタグと一致する。
- test開始前にtracked fileの変更がない。
- `.github/release-readiness.json`の版がタグと一致し、全MUST受け入れ、Windowsオーナー
  E2E受け入れ、正式公開承認の3 flagがすべて`true`である。

pre-release識別子とbuild metadataは初版の正式版workflowでは受け付けない。必要になった
場合は、正式版と混在させず、別の受け入れ契約を追加してから拡張する。

開発中のreadiness flagは`false`に固定する。オーナーがWindows実機E2Eとベンチマークの
受け入れ結果を提供し、requirements-statusの全MUSTが実装済みになった後、正式公開する
commitでだけ3 flagを明示的に`true`へ変更する。したがって、開発途中でversionと同名の
タグを誤ってpushしても正式Releaseは作成されない。

### 3.2 手動実行

`workflow_dispatch`では、既に存在するcanonicalタグ、タグが指すべき完全な40桁commit
SHA、固定文字列`PUBLISH_UNSIGNED_WINDOWS_RELEASE`の3項目を入力する。workflowはタグを
新規作成しない。既定値`DO_NOT_PUBLISH`のままでは必ず失敗する。

同じタグの実行は`concurrency`で直列化し、進行中の正式配布を後発runでcancelしない。
公開jobは`windows-production-release` environmentへ所属する。リポジトリ管理者は必要に
応じて、このenvironmentへrequired reviewerと許可タグ規則を設定できる。environment設定が
なくても上記の内部gateは省略されない。

## 4. 検証から公開まで

read-onlyのWindows jobが、次の順序をすべて成功させる。

1. タグ・commit・3箇所の版番号を検証する。
2. `npm ci`後、frontend test、production build、lintを実行する。
3. Rust format、workspace全test、全featureのClippy `-D warnings`を実行する。
4. 外向き通信を遮断したprocessで折り図PDFとSVG ZIPを生成し、固定版の`pypdf`と
   `PyMuPDF`で独立に監査する。
5. Tauri CLIを`--ci --no-sign --bundles nsis`で実行する。
6. NSIS内のアプリ、font、font licenseと固定digestを検査する。
7. NSISがAuthenticode未署名であることを検査する。
8. versionを含む固定名へcopyし、SHA-256を新たに計算する。
9. 独立したasset verifierでもう一度、完全な3ファイル集合、未署名状態、hash、
   日英release noteの警告を検査する。
10. 圧縮なし・30日保持のrelease candidate artifactとして保存する。

公開jobは同じタグを再checkoutして、build jobが記録したcommitと再照合する。さらに
downloadしたartifactのファイル集合と`SHA256SUMS.txt`をLinuxの`sha256sum --strict`で
独立に照合する。公開直前にもGitHub APIが返すタグのcommitを再照合する。検査済みの
installerとchecksumだけをdraft Releaseへ添付し、全uploadが成功した後に限りdraftを
解除する。

## 5. 権限と上書き防止

- workflow全体の既定permissionは空にする。
- build jobは`contents: read`だけを持つ。
- `contents: write`は最終publish jobだけが持つ。
- checkoutは`persist-credentials: false`とする。
- 全GitHub Actionを40桁commit SHAへ固定する。
- PAT、外部token、証明書、repository secretを使用しない。
- publish前に同名のdraftまたは公開済みReleaseが存在すれば失敗し、既存Releaseまたは
  assetを変更しない。
- `gh release upload --clobber`その他の上書き操作を使用しない。
- 同時実行と事後再実行のどちらでも、既存Releaseを置き換えない。

draft作成後のuploadまたは公開に失敗した場合は、証跡を残すためdraftを自動削除しない。
原因とdraft内容を人が確認し、必要ならGitHub上で明示的に処置してから再実行する。
GitHub側のimmutable releasesを利用できる場合は有効化し、公開後のタグ・asset変更も
repository policyで拒否する。

## 6. 無署名版の表示方針

初版はコード署名証明書を用意しない。ファイル名に`unsigned`を含め、build時に
Authenticode状態が`NotSigned`であることを強制し、Release notesでもWindows SmartScreen等
の警告があり得ることを日本語・英語の両方で明示する。両言語とも、利用者へ
`SHA256SUMS.txt`とPowerShell `Get-FileHash`による照合手順、およびmacOSが正式配布対象
ではないことを示す。`.github/release-notes-windows.md`を唯一のtemplateとし、
`.github/release-notes-windows-contract.json`の言語別必須文をpackaging時と独立検証時の
両方で照合する。

将来コード署名を導入するときは、このworkflowへsecretだけを追加して未署名検査を外しては
ならない。鍵の保管、署名対象、timestamp、失効、forkからのsecret遮断、署名後hash、
証明書更新費用を別契約として設計し、ファイル名とRelease notesも同時に変更する。

## 7. 自動回帰

`.github/tests/windows_release_workflow_contract.ps1`を通常CIの`windows-bundle` jobで実行する。
契約testは少なくとも次を固定する。

- release trigger、完全SHA action pin、最小権限、environment、明示確認。
- pull request起動、secret依存、macOS正式配布、上書きoptionが存在しない。
- package scriptが`--no-sign --bundles nsis`のままである。
- fixture Git repositoryで、正しいタグ・commit・versionだけがgateを通る。
- readiness未承認、明示確認不足、タグ/version不一致が失敗する。
- packaging後のhash照合が通り、installerの1 byte改変が拒否される。

この回帰が成功してもWindows 10/11実機でのインストール、起動、SmartScreen表示、
アンインストールまでは証明しない。これらはオーナーが実施する正式版E2Eの受け入れ証跡に
記録する。
