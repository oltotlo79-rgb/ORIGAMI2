# 3Dメッシュ書き出し基盤契約

## 1. 目的と現在の到達範囲

本書は、ORIGAMI2が将来、完成形をOBJ、binary STL、glTF 2.0へ書き出すための純粋Rust基盤を定める。現在の実装は、呼出側から渡された1個の静的indexed triangle meshを検証し、同じ検証済みmeshから3形式のbytesを決定論的に生成して再検査するところまでである。

この基盤だけでは要件IO-007/008を実装済みとは扱わない。現在projectの完成姿勢からmeshを構築する処理、現在姿勢とrevisionへの束縛、情報損失確認、native immutable stage、保存dialog、原子的保存、UI、外部アプリでの実機受入試験は未接続である。

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

STLは頂点indexと頂点normalを保持しない。triangle soupとfacet normalだけを保持することを、将来の情報損失確認UIで明示する必要がある。

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
- GLBのheader、chunk length/type、4-byte alignment、property順、POSITION/NORMAL/index、min/max、改変検出。
- 同じadmitted meshの反復bytes一致。
- source配列順とwindingの保持。
- 3形式が同じtriangle geometryを保持し、GLBだけ文書化したmm-to-meter変換を行うこと。

## 10. 未接続事項

この基盤は静的mesh serializerであり、次を提供しない。

- 現在project、折り手順、完成step、現在3D poseからのmesh生成。
- 紙厚の中央面近似をclosed shellまたはprintable solidへ変換する処理。
- 穴の閉鎖、manifold、watertight、self-intersection、overhang、minimum wall thickness、3D printabilityの判定。
- material、紙色、表裏、texture、UV。
- animation、複数step、camera、light、手指guide。
- Blenderでの編集操作、slicerでの印刷、Web viewerでのanimationの実機受入試験。
- project instance・project ID・revision・pose generationへの束縛。
- 情報損失確認、cancel、progress、native stage、atomic save、UI。

IO-007を完了するには、認証済み完成姿勢からこのDTOを構築し、3形式を利用者経路から保存して外部readerで受入確認する必要がある。IO-008を完了するには、Blender用途、3D printer用途、Web・他3Dアプリ用途のworkflowとanimation要件を別途接続しなければならない。
