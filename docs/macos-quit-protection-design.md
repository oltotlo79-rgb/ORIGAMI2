# macOS 終了要求・未保存データ保護設計

調査日: 2026-07-17
対象: ORIGAMI2 デスクトップ版（Tauri 2）
状態: 実装前の設計判断。macOS 実機での受け入れ試験を完了するまでは、Dock の「終了」と OS 終了要求を保護済みとは扱わない。

## 1. 結論

現行実装は、次の2経路をすでに保護している。

- ウィンドウの赤ボタン、`File > Close`、`Cmd+W`: WebView の `onCloseRequested` で未保存確認を行う。
- アプリメニューの `Quit`、`Cmd+Q`、`AppHandle::exit`: Tauri の `RunEvent::ExitRequested` でネイティブ確認を行う。

一方、現在固定されている Tao 0.35.3 の macOS delegate は `applicationShouldTerminate:` を実装していない。したがって、Dock の「終了」や macOS のログアウト・再起動・システム終了要求は `RunEvent::ExitRequested` を通らず、現行ガードを迂回する可能性が高い。これは依存クレートの実装から導いた結論であり、最終確定には macOS 実機試験が必要である。

採用する設計は、次の4層である。

1. 現行の WebView ウィンドウ終了ガードを移行完了まで維持し、その後は Tauri の Rust 側 `WindowEvent::CloseRequested` を終了ポリシーへ接続する。
2. 現行の Tauri `ExitRequested` ガードを維持する。
3. `applicationShouldTerminate:` を既存 Tao delegate に安全に追加する macOS 専用アダプターを実装する。
4. すべての異常終了に備え、Rust が管理するローカル復旧ファイルを原子的に更新する。

macOS 専用アダプターは終了判断を保留する `NSTerminateLater` を返し、確認・必要な復旧書き込みが終わった後、元の終了要求に `replyToApplicationShouldTerminate:` でちょうど1回回答する。ここで `AppHandle::exit` を再度呼んではならない。再呼び出しは別の終了要求を作り、二重確認や再入を招く。

この方式は App Store を必要とせず、クラウド、サーバー、外部DB、テレメトリーを必要としない。機能そのもののランニングコストは0円で、復旧データは端末内だけに保存する。

## 2. 調査対象の固定バージョン

`Cargo.lock` で確認したバージョンは次のとおりである。

| コンポーネント | バージョン | 役割 |
|---|---:|---|
| `tauri` | 2.11.5 | アプリAPI、`RunEvent` |
| `tauri-runtime` | 2.11.3 | ランタイム抽象化 |
| `tauri-runtime-wry` | 2.11.4 | Tao/Wry と Tauri のイベント変換 |
| `tao` | 0.35.3 | macOS の `NSApplication` と delegate |
| `wry` | 0.55.1 | WebView |
| `muda` | 0.19.3 | ネイティブメニュー |
| `tauri-plugin-dialog` | 2.7.1 | ネイティブ確認ダイアログ |
| `objc2` | 0.6.4 | Objective-C runtime バインディング |
| `objc2-app-kit` | 0.3.2 | AppKit バインディング |

依存更新時には本書の前提を再確認する。特に Tao が将来 `applicationShouldTerminate:` を実装した場合、ORIGAMI2 側で同じ selector を上書きしてはならない。

## 3. 現行実装

### 3.1 通常のウィンドウ終了

`apps/desktop/src/App.tsx` は `getCurrentWindow().onCloseRequested` を登録している。

- Rust コア処理中なら `event.preventDefault()` で閉じない。
- 未保存でなければそのまま閉じる。
- 未保存なら `window.confirm` を表示し、キャンセル時に閉じない。

Tao の `WindowEvent::CloseRequested` は `tauri-runtime-wry` の `on_close_requested` に渡されるため、赤ボタン、`File > Close`、`Cmd+W` はこの経路である。

最後のウィンドウが破棄されると、Tauri ランタイムは続けて `RunEvent::ExitRequested` を発生させる。ただしその時点ではウィンドウがすでに無く、現行 Rust コードは二重確認を避けるため `webview_windows().is_empty()` なら許可している。この設計は通常経路には妥当だが、JavaScript ガードの登録失敗や WebView 障害に対する Rust 側の代替ガードにはならない。復旧ファイルがその穴を補う。

### 3.2 アプリメニューの終了と `Cmd+Q`

`apps/desktop/src-tauri/src/lib.rs` は macOS の既定メニューを無効にし、ID `origami2_quit` の独自 `Quit` 項目と `CmdOrCtrl+Q` を登録している。そのメニューイベントで `app_handle.exit(0)` を呼ぶ。

