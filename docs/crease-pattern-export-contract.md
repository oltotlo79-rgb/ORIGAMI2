# 展開図書き出し契約

## 1. 目的と準拠先

本書は、ORIGAMI2の現在の一枚紙プロジェクトからFOLD 1.2、静的SVG、PDF 1.7の一枚展開図、またはDXF AC1021を書き出す範囲、情報損失の確認、資源上限、native保存、失敗時の状態保証を定める。本契約の4形式を実装することで、要件IO-006に列挙された展開図書き出し形式を満たす。複数ページの折り図・手順画像PDFは要件INS-010、OBJ・STL・glTFの完成形3D出力は要件IO-007であり、本契約とは別に実装・判定する。

意味と構文の基準は次とする。

- [FOLD file format specification 1.2](https://edemaine.github.io/fold/doc/spec.html)
- [W3C SVG 2](https://www.w3.org/TR/SVG2/)
- [ISO 32000-1:2008（PDF 1.7）](https://www.iso.org/standard/51502.html)
- [Adobe PDF Reference 1.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf)
- [Autodesk ASCII DXF Files](https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-DXF/files/GUID-20172853-157D-4024-8E64-32F3BD64F883.htm)
- [Autodesk General DXF File Structure](https://help.autodesk.com/cloudhelp/2019/ENU/AutoCAD-DXF/files/GUID-D939EA11-0CEC-4636-91A8-756640A031D3.htm)
- [Autodesk HEADER Section Group Codes](https://help.autodesk.com/cloudhelp/2021/ENU/AutoCAD-DXF/files/GUID-A85E8E67-27CD-4C59-BE61-4DC9FADBE74A.htm)
- [Autodesk Group Code Value Types Reference](https://help.autodesk.com/cloudhelp/2019/ENU/AutoCAD-DXF/files/GUID-2553CF98-44F6-4828-82DD-FE3BC7448113.htm)
- [Autodesk LUPREC System Variable](https://help.autodesk.com/cloudhelp/2022/ENU/AutoCAD-Core/files/GUID-5FFF39D6-EFC7-49F5-B56A-6023EB5C0DE7.htm)
- [IANA Media Types](https://www.iana.org/assignments/media-types/media-types.xhtml)

## 2. 利用者経路

```text
現在projectのID・revisionと形式を指定
  → Rustで一枚紙・展開図・作業量を検証
  → Rust内でFOLD/SVG/PDF/DXF bytesを決定論的に生成
  → 最新1世代だけをimmutable stage
  → UIへopaque ID・件数・サイズ・警告だけを返す
  → 利用者が形式と情報損失を確認
  → native保存ダイアログで保存先を選択
  → project instance・ID・revision・stage世代を再照合
  → 保存先と同じdirectoryで一時保存・同期・再読込照合
  → 原子的に置換
```

- 入口は編集画面上部の「書出し」とする。初期形式はFOLD 1.2で、確認画面内からFOLD、SVG、PDF、DXFを切り替えられる。
- 確認画面は形式、保存名候補、byte数、頂点・辺数、線種別本数、Cut有無、固定元revisionを表示する。
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
- SVG、PDF、DXFは辺から参照されない頂点を表現しないため、未参照頂点が一つでもあれば出力を拒否する。FOLDは頂点配列として保持できる。

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

SVGの色と線パターンは次の固定表現とする。2D Canvas、FOLD取込プレビュー、
SVG取込後の線種プレビューも、画面解像度に合わせてdash長だけを調整しながら、
`solid / dash-dot / dash / dot / dash-dot-dot`の意味を同じ順序で使う。色をすべて
黒へ置き換えても5つのpattern signatureが一意でなければならず、色だけを線種判別の
根拠にしない。

| 線種 | stroke | dash配列 | line cap | 白黒時の意味 |
|---|---|---|---|---|
| Boundary | `#111111` | solid | butt | 実線 |
| Mountain | `#d32f2f` | `[6, 2, 1, 2]` | butt | 一点鎖線 |
| Valley | `#1976d2` | `[3, 1.5]` | butt | 破線 |
| Auxiliary | `#757575` | `[0.5, 1.5]` | round | 点線 |
| Cut | `#000000` | `[8, 2, 1, 2, 1, 2]` | butt | 二点鎖線 |

## 6. PDF 1.7一枚展開図出力

PDFは`application/pdf`として、次の一ページだけのベクター展開図を出力する。このPDFは印刷・共有用の一枚展開図であり、工程、矢印、説明文、完成図を並べる折り図・折り手順PDFではない。

- headerは`%PDF-1.7`とし、PDF 1.7のclassic cross-reference tableを使う。ページ数は常に1とする。
- project座標の1 mmを`72 / 25.4` pointへ変換し、拡大縮小、snap、線の追加・削除を行わない。用紙外周を含む全辺のaxis-aligned boundsを配置基準とし、上下左右へ各10 mmの余白を置く。
- `/MediaBox`と`/CropBox`は同じ値とし、幅と高さをそれぞれ`(展開図bounds + 20 mm) × 72 / 25.4` pointにする。既定の1 user unit = 1 pointを使い、`/UserUnit`は指定しない。
- catalogの`/ViewerPreferences`へ`/PrintScaling /None`を設定する。これはviewerへ自動拡縮をしないよう指示するものであり、利用者にも印刷倍率100%・「用紙に合わせる」無効を案内する。プリンター機構による余白・倍率差までは保証しない。
- pageの幅または高さが14,400 pointを超える場合は出力を拒否する。上限と同じ14,400 pointは許可する。
- PDF real number tokenはASCII 64文字以下とし、超える値は丸めや切り詰めで収めず出力を拒否する。
- strokeはすべて`DeviceGray`の黒、塗りはなしとする。font、text描画、画像、script、action、attachment、外部resourceは出力しない。
- 作品名はdocument information dictionaryの`/Title`へUTF-16BE hexadecimal stringとして格納し、PDF構文を作品名から生成しない。`/Creator`と`/Producer`は固定値とし、日時、timezone、利用者名、host名、pathを含めない。

線種の固定表現は次のとおりとする。線幅は物理point、dash配列は物理mm、dash phaseは0である。

| 線種 | 線幅 | dash配列 | line cap |
|---|---:|---|---|
| Boundary | 0.50 pt | solid | butt |
| Mountain | 0.35 pt | `[6, 2, 1, 2]` | butt |
| Valley | 0.35 pt | `[3, 1.5]` | butt |
| Auxiliary | 0.25 pt | `[0.5, 1.5]` | round |
| Cut | 0.60 pt | `[8, 2, 1, 2, 1, 2]` | butt |

- PDFは頂点単独の表現を持たないため、未参照頂点が一つでもあれば黙って削除せず出力を拒否する。
- 辺の向きをcanonical化し、`Boundary`、`Mountain`、`Valley`、`Auxiliary`、`Cut`の順、同一線種内は始点・終点座標順に並べる。object番号、dictionary、数値表現、改行、cross-reference offsetも固定し、同じsnapshot、形式、作品名から完全に同じbytesを生成する。

## 7. DXF AC1021出力

DXFは`image/vnd.dxf`として、AutoCAD 2007相当の`AC1021`テキスト形式を出力する。

- UTF-8、BOMなしとし、group codeと値をそれぞれ独立した一行へ出力する。すべての行末をCRLFとし、最後の`0` / `EOF` pairの後にもCRLFを置く。
- headerへ`$ACADVER=AC1021`、`$INSUNITS=4`、`$MEASUREMENT=1`、`$LUNITS=2`、`$LUPREC=8`、`$LTSCALE=1`、`$EXTNAMES=0`、`$HANDSEED=10`を設定する。`$LUPREC`はAutoCADの有効範囲0〜8の上限であり、座標の内部精度は低下させない。`$EXTMIN` / `$EXTMAX`は全描画辺のbounds、`$LIMMIN` / `$LIMMAX`は用紙外周のboundsとし、3D成分は0にする。座標、drawing extents、paper limits、線種pattern長はmmであり、拡大縮小、座標原点の移動、Y軸反転、snapを行わない。
- 各辺を一つの`LINE` entityとして出力する。各座標の`-0`を`0`へ正規化し、両端を`(x, y)`の辞書順で小さい方から始点へ揃えた後、`Boundary`、`Mountain`、`Valley`、`Auxiliary`、`Cut`の順、同一線種内は始点・終点座標順に並べる。面、polyline、spline、hatch、寸法、block、3D entityを推測生成しない。
- `LINE` entityのZ座標は省略し、DXFの既定値0とする。線色と線種はentityへ重複記録せず、所属layerから継承させる。

5個のsemantic layer、ACI色、line typeは次の固定値とする。DXF構造上の既定layer `0`は別に定義できるが、展開図の`LINE` entityには使わない。pattern要素はmmで、正値は描画、負値は空白、0はdotを表す。

| 線種 | layer | ACI | line type | pattern合計 / 要素 |
|---|---|---:|---|---|
| Boundary | `ORIGAMI_BOUNDARY` | 7 | `CONTINUOUS` | solid |
| Mountain | `ORIGAMI_MOUNTAIN` | 1 | `ORI_MOUNTAIN` | 12 / `[8, -2, 0, -2]` |
| Valley | `ORIGAMI_VALLEY` | 5 | `ORI_VALLEY` | 9 / `[6, -3]` |
| Auxiliary | `ORIGAMI_AUXILIARY` | 8 | `ORI_AUXILIARY` | 2 / `[0, -2]` |
| Cut | `ORIGAMI_CUT` | 6 | `ORI_CUT` | 14 / `[8, -2, 2, -2]` |

- 作品名はgroup code `999`のcommentだけへ格納する。共通上限の512 Unicode scalar valueかつ2,048 UTF-8 bytes以下でなければならない。UTF-8のUnicode scalar境界で決定論的に分割し、各title payloadを224 bytes以下、最大10 pairとする。chunkには連番と総数を固定幅で付け、出力順にpayloadを連結すると元の作品名へ戻るようにする。固定prefixと連番を含むgroup value全体は最大245 bytesであり、DXF stringの255 bytes未満に収める。
- 空の作品名ではtitle commentを出力しない。固定のcreator・形式commentはtitle commentと分離し、title chunk数へ数えない。
- 作品名にLF、CR、NUL、tabを含むUnicode制御文字が一つでもあれば、置換やescapeを行わず出力を拒否する。改行を含む作品名をgroup codeやvalueとして解釈させない。
- group pairは「group code一行と値一行」の組を1 pairと数え、header、table、comment、entity、終端を含む全pairを100,000以下とする。100,000 pairは許可し、超過時は部分出力せず拒否する。
- DXF real number valueはASCII 64文字以下とし、超える値は丸めや切り詰めで収めず出力を拒否する。
- 未参照頂点が一つでもあれば、黙って削除せず出力を拒否する。
- section、table、layer、line type、handle、entityの順序と数値表現を固定し、locale、日時、random値、GUID、利用者名、host名、pathを含めない。canonicalな出力はedge・vertex配列順、endpointの向き、entity UUIDに依存せず、同じ幾何、形式、作品名から完全に同じUTF-8 bytesを生成する。

## 8. 情報損失

確認画面には、実際のproject状態と形式に応じて少なくとも次を表示する。

- 紙の表裏色、厚み、texture
- ORIGAMI2の頂点・辺ID、編集履歴、選択状態
- 現在の3D表示姿勢とcamera状態
- 1件以上存在する折り手順
- `cutting_allowed=true`だがCutがない場合の切断許可設定

形式ごとの保持範囲と追加警告は次のとおりとする。

- FOLDとSVGは2D mm座標、外周辺、5線種、作品名、Cut有無を機械可読に保持するため、これらを情報損失として扱わない。
- PDFは長さと形、外周、5線種とCutの視覚表現、作品名metadataを保持する。一方、原点を余白内へ移し、Y軸方向をPDF座標へ合わせるため元の絶対座標を保持せず、線種・頂点構造も機械可読に保持しない。この2点と、losslessな再取込ができないことを警告する。
- PDFの実寸印刷には印刷倍率100%と自動拡縮無効が必要であることを警告する。
- DXFは絶対mm座標、外周、5線種を表す`ORIGAMI_*` layer、作品名comment、Cut有無を保持する。ただし、layerの意味はORIGAMI2固有で一般CADが折り紙の意味として解釈する保証がなく、group code `999` commentはCADで保持・表示されない場合があることを警告する。

## 9. 資源上限

既定上限は次のとおりとし、上限を超えた入力は部分出力せず拒否する。

| 項目 | 上限 |
|---|---:|
| 出力bytes | 16 MiB |
| 頂点 | 10,000 |
| 辺 | 10,000 |
| 用紙外周頂点 | 1,414 |
| broad-phase交差候補 | 1,000,000 |
| 出力title | Unicode scalar value 512 |
| PDF page幅・高さ | 各14,400 pt |
| PDF real number token | ASCII 64文字 |
| DXF group pair | 100,000 |
| DXF real number value | ASCII 64文字 |
| DXF title UTF-8 bytes | 2,048 |
| DXF title comment | 10 pair、各payload 224 UTF-8 bytes（prefix込みvalueは最大245 bytes） |
| native active-edge包含判定 | 1,000,000 |

- 件数と交差候補は高コストの幾何検証・serializationより先に検査する。
- SVGの頂点参照と辺生成は`O(V + E)`、交差候補の検査は上表の明示上限内にする。
- byte上限はserialization後のbytesへ適用する。FOLD、SVG、DXFはUTF-8、PDFはbinary bytesとして数え、16 MiBを超えた場合は切り詰めない。

## 10. native stageとIPC境界

- WebViewへ返すのはopaque `export_id`、project ID、revision、形式、sanitize済み保存名候補、byte数、頂点・辺・線種別件数、Cut有無、固定した日本語警告だけとする。
- raw FOLD JSON、raw SVG、raw PDF、raw DXF、実保存path、現在project path、非永続instance ID、ファイルhandleをWebViewへ渡さない。
- stageは生成ID、project instance・ID・revision、形式、生成済みbytes、表示metadataを保持し、同時に最新1世代だけを有効にする。
- 古い生成が新しい生成より後に完了してもstageを上書きしない。旧tokenの取消が新しいstageを破棄してはならない。
- 保存commandは`export_id`、期待project ID・revision、警告確認flagだけを受け取り、形式、bytes、pathをUIから受け取らない。
- native保存ダイアログのfilterと最終拡張子はstageした形式から決め、利用者が異なる拡張子を入力した場合は`.fold`、`.svg`、`.pdf`、`.dxf`のうちstageした形式の拡張子へ正規化する。
- 保存名候補は先頭80文字を上限とし、制御文字とWindowsで使用できない`< > : " / \ | ? *`を`_`へ置換する。空、空白、末尾dotだけの場合は`Untitled`を使う。

## 11. 原子的保存と状態保証

- 選択先と同じdirectoryへ衝突しない`create_new`一時ファイルを作る。
- 全bytesを書き、fileを同期し、同じhandleを先頭へ戻して再読込し、stage bytesと完全一致することを確認する。
- OS dialogで利用者が正しい拡張子の保存先を確認した場合は原子的置換を許可する。拡張子を補正した保存先はatomic create-newへ固定し、Windowsでは検証したopen handle自体を`FILE_RENAME_INFO`へ渡して`ReplaceIfExists = false`、POSIXでは同一directoryのstageから排他的なhard linkを作成してstage名を外す。事前確認後に補正先が作られても既存内容を置換しない。POSIXではpublish前の親directory syncだけを失敗として返し、確定後のsyncはbest effortとする。見える出力先が既に変わった後で通常の保存失敗を返してstageを誤って再試行させない。
- 既存通常ファイル以外を走査・削除しない。確定前の失敗ではRAIIで今回の一時ファイルだけを削除する。
- dialog取消、I/O失敗、stale project、未知・旧token、未確認警告、生成失敗ではprojectを変更しない。
- 保存失敗ではstageを保持して再試行可能とする。成功時だけstageを一度消費し、同じtokenの再保存を拒否する。
- filesystem error本文、path、生成内容をIPC errorへ含めず、利用者向けの固定した日本語分類へ退避する。

## 12. 受入試験

- FOLD、SVG、PDF、DXFを同じsnapshotからそれぞれ2回生成し、形式ごとにbytesが一致する。
- FOLDを既存FOLD importerへ倍率1.0で戻し、座標、無向辺、5線種、外周、Cutを比較する。
- SVGを既存SVG importerへ倍率1.0と`data-origami-kind` mappingで戻し、同じ項目を比較する。
- PDFを構文解析し、version 1.7、一ページ、実寸変換、四辺10 mm余白、`PrintScaling=None`、14,400 point上限、黒一色、固定線幅・dash・cap、titleの安全な格納を確認する。
- DXFをgroup pairとして再解析し、`AC1021`、UTF-8、BOMなし、全行CRLF、固定header、mm単位、5個のsemantic layerと固定line type、canonical順の`LINE` entity、title commentの復元を確認する。
- JSON/XML構造を含む作品名がdataとしてescapeされ、scriptや別要素にならない。
- 非有限値、欠落端点、重複、交差、外周不整合、切断禁止、SVG/PDF/DXFの未参照頂点、PDFのpage・real number token上限、DXFのgroup pair・real number value上限、各共通上限の超過を拒否する。
- DXFのedge・vertex配列順、endpointの向き、UUIDだけを変更してもbytesが変わらず、座標または線種を変更するとbytesが変わる。
- LF、CRその他の制御文字を含むDXF作品名を拒否し、group code、section、entityを注入できない。
- preview DTOにbytes、content、pathが含まれない。
- token改変、別project、別revision、別instance、旧世代、保存済みtokenを拒否する。
- 保存先取消は無書込みでstageを保持し、確認画面から再試行できる。
- 拡張子補正の事前確認後にfile、directory、symlinkが補正先を占有しても既存内容、project、書き出しstageを維持し、今回の一時fileだけを清掃する。
- 成功時は選択先へstageと同じbytesだけが保存され、projectのdocument、dirty、revision、履歴が変化しない。
- 警告未確認ではnative保存ダイアログを開かない。
- 確認画面のTab/Shift+Tab、外部focus、IME中Escape、busy中の閉じる操作、失敗後のretryを実DOM eventで確認する。

## 13. 対象外

- 複数ページの折り図・手順画像PDF、PDF内の折り工程・矢印・説明文・完成図
- DXF取込、DXFの面・3D・寸法・block出力
- OBJ、STL、glTF、完成形3D、animation
- FOLDのface、層順、折り角度、3D/multi-frame
- SVGへの紙texture、3D姿勢、折り手順、編集metadataの埋込み
- ORIGAMI2 UUIDを外部形式からlosslessに戻すこと
- 形式間で表現不能な情報の近似生成
