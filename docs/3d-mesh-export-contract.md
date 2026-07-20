# 3Dメッシュ書き出し基盤契約

## 1. 目的と現在の到達範囲

本書は、ORIGAMI2の認証済み現在3D姿勢をOBJ、binary STL、glTF 2.0 GLBへ書き出す製品経路と、その純粋Rust形式基盤を定める。形式基盤は1個の静的indexed triangle meshを検証し、同じ検証済みmeshから3形式のbytesを決定論的に生成して、独立verifierで再検査する。GLBは紙の表色をPBR materialとして内包する。

製品経路は、現在表示中の認証済みapplied poseから紙の中央面meshを決定論的に構築する。紙厚が正なら、各材料面を法線方向へ紙厚の半分ずつ押し出し、表裏capと境界side wallを持つ面別closed solidへ変換する。隣接面同士のboolean union、折り目の隙間・重なり除去、self-intersection解消は行わない。出力はproject instance、project ID、revision、geometry fingerprint、pose generation、紙厚bit列へ束縛する。生成bytesはnativeのimmutable stageだけに保持し、UIにはbounded metadataと固定warningだけを返す。利用者が情報損失を明示確認した後にnative保存dialogを開き、保存直前にも同じ束縛を再検証して原子的に保存する。これにより要件IO-007の静的3形式書き出し経路を実装する。

要件IO-008は、Blender、3D printer、Web・他3Dアプリへ静的fileを受け渡せる段階、GLB表色material、および面別closed solidまでを部分実装とする。外部readerによる実機受入、animation、裏色・texture、面間unionおよび印刷可能性保証は、IO-008とQAの残課題である。

glTF/GLBの構造、単位、座標系、alignmentは[Khronos glTF 2.0 Specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)を基準とする。

## 2. Version固定の入力DTO

入力は`IndexedTriangleMeshV1`だけを受理し、`schema_version = 1`を必須とする。未知versionと未知JSON fieldを拒否する。

| field | 意味 |
|---|---|
| `schema_version` | 固定値`1` |
| `name` | mesh名。Unicode scalar valueとUTF-8 byteの両方で制限する |
| `positions_mm` | 頂点位置の`[x, y, z]`配列。単位はmm |
| `normals` | 頂点indexと一対一の頂点法線 |
| `triangles` | 3個の`u32`頂点indexからなるtriangle配列 |

配列順はDTOの意味の一部である。exporterは頂点、法線、triangle、triangle内のwindingを並べ替えない。同じ検証済みDTOと形式から常に同じbytesを生成する。source順と独立なcanonical順が必要な呼出側は、admission前にその順を確定しなければならない。

検証成功時だけprivate fieldを持つ`ValidatedIndexedTriangleMesh`を発行する。export APIはこの型だけを受理し、未検証DTOから直接bytesを生成できない。

## 3. Admission条件

- position、normalの全componentは有限binary64でなければならない。
- 全positionと正規化後normalの`-0.0`は`+0.0`へcanonicalizeする。
- normal件数はposition件数と一致し、各normalは非零かつ有限に正規化可能でなければならない。
- triangleの全indexは範囲内で、同じtriangle内の3 indexは相異ならなければならない。
- binary64上で正面積を証明できないtriangleを拒否する。
- binary STLのmm binary32、およびGLBのmeter binary32へ変換した後に崩壊するtriangleも事前に拒否する。
- binary STLで表現不能な有限範囲を超えるpositionを拒否する。
- STLが孤立頂点を保持できないため、どのtriangleからも参照されない頂点を拒否する。
- nameの制御文字、U+2028、U+2029を拒否する。OBJへはASCII英数字、dot、underscore、hyphen以外を`_HH`形式のUTF-8 byte表現へ変換し、改行、comment、recordを注入できないようにする。GLBのnameはJSON stringとしてescapeする。

近似、snap、triangleの自動修復、winding反転、頂点結合、重複triangle削除は行わない。

## 4. 単位と座標軸

検証済みmeshの正本は次のとおりである。

- 単位: millimeter
- 右手系
- `+X`: 右
- `+Y`: 前
- `+Z`: 上
- `X × Y = Z`

OBJとbinary STLはこの数値と軸を保つ。OBJには固定commentを、STLには固定80-byte headerを付け、mmと軸を明記する。

glTF 2.0のGLBはpositionを`mm × 0.001`のmeterで格納する。buffer内のlocal座標とindex順は正本meshを保ち、nodeの固定rotation matrixによって`(x, y, z) -> (-x, z, y)`へ写す。これにより、sourceの右・前・上をそれぞれglTFの`-X`・`+Z`・`+Y`へ対応させ、右手系を保ったままglTF sceneのY-upへ合わせる。normalにも同じnode変換が適用される。拡大縮小、鏡映、winding反転は行わない。