`AppHandle::exit` はランタイムへ `Message::RequestExit` を送り、`RunEvent::ExitRequested` を発生させる。現行 Rust コードは、未保存なら同期的に `api.prevent_exit()` を呼び、ネイティブダイアログを表示する。確認の重複は `dialog_open`、ユーザーが破棄を選んだ後の再入は `allow_once` で抑止している。

このため、ORIGAMI2 自身のアプリメニューと `Cmd+Q` は現行ガードの対象である。

### 3.3 Dock と OS 終了要求

macOS の標準 `Quit` は AppKit の `terminate:` action である。Muda 0.19.3 でも `PredefinedMenuItemType::Quit` は `terminate:` に対応付けられている。ORIGAMI2 のアプリメニューは独自項目へ差し替えているため、その項目だけは Tauri 経路へ寄せられている。しかし Dock のコンテキストメニューや OS の終了要求までは差し替えられない。

AppKit はアプリ終了時、delegate が実装していれば `applicationShouldTerminate:` に終了可否を問い合わせる。戻り値は次の3種類である。

| AppKit の戻り値 | 意味 |
|---|---|
| `NSTerminateNow` | 直ちに終了を許可する |
| `NSTerminateCancel` | 終了を拒否する |
| `NSTerminateLater` | 非同期判断のため保留し、後で `replyToApplicationShouldTerminate:` を必ず呼ぶ |

Tao 0.35.3 が生成する `TaoAppDelegateParent` は `applicationDidFinishLaunching:`, `applicationWillTerminate:`, URL open、reopen などを登録するが、`applicationShouldTerminate:` は登録していない。`applicationWillTerminate:` が呼ばれた時点では Tao の `AppState::exit()` が `LoopDestroyed` を通知するだけで、キャンセル可能な `ExitRequested` を生成しない。

したがって現行依存関係では、Dock の「終了」と OS 終了要求は次のように進むと推定する。

```text
Dock / ログアウト / 再起動 / システム終了
  -> NSApplication terminate:
  -> applicationShouldTerminate: は未実装なので既定で継続
  -> applicationWillTerminate:
  -> Tao AppState::exit()
  -> LoopDestroyed
  -> プロセス終了
```

この経路には Tauri のキャンセル可能な `RunEvent::ExitRequested` が無い。

### 3.4 経路別の現在の保護状況

| 終了操作 | 主なイベント経路 | 現行確認 | 追加対策 |
|---|---|---:|---|
| 赤ボタン | Window close request → WebView | あり | 復旧を追加 |
| `File > Close` / `Cmd+W` | Window close request → WebView | あり | 復旧を追加 |
| ORIGAMI2 メニュー `Quit` / `Cmd+Q` | `AppHandle::exit` → `ExitRequested` | あり | ポリシーを共通化 |
| プログラムからの `AppHandle::exit` | `RequestExit` → `ExitRequested` | あり | ポリシーを共通化 |
| Dock の「終了」 | AppKit `terminate:` | 未保護の可能性が高い | AppKit adapter + 復旧 |
| ログアウト・再起動・システム終了 | AppKit termination request | 未保護の可能性が高い | AppKit adapter + 復旧 |
| Force Quit、`SIGKILL`、クラッシュ | 即時停止 | 確認不能 | 復旧のみ |
| 電源断・OSクラッシュ | 即時停止 | 確認不能 | 原子的な復旧のみ |

## 4. 保護要件

### 4.1 必須要件

- 未保存でない場合は、どの通常終了経路も確認なしで速やかに終了する。
- 未保存の場合は、赤ボタン、`Cmd+W`、`Cmd+Q`、アプリメニュー、Dock 終了で少なくとも「変更を破棄して終了」「キャンセル」を選べる。
- コア処理または保存処理中は、状態が確定するまで終了を保留する。
- 同じ終了要求が連続してもダイアログを複数表示しない。
- キャンセル後は編集を継続でき、次の終了要求を正しく処理できる。
- Force Quit、クラッシュ、電源断の後でも、プラットフォームの永続化処理まで成功した最後の復旧世代を次回起動時に復元できる。物理媒体自体が書き込みを保証しない場合は最善努力であることを明示する。
- 復旧によって元ファイルを自動上書きしない。
- 外部サーバーを使用せず、ネットワーク接続なしで機能する。

### 4.2 保証できない範囲

