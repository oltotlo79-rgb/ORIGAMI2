# 展開図書き出し契約

## 1. 目的と準拠先

本書は、ORIGAMI2の現在の一枚紙プロジェクトからFOLD 1.2または静的SVG展開図を書き出す範囲、情報損失の確認、資源上限、native保存、失敗時の状態保証を定める。PDF、DXF、折り図PDF、3D形式の出力は本契約の対象外であり、要件IO-006全体の完了を意味しない。

意味と構文の基準は次とする。

- [FOLD file format specification 1.2](https://edemaine.github.io/fold/doc/spec.html)
- [W3C SVG 2](https://www.w3.org/TR/SVG2/)

## 2. 利用者経路

```text
現在projectのID・revisionと形式を指定
  → Rustで一枚紙・展開図・作業量を検証
  → Rust内でFOLD/SVG bytesを決定論的に生成
  → 最新1世代だけをimmutable stage
  → UIへopaque ID・件数・サイズ・警告だけを返す
  → 利用者が形式と情報損失を確認
  → native保存ダイアログで保存先を選択
  → project instance・ID・revision・stage世代を再照合
  → 保存先と同じdirectoryで一時保存・同期・再読込照合
  → 原子的に置換
```

- 入口は編集画面上部の「書出し」とする。初期形式はFOLD 1.2で、確認画面内からSVGへ切り替えられる。
- 確認画面は形式、保存名候補、UTF-8 byte数、頂点・辺数、線種別本数、Cut有無、固定元revisionを表示する。
- 警告が1件以上ある場合、利用者が情報損失を確認するまで保存操作を有効にしない。native commandも同じ確認flagを再検証し、UIだけの制御に依存しない。
- 保存先選択の取消ではstageを保持して確認画面へ戻り、同じbytesを再試行できる。確認画面自体の取消、別形式のpreview、保存成功では対象stageを破棄する。
- 書き出しはprojectの保存先、dirty状態、保存済みbaseline、revision、Undo/Redo、選択状態を変更しない。

## 3. 共通入力条件

- 現在の`CreasePattern`と`Paper`をimmutable snapshotとして取得し、生成開始時の非永続project instance ID、永続project ID、revisionへ束縛する。
- 頂点座標と辺のspanは有限なbinary64でなければならない。
- 頂点ID、頂点位置、辺ID、無向辺はそれぞれ重複してはならない。辺の両端は存在し、異なる頂点でなければならない。
- 用紙外周は一つの単純閉路であり、対応する`Boundary`辺を過不足なく持たなければならない。
- 山折り、谷折り、Cutの中点は用紙内部になければならない。
- Cutがある場合はprojectの`cutting_allowed=true`でなければならない。
- 不正な形状を近似、snap、交差分割、隙間補修、線の削除によって出力可能に変換しない。

## 4. FOLD 1.2出力

FOLDはUTF-8 JSONとして次を出力する。

| FOLD field | ORIGAMI2の値 |
|---|---|
| `file_spec` | `1.2` |
| `file_creator` | `"ORIGAMI2"` |
| `file_title` | 現在の作品名 |
| `file_classes` | `["singleModel"]` |
| `frame_classes` | `["creasePattern"]` |
| `frame_attributes` | Cutなしは`["2D"]`、Cutありは`["2D", "cuts"]` |
| `frame_unit` | `"mm"` |
| `vertices_coords` | projectの2D mm座標。配列順を保持 |
| `edges_vertices` | 頂点配列indexの組。辺配列順を保持 |
| `edges_assignment` | `Boundary=B`、`Mountain=M`、`Valley=V`、`Auxiliary=F`、`Cut=C` |

- 折り角度を推測しないため`edges_foldAngle`を出力しない。
- 面、層順、3D座標、複数frameを生成しない。
- 現在のFOLD importerへ戻した場合、`file_creator`、`file_classes`、`frame_attributes`はprojectへ永続化しないmetadataとして警告される。geometry、作品名、単位、5線種の往復には影響しない。
- JSON文字列のescapeは`serde_json`で行い、作品名を構造として解釈しない。
- 同じsnapshot、形式、作品名からは同じbytesを生成する。entity UUID自体はFOLDへ埋め込まない。

## 5. SVG出力

SVGはUTF-8 XMLとして次の静的subsetだけを出力する。

- rootは`xmlns="http://www.w3.org/2000/svg"`、`version="1.1"`、用紙外周の有限なaxis-aligned boundsを`viewBox`へ設定する。
- `width`と`height`は同じboundsを`mm`単位で指定し、再取込時の初期倍率を`1 SVG unit = 1 mm`にできるようにする。
- 作品名は`title`へXML textとしてescapeして出力する。
- 各辺は一つの`line`とし、座標をmmのまま出力する。
- 各`line`へ`data-origami-kind="boundary|mountain|valley|auxiliary|cut"`を付ける。色とdashは視覚上の補助であり、意味の正本はこの属性とする。
- script、event handler、animation、CSS、外部resource、`image`、`text`、`use`、foreign namespaceを出力しない。
- SVG直線subsetで保持できない未参照頂点がある場合、黙って削除せずSVG出力を拒否する。FOLDは頂点配列として保持できる。
- XML 1.0で許されない作品名の文字は拒否し、制御文字を置換して意味を変えない。

## 6. 情報損失

確認画面には、実際のproject状態と形式に応じて少なくとも次を表示する。

- 紙の表裏色、厚み、texture
- ORIGAMI2の頂点・辺ID、編集履歴、選択状態
- 現在の3D表示姿勢とcamera状態
- 1件以上存在する折り手順
- `cutting_allowed=true`だがCutがない場合の切断許可設定

FOLD/SVGへ保持する座標、外周、5線種、作品名、Cut有無を情報損失として扱わない。将来のexporterは形式固有の警告を追加し、同じ確認・native再検証経路を使う。

## 7. 資源上限

既定上限は次のとおりとし、上限を超えた入力は部分出力せず拒否する。

| 項目 | 上限 |
|---|---:|
| 出力bytes | 16 MiB |
| 頂点 | 10,000 |
| 辺 | 10,000 |
| 用紙外周頂点 | 1,414 |
| broad-phase交差候補 | 1,000,000 |
| 出力title | Unicode scalar value 512 |
| native active-edge包含判定 | 1,000,000 |

- 件数と交差候補は高コストの幾何検証・serializationより先に検査する。
- SVGの頂点参照と辺生成は`O(V + E)`、交差候補の検査は上表の明示上限内にする。
- byte上限はserialization後のUTF-8 bytesへ適用する。上限超過時に切り詰めない。

## 8. native stageとIPC境界

- WebViewへ返すのはopaque `export_id`、project ID、revision、形式、sanitize済み保存名候補、byte数、頂点・辺・線種別件数、Cut有無、固定した日本語警告だけとする。
- raw FOLD JSON、raw SVG、実保存path、現在project path、非永続instance ID、ファイルhandleをWebViewへ渡さない。
- stageは生成ID、project instance・ID・revision、形式、生成済みbytes、表示metadataを保持し、同時に最新1世代だけを有効にする。
- 古い生成が新しい生成より後に完了してもstageを上書きしない。旧tokenの取消が新しいstageを破棄してはならない。
- 保存commandは`export_id`、期待project ID・revision、警告確認flagだけを受け取り、形式、bytes、pathをUIから受け取らない。
- native保存ダイアログのfilterと最終拡張子はstageした形式から決め、利用者が異なる拡張子を入力した場合は`.fold`または`.svg`へ正規化する。
- 保存名候補は先頭80文字を上限とし、制御文字とWindowsで使用できない`< > : " / \ | ? *`を`_`へ置換する。空、空白、末尾dotだけの場合は`Untitled`を使う。

## 9. 原子的保存と状態保証

- 選択先と同じdirectoryへ衝突しない`create_new`一時ファイルを作る。
- 全bytesを書き、fileを同期し、同じhandleを先頭へ戻して再読込し、stage bytesと完全一致することを確認する。
- Windowsでは検証したopen handle自体を`FILE_RENAME_INFO`で置換し、path差替えを許さない。POSIXでは同一directory rename後に親directoryも同期する。
- 既存通常ファイル以外を走査・削除しない。確定前の失敗ではRAIIで今回の一時ファイルだけを削除する。
- dialog取消、I/O失敗、stale project、未知・旧token、未確認警告、生成失敗ではprojectを変更しない。
- 保存失敗ではstageを保持して再試行可能とする。成功時だけstageを一度消費し、同じtokenの再保存を拒否する。
- filesystem error本文、path、生成内容をIPC errorへ含めず、利用者向けの固定した日本語分類へ退避する。

## 10. 受入試験

- FOLDとSVGを同じsnapshotから2回生成し、bytesが一致する。
- FOLDを既存FOLD importerへ倍率1.0で戻し、座標、無向辺、5線種、外周、Cutを比較する。
- SVGを既存SVG importerへ倍率1.0と`data-origami-kind` mappingで戻し、同じ項目を比較する。
- JSON/XML構造を含む作品名がdataとしてescapeされ、scriptや別要素にならない。
- 非有限値、欠落端点、重複、交差、外周不整合、切断禁止、未参照SVG頂点、各上限超過を拒否する。
- preview DTOにbytes、content、pathが含まれない。
- token改変、別project、別revision、別instance、旧世代、保存済みtokenを拒否する。
- 保存先取消は無書込みでstageを保持し、確認画面から再試行できる。
- 成功時は選択先へstageと同じbytesだけが保存され、projectのdocument、dirty、revision、履歴が変化しない。
- 警告未確認ではnative保存ダイアログを開かない。
- 確認画面のTab/Shift+Tab、外部focus、IME中Escape、busy中の閉じる操作、失敗後のretryを実DOM eventで確認する。

## 11. 対象外

- PDF、DXF、折り図・手順画像PDF
- OBJ、STL、glTF、完成形3D、animation
- FOLDのface、層順、折り角度、3D/multi-frame
- SVGへの紙texture、3D姿勢、折り手順、編集metadataの埋込み
- ORIGAMI2 UUIDを外部形式からlosslessに戻すこと
- 形式間で表現不能な情報の近似生成
