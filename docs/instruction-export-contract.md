# 折り手順書き出し契約

## 1. 目的と適用範囲

本書は、折り手順タイムラインから次の二形式をローカルで生成・保存する初版契約である。

- PDF 1.7の複数ページ折り図
- 手順ページごとの静的SVGを収めたZIP

対象は、現在のproject revisionに保存済みの手順、各手順の終端折り角度、作品名、手順名、説明、注意事項、所要時間、および現在の紙面・ヒンジ幾何である。生成にクラウドサービス、外部API、外部font、外部画像、利用状況送信を必要とせず、配布後のランニングコストを発生させない。

本機能は、[展開図書き出し契約](crease-pattern-export-contract.md)の一ページ展開図PDFとは別機能である。展開図PDFを手順書PDFへ暗黙に切り替えず、UIでも「展開図を書き出す」と「折り手順を書き出す」を分ける。

本契約の実装により、要件INS-010の「手順ごとの画像」と「PDF形式で出力」の部分を満たす。ただし、アプリ内アニメーションは本契約の対象外であるため、INS-010全体は部分実装のままとする。INS-004の作家指定camera・矢印・注目箇所と、INS-005の手・指・つまみ・押さえガイドも別契約である。

## 2. 初版の固定profile

初版の書き出しprofileを`instruction_export_v1`、投影profileを`orthographic_isometric_v1`とする。同一入力と同一optionからは、OS、GPU、display倍率、現在の3D viewport、現在のcamera、timezoneに関係なく同一bytesを生成しなければならない。

| 項目 | 初版の固定値 |
|---|---|
| PDF | PDF 1.7、`application/pdf`、拡張子`.pdf` |
| SVG page archive | ZIP、`application/zip`、拡張子`.zip` |
| 用紙 | A4縦、210 mm × 297 mm |
| 投影 | `orthographic_isometric_v1` |
| 背景 | 白 |
| font | 同梱Noto Sans JP、weight 400 |
| locale | 保存済み文字列をそのまま使用し、自動翻訳しない |
| 時刻・乱数 | 出力へ含めない |
| 外部resource | 取得・参照しない |

profileの意味を変更する場合は、同名profileの挙動を変更せず、新しいprofile名を追加する。

## 3. 入力snapshotと生成前検証

### 3.1 固定する入力

生成開始時にnative Rust側で次を一つのimmutable snapshotとして固定する。

- 起動ごとに異なるproject instance ID
- 永続project ID
- project revision
- 現在の折りモデル指紋
- 現在の面、境界、ヒンジ、線種、表裏色
- 永続化済みの手順配列と各手順の絶対ヒンジ姿勢
- 出力形式とprofile
- 固定layout option

WebViewがproject JSON、折り角度配列、font bytes、生成済みPDF/SVG/ZIP bytesを再送して生成内容を決めてはならない。

### 3.2 受理条件

次をすべて満たす場合だけ生成する。

1. 手順が一件以上存在する。
2. [折り手順タイムライン初期設計](instruction-timeline-design.md)の件数、文字列、有限数、ID、角度、継続時間の検証を満たす。
3. 現在の面・境界・ヒンジが、初版rendererの対応topologyである。
4. 各手順が現在の全ヒンジを重複なく一度ずつ含む。
5. 全手順の折りモデル指紋が現在の折りモデル指紋と一致する。
6. font assetとlicense assetのSHA-256が本契約の固定値と一致する。
7. 全入力と予測出力が第9章の資源上限内である。

初版で対応するtopologyは、平面状態、または一つの連結成分からなり面間ヒンジgraphが木である状態とする。cycle、切断により複数成分となった状態、非多様体、欠損参照、自己矛盾する面境界は`unsupported_topology`として全体を拒否する。将来対応範囲を増やす場合も、既存profileの結果を変えない。

一件でもstaleな手順がある場合は、手順を飛ばす、古い幾何で描く、最新手順だけを採用する、途中まで出力する、といった回復を行わず、書き出し全体を拒否する。UIへは固定categoryと上限付きの手順indexだけを返し、作品内容や内部pathをerror文字列へ含めない。

## 4. canonical instruction plan

### 4.1 一つの正本

検証後、serializerから独立した`CanonicalInstructionPlanV1`を一度だけ生成する。PDFとSVG page ZIPは必ずこの同一planを消費する。形式ごとに投影、改行、page break、文字計測、図形順序、変更ヒンジ判定を再計算してはならない。