## 5. OBJ

- UTF-8 text、LF改行。
- `model/obj`、拡張子`.obj`。
- 固定headerに`unit: millimeter`と`right-handed X-right Y-forward Z-up`を記録する。
- 1頂点につき`v`を1件、1頂点法線につき`vn`を1件、1triangleにつき`f v//vn`を1件出力する。
- OBJの1-origin indexへだけ変換し、positionとnormalのindexを一致させる。
- 有限binary64をround-trip可能な10進表現とし、`-0`、NaN、Infinityを出力しない。
- object名はboundedなASCII安全名だけを用いる。

OBJ verifierはserializerと別に全行を読み直し、固定header、行数、行長、有限かつcanonicalな数値token、1-origin index、頂点・法線・face順、終端を検証済みmeshとbit単位で照合する。

## 6. Binary STL

- `model/stl`、拡張子`.stl`。
- 80-byte header、little-endian `u32` triangle count、各50-byte triangle recordを用いる。
- headerは`ORIGAMI2 BINARY STL`、`UNIT=MM`、軸、bounded安全名を含み、残りをASCII spaceで埋める。
- 各recordは、出力後のbinary32頂点から再計算した有限なunit facet normal、3頂点、`u16 = 0`のattribute byte countを格納する。
- file sizeは厳密に`84 + 50 × triangle_count` bytesとする。

STL verifierはheader、little-endian count、宣言countとfile size、全normal・頂点の有限性とbinary32 bit、winding、attribute byte countを独立に照合する。

STLは頂点indexと頂点normalを保持しない。各triangle recordが独立したtriangle soupとなり、法線は出力後のbinary32頂点から再計算したfacet normalへ置き換わる。これは3D printabilityの非保証とは別の固定warningとしてpreviewに必ず含め、利用者が保存dialogを開く前に明示確認する。nativeとfrontendは同じ順序の閉じたwarning allowlistを検証し、欠落、並べ替え、未知warningを拒否する。

## 7. glTF 2.0 Binary（GLB）

- `model/gltf-binary`、拡張子`.glb`。
- GLB magic `glTF`、version `2`、宣言全長、JSON chunk、BIN chunkをlittle-endianで出力する。
- JSON chunkはASCII space、BIN chunkは必要な場合にzeroで4-byte境界へpaddingする。現在の3 buffer viewはすべて自然に4-byte alignedである。
- `asset.version = "2.0"`、固定generator、単位・source軸・node軸変換の`asset.extras`を持つ。
- default scene、scene 1件、node 1件、mesh 1件、triangle primitive 1件を持つ。
- primitiveはindexed `POSITION`と`NORMAL`を持ち、modeは`TRIANGLES`とする。
- POSITION、NORMALはbinary32 `VEC3`、indexはlittle-endian `UNSIGNED_INT SCALAR`とする。
- position、normal、indexを別々のaligned buffer viewへ置き、各accessorに`min`と`max`を記録する。
- 外部URI、data URI、material、texture、image、skin、morph target、animation、extensionを生成しない。

GLB verifierは全体長、chunk type・length・padding、JSON byte上限を先に検証する。その後、未知field拒否の独立DTOでJSONを読み、asset、scene、node、mesh、primitive、buffer view、accessor、alignment、component type、count、min/maxを照合する。最後にBINをlittle-endianで読み、全POSITION、NORMAL、indexを検証済みmeshから期待されるbinary32 payloadとbit単位で比較する。

## 8. 非緩和の資源上限

| 項目 | hard ceiling |
|---|---:|
| 頂点 | 100,000 |
| triangle | 200,000 |
| 1形式の出力 | 64 MiB |
| name | Unicode scalar value 120 |
| name UTF-8 | 512 bytes |
| OBJ 1行 | 2,048 bytes |
| GLB JSON chunk | 64 KiB |

caller指定値はhard ceilingを小さくできるが、大きくして緩和できない。件数とnameはallocation・serialization前に検査する。STL/GLBはchecked arithmeticで正確な出力長を先に求め、上限超過時に部分bytesを返さない。OBJは各行追加前に累積byte数を検査する。3形式とも検証完了前のbytesをartifactとして返さない。

## 9. 決定性と回帰

少なくとも次を自動試験で固定する。