- Force Quit、`SIGKILL`、プロセス abort、突然の電源断では確認ダイアログを表示できない。
- OS のログアウト・終了には応答期限があり、アプリが無期限に終了を止められるとは保証しない。
- `applicationShouldTerminate:` が扱うのは通常の `terminate:` 経路である。bundle が `NSSupportsSuddenTermination` を有効にすると、macOS は通知や delegate 呼び出しなしにプロセスを停止できる。現行設定はこの opt-in を行っていないが、生成された `Info.plist` でも無効であることを配布ごとに検査する。
- 編集直後、最初の復旧ファイル commit より前にプロセスが停止した変更は復旧できない。
- ストレージ故障、空き容量不足、権限異常、媒体側のキャッシュ喪失のときに書き込み成功は保証できない。atomic rename は中間状態を見せないための性質であり、電源断後の永続性とは別である。失敗を可視化し、終了時は安全側へ倒す。

## 5. 選択肢の比較

| 案 | Dock/OS を止める | 強制終了後の復旧 | 依存更新リスク | 判断 |
|---|---:|---:|---:|---|
| A. 現行 `RunEvent` のみ | いいえ | いいえ | 低 | 不採用 |
| B. `applicationWillTerminate` で保存 | いいえ。通知時点が遅い | 最善努力のみ | 中 | 単独では不採用 |
| C. `NSApplication` delegate を丸ごと置換 | はい | いいえ | 非常に高い | 不採用 |
| D. 既存 Tao delegate に selector を追加 | はい | いいえ | 中～高 | macOS ガードとして採用 |
| E. ローカル復旧ファイル | いいえ | はい | 低 | 全OSの基盤として採用 |
| F. Tauri plugin の `on_event` だけ | いいえ | いいえ | 低 | 不採用 |
| G. ネイティブ処理を内製 plugin に隔離 | D と同じ | E と併用可能 | 中～高 | コード構成の選択肢 |

### 5.1 delegate の丸ごと置換を避ける理由

Tao は自分で `NSApplication` delegate を生成し、起動完了、URL open、reopen、終了通知などを処理している。別 delegate へ置き換える場合、Tao の全挙動を正確に転送し続ける必要がある。Tao の内部変更にも追従する必要があり、URL open やウィンドウ再表示を壊す危険が大きい。

既存 delegate のクラスへ不足している selector だけを追加する方が変更面積は小さい。ただし第三者クレートの内部クラスへ接続するため、依存更新ごとの検証は必須である。

### 5.2 plugin だけでは解決しない理由

Tauri plugin の `on_event` が受け取るのは Tauri の `RunEvent` である。Tao が Dock/OS 終了を `ExitRequested` へ変換していない以上、純粋な plugin event hook だけでは捕捉できない。

macOS Objective-C runtime 接続を plugin の `setup` に隔離することはできる。その場合も保護を実現するのは plugin API ではなく、同じ AppKit selector 追加処理である。単一アプリである現段階では `cfg(target_os = "macos")` の内部モジュールが最も単純で、再利用需要が出た時点で内製 plugin 化する。

## 6. 採用アーキテクチャ

### 6.1 一つの終了ポリシー

JavaScript と Rust に別々の判断規則を増やさず、純粋な Rust の終了ポリシーへ判断を集約する。OS 接続層は経路を報告し、ポリシーの結果だけを実行する。

入力例:

- 終了経路: `WindowClose`, `RuntimeExit`, `NativeTerminate`
- `is_dirty`
- `operation_busy`
- `recovery_generation` と `recovery_committed_generation`
- `dialog_pending`
- 保留中の終了 request token
- ユーザー判断: `Discard`, `Cancel`。将来は `Save` を追加可能にする。

出力例:

- `AllowNow`
- `Deny`
- `Defer { flush_recovery, show_dialog }`
- `IgnoreDuplicate`
- `ReplyNativeTerminate(bool)`

状態遷移の基準は次のとおりである。

```text
終了要求
  ├─ clean かつ処理中でない ─────────────> 許可
  ├─ 処理中 / 復旧未commit ───────────────> 保留して復旧をflush
  ├─ dirty かつ確認なし ─────────────────> 1個だけ確認を表示
  ├─ 確認中に再度要求 ───────────────────> 重複を無視して保留継続
  ├─ キャンセル ─────────────────────────> 元の要求を拒否、状態をIdleへ
  └─ 破棄 ───────────────────────────────> 復旧を破棄扱いにして元の要求を許可
```