planには少なくとも次を含める。

- profile名と投影profile名
- A4上の固定領域、余白、座標系
- page順、page種別、対応step index、continuation番号
- 配置済みtext run、開始位置、font size、色、および固定fontによるadvance計測結果
- 面polygon、表裏、depth順、clip範囲
- ヒンジ線種、変更有無、線幅、dash
- 全page数と、生成時に照合する全glyph数・投影点visit数
- 固定warning category

PDFとSVG ZIPのpage数、step開始page、continuation page、文字位置、図形bounds、面順、ヒンジ順、変更ヒンジはplan上で同一でなければならない。

### 4.2 `orthographic_isometric_v1`

投影はCPUだけで計算し、Three.js scene、GPU raster結果、viewport size、device pixel ratio、利用者の現在camera、OS graphics APIを参照しない。

- 初期紙面を`(x, 0, z)`へ置き、右方向を`+x`、紙面上方向を`+z`、表側法線を`+y`とする。
- viewer方向を`(0.5773502691896258, 0.5773502691896258, 0.5773502691896258)`、画面右方向を`(0.7071067811865476, 0, -0.7071067811865476)`、画面上方向を`(-0.4082482904638631, 0.8164965809277261, -0.4082482904638631)`に固定する。
- world点`p`の画面座標を`(dot(p, right), dot(p, up))`、depthを`dot(p, viewer)`とする。実行時に三角関数やvector正規化で基底を再生成しない。
- 右手座標系の固定orthographic isometric基底をprofileに定数として持つ。
- 全手順の全対応面を同じ基底へ投影し、全手順共通のglobal boundsを求める。
- global boundsに固定paddingを加え、全pageで同じscaleと中心を使用する。
- 3D変換、depth、2D座標は有限値だけを受理する。
- 比較・整列・serialization前の投影値は`1e-9`単位へ丸め、負のzeroをzeroへ正規化する。
- 面は平均depthとcanonical geometry keyで決定論的にpainter orderへ並べる。
- 同一depthの順序にUUID、hash map反復順、入力配列の偶然の順序を使わない。
- 面の表裏色は保存済みのsolid colorを白背景へ合成する。texture、照明、影、透明効果は描かない。
- 面外周と山折り・谷折りヒンジはprofile固定の線幅、色、dashへ写像する。初版で受理しない切断・複数成分は描画せず、topology検証で全体を拒否する。
- 一つ前の手順から角度が変わるヒンジを変更ヒンジとして強調する。最初の手順は全ヒンジの0度状態と比較する。

現在の3D表示と同じ見た目になることは保証しない。texture等が省略される場合は、保存dialogより前のpreviewに固定warningを表示する。

## 5. A4 page layout

### 5.1 step開始規則

各stepは必ず新しいA4 pageから開始する。前stepのpageに空きがあっても、次stepを詰め込まない。step開始pageには固定順で次を置く。

1. 作品名
2. `手順 n / N`
3. 手順名
4. `orthographic_isometric_v1`によるcanonical diagram
5. 所要時間
6. 変更ヒンジの要約
7. 説明
8. 注意事項
9. page番号

作品名や手順名が空であることを永続modelが許す場合は、固定の無題表記を用いる。表示文字を現在のUI localeで差し替えず、profileで定義した固定日本語labelを使用する。

### 5.2 長文継続

説明または注意事項がstep開始pageに収まらない場合は、同じstepのcontinuation pageを必要数追加する。

- 長文を省略、末尾三点、縮小font、途中切断してはならない。
- continuation pageには作品名、`手順 n / N（続き k）`、継続するsection名、本文、page番号を置く。
- 説明をすべて流した後に注意事項を流す。
- 改行と禁則処理はprofile固定とし、OSのtext layout APIへ委譲しない。
- 一つのglyph clusterをpage間で分割しない。
- 次stepはcontinuationの終了後、必ず新しいpageから始める。

page余白、各領域の位置、font size、line height、折返し幅、図の最大bounds、線幅はprofile定数とする。出力形式による余白調整を禁止する。

### 5.3 固定layout metric

canonical planの座標原点は用紙左上、`+x`は右、`+y`は下、単位はPDF point（`72 / 25.4` point/mm）とする。PDFとSVGは同じplan座標を使い、SVGは`width="210mm" height="297mm"`とA4 point座標の固定`viewBox`を持つ。