- schema versionと未知field。
- NaN、正負Infinity、`-0.0`。
- normal件数、零normal、normalの決定論的正規化。
- 範囲外index、反復index、binary64退化triangle、binary32変換後の退化triangle。
- 孤立頂点、binary32範囲超過。
- 頂点、triangle、name文字数、name byte数、出力byte数のexact limitとone-short。
- hard ceilingをcallerが緩和できないこと。
- nameの改行・JSON・OBJ injection。
- OBJの数値・face改変検出。
- STLのlittle-endian count、80-byte header、count/size整合、attribute、切詰め検出。
- STLだけが頂点index・頂点normalの喪失とtriangle soup・facet normal化の固定warningを、印刷適合性非保証warningより前に持つこと。
- nativeとfrontendが形式別warningの欠落、追加、並べ替えを拒否すること。
- GLBのheader、chunk length/type、4-byte alignment、property順、POSITION/NORMAL/index、min/max、改変検出。
- 同じadmitted meshの反復bytes一致。
- source配列順とwindingの保持。
- 3形式が同じtriangle geometryを保持し、GLBだけ文書化したmm-to-meter変換を行うこと。

## 10. 接続済みの製品経路

### 10.1 現在姿勢とmesh生成

- 書き出し元は3D previewへ現在適用済みの認証済みposeだけとする。未生成、stale、非対応topologyの姿勢からは書き出しcapabilityを発行しない。
- material faceの境界を現在poseへ写し、exact predicateを用いる決定論的な単純多角形三角形分割から中央面meshを構築する。
- 切断、穴、seam、非単純faceなど、初版のmaterial tree姿勢として認証できないtopologyをflattenまたは黙って省略しない。
- 設定紙厚はpreview metadataとstage束縛へ含めるが、頂点へ層ずらしを適用しない。出力geometryは常に`authenticated_mid_surface_triangle_mesh_v1`である。

### 10.2 姿勢束縛とimmutable stage

- preview開始時にproject instance、project ID、revision、geometry fingerprint、pose generation、紙厚のbinary64 bit列、formatをnativeで捕捉する。
- 形式検証と独立verifierを通過したbytesだけをnativeのimmutable `Arc<[u8]>` stageへ格納する。同時に有効なstageは最新世代1件だけとする。
- 遅れて完了した旧preview、旧export ID、別project、別instance、stale revision、変更済みgeometry・pose・紙厚を拒否する。旧previewの取消が新しいstageを破棄してはならない。
- WebViewへraw mesh、encoded bytes、保存path、project path、file handleを渡さない。frontendはexact DTOと形式別warning allowlistを検証する。

### 10.3 UI、warning確認、保存

- UIの選択肢はOBJ、binary STL、GLBの閉じた3形式だけとし、形式、単位、座標軸、推奨file名、byte数、face・vertex・triangle数、source revision・pose generation、設定紙厚を保存前に表示する。
- 紙厚0では中央面のみ、紙厚正では面別closed solidであることを表示する。GLBの表色material、未対応の裏色・texture・animation、project意味情報の欠落を表示する。STLでは、頂点index・頂点normalが失われtriangle soup・facet normalとなることと、面別solidの重なり・隙間を含め3D printabilityを保証しないことを追加表示する。
- 利用者のwarning確認をfrontendのbutton活性条件にするだけでなく、nativeも確認flagを必須とする。format変更またはpreview再生成時は確認を解除する。
- native保存dialogの取消では同じimmutable stageを保持して再試行できる。保存直前にstageの全束縛を再検証し、成功時だけ生成済みbytesを一度消費する。
- 保存は既存のnative原子的保存境界を共用する。書き出しによってprojectのrevision、dirty、保存先、Undo/Redo、現在姿勢を変更しない。

## 11. IO-008とQAの残課題

形式内verifierと自動回帰は、構文、単位、軸、index、winding、数値範囲、決定性、stage束縛および情報損失確認を検証する。一方、第三者アプリのversion差や解釈まで自己verifierだけで保証してはならない。次はIO-008または配布候補QAとして別に受け入れる。

- OBJとGLBを対象Blender versionへ読み込み、mm/mのscale、Z-up/Y-up変換、右手系、表裏、法線、編集可能性を確認する。
- STLを対象slicerへ読み込み、mm scale、向き、triangle数を確認する。紙厚正では各面が幾何学的に閉じていることを確認するが、面間のunionや印刷可能性を期待値にしない。
- GLBを対象Web viewerおよび他3Dアプリへ読み込み、scene、軸、scale、法線、単一静的meshの表示を確認する。外部resource取得に依存しないことも確認する。
- 外部reader受入結果には、ORIGAMI2 build、OS、reader名・version、入力format、fixture、期待値、結果を記録する。Windows正式版の候補buildで再実施する。
- 折り手順の複数stepを時間軸へ変換するanimation、camera、light、手指guideを設計・実装する。
- Blender向け裏面material、texture・UV、編集workflowを実装・受け入れる。
- 3D printer向けに面別closed solidをunionし、折り目の穴閉鎖、manifold、watertight、self-intersection、overhang、minimum wall thicknessなどを判定する。現行の静的STLはこれらを保証しない。

以上の外部reader受入と用途別workflowはIO-008/QAの残課題であり、接続済みIO-007のnative静的3形式保存経路を「未接続」へ戻す理由にはしない。
