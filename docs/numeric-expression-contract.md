# 数式入力・高精度評価契約

更新日: 2026-07-19

## 目的と状態

`ori-numeric`はEDT-004/005の内部基盤である。利用者UI、座標・長さ入力、
project modelへの式の関連付け、式と評価値の表示切替はまだ接続していないため、
この段階だけでEDT-004/005を実装済みとは判定しない。

## 対応構文

- 整数、小数、10進指数表記
- `/`による分数・除算
- `+`、`-`、`*`、`/`と標準の演算優先順位
- 単項`+`、単項`-`
- 括弧
- `pi`または`π`
- `sqrt`または`√`

暗黙の乗算は行わない。例えば`2pi`は`2*pi`へ自動修復せず構文エラーとする。
入力原文は空白を含めて`ScalarExpression`へ保持し、serdeでは原文1文字列だけを
保存する。読み込み時は既定上限でもう一度parseし、不正な保存値からASTを構築しない。

## 数値モデル

10進リテラルと有理四則演算は`BigRational`で厳密に保持する。`pi`はMachin公式と
交代級数の剰余境界、平方根は整数平方根による外向きdyadic丸めを使い、結果を
`[lower, upper]`の閉有理区間で返す。既定精度は192 bit、許可範囲は32–512 bitである。
無理数を`f64`へ黙って丸めない。

除数区間が0を含む場合は`DivisionByZero`、平方根の入力区間が負値を含む場合は
`NegativeSquareRoot`とし、推測値を返さない。

## 資源上限

呼出側は次を小さくできるが、公開hard ceilingを超えて拡張できず、0または不正な
精度にはできない。hard ceilingはRust APIからも確認できる。

| 対象 | 公開定数 | hard ceiling |
|---|---|---:|
| source byte数 | `HARD_MAX_SOURCE_BYTES` | 4,096 |
| token数 | `HARD_MAX_TOKENS` | 1,024 |
| AST node数 | `HARD_MAX_AST_NODES` | 1,024 |
| 入れ子深さ | `HARD_MAX_NESTING_DEPTH` | 64 |
| literal digit数 | `HARD_MAX_LITERAL_DIGITS` | 1,024 |
| 10進指数・小数scale | `HARD_MAX_DECIMAL_EXPONENT` | 4,096 |
| 評価演算数 | `HARD_MAX_OPERATIONS` | 20,000 |
| 有理数の分子・分母および平方根中間値のbit数 | `HARD_MAX_VALUE_BITS` | 32,768 |

精度範囲も`MIN_PRECISION_BITS = 32`と`MAX_PRECISION_BITS = 512`として公開する。
hard ceilingを超える設定は入力を読む前に`InvalidLimits`とする。

各上限超過、checked算術のoverflow、巨大指数、深い括弧は
`ResourceLimit`へ閉じる。panic、非有限値、部分評価値は公開しない。

parse時にはsource byte数、token数、AST node数、最大入れ子深さ、最大literal
digit数、10進指数と小数scaleの最大絶対量をfootprintとして式に保持する。
`evaluate`でこれらをparse時より小さくした場合も、該当する`ResourceLimit`を返し、
一度大きい設定でparseすることによる評価境界の迂回を認めない。ASTはarena内の
連続したnodeとindex参照で保持し、左結合ASTの評価は明示stackによる反復postorder
とする。評価だけでなくclone、debug表示、破棄もRustのcall stackへAST深さを
載せない。parserで再帰するのは単項演算子と括弧だけで、入れ子hard ceiling 64を
適用する。

`parse`は借用文字列のbyte長を空白走査や所有化より先に検査する。serde読み込みは
既定上限で再parseしてfootprintを復元する。ただしserde format decoderがvisitorへ
渡す前にJSON文字列をmaterializeする可能性はcrate単独では除けないため、project
reader側でもファイル全体のbyte上限を設ける。

## 後続接続

1. 長さ・座標・角度入力へ式原文と評価区間を関連付ける。
2. project schemaへversion付きで保存し、既存数値projectとの互換migrationを加える。
3. UIで原文と評価値を切り替え、評価区間が利用可能精度へ収束しない場合は編集を
   保持したまま幾何更新を遮断する。
4. 制約solverへ渡す単位と次元を式とは別の型で検証する。
