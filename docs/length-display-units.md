# 長さ表示単位の契約

## 目的

ORIGAMI2の幾何、紙厚、編集command、保存された座標、外部形式の正本単位は
引き続きmmとする。その上で、利用者がmm、cm、inch、または選択した紙の輪郭辺を
1とする比率で、同じ作品の長さを表示・入力できるようにする。

表示単位の変更は幾何変更ではない。revision、dirty、Undo/Redoの対象にはするが、
fold-model fingerprintと現在の3D意味姿勢は変えない。

## 保存形式

`Paper.length_display_unit`へ次のいずれかを保存する。

```json
"mm"
"cm"
"inch"
{"paper_edge_ratio":{"reference_edge":"<EdgeId UUID>"}}
```

fieldを持たない既存のproject JSONと`.ori2`は`"mm"`として読む。project format
versionは1のままとし、`.ori2`のrequired featureは追加しない。不正な保存済み参照は
読み込み時に自動修復、自動再選択、またはmmへの永続変更を行わず、その値を保持する。

## 換算

絶対単位のmm倍率は次のとおりである。

| 表示単位 | 1表示単位のmm |
|---|---:|
| mm | 1 |
| cm | 10 |
| inch | 25.4 |

紙辺比は、保存された`reference_edge`の現在の長さを`L mm`として、
`表示値 = mm値 / L`、`mm値 = 表示値 × L`とする。辺の向きは問わない。
基準辺端点を正常に移動した場合は、commit後の現在長へ追従する。

UIは表示したsnapshotの基準長で表示値をmmへ戻し、同じsnapshotの
`expected_revision`とともにcommandを送る。別編集が先に完了した場合はstaleとして
拒否し、異なる基準長で解釈し直さない。

## 紙辺比の参照条件

紙辺比を設定するときは、次をすべて満たす必要がある。

1. `EdgeId`がcrease pattern内に一件だけ存在する
2. そのedgeが`Boundary`である
3. 紙のcyclic boundaryが3頂点以上で、頂点IDを重複して保持しない
4. edgeの両端がcyclic boundaryの連続する一区間に一意に一致し、同じ両端を持つ
   別の`Boundary` edge recordが存在しない
5. 両端点のvertex recordが各一件だけ存在する
6. 両端点座標と辺長がfiniteである
7. 辺長が0より大きい

欠損、曖昧、非有限、ゼロ長、またはoverflowは、revision、document、履歴を変えず
原子的に拒否する。

UIの参照候補収集はvertex、edge、cyclic boundaryを各一回走査する。輪郭区間ごとに
全edgeを再走査せず、1万要素規模でも`O(V + E + B)`の線形workに収める。

有効な基準辺を設定した後は、次の操作で参照を暗黙に引き継がない。

- 基準辺の`SplitBoundaryEdge`
- 基準辺を輪郭carrierとして分割する`ConnectTJunction`
- 基準辺を削除または別の両端へ付け替える`RemoveBoundaryVertex`

基準辺の正常な端点移動と、基準辺以外の輪郭編集は許可する。端点移動によって基準辺が
ゼロ長、非有限またはoverflowになる場合だけ、mutation前に拒否する。

## UI

同じproject snapshotから解決した一つの表示単位を、次へ一貫して適用する。

- 紙の幅・高さ
- 2D canvasの計測値
- 選択頂点のX/Y表示と入力
- 選択線の始点、終点、ΔX、ΔY、長さ
- 紙厚の表示と入力
- 3D紙厚の説明

角度、pixel、benchmark件数、import元の倍率、exportの固定余白など、
projectの物理長でない値は換算しない。

長方形を紙辺比で表示するときは、基準辺に平行な寸法を`1`としてread-onlyにし、
直交寸法だけを編集できる。物理的な一様scaleまたは基準辺方向の長さ変更には、
mm、cm、inchのいずれかへ切り替えるよう案内する。Coreのresize commandは従来どおり
absoluteな`width_mm`と`height_mm`だけを受け取る。

保存済みの紙辺比参照が不正な場合、画面は「参照辺が無効」と明示し、修復用表示だけを
mmにする。利用者は有効な輪郭辺または絶対単位を明示的に選択して修復する。

## 入力精度

表示のための除算と再入力の乗算だけで、未編集の保存値を変えてはならない。
長さ入力は元のbinary64 mm値と表示単位・基準長からsource tokenを作り、利用者が
編集していないfieldをsubmitするときは元のmm値をそのまま返す。

紙厚の直接入力は`0.075 mm`などの精度を丸めない。上下buttonとArrow keyだけは、
どの表示単位でも物理量として正確に`0.01 mm`ずつ増減する。buttonのaccessible nameと
spinbuttonのaccessible descriptionにも物理stepを明記する。換算表示の文字列をmmへ
再乗算した値は証拠にせず、元のmm値へ
decimal `0.01 mm` stepを適用したbinary64値とsource tokenをinputへ保持する。submit時は
同じsnapshot・表示単位・canonical表示文字列へ再結合できた場合だけ、そのmm値を採用する。

## 外部形式

表示単位は`.ori2`だけが保持する。FOLD、SVG、実寸PDF、DXFの出力bytesと単位契約は
表示単位に依存せず、既存どおりmm正本とする。FOLD/SVG importの元倍率もmmへの
変換倍率として明示し、projectの表示単位では変換しない。

## 完了条件

Rust domain/core/formats/desktop、frontend純粋換算、DOM、accessibility、
production build、lint、format、Clippyを通す。保存互換、4形式round-trip、
参照の全拒否分類、正常Move、無効Move、輪郭変更guard、非基準編集、Undo/Redo、
dirty、fingerprint・3D姿勢不変、未編集値のbit保持、紙厚`0.01 mm`step、
外部export bytes不変を回帰する。
