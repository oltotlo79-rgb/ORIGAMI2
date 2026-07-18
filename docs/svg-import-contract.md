# SVG取込契約

更新日: 2026-07-18

本書は、ORIGAMI2初版が取り込む静的なSVG線図の範囲、利用者による線種・外周確認、情報損失、失敗時の状態保証を定める。SVG 1.1/2全体への対応や、ブラウザーと同じ描画結果を保証するものではない。本契約の利用者経路と受入試験を実装し、FOLD取込と合わせて要件IO-004、線種割当画面としてIO-005を実装済みとする。

## 1. 参照仕様と参考実装

- [W3C SVG 2](https://www.w3.org/TR/SVG2/)
- [W3C XML 1.0 Fifth Edition](https://www.w3.org/TR/xml/)
- [quick-xml API documentation](https://docs.rs/quick-xml/)
- [quick-xml repository](https://github.com/tafia/quick-xml)
- [svgtypes API documentation](https://docs.rs/svgtypes/)
- [svgtypes repository](https://github.com/linebender/svgtypes)

`quick-xml`と`svgtypes`はRust実装の候補であり、ライブラリが解釈できるSVG機能をそのまま取込対象にはしない。採用版は実装時に`Cargo.lock`で固定し、本書の許可listとORIGAMI2側の検証を正本とする。

## 2. 入力と信頼境界

- 入力はUTF-8のstandalone SVG 1.1/2とする。UTF-8 BOMは許可するが、XML宣言が別encodingを指定する入力、整形式でないXML、rootが`svg`でない入力は拒否する。
- DTD、DOCTYPE、外部・内部entity宣言は拒否する。XMLの定義済みentityと数値文字参照以外を展開しない。
- Rust側でXMLをstreaming解析し、raw SVGをWebView、HTML DOM、画像rendererへ渡さない。
- script、event handler、animation、link遷移を実行しない。ネットワーク通信、外部file参照、外部font・画像・stylesheet・paint serverの取得を行わない。
- 外部stylesheet参照は拒否する。外部resourceへ依存する要素は取得せず、その要素を具体名・件数付きで除外して警告する。依存範囲を安全に分離できない場合は取込を拒否する。
- path、実ファイル名、raw XML、未認識または無制限長のsource属性値をWebViewや標準診断へ公開しない。UIへ返すのは、mappingに必要なclass、layer、`data-origami-kind`、代表IDを個別上限内で無害化した値と、上限付きの集計、選択肢、線プレビューだけとする。

## 3. 対応するSVG subset

### 3.1 構造と直線geometry

対応する構造要素はroot `svg`、入れ子の`g`と`a`である。`a`は単なるcontainerとして扱い、`href`を開かない。対応するgeometryは次に限る。

| 要素 | 対応範囲 |
|---|---|
| `line` | 有限な`x1`、`y1`、`x2`、`y2` |
| `polyline` | 2点以上の有限な点列 |
| `polygon` | 3点以上の有限な点列。末尾から先頭を閉じる |
| `rect` | 有限・正の`width`と`height`、`rx=0`かつ`ry=0` |
| `path` | `M/m L/l H/h V/v Z/z`だけから成るsubpath |

`path`は複数subpath、絶対・相対command、暗黙に反復される座標組を許可する。退化線分、非有限値、数値overflow、構文不正は拒否する。未対応commandを1つでも含む`path`は部分的に採用せず、要素全体を除外してcommand名と件数を警告する。

rootの`viewBox`、`width`、`height`と、root以下の対応要素にある入れ子の`transform`を処理する。`matrix`、`translate`、`scale`、`rotate`、`skewX`、`skewY`を文書順に合成する。root以外の`svg`による別viewportは未対応である。

座標はSVGの見た目の上下方向を維持し、Y軸を反転しない。root `viewBox`の原点と座標範囲、各nested transformを決定論的に反映した後、利用者が確認した一様倍率でmmへ変換する。

### 3.2 styleの読取り

線種候補の集約に必要な範囲として、presentation attribute、inline `style`、継承、および埋込み`style`の単純な`.class` selectorを処理する。対象propertyは少なくとも`stroke`、`stroke-dasharray`、`display`、`visibility`、`color`、`opacity`、`stroke-opacity`とする。

複雑なselectorは規則全体を無視して具体警告を出す。対応済みgeometryは正規化したclass集合をgroup keyに残すため、利用者が線種を明示mappingできる。CSS variable、外部参照、`url(...)` paint、解決不能なstroke・dash・単位や宣言は推測せず、中心線の意味を安全に復元できないgeometryを要素単位で除外して具体警告と確認を要求する。`display:none`または`visibility:hidden`のgeometryも除外して件数を警告する。

fill、gradient、pattern、stroke幅、line cap/join、marker、opacityなどの描画情報はORIGAMI2の線geometryへ保存しない。中心線を復元できない塗り潰し輪郭や展開済みstrokeを自動的に中心線へ変換しない。

## 4. 除外する機能

次は実行も近似もしない。存在する場合は要素名またはpath command名と件数を警告し、該当要素全体を除外する。除外後に有効な直線が残らない場合は取込を拒否する。

- `C/c S/s Q/q T/t A/a`を含む曲線path
- `circle`、`ellipse`、角丸`rect`
- `text`、`image`、`use`、symbol参照、入れ子の`svg`
- script、event属性、animation要素
- filter、clip、mask、marker、pattern、gradient、`foreignObject`
- raster画像、font、外部resource、外部stylesheetへ依存する描画

曲線を線分へ黙ってflattenしない。曲線近似を将来追加する場合も、許容誤差、生成線分数、情報損失を別契約で明示し、初版の既定動作にはしない。

## 5. source groupと線種mapping

対応する直線を、次の正規化済みkeyの組合せで最大64 groupへ決定論的に集約する。

```text
SvgSourceGroupKeyV1
├─ computed stroke: none / canonical solid color / unresolved paint
├─ normalized stroke-dasharray: solid / finite dash list / unresolved
├─ sorted class token set
├─ named ancestor groupのlayer path
├─ nearest data-origami-kind
└─ style resolution state
```

- layer名は、祖先`g`の`data-origami-layer`、Inkscape layer label、祖先groupの`id`をこの優先順で表示用hintとして得る。特定editorの属性がなくても取込可能でなければならない。
- `data-origami-kind`のcanonical値は`boundary`、`mountain`、`valley`、`auxiliary`、`cut`、`ignore`とする。最も近い祖先または要素自身の値をhintにし、要素自身を優先する。
- `id`は代表例として表示するが、objectごとの一意IDでgroupが過分割されないよう既定keyには含めない。その他の`data-*`は無害化した属性名だけをhintとして表示できるが、自動的な意味付けをしない。
- 保持したgroup行にはstroke見本、dash見本、layer path、class、canonicalな`data-origami-kind`、source要素件数、生成線分件数、無害化した代表ID、表示属性を永続化しない旨の損失badgeを表示する。未解決paint、非表示、未対応geometry等は保持groupへ混ぜず要素ごと除外し、全体警告へ種類別件数を表示して確認を要求する。
- group数が64を超えた場合、似たstyleを黙って統合せず資源上限エラーにする。

UIへ渡すlayer path、`data-origami-kind`等の表示hintは120 Unicode scalar value以内とし、代表IDだけは制御文字を除いて120で切り詰める。classは最大32 token、各tokenはASCII英数字・`-`・`_`だけの64 byte以内とする。CSS selectorとstyle property値はそれぞれ120 Unicode scalar value、`title`/`desc`の解析保持は512 Unicode scalar valueを上限とし、超過入力を無制限に保持・表示しない。

利用者は全groupを`Boundary / Mountain / Valley / Auxiliary / Cut / Ignore`のいずれかへ明示mappingする。属性や色に基づくpresetは初期選択を提案するだけで、未選択・競合groupが1つでもあれば適用できない。赤と青の山谷対応を普遍的な規則として固定しない。

`Cut`が1本でも残る場合は、取込後projectの`cutting_allowed=true`を利用者へ明示して確認する。確認しない場合は`Cut`を`Ignore`等へ変更するまで適用できない。SVGのlayer、class、ID、styleは初版projectへ永続化せず、その情報損失を警告する。

## 6. 用紙外周の選択

ORIGAMI2初版は穴のない一枚紙だけを取り込む。次から最大64件の閉路候補を作る。

1. closed `polygon`
2. 非角丸`rect`
3. `Z/z`で明示的に閉じた直線`path` subpath
4. `Boundary`へmappingしたgroupから得られる閉路
5. 利用者が「viewBoxから外周を生成」を選んだ場合のroot `viewBox`矩形

明示閉路候補の選択単位はstyle group全体ではなく、検証済みの閉路1件である。同一groupに複数の非接続閉路があれば別候補として表示する。利用者はpreview上の強調、由来、辺数、幅、高さを見て候補1件を選ぶか、「線種割当で用紙境界を指定」を明示的に選ぶ。後者はBoundaryへ割り当てた全groupを合成し、適用時のRust再検証で単純な閉路がちょうど1件にならなければprojectを変更せず確認画面へエラーを返す。

- 最大面積、最外周、黒線、先頭要素を黙示的に外周へ採用しない。これらは候補の説明または並べ替えhintにしか使わない。
- viewBox矩形は利用者が明示的に生成・確認した場合だけ使用する。
- 選択した閉路の線分は、source groupのmappingより優先して`Boundary`になる。同じgroupの選択外線分にはgroup mappingを適用する。
- `Boundary`へmappingされた選択外線分が残る場合は、一枚紙の境界が一意にならないため適用を拒否する。
- 選択閉路は3辺以上、有限、非ゼロ面積、単純、連結、自己交差なしでなければならない。穴、複数外周、分断、開いた輪郭は未対応である。
- 候補が64件を超えた場合は一部を隠さず、資源上限エラーにする。

## 7. 縮尺と単位

rootの物理寸法と`viewBox`から、XとYで同じ一様な`mm / SVG unit`を一意に求められる場合だけ初期値を提案する。

| SVG単位 | mm換算 |
|---|---:|
| `mm` | 1 |
| `cm` | 10 |
| `in` | 25.4 |
| `pt` | 25.4 / 72 |
| `pc` | 25.4 / 6 |
| `q` / `Q` | 0.25 |
| `px`または初期CSS user unit | 25.4 / 96 |

`width`または`height`の欠落、percentage、font相対単位、X/Yで異なる倍率、解決不能な`preserveAspectRatio`、矛盾する寸法は自動決定しない。利用者が有限かつ正の`mm / SVG unit`を手動入力する。非一様な作者transform自体はそのままgeometryへ適用するが、ORIGAMI2が追加する縮尺は常に一様とする。

確認画面にはrootのwidth/heightごとの正規化mm値と元単位、`viewBox`、提案根拠、選択外周の最終幅・高さを表示する。CSS基準の`px = 1/96 in`または単位なし寸法を解釈した場合は、自動提案の成否に関係なく作者の意図と一致する保証がない旨を警告し、利用者が確認する。倍率未確定、0以下、非有限、上限超過では適用できない。

## 8. 平面graph化

transformと縮尺を適用した直線を、次の順で平面graphへ変換する。

1. binary64座標がX・Yとも完全一致する端点だけを同一頂点へ統合する。
2. proper X交差を交点で自動splitする。
3. 端点が別線分の内部に正確に載るT接点を自動splitする。
4. 折り線・切断線と選択外周の正確な接点で外周を自動splitする。
5. split片は元source groupと最終線種を継承する。

初版では距離許容差による端点merge、近傍線分へのsnap、隙間補修を行わない。座標を勝手に移動しない。collinear overlap、重複線分、異なる線種が同一区間を占める状態、判定不能な交差は推測して統合せず拒否する。split後にも外周の単純閉路、山折り・谷折り・切断線の紙内包含、退化、重複、交差、作業量を再検証する。補助線は作図guideとして紙外へ延長できる既存project規則に従う。

## 9. プレビュー、警告、適用条件

previewは無害化済みの直線だけを最大5,000本返す。外周候補と選択外周を優先し、残りは決定論的に抽出する。preview省略は取込本体の線を削減しない。

警告は同じ種類を件数集約し、少なくとも次を具体名で示す。

- 除外した曲線command、要素、非表示geometry、外部依存
- 無視するfill、stroke表現、effect、style property
- 解決できないstyleと手動mappingの必要性
- 永続化しないlayer、class、ID、`data-*`、描画属性
- viewBox外周の生成、CSS px換算、手動縮尺
- previewで省略した線数

警告が1件でもある場合は、一覧末尾の「取り込まれない情報と変換内容を確認した」を利用者が明示確認するまで適用できない。個々の要素を64個以上の警告行へ展開せず種類別に集約するが、異なる警告種類が64を超える場合は情報を隠さず資源上限エラーにする。

次のいずれかでは適用buttonを無効にする。

- 作品名、正倍率、最終寸法の検証が未完了
- 未mappingまたは競合するsource groupがある
- 外周が未選択・未確認、または検証不合格
- 選択外の`Boundary`線が残る
- `Cut`許可が未確認
- 警告確認が未完了
- overlap、無効geometry、未解決の外部依存、資源上限がある
- stageがbusy、stale、または既に適用・取消済みである

## 10. stage、適用、取消

FOLD取込と同じ状態契約を用いる。

1. native file dialogで選択し、Rust側が最大16 MiBまでbytesを読む。
2. strict・bounded parse、style集約、外周候補、geometry検証を行う。
3. 検証済みbytesを最新1世代だけmemoryへstageし、opaque token、開始時のproject instance、project ID、revisionと無害化済みDTOを保持する。
4. UIで作品名、縮尺、全group mapping、外周、切断許可、警告を確認する。
5. UIで選んだ縮尺、全group mapping、外周選択をRustへ送り、stageした同一bytesから最終外周、幅・高さ、変換後のCut有無を非破壊で事前検証する。結果にはopaqueなvalidation IDを発行し、token、project instance・ID・revision、倍率のbinary64 bit、canonical mapping、外周選択へ束縛する。
6. 利用者はRust検証済みの最終幅・高さとCut有無を確認する。縮尺、mapping、外周を変更した時点で旧validationを無効化し、新たに検証するまで外周確認と適用を許可しない。
7. 現在projectが未保存なら、全入力検証後かつ適用直前に破棄確認を行う。
8. Rust側はstage bytesを再parseし、token、validation ID、instance、project ID、revision、倍率、mapping対象group、外周選択を再照合する。
9. 全検証成功時だけ、履歴・折り手順・保存先を引き継がないrevision 0、dirtyの新規projectへ原子的に置換する。

失敗時は現在projectを一切変更せず、stageを維持して利用者が設定を修正または取消できる。取消stateはpendingと直近に実際に取消したtokenのtombstoneを保持し、同じtokenの重複取消が成功するのは新stage開始前だけとする。新stage開始時はtombstoneを消去する。成功applyはtombstoneを作らず、適用済みtokenの取消を拒否する。random token、別世代token、project差替え後のstale適用も拒否する。

## 11. 資源上限

| 対象 | 上限 |
|---|---:|
| 入力ファイル | 16 MiB |
| XML depth | 64 |
| XML element | 50,000 |
| attribute | 1 elementあたり64 |
| source線分 | 10,000 |
| split後の最終線分 | 10,000 |
| 埋込みstyle rule | 128 |
| 表示hint / CSS selector / style property値 | それぞれ120 Unicode scalar value |
| class | 最大32 token、各ASCII 64 byte |
| `title` / `desc` | 各512 Unicode scalar value |
| path command | 合計20,000 |
| source group | 64 |
| 外周候補 | 64 |
| 警告種類 | 64 |
| 交差候補・判定作業 | 1,000,000 |
| CSS ruleとelementの照合 | 1,000,000 |
| UI preview線分 | 5,000 |

上限は未対応・非表示要素を含む入力走査段階から数える。preview以外の超過は省略で続行せず資源上限エラーにする。source線分は対応geometryを直線へ展開した直後、最終線分はX/T/外周接点split後にそれぞれ再検査する。

## 12. 受入条件

実装完了には少なくとも次を自動testとUI testで確認する。

- UTF-8、namespace、SVG 1.1/2、全対応要素、pathの絶対・相対command、複数subpathを正しく処理する。
- 入れ子transform、root `viewBox`、対応物理単位を決定論的に変換し、Y反転せずsource外観を保つ。
- presentation attribute、inline style、単純な埋込みCSS、継承から同じstyle groupを得る。属性順やclass順でgroup IDが変わらない。
- 64 group以内の全groupに明示mappingを要求し、未選択・競合・65 group目を安全に拒否する。
- closed polygon、rect、linear path、Boundary group、明示viewBox生成から外周候補を作り、複数候補を分離する。最大面積を自動確定しない。
- 選択外周がgroup mappingより優先し、外周未確認、複数境界、穴、開路、自己交差を拒否する。
- exact端点だけを統合し、X/T/外周接点をsplitする。collinear overlapと競合重複を拒否し、split片が線種を継承する。
- 曲線、circle/ellipse、text/image/use、script、animation、filter/clip/mask等を実行・flattenせず、具体警告と確認を要求する。
- DTD/entity宣言、XXE、外部stylesheet/resource、再帰的構造、深いXML、過大属性、NaN/Infinity、数値overflowをnetwork accessなしで安全に拒否または除外する。
- 各資源上限の境界値と上限+1をtestし、10,000本の有効線図を上限内で処理する。preview 5,000本への省略が本体を削減しない。
- CSS selectorとstyle property値はbyte数ではなくUnicode scalar value数で120まで受理し、121を入力段階で拒否する。
- warning acknowledgement、dirty確認、取消、重複取消、token世代、instance/project/revisionのstale検出を確認する。
- 実DOMへ確認画面をrenderし、Tab/Shift+Tabと外部focusのtrap、IME変換中Escapeの無視、縮尺・mapping・外周変更時のvalidation/確認解除、検証・取消・適用失敗時のdialog保持をeventで確認する。
- 事前検証IDを倍率・mapping・外周・project identityへ束縛し、旧世代・改変・未検証のIDによる適用を拒否する。変換後のCut有無と最終幅・高さが事前検証とapplyで一致する。
- apply中の再parseまたは再検証失敗で旧projectが不変であり、成功時だけrevision 0、dirtyの新規projectへ一度だけ置換される。
- WebView、IPC error、診断logにpath、実ファイル名、raw XML、未認識または上限外のsource属性値が現れない。UIに許可したmapping hintも個別上限と無害化を通る。

## 13. 初版で未対応

- 曲線折り・曲線切断と曲線flatten
- 円、楕円、角丸矩形、text、画像、symbol、`use`
- filter、clip、mask、marker、gradient、pattern、animation、script
- 外部CSS、外部resource、font、画像の取得
- nested `svg` viewportと複数page/artboard
- 穴、複数外周、複数枚の紙
- 許容誤差付き端点merge、自動gap修復
- SVG layer、style、ID、任意metadataのproject永続化
- SVG書出しとSVGへのlossless round-trip

本契約の対応subset外は、推測変換せず警告または拒否する。将来subsetを拡張する場合は、許可list、情報損失、資源上限、受入試験を同時に更新する。
