# 展開プロジェクトフォルダー filesystem 契約

## 1. 適用範囲

この契約は、`ori-formats::project_folder`のin-memory admission boundaryと、
desktop native filesystemを接続するIO-003 adapterに適用する。

初回adapterは次だけを実装する。

- native directory pickerで選択した既存folderの安全な読込
- native directory pickerで選択した既存parent直下への、新規target限定の保存
- project instance、project ID、revisionへ束縛したimmutable capture
- staleになった保存結果を公開しないgeneration fence

既存targetの置換は本書4章のjournal方式が完成するまで実装しない。既存folderへ
一度退避してからrenameするだけのbest-effort上書きや、folder内fileを順次更新する
保存は禁止する。このため初回adapter完成後もIO-003は部分的な内部基盤であり、
日英desktop UIから利用できる「部分実装」とする。IO-003を実装済みにするには、
既存targetのjournal置換とオーナー実施のWindows実機E2Eを完了する必要がある。

## 2. privacyとtrust boundary

- filesystem pathはnative directory pickerとnative process内だけで扱う。
- path、親directory名、OS error本文をWebView response、log、diagnosticsへ出さない。
- WebViewへ返せるのは固定error category、cancel有無、認証済みproject snapshotだけ。
- WebViewから絶対path、相対path、target child名を受け取らない。
- 保存target child名はnativeが認証済みproject名からASCII allowlistで生成する。
- `project.json`、履歴、previewの意味検証は必ず
  `read_project_folder_v1_with_limits`へ集約し、filesystem adapter独自の緩いreaderを
  作らない。

## 3. 読込契約

### 3.1 許可tree

選択rootで許可するtreeは次だけである。

```text
selected-root/
├─ manifest.json
├─ project.json
├─ editor-history.json          # 任意
└─ preview/
   └─ crease-pattern.svg
```

rootと`preview`は実directory、3または4個のpayloadは通常fileでなければならない。
上記以外のentry、caseだけが異なるentry、同一名、nested directory、special fileを
拒否する。

### 3.2 linkとobject identity

次を全て拒否する。

- symbolic link
- Windows junctionおよび全reparse point
- hard-link countが1以外のpayload file
- socket、FIFO、device等の通常file/directory以外

Unixではroot directoryを`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`で開き、そのfdから
独立したopen-file descriptionを作って`fdopendir/readdir`し、
`openat(..., O_NOFOLLOW)`を使う。Windowsではdirectoryとfileを
`FILE_FLAG_OPEN_REPARSE_POINT`で開き、reparse attributeを検査する。rootとpreviewの
directory handleは`FILE_SHARE_DELETE`を許さず最終componentを読込中固定し、
payload handleは書込・削除共有を許さない。

列挙前後で次を再照合する。

- root handleとpicker pathから再度開いたrootのobject identity
- rootおよび`preview`の固定entry集合
- 各payloadの読込handleと同じ相対名から再度開いたhandleのobject identity
- file type、reparse状態、hard-link count、size

Unix object identityはdevice IDとinode、Windowsはvolume serialとfile indexを使う。
照合不能、列挙error、途中差替え、名前の消失はすべて失敗へ閉じる。Windowsは
delete sharingを抑止してroot/previewのrename ABA自体を遮断する。Unixを含む全経路で
payloadは開いたhandleから読み、最後にcoreのsize/hash/project ID/history/preview
再生成照合を通すため、path差替えから未認証bytesを採用しない。

### 3.3 bounded read

各payloadはrole別hard ceilingと全entry共通ceilingの小さい方を使い、
`ceiling + 1` bytesまでしか読まない。宣言sizeだけを信頼しない。

- `manifest.json`: 1 MiB
- `project.json`: 128 MiB
- `editor-history.json`: 64 MiB
- `preview/crease-pattern.svg`: 16 MiB
- 合計: 256 MiB
- payload file数: 3または4
- directory entry数も固定集合を超えた時点で拒否

読込bytesを固定canonical順でin-memory coreへ渡し、core admission後だけprojectを
置換候補にする。numeric expressionと到達可能な履歴poseも通常`.ori2` openと同じ
native validatorへ通す。

## 4. 保存とクラッシュ復旧

### 4.1 初回実装: 新規target限定

native directory pickerは保存parentを選ぶ。target child名はproject名からnativeが
生成する英数字、`-`、`_`だけの最大64文字base、project IDのcanonical先頭8 byteを
小文字hex 16文字にしたsuffix、固定suffix`.origami2-folder`で構成する。日本語だけの
project名等がASCII baseを持たない場合も、project ID suffixにより別project同士を
同一の既定名へ潰さない。

保存は次の順で行う。

1. project lockを一度だけ取得し、instance ID、project ID、revision、
   `ProjectDocument`、`EditorHistoryV1`を同じ瞬間からcaptureする。
2. lockを解放し、core writerでcanonical artifactを生成する。
3. pickerでparentを選択する。parent pathはnative外へ出さない。
4. 同一parentにcreate-newの固有staging directoryを作る。
5. 固定entryだけをcreate-newで書き、各fileをflush、`sync_all`、同じhandleから
   readbackして元bytesと一致させる。
6. staging treeをfilesystem readerとcore readerで再読込し、captureしたarchiveと
   entry bytesに一致させる。
7. 対応OSでは`preview`、staging、parent directoryをpublish前に同期する。
8. project lockを再取得し、instance ID、project ID、revisionがcaptureと一致する
   場合だけ、stagingのexact treeと全payloadをもう一度再検証し、no-replace renameで
   stagingをtargetへ一回で公開する。
