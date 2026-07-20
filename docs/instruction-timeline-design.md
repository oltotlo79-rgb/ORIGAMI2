# 折り手順タイムライン初期設計

## 目的

最初の実用範囲は、3Dプレビューへ実際に適用された姿勢を手動の折り手順として登録する経路と、名前付き折り技法を説明専用の手順案として登録する経路を持つ。どちらも説明を編集して`.ori2`へ保存できるが、後者は3D姿勢や物理操作を一切実行しない。

要求角度と衝突判定後の表示角度が異なる場合があるため、手順へ記録する正本はスライダーの要求値ではない。`FoldPreview`が描画へ適用できた姿勢を、Three.jsやmotion runnerへの参照を含まない読み取り専用DTOとして通知し、その値だけを登録候補にする。

## 初期範囲

- 平面姿勢、単一ヒンジ姿勢、木構造の複数ヒンジ姿勢を手動登録する
- 一つの手順へ、現在の全ヒンジ角度と固定面を保存する
- タイトル、説明、注意事項、表示時間を編集する
- 手順を並べ替え、削除し、現在の3D姿勢で更新する
- 通常編集と同じrevision、Undo、Redo、dirty判定を使用する
- `.ori2`へ保存し、旧`.ori2`を空タイムラインとして読み込む
- 保存した完全姿勢を順番に適用する段階再生を行う
- 名前付き折り技法の技法情報、設定値、前提条件、操作を元の順序の説明専用手順へ決定的に変換する
- 全追加内容を事前表示し、明示確認後に一つの原子的編集として登録する

初期範囲の再生は、複数ヒンジを滑らかな一つの運動として補間するものではない。各手順の完全姿勢を離散的に適用する。手順間の連続経路が衝突しないことや、実物として折れることは保証しない。

## 永続モデル

```text
InstructionTimeline
└─ steps: InstructionStep[]
   ├─ id: InstructionStepId
   ├─ title
   ├─ description
   ├─ caution
   ├─ duration_ms
   └─ pose: InstructionPose
      ├─ model: absolute_hinge_angles_v1 | declarative_only_v1
      ├─ source_model_fingerprint
      ├─ fixed_face: FaceId | null
      └─ hinge_angles: InstructionHingeAngle[]
         ├─ edge: EdgeId
         └─ angle_degrees
```

各poseは前手順との差分ではなく完全角度ベクトルである。順序に依存する上書き列を保存しないため、各手順を単独で検証・編集・再生できる。

平面姿勢は`fixed_face = null`かつ空の角度ベクトルとする。折られた姿勢は現在モデルに存在する固定面と、現在の全ヒンジを一度ずつ含む角度ベクトルを必要とする。角度ベクトルは`EdgeId`のRFC UUID byte順で正規化する。

`declarative_only_v1`は人が読む説明だけを表す。`fixed_face`は常に`null`、`hinge_angles`は常に空であり、実行可能姿勢へ置換できない。`source_model_fingerprint`は保存・履歴DTOの正規形を維持するため記録するが、stale判定や3D適用の権限には使わない。

## 折りモデル指紋

手順が現在の展開図用かをrevision番号で判断しない。手順編集だけでもrevisionは進み、ファイル読込後はrevisionが0へ戻るためである。

`fold_model_fingerprint_v1`は次を正規化したSHA-256小文字16進64桁である。

- ID順の頂点IDと座標のbinary64 bit列
- ID順の辺ID、無向端点、線種
- 循環開始位置と反転を正規化した紙境界
- 切断可否
- 紙厚のbinary64 bit列

作品名、表裏色、テクスチャ、project ID、revision、折り手順自体は含めない。保存順や無向辺の向きだけを変えても同じ指紋となり、座標、線種、境界、切断可否、紙厚が変われば異なる指紋となる。

手順の`source_model_fingerprint`と現在値が異なる場合、その手順を「古い展開図用」とする。古い手順は説明編集、移動、削除を許可するが再生しない。「現在の姿勢で更新」により、角度、固定面、指紋を現在値へ置換できる。展開図をUndoして元の内容へ戻し指紋が一致すれば、手順は再び現在用となる。

## 信頼境界

### 3Dプレビュー

`FoldPreview`は次だけを通知する。

- project IDとrevision
- 固定面ID
- 実際に描画された全ヒンジ角度
- `stable`、`running`、`blocked`、`indeterminate`の観測状態

scene、mesh、runner token、衝突解析authority、request keyは通知しない。`running`姿勢は登録候補にしない。`blocked`または`indeterminate`の表示姿勢を登録する場合も、経路安全認定済みとは扱わない。

### TypeScript

IPC snapshotは固定field、上限、ID、指紋、文字、時間、角度、重複、正規順をfail-closedで検証する。現在のproject、revision、fold modelと一致しない実姿勢通知は保持しない。

### Rustデスクトップ

フロントエンドは`source_model_fingerprint`を指定できない。追加と姿勢更新ではRustが次を実行する。

1. project IDとrevisionを照合する
2. 同じrevisionのpaperとcrease patternを捕捉する
3. project lock外でtopologyを解析する
4. 再ロック後にproject、revision、捕捉内容を再照合する
5. 平面または接続された木構造であることを確認する
6. 固定面と現在の全ヒンジが完全一致することを確認する
7. Rust側で現在の指紋を付与する
8. `EditorState`の一コマンドとして登録する

解析中にプロジェクトが変わった場合は登録しない。循環hinge、欠落・余分・重複hinge、不明な固定面、非有限角度、範囲外角度は拒否する。

