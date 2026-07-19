# 編集履歴永続化V1

## 1. 目的と適用範囲

`editor_history_v1`は、通常の`.ori2`保存と再読込の間で次を保持する初版形式である。

- Undo stack
- Redo stack
- project/sessionに設定された履歴件数上限

この形式はHIS-002の通常保存経路と、dirty documentをcheckpointする
クラッシュ復旧経路の両方を対象とする。復旧archiveにも、そのcheckpoint時点の
Undo/Redo履歴と履歴件数上限を含める。保存path、保存済みbaseline、runtimeの
current 3D poseはどちらの履歴entryにも含めない。

## 2. `.ori2`コンテナ

履歴を持つcontainer v1は、次の固定順序の3 entryで構成する。

```text
manifest.json
project.json
editor-history.json
```

既定上限128件かつUndo/Redoがともに空の場合、writerは
`editor-history.json`を省略し、従来の2 entry archiveと
byte-for-byteで同じ出力を維持する。

履歴entryが存在する場合は、次をすべて満たさなければならない。

- `manifest.json.required_features`が`editor_history_v1`を含む。
- `manifest.json.editor_history.path`が正確に`editor-history.json`である。
- `manifest.json.editor_history.schema_version`が`1`である。
- manifestの非圧縮byte数と実entryのbyte数が一致する。
- manifestのSHA-256と実entryのSHA-256が一致する。
- 履歴envelope内の`project_sha256`と`project.json`のSHA-256が一致する。
- 履歴内の`project_id`と`project.json`のproject IDが一致する。

feature、manifest descriptor、実entryは三者同時に存在するか、三者とも
存在しないかのどちらかに限定する。document-only APIは履歴entryを含む
archiveを拒否し、履歴を暗黙に破棄して再保存してはならない。

履歴entryの非圧縮hard ceilingは64 MiBとする。callerが上限を緩和しても
この値を超えられない。archive全体には通常の`.ori2`上限も同時に適用する。

## 3. JSON envelope

`editor-history.json`は次のenvelopeを持つ。全objectは未知fieldを拒否する。

```json
{
  "project_sha256": "<project.jsonの小文字SHA-256>",
  "history": {
    "schema_version": 1,
    "project_id": "<canonical non-nil UUID>",
    "history_entry_limit": 128,
    "undo_stack": [
      {
        "forward": { "kind": "<command kind>" },
        "inverse": { "kind": "<inverse kind>" }
      }
    ],
    "redo_stack": []
  }
}
```

stackの配列順は古いentryから新しいentryとし、末尾を次に移動するentryとする。
Undo/Redoの各stackはそれぞれ`history_entry_limit`件以下でなければならない。
`history_entry_limit`は1以上128以下とする。

全indexは非負の`u32`で表す。全浮動小数点値は有限でなければならない。
commandの意味で符号付きゼロが区別される箇所はbit-exactに保持する。

## 4. V1 command vocabulary

`forward.kind`は次の22種だけを受理する。

| JSON kind |
|---|
| `add_vertex` |
| `move_vertex` |
| `remove_vertex` |
| `add_edge` |
| `remove_edge` |
| `set_cutting_allowed` |
| `update_paper_properties` |
| `set_length_display_unit` |
| `resize_rectangular_paper` |
| `split_edge` |
| `connect_edge_intersection` |
| `connect_t_junction` |
| `connect_intersection_cluster` |
| `split_boundary_edge` |
| `remove_boundary_vertex` |
| `add_geometric_constraint` |
| `remove_geometric_constraint` |
| `add_instruction_step` |
| `update_instruction_step_metadata` |
| `replace_instruction_step_pose` |
| `remove_instruction_step` |
| `move_instruction_step` |

runtimeへ新しいcommandを追加してもV1へ自動的には追加しない。V1変換は
runtime enumを網羅的にmatchし、未決定の永続化意味をcompile errorとして検出する。

## 5. V1 inverse vocabulary

`inverse.kind`は次の19種だけを受理する。