| 項目 | 固定値 |
|---|---:|
| 用紙 | `595.2755905511812 × 841.8897637795277 pt` |
| 左右・開始上余白 | `36 pt`（12.7 mm） |
| 本文右端 | `559.2755905511812 pt` |
| 本文下端 | `795 pt` |
| diagram | 幅`523.2755905511812 pt`、高さ`360 pt` |
| diagram内padding | `18 pt` |
| diagram内legend領域 | `26 pt` |
| footer baseline | `821 pt` |

作品名と手順名は固定幅で必要行数だけ折り返すため、diagramの開始Yはheaderの実際の行数から決定する。diagramの幅・高さ、本文下端、footer位置は変えない。global boundsを縦横比を保ってdiagram内へ収め、全pageで同じscaleと中心を使う。退化して幅または高さがzeroになる投影は拒否する。

| 文字種 | font size | line height | alignment |
|---|---:|---:|---|
| 作品名 | `9 pt` | `12 pt` | 左 |
| 手順名 | `17 pt` | `22 pt` | 左 |
| continuation手順名 | `14 pt` | `19 pt` | 左 |
| metadata | `9 pt` | `18 pt` | 左 |
| section見出し | `11.5 pt` | `17 pt` | 左 |
| 説明・注意事項 | `10.5 pt` | `15.5 pt` | 左 |
| footer | `7.5–8 pt` | 固定baseline | 左・page番号は右 |

折返しは保存済み改行を先に適用し、それ以外はUnicode scalarの並びをfont advanceの累積が領域幅を超える直前で文字単位に折り返す。tabは固定4 spaceへ展開し、spaceを自動増減して両端揃えにしない。初版は言語依存の禁則・shaping engineを持たず、fontに存在しないscalarは全体を拒否する。

### 5.4 初版の図記号

線幅とdashもmm単位とし、line capは`round`、line joinは`round`、dash offsetはzeroに固定する。

| 意味 | 色 | 線幅 | dash |
|---|---|---:|---|
| 面外周 | `#202124` | `0.7 pt` | solid |
| 山折り | `#D93025` | `1.0 pt` | `11.338583,2.834646,2.834646,2.834646 pt` |
| 谷折り | `#1A73E8` | `1.0 pt` | `5.669291,2.834646 pt` |
| 変更ヒンジのhalo | `#F9AB00` | `3.4 pt` | solid |

変更ヒンジはhaloを先に描き、その上に本来の山折り/谷折り線を描く。変更ヒンジが一件以上あるstepではmetadataに`今回動かす折り線: k本`を表示し、色だけに依存しない。初版は折り方向矢印、回転矢印、拡大図、完成markを自動生成しない。

## 6. PDF 1.7契約

PDFは`%PDF-1.7`で始まる静的documentとし、各pageの`MediaBox`と`CropBox`をA4縦に固定する。

- canonical planのpageを同じ順序で一pageずつ出力する。
- 面、ヒンジ、矢印代替強調、文字glyphをvector pathとして出力する。
- JavaScript、action、launch、attachment、embedded file、form、動画、音声、外部URL、外部font、外部画像を含めない。
- object番号、resource名、dictionary key、path、xref、数値表現をcanonical順にする。
- creation/modification日時、timezone、利用者名、端末名、保存path、project path、乱数、生成ごとに変わるdocument IDを含めない。
- 数値は有限値だけを固定小数規則で出力し、locale依存の小数点や指数表記を使わない。
- 日本語と英語のglyphは第8章のfontから得たvector outlineとして埋め、OS fontへfallbackしない。
- document titleをmetadataへ入れる場合は、検証済み作品名から固定encodingで生成し、改行その他の禁止制御文字を拒否する。

PDFは印刷用の静的折り図であり、再生、編集、projectへの再importを保証しない。

## 7. SVG page ZIP契約

### 7.1 archive構造

entry名と順序を次に固定する。

```text
manifest.json
pages/page-0001.svg
pages/page-0002.svg
...
licenses/NotoSansJP-OFL.txt
```

`manifest.json`はUTF-8のcanonical JSONとし、schema ID `origami2.instruction-svg-pages.v2`、profile、projection profile、page数、step数、各pageのfile名・step index・step開始/continuation種別、continuation番号、`font.rendering = "glyph_outlines"`、outline生成に使ったfont sourceのSHA-256、license pathとSHA-256を含める。projectの保存path、端末情報、生成日時、stage IDは含めない。旧v1はfont fileをZIPへ同梱するarchive構造であり、fontを参照しないoutline-only構造へ変更したv2と同一schemaとして扱わない。