9. publish成功後に同じlock内でsaved baselineを更新する。folder pathはsnapshotへ
   格納・返却しない。

targetが既に存在する、またはpicker後に別processがtargetを作った場合はno-replace
publishを失敗させ、既存targetを一切変更しない。

クラッシュ状態は次の二つだけである。

- publish前: hidden stagingだけが残り、targetは存在しない
- publish後: 全fileを検証済みの完成targetが一回で見える

staging cleanupは所有する固定entry名をentry単位でunlinkし、未知entryを再帰削除
しない。cleanup失敗はtarget公開へ昇格させない。

### 4.2 将来実装: 既存target置換journal

既存target置換を追加するときは、同一parentに次をcreate-newで作る。

```text
.origami2-folder-txn-<id>.json
.origami2-folder-stage-<id>/
.origami2-folder-backup-<id>/
target/
```

journalは未知fieldを拒否するversion 1 JSONとし、target basename、transaction ID、
old/new manifest SHA-256、状態を持つ。絶対pathを格納しない。状態は単調に次だけを
遷移する。

```text
prepared
  -> old_moved
  -> new_published
  -> cleanup_complete
```

各遷移はjournalを同一directoryのcreate-new一時fileへ書いて同期し、原子的に
journal本体へ置換後、parent directoryを同期してから次のrenameを行う。

1. `prepared`: new stagingを全検証済み、old targetは未変更
2. old targetをno-replaceでbackupへrenameし、parent sync
3. `old_moved`をdurable化
4. stagingをno-replaceでtargetへrenameし、parent sync
5. `new_published`をdurable化
6. backupを固定tree verifierで確認後にentry単位で削除し、parent sync
7. `cleanup_complete`をdurable化してjournalを削除、parent sync

起動時または次回folder操作前のrecoveryは、journal、stage、backup、targetを全て
no-followで検査し、hashとtransaction IDが一致した場合だけ次を行う。

| durable state | target | backup | staging | recovery |
|---|---|---|---|---|
| `prepared` | old | なし | new | stagingとjournalを安全に破棄 |
| `old_moved` | なし | old | new | newをtargetへ公開、失敗時はoldをtargetへ戻す |
| `old_moved` | new | old | なし | `new_published`へ進める |
| `new_published` | new | old | なし | backup cleanupへ進める |
| 不整合 | 任意 | 任意 | 任意 | 自動変更せず固定errorで停止 |

同じtargetに複数journalがある、未知entry、ID/hash不一致、link、object差替え、
renameのatomic/no-replace保証が使えないplatformでは既存targetを変更しない。
このrecoveryとfailure injection testが完成するまで上書きUIを有効化しない。

## 5. stale completionとproject state

保存captureは次の三つへ同時に束縛する。

- 非永続のproject instance ID
- 永続project ID
- editor revision

serialization、picker、staging中にいずれかが変わった場合はpublishしない。revisionが
同じ別instanceへ戻るABAも拒否する。公開とsaved baseline更新の間はproject lockを
保持し、公開済みbytesと異なるrevisionをcleanとして表示しない。

folderからのopenもpicker前の三値へ束縛し、読込中にactive projectが変わった場合は
置換しない。認証済みfolderを開いたprojectは標準`.ori2` pathを持たない状態にし、
通常Saveがdirectoryを単一fileとして扱う事故を防ぐ。

native dialogのタイトルはstrictな`ja` / `en`だけを受け付ける。single-flight permitは
dialog中はcommand futureが所有し、blocking worker開始後はworkerへmoveする。WebViewの
reloadやfuture中止後もworker完了までbusyを維持し、二つのopen/saveが重ならない。

## 6. desktop UIとstrict IPC

- UIは「展開フォルダーを開く」と「展開フォルダー保存」を日英で表示する。
- 保存UIは新しいtargetだけを作り、既存folderを上書きしないことを明示する。
- dirty projectからopenする場合は通常openと同じ破棄確認を行う。
- cancel時は現在snapshotを置換せず、選択前の編集状態を維持する。
- WebView requestは`locale`だけで、path、target名、bytesを受け付けない。
- responseは`canceled`とpathlessな認証済みproject snapshotだけをexact-keyで受け付ける。
- nativeの固定error codeだけを閉じたcategoryへ写し、任意error本文をUIへ反射しない。

## 7. 必須回帰

- canonical folderのread/write/read
- manifest/history有無と全roleのexact/one-short byte limit
- root/previewのextra entry、case衝突、nested directory
- symlink、dangling symlink、junction/reparse point、hard link、FIFO/special file
- 列挙後またはfile open後の差替えをhookで発生させ、固定failureになること
- file object identity、root identity、sizeの前後不一致
- target事前存在とpublish直前raceで既存内容を変更しないこと
- prepare後の未知entry追加または既知payload変更を公開しないこと
- staging write/readback/core再検証、file sync、directory sync failure
- publish前failureでtargetがなく、stagingが固定entry cleanupされること
- stale revision、project ID、instance ABAでpublishしないこと
- response/error JSONにselected path、OS error、staging名を含めないこと
- strict locale、cancel非置換、worker所有permit、stale bindingを拒否すること
- Windowsでroot/previewのrename ABAをhandleが遮断すること
- WindowsとUnixの条件付きlink/object identity/no-replace test
- 将来上書き実装時は4.2の全stateでprocess kill相当failure injectionと冪等recovery