`allow_once` のような再入抑止は経路ごとに持たず、要求 token と状態遷移で表現する。ウィンドウ終了とアプリ終了が近接した場合でも、同じプロジェクトに対してダイアログは1個だけにする。

ウィンドウ終了も最終的には `RunEvent::WindowEvent { event: WindowEvent::CloseRequested { api }, .. }` で同期的に `api.prevent_close()` し、同じポリシーへ渡す。破棄を選んだ後は再び close request を作る `close()` ではなく、許可 token を確定してから対象ウィンドウを `destroy()` する。最後のウィンドウ破棄に続く `ExitRequested` は同じ token で二重確認を抑止する。Rust 経路へ切り替える変更では、Tauri が JavaScript listener の存在だけで close を事前抑止するため、`App.tsx` の `onCloseRequested` listener を同時に削除する。移行中に両方から確認を表示してはならない。

### 6.2 macOS AppKit adapter

`cfg(target_os = "macos")` の小さなモジュールを追加し、Tauri `Builder::setup` 中にインストールする。Tao は event loop 作成時に delegate を先に設定するため、setup 時点で現在の `NSApplication.delegate` を検査できる。

実装手順:

1. main thread で `NSApplication` と現在の delegate を取得する。
2. delegate が存在すること、実オブジェクトの runtime class が固定済み Tao の `TaoAppDelegateParent` であることを診断情報へ記録する。名前から別の class を取得して変更せず、実 delegate の class だけを対象にする。
3. exact class 自身と継承元のそれぞれについて `applicationShouldTerminate:` の有無を確認する。未知の実装が見つかった場合は上書きせず、adapter install を失敗させる。`class_addMethod` は継承実装を override できるため、`respondsToSelector` だけを見て盲目的に追加してはならない。
4. selector が全階層に無い場合だけ Objective-C runtime の `class_addMethod` で追加し、その戻り値を必ず検査する。
5. 追加後に `respondsToSelector`、実際の IMP、引数数、戻り値と各引数の type encoding を検証する。
6. callback が参照する bridge/state を先に初期化し、その後に selector を公開する。途中失敗で selector だけが残る状態を作らない。
7. インストール処理を冪等にし、2回目は何もしない。