- entry名はASCIIの固定名だけとし、絶対path、drive prefix、`..`、backslash、NUL、重複名を許さない。
- entry順、CRC、圧縮方式、圧縮level、extra field、外部属性、commentを固定する。
- archiveとentryのtimestampは固定epochとし、現在時刻を使わない。
- OS固有owner、permission、host属性を意味のある出力差へしない。

### 7.2 SVG page

各SVGはUTF-8の静的vector documentとし、A4縦の固定`viewBox`、mm幅・高さ、canonical planのpage内容を持つ。

- `script`、event handler、animation、`foreignObject`、外部URL、network参照、外部画像、再帰参照を含めない。
- textとattributeをcontextに応じてescapeし、利用者文字列をmarkupとして解釈しない。
- text runはcanonical planで改行・開始位置・font sizeを固定し、すべての文字をfontから得た`path` outlineとして配置する。`text`、`@font-face`、font URL、OS font、network fallback、SVG側の自動折返しを使用せず、PDFと同じadvance前提を使用する。
- page間参照を作らず、各pageの図形内容を自己完結させる。
- canonical element順、attribute順、数値書式、改行、UTF-8 encodingを使用する。

ZIPは「複数SVG pageを一つに配布するcontainer」であり、IO-003の展開済みproject folder、編集可能な折り紙source、HTML viewerではない。

## 8. fontとlicense

初版は未改変のNoto Sans JP variable fontを同梱し、weight 400として使用する。

| asset | 固定値 |
|---|---|
| upstream | `google/fonts` |
| upstream path | `ofl/notosansjp/NotoSansJP[wght].ttf` |
| pinned commit | `389b770410cc0b7c21c85673bfa2077420fe7f65` |
| font SHA-256 | `c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f` |
| license | SIL Open Font License 1.1 |
| license SHA-256 | `1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9` |

source記録は`crates/ori-formats/assets/fonts/FONT-SOURCE.md`、license全文は`NotoSansJP-OFL.txt`を正本とする。application packageにはoutline生成に必要なfontとlicenseを同梱する。PDFとSVGはいずれもglyph outlineへ変換し、SVG ZIPには重複するfont fileを入れず、license全文とfont sourceのSHA-256を含める。

必要なUnicode scalarをfontのglyphへ決定論的に割り当てられない場合は、replacement character、豆腐glyph、system fallbackで代替せず、書き出し全体を`unsupported_glyph`として拒否する。禁止制御文字、未paired surrogateに相当する不正入力、非正規な内部文字列も生成前に拒否する。

fontまたはlicenseを更新する場合は、commit、両SHA-256、license確認、golden fileを同時に更新し、新しいexport profileとして扱う。

## 9. 資源上限

初版は既存timeline上限に加え、次を適用する。上限値と等しい入力は受理し、1を超えた時点で全体を拒否する。checked arithmeticを使い、overflowを上限超過として扱う。

| 項目 | 上限 |
|---|---:|
| 最終PDFまたはZIP bytes | 128 MiB (`134,217,728` bytes) |
| source頂点数 | 100,000 |
| source辺数 | 100,000 |
| canonical planの総page数 | 2,048 |
| 一pageのserialized payload | 4 MiB (`4,194,304` bytes) |
| 全pageに配置する総glyph数 | 500,000 |
| 全手順の総投影点visit数 | 1,000,000 |

数え方を次に固定する。

- page数はstep開始pageと全continuation pageの合計である。
- page payloadは、PDFではそのpageのcanonical content streamとpage固有resource、SVGでは一SVG entryの非圧縮bytesを数える。共通resourceをpage数分重複加算しない。
- glyph数はunique glyph種数ではなく、title、label、本文、page番号を含む配置回数である。合字はshaping後に配置する一glyphとして数える。
- 投影点visitは、全手順について3Dから投影した面境界の各点と、ヒンジ両endpointの各回を数える。cache hitでも入力規模の上限判定では一visitとして数える。
- 最終bytesは利用者へ提示し保存するexact bytesの長さである。

上限超過時に画質を落とす、stepを省く、長文を切る、fontを変更する、ZIPだけを部分保存する、といった自動縮退を行わない。

## 10. stage、preview、cancel

### 10.1 native stage

