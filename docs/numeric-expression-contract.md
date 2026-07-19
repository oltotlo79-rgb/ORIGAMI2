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

## native評価IPC v1

Tauri command `evaluate_numeric_expression`は、`source`と
`precisionBits`だけを持つunknown field拒否のrequestを受け取る。commandはTauriの
`Request`からbody全体を受け、raw body、missing、null、型違反、outerまたはrequest内
のunknown fieldをcommand内で`invalid_request`へ閉じる。sourceはUTF-8で4,096 byte
以下、精度は32–512の整数に限定し、残りの評価上限は公開hard ceilingの既定値から
拡張しない。評価はblocking workerで実行し、panic、worker failure、構文・定義域
エラー、資源上限、finite binary64で囲めない結果を固定categoryへ閉じる。raw error、
path、AST、中間値はWebViewへ返さない。

process全体で同時に走れる評価workerは1件だけとする。commandはworkerをspawnする
前に`AtomicBool` gateを取得し、busy中の2件目を`resource_limit`としてspawnしない。
permitはblocking closure自身へ移し、成功、評価error、panicのどの終了でもRAIIで
解放する。commandを待つfutureが破棄されても実計算中にgateを早期解放せず、
closureが終了するまで次の高負荷評価を開始しない。

応答schemaは`origami2.numeric-expression-evaluation.v1`であり、次だけを返す。

- 上限内の入力原文と要求精度
- 厳密な有理区間が一点かを示す`exact`
- 上限内の演算数
- finite binary64の`lowerBound`と`upperBound`
- 各端点を小数点以下17桁の科学表記にした32 byte以下の表示文字列

binary64端点は最近傍変換をそのまま保証値としない。候補binary64を正確な有理数へ
戻して元の有理端点と比較し、内側へ丸められていた端点だけを1 ULP外側へ補正する。
したがって`lowerBound <= exact lower <= exact upper <= upperBound`を維持する。
補正後に無限大となる場合を含め、有限な両端点で囲めない値は
`result_out_of_range`とし、巨大な分子・分母や無制限の10進文字列へfallbackしない。
表示文字列は対応するbinary64端点へround-tripする固定長labelであり、別の10進有理数
として保証を主張しない。区間保証のauthorityはfiniteな`lowerBound`と`upperBound`
である。

frontend transportはrequestをIPC前に同じsource byte数・Unicode scalar・精度範囲で
検査し、応答をplain data object、完全一致field集合、schema、全scalar上限、
finite値、端点順序、canonical表示文字列で一度だけ検証する。原文または精度のechoが
要求と異なる遅延応答は`stale_response`へ閉じる。hostile Proxy、accessor、unknown
field、`NaN`、Infinity、raw `BigInt`、oversize文字列、同期例外、非同期rejectは
いずれも固定category以外を画面側へ伝えない。Tauriでないbrowser実行はnative
commandを呼ばず`native_unavailable`だけを返す。

このIPCは利用者入力欄、editor command、project schema、solverへまだ接続しない。
原文を保存したり幾何を更新したりするauthorityではなく、EDT-004/005の完成判定も
引き上げない。

## 後続接続

1. 長さ・座標・角度入力から上記native transportへ明示的に接続する。
2. 式原文と評価区間を入力fieldへ関連付ける。
3. project schemaへversion付きで保存し、既存数値projectとの互換migrationを加える。
4. UIで原文と評価値を切り替え、評価区間が利用可能精度へ収束しない場合は編集を
   保持したまま幾何更新を遮断する。
5. 制約solverへ渡す単位と次元を式とは別の型で検証する。