`.ori2`読込時は、現在の指紋を持つ手順だけを現在topologyへ再照合する。異なる指紋を持つ過去手順はstaleな編集可能記録として保持する。

### 名前付き折り技法からの説明案

選択済みの厳格な技法文書から、技法情報、parameter、precondition、ordered operationの順に最大512手順を生成する。各source objectは決定的な`source-json-v1`表現として説明へ保持し、4,000文字を超える説明だけを連続chunkへ分割する。中割り、かぶせ、沈め折り、層選択、折り重ねを含め、物理操作は実行せず、未対応操作は注意付き説明テンプレートにする。

IPCは2 MiB以下のstrict DTOだけを受理する。未知field、不正ID、順序逆転、欠落・再登場source、不連続chunk、project instance・project ID・revisionの不一致を拒否する。preview中にproject、revision、技法文書または選択技法が変わった場合はstaleとして確定できない。取消と失敗ではtimeline、revision、dirty、Undo/Redoを一切変更しない。

## 履歴と保存

次の操作を`EditorState`の通常履歴へ統合する。

- `AddInstructionStep`
- `AppendInstructionSteps`
- `UpdateInstructionStepMetadata`
- `ReplaceInstructionStepPose`
- `RemoveInstructionStep`
- `MoveInstructionStep`

候補タイムライン全体を検証してから一括置換し、失敗時はpattern、paper、timeline、revision、Undo/Redoを変えない。手順編集もdirty判定へ含め、保存内容までUndoした場合はdirtyを解除する。

履歴entryはタイムライン全体を保持せず、追加ID、変更前metadata、変更前pose、削除stepと元index、移動前indexだけを保存する。候補全体の検証は適用時とUndo時のどちらでも維持する。Editor全体のUndo/Redoは最新128件を固定安全上限とし、利用者が件数または容量を設定する機能は別途実装する。

`AppendInstructionSteps`は説明案全体を一revision・一履歴entryで末尾へ追加する。Undoは記録されたID列が現在timelineの完全なsuffixである場合だけ全件を除去し、Redoは同じ順序で全件を戻す。suffixが一致しない履歴はfail-closedで拒否する。

`ProjectDocument`のformat versionは1を維持し、`instruction_timeline`をdefault付きfieldとして追加する。旧v1は空タイムラインとして読める。

非空タイムラインを持つ`.ori2`はmanifestへ`instruction_timeline_v1`を必須機能として記録する。新アプリはこの既知機能を受理し、旧アプリは未対応機能としてファイルを拒否するため、手順を認識せず再保存して失うことを防げる。未知の必須機能は引き続き拒否する。

説明専用手順を一件でも持つ`.ori2`とProject Folderは、さらに`declarative_instruction_steps_v1`を必須機能として記録する。この宣言を欠く内容は読込を拒否する。旧版は未知の必須機能として安全に拒否し、説明専用手順を黙って姿勢手順へ解釈したり消失させたりしない。

## 段階再生

再生状態は次の順で遷移する。

```text
idle
→ applying
→ 3D実適用姿勢の一致確認
→ holding
→ applying(next)
→ complete
```

project、revision、fold model、手動3D姿勢、性能テスト、ファイル操作、画面表示状態が変わった場合、または姿勢適用が失敗した場合は停止する。再生自体はプロジェクトを編集せず、Undo/Redoやdirtyへ影響しない。

`declarative_only_v1`を単独で「3Dに表示」しようとした場合は、3D姿勢がないことを明示して適用しない。段階再生では説明専用手順を物理適用対象から除外し、残る実姿勢手順だけを元の相対順序で再生する。画面に示す手順番号と選択位置は元timelineの番号を維持し、説明専用手順しかない場合は再生を開始しない。先頭が説明専用の場合の直接表示操作は「最初の実姿勢手順」を明示して対象とする。

## 上限

| 対象 | 上限 |
|---|---:|
| 手順数 | 512 |
| 一手順のヒンジ数 | 10,000 |
| タイムライン全体の角度record数 | 100,000 |
| タイトル | 120文字 |
| 説明 | 4,000文字 |
| 注意事項 | 2,000文字 |
| 表示時間 | 100～600,000 ms |
| 角度 | 0～180度の有限値 |

タイトルは空と制御文字を許可しない。説明と注意事項は改行とtab以外の制御文字を許可しない。

## 手順書き出し

現在の折りモデル指紋と一致する全手順を、A4縦の複数ページPDF 1.7またはSVGページ画像ZIPとして書き出せる。各手順は新しいページから開始し、収まらない説明・注意事項は省略せず継続ページへ送る。投影、資源上限、警告、stage、保存契約は[折り手順書き出し契約](instruction-export-contract.md)を正本とする。

説明専用手順はstale姿勢検査とkinematic solveの対象外であり、図の代わりに「説明専用・3D姿勢・物理操作なし」のplaceholderを描く。タイトル、説明、注意事項、時間はPDF/SVGの共通layoutへそのまま保持する。

## 未実装

- 複数ヒンジ間の滑らかな連続経路と、その衝突安全認定
- 手、指、把持位置、押さえ位置、持ち替え
- カメラ位置、矢印、注目箇所
- 3D操作の自動記録、手順の自動分割・結合
- 名前付き技法を3D運動へ自動変換・実行する機能
- 連続経路を補間するアプリ内アニメーションと動画出力
- 編集履歴そのものの`.ori2`永続化