生成に成功したexact bytesはnative memoryへ最新一世代だけstageする。stageには次を束縛する。

- 推測困難なopaque export ID
- project instance ID、project ID、revision、折りモデル指紋
- format、profile、option
- exact bytes
- bytes、step数、page数、注意事項数
- 固定warning category

WebViewへ返してよいものは、opaque export ID、期待project ID・revision、format、profile、安全化済み推奨file名、bytes、step数、page数、注意事項数、固定warningだけである。project instance ID、font bytes、raw output bytes、保存path、project path、file handle、内部error、作品座標を渡さない。

保存要求はopaque export ID、期待project ID・revision、warning確認flagだけを受け取る。未知・消費済みtoken、旧世代、別project、別instance、stale revision、変更済みoption、未確認warningを拒否する。

### 10.2 非同期処理とcancel

検証、plan作成、serializationはUI threadを占有しないbounded jobとして実行する。nativeはtopology解析前後・serialization前後でcancelを確認し、各library loop自体は第9章の上限により完了時間とmemoryを制限する。初版は一つのlibrary loopの途中停止までは保証しない。

- 新しい生成開始時は以前のstageと旧世代jobを無効化する。
- 生成中cancelでは新しいstageを作らず、既存projectを変更しない。
- 明示的なstage取消はidempotentにbytesを破棄する。
- 保存dialogの取消ではstageを保持し、同じexact bytesで再試行できる。
- 生成・保存の失敗でもproject revision、dirty、保存先、Undo/Redoを変更しない。

進捗値は単調増加する固定phase単位とし、作品本文、path、font内容をeventやlogへ含めない。

## 11. 原子的保存

利用者がpreviewとwarningを確認した後、native保存dialogを開く。保存直前にもproject instance ID、project ID、revision、折りモデル指紋、token、formatを再照合する。

1. 利用者が選んだ最終fileと同じdirectoryに、application所有のcreate-new一時fileを作る。
2. stageしたexact bytesを書き、fileをflush・syncする。
3. 同じhandleから再読込し、長さとbytesがstage内容に一致することを確認する。
4. OS dialogで正しい拡張子の保存先が確認済みなら、Windowsではhandleを基準とする置換、POSIXでは同一filesystem上のrenameを行う。拡張子を補正した保存先はatomic create-newへ固定し、Windowsでは`ReplaceIfExists = false`、POSIXでは同一directoryのstageから排他的なhard linkを作成してstage名を外す。
5. POSIXではpublish前に親directoryをopen・syncする。publish後もbest effortでsyncするが、見える出力先が既に変わった後のdurability failureを通常の保存失敗として返さない。
6. 成功時だけstageを一度消費する。

失敗時はapplicationが作った一時fileだけをRAIIで削除し、既存の出力先を変更しない。特に拡張子補正の事前確認後にfile、directory、symlinkが補正先を占有した場合も原子的に拒否する。保存失敗では生成stageを保持して再試行を許す。errorは固定categoryとし、raw path、OS user名、file内容をWebViewや標準logへ露出しない。

## 12. warningと保証しない事項

保存dialogより前に、該当する固定warningを表示し、利用者の明示確認を要求する。

- 固定自動cameraであり、現在のcameraや作家指定cameraを使わない。
- texture、照明、影、透明効果を省略し、solid表裏色と白背景で描く。
- camera遷移、矢印、注目箇所、指先、つまみ、押さえ、手の移動、持ち替えを出力しない。
- 各stepは保存済み終端姿勢であり、step間の連続motionを表さない。

特に次は初版の保証対象外である。

- 連続折り経路が存在すること、途中衝突がないこと
- 物理的な紙の厚さ、弾性、摩擦、層順、自己接触の正しさ
- 人が実際に折れること、手が届くこと、完成精度
- 現在のinteractive 3D viewとpixel単位で一致すること
- 折り図からprojectや編集可能なtimelineを復元できること
- PNG、GIF、動画、interactive HTMLの生成

## 13. error境界

形式libraryは`InstructionExportError`と`InstructionDiagramError`の閉じたvariantで、入力不正、stale手順、非対応topology、非有限geometry、font asset不一致、未収録glyph、layout上限、page bytes上限、output bytes上限、ZIP・I/O・JSON失敗を区別する。native IPC境界では内部pathや作品内容を付加せず、安全な日本語メッセージへ変換する。WebViewはerror文字列を制御分岐に使わず、成功したopaque previewだけを保存可能状態として扱う。

