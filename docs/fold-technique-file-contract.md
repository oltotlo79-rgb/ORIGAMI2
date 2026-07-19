# 名前付き折り技法共有ファイルV1契約

## 1. 目的と初版境界

本契約はINS-008/009の前提として、利用者が定義した名前付き複合折り技法を、純粋な宣言データとして保存・共有する境界を定める。正本実装は`ori-instructions::fold_technique_file`である。

V1ファイルは「折り方の説明テンプレート」であり、実行計画、plugin、macro、project command、3D運動証明ではない。読み込みに成功しても、次の権限や保証を一切生成しない。

- code、script、式、pluginまたはOS commandの実行
- pathの読み書き、URLの取得、source citationの自動参照
- project、展開図、3D姿勢、timelineまたはUndo/Redoの変更
- 衝突なし、平坦折り可能、折り重ね可能、または物理技法を自動実行可能という証明

初版SIM-010は「一直線をまたぐ全対象層の一括折り」に限定される。中割り折り、かぶせ折り、沈め折り、層を選んでめくる動作は初版SIM-010の実行範囲外である。V1ではこれらを`inside_reverse_fold`、`outside_reverse_fold`、`sink_fold`等のmetadataとして表現できるが、各operationは対応する未対応物理操作を`unsupported_physical_operation`として明示しなければならない。

この基盤だけではINS-008/009を完成扱いにしない。作成・編集UI、ファイル選択、適用先との照合、利用者確認、timelineへの登録は別の利用者経路で実装する。

## 2. Envelopeとversion

JSON rootは次のfieldだけを持つ。

| field | 意味 |
|---|---|
| `schema` | 固定文字列`origami2_fold_technique_file` |
| `version` | 整数`1` |
| `package_id` | 利用者定義の安全な不変ID |
| `metadata` | author、source provenance、SPDX license ID |
| `techniques` | 1〜64件の名前付き技法 |

未知field、未知enum、field重複、schema違い、version違いはファイル全体を拒否する。将来versionをV1として推測読込しない。各techniqueの`version`は利用者定義技法の改訂番号であり、file schema versionとは別である。

## 3. 宣言モデル

### 3.1 Technique

各techniqueは次を持つ。

- 安全な利用者定義`id`と`version`
- locale別の`names`と`descriptions`
- typed `parameters`
- named `preconditions`
- 2件以上のordered `operations`

operationsの順序だけは意味を持つためcanonical化でも維持する。technique、parameter、precondition、locale、capability、参照集合等、順序が意味を持たない集合はIDまたはenum順にcanonical化する。

### 3.2 Localized text

localeは`ja`、`en`、`ja-jp`等のlowercase ASCII subsetへ正規化済みの値だけを受理する。同じfield内のlocale重複を拒否する。名称は最大120文字かつ480 UTF-8 bytes、説明・prompt・instructionは最大2,048文字かつ8,192 UTF-8 bytesである。

空文字、前後空白、制御文字、双方向表示override/isolateは拒否する。表示consumerはそれでも必ずplain textとして扱い、HTMLとして挿入しない。

### 3.3 Typed parameter

V1は次の閉じた型だけを持つ。

| type | exact表現 | 絶対範囲 |
|---|---|---|
| `length_micrometres` | 1 µm単位の`i64` | 0〜10,000,000,000 |
| `angle_microdegrees` | 0.000001°単位の`i64` | -180,000,000〜180,000,000 |
| `ratio_millionths` | 0.000001単位の`i64` | 1〜1,000,000,000 |
| `integer` | `i64` | -1,000,000,000〜1,000,000,000 |
| `boolean` | JSON boolean | `false` / `true` |
| `choice` | option ID | 1〜32 options |

数値型は`minimum <= default <= maximum`を必須とする。NaN、Infinity、binary floating point、単位なし小数、数式文字列は存在しない。choiceのdefaultと比較literalは宣言済みoptionを参照しなければならない。

### 3.4 Preconditions

preconditionは自由形式の式ではなく、次の閉じたtyped ASTだけである。

- `all` / `any` / `not`
- `parameter_comparison`
- `capability_available`
- `user_confirmation`

parameter比較は宣言済みparameter IDを参照し、literal型と値域を一致させる。booleanとchoiceは`equal`または`not_equal`だけを許可する。source expression、変数展開、関数呼出し、property access、文字列補間は持たない。

ASTは深さ8以下、1技法あたり合計512 nodes以下とする。1技法あたりnamed preconditionは128件以下、1 operationからの参照は32件以下である。dangling参照と重複参照を拒否する。

### 3.5 Ordered operationsとcapability