| JSON kind |
|---|
| `command` |
| `restore_vertex` |
| `restore_edge` |
| `restore_paper_properties` |
| `restore_length_display_unit` |
| `restore_vertex_positions` |
| `restore_boundary_split` |
| `restore_edge_split` |
| `restore_edge_intersection` |
| `restore_t_junction` |
| `restore_intersection_cluster` |
| `restore_boundary_vertex_removal` |
| `remove_added_geometric_constraint` |
| `restore_removed_geometric_constraint` |
| `remove_added_instruction_step` |
| `restore_instruction_step_metadata` |
| `restore_instruction_step_pose` |
| `restore_removed_instruction_step` |
| `restore_instruction_step_order` |

inverse内のvector index、ID、現在要素との対応は適用前に検証する。不正indexを
trusted runtime用のinverse適用処理へ渡してpanicさせてはならない。

## 6. 意味再認証

JSON構造、hashおよびIDの検証だけでは履歴を受理しない。読込時は次の順で
全entryを意味再認証する。

1. `project.json`から現在文書を構築し、bit-exactな期待値を保持する。
2. Undo stackを末尾から先頭へ、保存されたinverseで安全に巻き戻して基底文書を得る。
3. 基底文書からUndo stackのforward commandを先頭から再生する。
4. 再生時に生成されたcanonical inverseと保存inverseをbit-exactに比較する。
5. 再構築した現在文書と`project.json`をbit-exactに比較する。
6. 現在文書からRedo stackを実際のRedo適用順に全再生する。
7. 各Redo commandについても生成inverseと保存inverseをbit-exactに比較する。
8. desktop境界で、復元editorのcurrent endpointからUndoを末尾まで実行し、
   currentを含む全Undo到達endpointのinstruction pose topologyを検証する。
9. 同じcurrent endpointからRedoを末尾まで実行し、全Redo到達endpointの
   instruction pose topologyを検証する。

一つでも失敗した場合は履歴全体を拒否する。検証前のwire entryをlive stackへ
部分的に入れてはならない。通常open経路では既存project、保存path、履歴および
runtime poseを変更せずに失敗する。

全endpoint検証が必要なのは、最終documentでは古いfingerprintとして意図的に
許可されるinstruction poseが、UndoまたはRedo後に同じfingerprintのcurrent poseへ
戻る場合があるためである。最終documentだけの検証では、過去endpointの不正な
FaceIdまたはhinge registryを見逃す。Undo/Redoは各128件以下なので、検証対象は
current 1状態とUndo/Redo到達先合計最大256状態、総検証最大257状態で有界とする。

復元後のdocument revisionは`0`、runtime current 3D poseは`None`とする。
pose certificate、衝突certificateおよびその他のruntime authorityは永続化しない。
履歴entryがgeometry変更時にposeを破棄するか、非geometry変更時にposeを保持するかは、
再生した前後のfold-model fingerprintから再導出する。

## 7. 保存時の規則

- 保存対象project IDはnon-nilでなければならない。
- 現在文書と全履歴の数値は有限でなければならない。
- 保存前にも読込時と同じ意味再認証を行う。
- 通常保存はdocumentと履歴を同じimmutable snapshotから生成する。
- dirty documentの自動復旧checkpointもdocumentと履歴を同じimmutable snapshotから
  生成し、通常`.ori2`と同じstrict archive readerと意味再認証を通す。
- 明示保存と自動復旧checkpointは、公開前に全Undo/Redo到達endpointのinstruction
  pose topologyを検証する。不正endpointがある場合は固定categoryで保存を拒否する。
- 自動復旧checkpointはproject lock内ではdocumentと履歴のsnapshot取得だけを行い、
  lock解放後にdetached snapshotのcurrent 1状態と、全Undo・全Redoの到達先合計
  最大256状態（総検証最大257状態）を再構築・検証してから保存処理へ渡す。
- stageを通常readerで再読込し、byte照合後にだけ原子的に公開する。
- 保存失敗時は既存保存file、live project、両stack、revisionおよびruntime poseを
  変更しない。
- 履歴上限の変更は従来どおり作品編集commandではなく、revision、document、
  dirtyおよびruntime poseを変えないsession policy変更とする。history-only変更は
  自動保存を単独では起動しない。変更後の履歴と上限は、次に成功した明示保存、
  またはdocumentがdirtyな間に成功した自動復旧checkpointへ含める。

## 8. クラッシュ復旧への適用