## 14. 受入試験

### 14.1 決定論性とcanonical plan

- 同一snapshot・同一optionを同一OSで二回生成し、PDF bytesおよびZIP bytesがそれぞれ完全一致する。
- WindowsとmacOSのCIでgolden planを生成し、page mapping、glyph位置、図形bounds、線種、変更ヒンジ、warningが一致する。
- hash map seed、thread数、UI locale、timezone、viewport、device pixel ratio、現在cameraを変えても結果が変わらない。
- 同じplanをPDFとSVG ZIPへ渡し、page数、step開始page、continuation、文字位置、図形bounds、面順、変更ヒンジが一致する。

### 14.2 page layout

- すべてのstepが新しいA4 pageから始まる。
- 短文stepは一page、長い説明と注意事項はcontinuationへ全文が流れ、文字欠落・重複・順序逆転がない。
- 2,048 pageちょうどを受理し、2,049 pageを生成前に全体拒否する。
- 日本語、英語、改行、句読点、境界長のtitle・本文を固定位置へ配置する。

### 14.3 PDF

- parserでPDF 1.7、全pageのA4 `MediaBox`/`CropBox`、xref、page treeを検証する。
- 日本語と英語をfont outlineで表示でき、system fontを無効化しても結果が変わらない。
- JavaScript、action、attachment、外部resource、時刻、端末名、path、乱数IDが存在しない。
- 各page payload 4 MiB、全体128 MiBちょうどを受理し、それぞれ1 byte超過を全体拒否する。

### 14.4 SVG ZIP

- entry名、順序、固定timestamp、重複なし、path traversalなし、manifest mapping、CRCを検証する。
- 各SVGをXML parserで読み、A4寸法、固定`viewBox`、well-formed、script/event/`foreignObject`/外部URLなしを検証する。
- 各文字が`path` outlineであり、`text`、`@font-face`、font URL、font file entryが存在しないことを検証する。
- OFL全文が存在し、manifestのfont source SHA-256とlicense SHA-256が第8章の固定値と一致する。
- networkを遮断しsystem fontを無効化した環境でpageをrenderできる。
- archiveの展開時に指定directory外へfileを作れない。

### 14.5 拒否、上限、stage

- 空timeline、非有限値、欠損・重複ヒンジ、不正topology、不正font、不正glyphを部分出力せず拒否する。
- 先頭・中間・末尾のいずれか一stepだけがstaleでも、PDF/ZIPとも一切stageしない。
- 投影点1,000,000、glyph 500,000、page payload 4 MiB、page 2,048、output 128 MiBの境界値と`+1`を試験する。
- checked arithmeticのoverflowをpanicやwraparoundではなく上限超過として拒否する。
- 旧token、別project、別instance、revision変更、option変更、未確認warningを保存前に拒否する。
- 生成cancel、明示stage取消、保存dialog取消、書込失敗、置換失敗、再試行、成功後の二重保存を試験する。
- 成功時に保存fileがstage bytesと完全一致し、失敗時に既存fileとproject状態が変わらない。

### 14.6 UIと配布

- keyboardだけでformat選択、生成、warning確認、保存、取消、再試行ができる。
- modalのfocus trap、focus復帰、IME変換中のEscape、busy中の二重実行防止、進捗、cancelを試験する。
- 日本語表示を200% scaleで確認し、重要なwarningと件数が切れない。
- Windows配布物で完全offline生成を確認する。
- macOSは所有実機を前提とする手動確認項目にせず、CI build、unit test、golden plan、serializer testを合格条件とする。

## 15. 要件と設計判断

| 項目 | 初版の扱い |
|---|---|
| INS-010 手順画像 | SVG page ZIPで部分達成 |
| INS-010 PDF | 複数page PDF 1.7で部分達成 |
| INS-010 アプリ内animation | 本契約外。アプリ内では実装済み。静的PDF・SVG ZIPは各stepの終端姿勢を出力する |
| INS-004 camera・矢印・注目箇所 | 固定cameraと変更ヒンジ強調のみ。作家指定情報は別設計 |
| INS-005 手のguide | 本契約外 |
| IO-006 一枚展開図PDF | [展開図書き出し契約](crease-pattern-export-contract.md)で別に達成 |
| OQ-007 PDF図記号とlayout | 本契約の`instruction_export_v1`として初版解決 |