依存は macOS target のみに直接宣言する。

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = { version = "0.6.4", default-features = false, features = ["std"] }
objc2-app-kit = { version = "0.3.2", default-features = false, features = ["std", "NSApplication", "NSResponder"] }
```

必要な feature は実装 spike で最小化する。現在は推移依存として存在するが、ORIGAMI2 が直接 API を使うなら直接依存にしてバージョンと feature を明示する。

callback の動作:

- clean で処理中でなければ `NSTerminateNow`。
- dirty、処理中、または最新世代の復旧が未commitなら `NSTerminateLater`。
- 保留 request を一つだけ登録し、main thread で復旧 flush とネイティブ確認を進める。
- キャンセル時は、保留中の元 request に `replyToApplicationShouldTerminate(false)`。
- 破棄時は、保留中の元 request に `replyToApplicationShouldTerminate(true)`。
- 保存・復旧 flush が失敗した状態で暗黙に終了を許可しない。エラーを表示し、既定はキャンセルとする。
- bridge 未初期化、state lock 失敗、panic など `NSTerminateLater` を返す前の失敗は `NSTerminateCancel` を返す。`NSTerminateLater` を返した後の開始・I/O・dialog 失敗は必ず main thread へ戻して `replyToApplicationShouldTerminate(false)` とする。

重要な安全条件:

- Objective-C callback 境界から Rust panic を unwind させない。`catch_unwind` と安全側の応答を用意する。
- `NSTerminateLater` を返した request ごとに `Pending -> Replied(bool)` の一方向遷移を持たせ、成功、キャンセル、I/O失敗、dialog開始失敗、stale callback の全経路で reply が0回にも2回にもならないようにする。`NSTerminateNow` / `NSTerminateCancel` を返した request には reply しない。
- delegate object、selector、引数、戻り値の ABI/type encoding を固定文字列だけに頼らずテストする。
- callback 内で長時間のファイルI/O、mutex 待機、WebView 応答待ちをしない。まず `NSTerminateLater` を返す。
- main thread 制約を守る。UI と `NSApplication` reply は main thread で実行する。
- AppKit は `NSTerminateLater` 後に `NSModalPanelRunLoopMode` で待機する。Tao 0.35.3 の proxy source は common modes に登録されているが、Tauri task、dialog completion、background flush completion、reply がこの待機中にも進行することを専用 harness で確認する。Tauri event queue だけが通常 run loop に戻ることを前提にしない。
- `tauri-plugin-dialog` 2.7.1 の非同期 `show` は内部の `run_on_main_thread` 失敗を呼び出し側へ返さない。native termination 経路では「`show` を呼んだ」だけを開始成功とせず、開始失敗と completion を観測できる AppKit bridge または同等の wrapper を使う。
- 生ポインターで `AppHandle` を保持しない。プロセス寿命を満たす同期状態または明示的な bridge を使う。
- 将来の Tao delegate が selector を実装していたら上書きしない。debug/CI で明示的に失敗させ、adapter を更新する。
- adapter のインストール失敗を黙殺しない。ログと診断状態に残し、復旧層は必ず動かす。

### 6.3 ダイアログ

第1段階では現行挙動とそろえ、次の2ボタンとする。

- 「変更を破棄して終了」
- 「キャンセル」

Dock/OS 経路では WebView の `window.confirm` に依存せず、ネイティブダイアログを使用する。ウィンドウが存在すれば親にし、無ければ app-modal とする。

将来「保存して終了」を追加する場合は3択の状態として設計する。既存パスが無い新規プロジェクトでは Save As が必要になり、OS 終了の応答期限内にファイル選択を完了できない場合がある。そのため、2択の保護を先に完成させ、「保存して終了」は独立したUX改善として追加する。

### 6.4 ローカル復旧ファイル

復旧は macOS 専用ではなく、Windows/macOS 共通の Rust 機能にする。保存先は一時キャッシュではなく、Tauri の `app_data_dir()/recovery/` とする。識別子 `dev.origami2.editor` がアプリ用ディレクトリを分離する。

単一プロジェクト運用でも、アプリ全体で共有する固定名1スロットにはしない。現行アプリは single-instance を強制していないため、2プロセスを起動すると互いの復旧を上書きできる。初期実装から project ID ごとのスロット名を使い、さらに次のどちらかを完了条件にする。

- デスクトップアプリを single-instance にして、2プロセス目が同じ recovery store を書かないようにする。
- 複数プロセスを許可する場合は、project ID ごとの OS ロックまたは同等の排他と世代比較を実装する。同じ `.ori2` を2プロセスで開く場合があるため、ファイル名に project ID を含めるだけでは不十分である。

復旧レコードは次を持つ。

| 項目 | 用途 |
|---|---|
| schema version | 将来の移行・非対応判定 |
| app version | 作成側の診断 |
| project ID | 別プロジェクトとの混同防止 |
| generation / revision | 新旧判定と競合防止 |
| 元ファイルパス（任意） | 保存済みファイルとの比較。機微情報として扱う |
| saved baseline identity/hash | 保存済み内容と同一か判定 |
| timestamp | ユーザーへの候補表示 |
| `.ori2` payload | 既存 serializer と validator を使う復旧本体 |

書き込み規則:

1. 成功した編集コマンドごとに recovery generation を進める。
2. mutex 内では document snapshot と generation を複製し、serialization とI/Oの前に lock を解放する。
3. 単一 writer が最新世代へ集約し、古い世代の完了で新しい世代を上書きしない。
4. 通常時は短い debounce で集約するが、各成功編集が最終的には復旧対象になるようにする。
5. 終了要求では最新世代の即時 flush を要求し、commit 完了または失敗が確定するまで終了を保留する。
6. 既存の `.ori2` serializer、読み戻し検証、同一ディレクトリの staged file、`write_all`、`sync_all`、プラットフォーム別 atomic replace を再利用する。Windows は外部の書込み・削除・rename共有を禁止して検証済みハンドル自体を `SetFileInformationByHandle(FileRenameInfo)` で置換する経路を使い、`AtomicWriteFile` の汎用 Windows rename へ戻さない。
7. staged file の同期・検証・atomic replace 後、対応可能なOSでは親ディレクトリまたは rename metadata の durability barrier も完了してから generation を committed とする。macOS では通常の `fsync` と、必要なら `F_FULLFSYNC` を含む最善努力の保証範囲を実機で測定し、未対応または失敗時に「電源断耐性あり」と報告しない。
8. 書き込み失敗をステータス表示し、次の編集または明示的な再試行で復旧する。

macOS で将来 `NSSupportsSuddenTermination` を有効にする場合は、dirty または未commit世代が発生した時点から durable commit、明示保存、または破棄が完了するまで `NSProcessInfo` の sudden termination を無効化し、必ず対になる enable を行う。初期版は opt-in しない方針とし、生成 bundle の `Info.plist` をCIで検査する。

明示保存との順序:

1. 元ファイルへの atomic save を完了する。
2. 保存した revision が現在の revision と一致する場合だけ saved baseline を更新する。
3. その project ID と generation がまだ最新で、現在 clean の場合だけ復旧スロットを削除する。
4. 保存中に新しい編集が入った場合は、その新しい復旧世代を残す。

元ファイルの保存 commit より先に復旧を削除してはならない。

起動時の規則:

- 復旧レコードを bounded reader と既存 validator で検証する。
- 壊れている場合は元ファイルへ触れず、隔離または削除の選択を表示する。
- 保存済みファイルと実質的に同一なら、復旧済みとして静かに破棄できる。
- 保存済みより新しい、または内容が異なる場合は「復元」「破棄」を提示する。
- 復元時は新しい未保存ドキュメントとして開き、元ファイルを自動上書きしない。
- ユーザーが終了時に明示的に破棄を選んだら復旧も削除する。削除失敗は終了を無限に止めず、次回に古い候補が出る安全側の失敗とする。

## 7. 実装順序

### Phase 1: 終了ポリシーの純粋化

- dirty、処理中、dialog pending、request token を一つの Rust state machine にする。
- 現行 `ExitRequested` をそのポリシーへ接続する。
- Rust の `WindowEvent::CloseRequested` をそのポリシーへ接続し、同じ変更で JavaScript close listener を削除する。それまでは現行 JavaScript guard を維持するが、両方の dialog を同時に有効にしない。
- 全分岐の unit test を先に作る。

### Phase 2: 復旧ファイル

- recovery store と metadata を定義する。
- single-instance またはプロセス間排他のどちらを採るかを実装し、二重起動試験を追加する。
- editing command 後の generation 更新と単一 writer を実装する。
- 起動時の検出・復元・破棄を実装する。
- crash 相当の中断を含むファイルテストを追加する。

この段階だけでも Force Quit や突然の停止への耐性が大きく上がり、Windows にも利益がある。

### Phase 3: macOS AppKit adapter spike

- `applicationShouldTerminate:` の selector 注入を最小コードで実装する。
- clean 時の `NSTerminateNow`、dirty 時の `NSTerminateLater` と reply を検証する。
- Tao delegate の既存機能、URL open、reopen、通常 close が壊れていないことを確認する。
- adapter の install 診断をテスト可能な形で露出する。

### Phase 4: 統合と実機受け入れ

- Dock、ログアウト、再起動、システム終了を実機で確認する。
- 重複要求、復旧 flush 失敗、ウィンドウなしを確認する。
- release checklist と依存更新 checklist を文書化する。

## 8. テスト設計と自動化の境界

### 8.1 全OSで自動化する unit test

終了ポリシーは AppKit/Tauri から分離し、次を表形式で網羅する。

- clean / dirty
- idle / operation busy / recovery writing
- `WindowClose` / `RuntimeExit` / `NativeTerminate`
- Cancel / Discard
- 同じ request の再入、異なる経路からの近接要求
- dialog 表示中の重複抑止
- Cancel 後の再要求
- recovery flush 成功 / 失敗
- stale token の callback を無視できること

復旧 store は temporary directory で次を自動化する。

- `.ori2` の正常 round-trip
- atomic commit 前に中断したとき旧レコードが残ること
- truncated、破損、巨大、不正 schema の拒否
- project ID、revision、generation の新旧判定
- 古い writer が新しい generation を上書きしないこと
- 明示保存中に新規編集されたとき復旧を消さないこと
- 保存済みファイルと同一 / 異なる判定
- 復元しても元ファイルを変更しないこと
- 削除競合、権限エラー、容量エラーを模した失敗処理

### 8.2 macOS GitHub Actions で自動化する範囲

現行CIは `macos-latest` で次を実行している。

- Rust workspace test
- Clippy `--all-targets --all-features -D warnings`
- unsigned `.app` bundle build

追加する自動検査:

- macOS target で AppKit adapter が compile/link すること。
- installer が selector の有無、delegate 存在、二重 install を正しく判定する unit test。
- Objective-C callback ABI/type encoding の検査。
- AppKit を trait の背後へ置き、`TerminateNow/Cancel/Later` と reply を mock する bridge test。
- `NSTerminateLater` を返した全経路で reply がちょうど1回、その他では0回であることを検査する。
- 生成 `.app/Contents/Info.plist` が `NSSupportsSuddenTermination=true` を含まないことを検査する。
- bundle 起動用の専用 smoke harness で `[NSApp terminate:nil]` 相当を送り、期待する診断ログを artifact にする試験を検討する。

テスト runner 自身を終了させる AppKit 試験を通常の unit test 内で行わない。専用アプリ/harness、watchdog、終了コード、ログ artifact を分離する。

### 8.3 実機またはGUIセッションが必要な範囲

GitHub-hosted macOS runner のGUI状態、Dock操作、Accessibility権限、ログアウト操作は安定した前提にできない。AppleScript で Dock を操作するテストも権限とUI変更に弱い。次は少なくとも配布前の実機チェックリストとする。

| シナリオ | clean | dirty + Cancel | dirty + Discard |
|---|---:|---:|---:|
| 赤ボタン | 必須 | 必須 | 必須 |
| `File > Close` / `Cmd+W` | 必須 | 必須 | 必須 |
| アプリメニュー Quit / `Cmd+Q` | 必須 | 必須 | 必須 |
| Dock の「終了」 | 必須 | 必須 | 必須 |
| ログアウト | 必須 | 必須 | 必須 |
| 再起動 / システム終了 | 必須 | 必須 | 必須 |
| ウィンドウが閉じた・非表示の状態 | 必須 | 必須 | 必須 |

さらに次を確認する。

- ダイアログ中に終了操作を連打しても1個しか出ない。
- Cancel 後に編集・保存・再終了できる。
- 復旧書き込み失敗時に暗黙終了しない。
- Force Quit 後、再起動時に最後の復旧世代を提示する。
- 古い復旧レコードで保存済みファイルを上書きしない。
- URL open、Dock 再クリック、ウィンドウ reopen が Tao の従来どおり動く。
- 対象に含めるなら Apple Silicon と Intel の双方で ABI を確認する。

unsigned `.app` のCI build はコンパイル確認には有効だが、Gatekeeper、署名、notarization、一般ユーザー環境での配布UXまでは検証しない。

## 9. 配布・費用・プライバシー

- AppKit の delegate API は通常の macOS アプリ機能で、Mac App Store への公開は不要である。
- `.app` または DMG の直接配布が可能である。
- 未署名アプリを手元・限定範囲で配ることはできるが、一般公開では Gatekeeper の警告を減らすため Developer ID 署名と notarization を検討する必要がある。これはサーバーのランニングコストとは別の配布判断である。
- 復旧機能はローカルディスクだけを使用し、クラウド利用料や常時稼働サーバー費用は発生しない。
- GitHub Actions の利用可否・無料枠はリポジトリ種別と契約条件に依存するため、機能の「0円設計」とCI枠を混同しない。
- 復旧 metadata の元ファイルパスは個人情報・制作情報になり得る。`app_data_dir` 外へ送信せず、ログへ無条件に出さない。
- テレメトリーやクラッシュ送信は本設計に含めない。

## 10. 完了条件

次をすべて満たした時点で、この機能を完成とする。

- 終了判断の純粋 state machine と全分岐 unit test がある。
- 復旧ファイルが atomic、世代安全、bounded、既存 `.ori2` validator 利用である。
- 復旧 commit の完了定義が file sync、atomic replace、対応可能な metadata durability barrier を含み、保証できない媒体条件をUI/文書で過大表現しない。
- 二重起動時に異なるプロジェクトまたは同じ project ID の復旧スロットを相互上書きしない。
- 明示保存と復旧削除の競合テストが通る。
- macOS adapter が selector の有無と ABI を起動時に検証する。
- Dock 終了で dirty + Cancel がアプリを終了させない。
- Dock 終了で dirty + Discard が二重確認なしに終了する。
- ログアウト・再起動・システム終了要求で同じ基本挙動を実機確認する。
- Force Quit 後に復旧候補が提示され、元ファイルを自動上書きしない。
- Windows と macOS の既存 close/save/undo/redo test に回帰がない。
- `cargo fmt`, workspace test, Clippy, frontend test/build/lint、macOS `.app` build がすべて通る。
- release checklist に実機終了経路テストが追加されている。

## 11. 依存更新時の監査項目

Tauri、Tao、Muda、objc2 のいずれかを更新するたびに次を確認する。

1. Tao delegate のクラスと設定時期が変わっていないか。
2. Tao が `applicationShouldTerminate:` を新たに実装していないか。
3. Dock/OS 終了が Tauri `ExitRequested` に変換されるようになっていないか。
4. `NSApplicationTerminateReply` の型と Objective-C method encoding が一致するか。
5. Muda の Quit action が変わっていないか。
6. selector install が失敗時に明示的な診断を返すか。
7. 実機受け入れマトリクスが引き続き通るか。
8. 生成 `Info.plist` の sudden/automatic termination opt-in が変わっていないか。
9. `tauri-plugin-dialog` の main-thread scheduling と completion error の契約が変わっていないか。

Tauri/Tao 側が公式にキャンセル可能な native termination hook を提供した場合は、内部 selector 追加を廃止し、その公開APIへ移行する。

## 12. 調査根拠

ローカルの固定済みクレートソースを一次根拠として確認した。

- `tauri-runtime-wry-2.11.4/src/lib.rs:4307-4323`: window destroyed 後、window が空になると同期的に `ExitRequested` を通知し、prevent を確認する。
- `tauri-runtime-wry-2.11.4/src/lib.rs:4353-4366`: `Message::RequestExit` を `ExitRequested` へ変換し、prevent を確認する。
- `tauri-2.11.5/src/app.rs:220-232`: `RunEvent::ExitRequested` と `RunEvent::Exit` の定義。
- `tauri-2.11.5/src/app.rs:573-579`: `AppHandle::exit` が `ExitRequested` と `Exit` を発生させる契約。
- `tao-0.35.3/src/platform_impl/macos/event_loop.rs:170-180`: Tao delegate を生成して `NSApplication` へ設定する。
- `tao-0.35.3/src/platform_impl/macos/app_delegate.rs:47-89`: 登録 selector 一覧。`applicationWillTerminate:` はあるが `applicationShouldTerminate:` は無い。
- `tao-0.35.3/src/platform_impl/macos/app_delegate.rs:130-135`: termination notification で `AppState::exit()` を呼ぶ。
- `tao-0.35.3/src/platform_impl/macos/app_state.rs:272-280`: `AppState::exit()` が `LoopDestroyed` を通知する。
- `tao-0.35.3/src/platform_impl/macos/event_loop.rs:324-353`: event proxy の CFRunLoop source を common modes へ登録する。
- `muda-0.19.3/src/platform_impl/macos/mod.rs:992-995`: predefined Quit と `terminate:` の対応。
- `objc2-app-kit-0.3.2/src/generated/NSApplication.rs:1053-1066`: `NSApplicationTerminateReply` の3値。
- `objc2-app-kit-0.3.2/src/generated/NSApplication.rs:1108-1119`: `applicationShouldTerminate:` の契約。
- `objc2-app-kit-0.3.2/src/generated/NSApplication.rs:780-783`: `replyToApplicationShouldTerminate:` の契約。
- `objc2-0.6.4/src/ffi/class.rs:78-84`: Objective-C runtime の `class_addMethod` binding。
- `tauri-2.11.5/src/app.rs:97-105`: Rust 側 window close の同期的な `prevent_close` API。
- `tauri-plugin-dialog-2.7.1/src/desktop.rs:215-255`: 非同期 message dialog が `run_on_main_thread` の失敗を呼び出し側へ返さない現行実装。
- `tauri-2.11.5/src/path/desktop.rs:243-269`: `app_data_dir`, `app_local_data_dir`, `app_cache_dir` の解決。
- `apps/desktop/src-tauri/src/lib.rs`: 現行 custom Quit、`ExitRequested` guard、atomic `.ori2` 保存。
- `apps/desktop/src/App.tsx`: 現行 window close guard。
- `.github/workflows/ci.yml`: Windows/macOS Rust CI と unsigned macOS app bundle build。

Apple の公開API契約も一次根拠として確認した。

- [applicationShouldTerminate(_:)](https://developer.apple.com/documentation/appkit/nsapplicationdelegate/applicationshouldterminate(_:)): 通常の `terminate:` から呼ばれ、Now / Cancel / Later を返す契約。
- [reply(toApplicationShouldTerminate:)](https://developer.apple.com/documentation/appkit/nsapplication/reply(toapplicationshouldterminate:)): Later を返した後に必ず回答する契約。
- [terminateLater](https://developer.apple.com/documentation/appkit/nsapplication/terminatereply/terminatelater): Later 中は modal panel run-loop mode で待機する契約。
- [class_addMethod](https://developer.apple.com/documentation/objectivec/class_addmethod(_:_:_:_:)): 同一classの既存実装は置換せず、継承実装は override する契約。
- [ProcessInfo](https://developer.apple.com/documentation/foundation/processinfo): sudden termination opt-in 時は通知なし停止があり、遅延書き込み中は明示的に無効化する契約。