自動復旧slotは`Ori2ProjectArchive`としてこの履歴entryを使用する。dirty documentの
checkpointを取得するとき、同じproject lock内でdocument、Undo/Redo両stackおよび
履歴件数上限を一つのsnapshotへ束縛する。書込み、再読込、hash検証および意味再認証は
通常の`.ori2`と同じ境界を使用する。到達endpointの再構築・検証は、UIを長時間
占有しないようproject lockを解放した後のdetached snapshotに対して実行する。

最後に永続化したdirty checkpointとの重複判定は、project instance、project ID、
revisionに加え、永続化対象のUndo/Redo両stack・履歴件数上限全体を決定論的JSONへ
直列化したSHA-256 digestへ束縛する。revisionを変えない履歴上限変更や履歴trimが
dirty中に発生した場合も、digestが異なれば次のtimer tickを重複扱いせず新しい
checkpointへ反映する。digestまで同一の場合だけ`Duplicate`としてI/Oを省略する。

復旧後は保存済みproject IDとcheckpoint時点のUndo/Redo・上限を維持しつつ、
fresh project instance、保存pathなし、revision 0、dirty、保存baselineなし、
runtime current 3D poseなしで開始する。履歴がないlegacy復旧archiveだけは
既定128件の空履歴として開く。

履歴だけが変わってdocumentがcleanな場合は、新しい復旧checkpointを作らない。
例えば履歴上限だけを変更した直後の異常終了では、そのhistory-only変更が直前の
復旧checkpointへ反映されていない場合がある。document dirtyを既存どおり作品の
未保存変更の指標として維持し、履歴永続化だけを理由に終了確認を発生させないための
意図した境界である。

復旧archiveの履歴がhash、project binding、core意味再認証または全到達endpointの
instruction pose topology検証に失敗した場合、文書だけを救済して開いてはならない。
候補全体を起動時から`invalid`として隔離・破棄workflowへ閉じ、未検証の履歴を
live projectへ入れない。HIS-002の通常再読込とHIS-005のクラッシュ復旧は、
それぞれの利用者経路で別々に受け入れを判定する。

## 9. 受入れ条件

HIS-002を実装済みと判定するには、core/formatsの検証だけでなく、Windows正式版の
利用者経路で次を満たす必要がある。

1. UndoとRedoの両方が非空で、履歴上限が既定値でないprojectを通常保存できる。
2. projectを閉じて通常openし、revision 0から保存前と同じUndo/Redo順序と上限を
   利用できる。
3. 復元したUndoとRedoの実行結果が保存前とbit-exactに一致する。
4. 既定128件の空履歴を持つprojectは従来の2 entry archiveと同じbytesを維持する。
5. legacyの2 entry archiveは既定128件の空履歴として開ける。
6. hash、project binding、schema、未知field、index、inverseまたは再生意味を
   改変した履歴を拒否し、既存projectを変更しない。
7. 履歴を持つarchiveを旧document-only保存経路で開いて暗黙に履歴を失わない。
8. dirty documentの復旧checkpointにも同じUndo/Redoと上限を保存し、復旧後に
   fresh instance、revision 0、dirtyのまま利用できる。
9. history-only変更がdocument dirtyを変えず、次の明示保存またはdirty documentの
   自動復旧checkpointでだけ永続化される。dirty中に履歴だけを変更した場合は、
   revisionが同じでも履歴全体のdigest差を検出して次のcheckpointへ反映する。
10. 最終documentではstaleだがUndoまたはRedo後にcurrentとなる不正instruction poseを
    持つ履歴を、通常openでは既存project不変で拒否し、復旧では起動時`invalid`へ
    分類し、明示保存と自動復旧checkpointでも公開しない。

## 10. 実装上の正本

- core wireと意味再認証:
  `crates/ori-core/src/editor/history_persistence.rs`
- `.ori2` envelope、hash、resource limitと互換境界:
  `crates/ori-formats/src/ori2.rs`
- native通常open/saveの原子性:
  `apps/desktop/src-tauri/src/project_persistence.rs`
- native自動復旧のsnapshot、重複抑止および復元:
  `apps/desktop/src-tauri/src/recovery.rs`

形式変更は既存V1の意味を暗黙に変更せず、新しいschemaまたはrequired featureとして
導入する。