各operationはID、localized name、closed action、parameter binding、precondition参照、required capability、execution supportを持つ。required capabilityは「hostへ要求する条件」であり、共有ファイルがhostへ与える権限ではない。全operationで1件以上を必須とし、action固有capabilityを欠く、重複する、または別の未対応物理capabilityを混在させる値を拒否する。

V1の`execution_support`には`executable`または`supported`値が存在しない。

| action | 必須capability | 必須execution support |
|---|---|---|
| `instruction_cue` | `human_interpretation_v1` | `declarative_only` |
| `straight_line_stacked_fold` | `straight_line_stacked_fold_v1` | `declarative_only` |
| `inside_reverse_fold` | `inside_reverse_fold_motion_v1` | 同名の`unsupported_physical_operation` |
| `outside_reverse_fold` | `outside_reverse_fold_motion_v1` | 同名の`unsupported_physical_operation` |
| `sink_fold` | `sink_fold_motion_v1` | 同名の`unsupported_physical_operation` |
| `layer_selective_manipulation` | `layer_selective_motion_v1` | 同名の`unsupported_physical_operation` |

`declarative_only`も自動実行許可ではない。将来hostが一部actionを実行できるようになっても、別versionのhost側admission、project/revision照合、衝突・経路証明、明示操作および原子的commandが必要である。

## 4. Attribution metadata

authorsは1〜8件、各120文字かつ480 UTF-8 bytes以下とする。sourceは`user_authored`、`adapted`、`published_reference`の閉じたprovenanceである。後二者の`citation_text`は最大1,024文字かつ4,096 UTF-8 bytesの不活性なplain textであり、URLやpathとして解釈・取得しない。

licenseは最大64 ASCII bytesの単一SPDX identifierを格納する。V1はlicense expression evaluatorを持たない。

## 5. Resource limit

すべてhard ceilingであり、callerが緩和するAPIを公開しない。

| 対象 | 上限 |
|---|---:|
| encoded JSON | 1 MiB |
| raw JSON structural depth | 32 |
| techniques / file | 64 |
| authors / file | 8 |
| locales / localized field | 8 |
| parameters / technique | 64 |
| choice options / parameter | 32 |
| named preconditions / technique | 128 |
| precondition depth | 8 |
| precondition nodes / technique | 512 |
| ordered operations / technique | 256 |
| parameter bindings / operation | 32 |
| precondition references / operation | 32 |
| required capabilities / operation | 8 |
| identifier | 96 ASCII bytes |

untrusted bytesは1 MiBとraw depthを検査してからSerdeへ渡す。semantic validation後もcanonical JSONを再生成して1 MiB以下を確認するため、escaped inputとcaller生成documentのどちらも同じ出力上限に従う。

## 6. ID、参照、重複

package、technique、parameter、precondition、operation、binding role、choice optionのIDはlowercase ASCII英字で始まり、lowercase ASCII英数字で終わる。途中はlowercase ASCII英数字と単独の`.`、`-`、`_`だけを許可する。`/`、`\`、`:`、空白、連続separatorを拒否し、IDをpathやURLとして再利用できないようにする。

次を全件検査する。

- technique、parameter、precondition、operation、choice option、binding role、locale、author、capability、precondition参照の重複
- parameter binding、parameter comparison、choice default/literal、operation preconditionのdangling参照
- parameter literalの型・値域・comparison整合
- action、required capability、unsupported physical operationの三者整合

エラーへuntrusted文字列を反射せず、固定categoryだけを返す。

## 7. Deterministic JSONとread-back

`validate_fold_technique_file_v1`は意味上unorderedな集合をcanonical順へ並べ、全参照とhard ceilingを検証したprivate handleを返す。private handleはread-only documentだけを公開し、不正なfield変更を許可しない。

`write_fold_technique_file_v1`は次をすべて成功した場合だけbytesを返す。

1. field順が固定されたstructからcompact JSONを生成
2. encoded sizeを再検査
3. 公開readerへbytesを戻して独立にstrict parse・semantic validation
4. canonical handleをbit-exactなRust値として比較

operation順序を除くunorderedな入力順が異なっても同じcanonical bytesとなる。同一bytesをread/writeしても変化しない。

## 8. 必須回帰

正本testは少なくとも次を固定する。

- 中割り、かぶせ、open sinkのfixtureが、対応する未対応物理操作を明示したmetadataとしてround-tripする
- unknown field/enum、duplicate JSON key、script、code、path、URL、project command、式注入を拒否する
- file bytes、raw/semantic depth、件数、文字、UTF-8 bytes、数値範囲、ID、重複、dangling参照、型違いを拒否する
- unsupported physical operationの`declarative_only`への偽装、別operation名への差替え、必要capability欠落を拒否する
- unordered集合の順序差が同じbytesへcanonical化される
- boundedな多数の利用者定義fixtureについて`write(read(write(x))) == write(x)`を確認する
