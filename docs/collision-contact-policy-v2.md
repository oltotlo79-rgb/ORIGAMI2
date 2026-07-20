# 衝突接触ポリシー v2

## 1. 地位と互換性

`topology_contact_policy_v2`は、正厚材料どうしの境界面接触を欠落なく表すための正式な分類表である。初版の[`topology_contact_policy_v1`](collision-contact-policy.md)が定めた40セルの意味は変更せず、`boundary_line_contact`の直後へ`boundary_area_contact`を追加する。

- 正規corpus: [`collision-contact-policy-v2.json`](collision-contact-policy-v2.json)
- 実装カバレッジ台帳: [`collision-classification-coverage-v2.md`](collision-classification-coverage-v2.md)
- 正厚モデル: `centered_mid_surface_v1`
- 共有関係: v1と同じ4種類
- 交差証拠: 11種類
- 完全表: 4×11＝44セル
- 実装: frontendとnativeは同じ正規corpusへ全44セルを照合する

v1の証拠をv2へ同名で写像した40セルは、必ずv1と同じdecisionを返す。v1の正規corpusと純粋分類関数も互換性検証用に残し、再解釈や暗黙の移行を禁止する。新しいnative衝突証拠生成器はv2だけを証明書へ結合する。

## 2. 追加証拠

### `boundary_area_contact`

`t > 0`の二つの閉じた材料領域について、完全な交差集合が両領域の境界上にある正面積の面領域であり、材料内部どうしの交差および正体積重なりがないことを肯定的に証明した場合だけ生成できる。

この証拠は、中央面の`coplanar_area_overlap`とは異なる。中央面が正面積で重なる場合は、従来どおり`coplanar_area_overlap`であり、`same_face`以外では`penetrating`とする。正面積らしく見える、SATの間隔がmargin内である、またはraw分類が`touching`であるという理由だけで`boundary_area_contact`を発行してはならない。

次の場合は`boundary_area_contact`へ丸めず`indeterminate`とする。

- 正体積0を証明できない
- 接触面積が正であることを証明できない
- 近平行、退化、数値marginまたは作業上限のため交差次元を確定できない
- 入力姿勢、材料厚、面identityまたは三角形対へのprivate provenanceを再結合できない

共有ヒンジ辺の境界面接触は、この表だけでは許容しない。有限共有軸、材料半平面、全三角形対および有限corridorを再検証するヒンジモデルへ必ず渡す。共有軸上の正厚矩形接触だけが証明された場合は`boundary_contact`として許容できるが、corridor外へ広がる面接触は`outside_hinge_contact`としてblockingにする。

## 3. 共有関係4種×交差証拠11種の完全表

| 共有関係＼交差種別 | 離間 | 点接触 | 境界線接触 | 境界面接触 | 共有要素のみ | 共有要素近傍の中央面基準正厚重なり | 共有要素平坦積層 | 共面正面積重なり | 横断 | 正体積重なり | 不明 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 共有なし | `separated` | `touching` | `touching` | `touching` | `indeterminate` | `indeterminate` | `indeterminate` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 共有頂点 | `indeterminate` | `touching` | `touching` | `touching` | `allowed_shared_vertex_contact` | `allowed_shared_vertex_contact` | `indeterminate` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 共有ヒンジ辺 | `indeterminate` | `indeterminate` | `indeterminate` | `requires_hinge_model` | `requires_hinge_model` | `requires_hinge_model` | `requires_hinge_model` | `penetrating` | `penetrating` | `penetrating` | `indeterminate` |
| 同一面 | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` | `ignored_self` |

新列のdecisionは次の理由で固定する。

- `no_shared_feature`: 非貫通の正面積境界接触なので`touching`
- `shared_vertex`: 共有頂点による免除ではなく、一般の非貫通境界接触として`touching`
- `shared_hinge_edge`: 共有軸固有の許容範囲かを表だけでは確定できないため`requires_hinge_model`
- `same_face`: 同じ正規面identityの自己組なので`ignored_self`

## 4. 証明と実行時境界

文字列、公開enumまたは表のdecisionだけでは、停止除外や共有要素由来の許容を認可しない。証拠生成器は、現在姿勢、面identity、三角形index、トポロジーsnapshot、材料厚、変換、数値marginおよびpolicy versionを一つのprivate certificateへ結合する。

実行時dispatcherは、同じ解析呼出しで発行されたcertificateを元の入力参照へ再結合し、v2表のdecisionと一致した場合だけ結果を採用する。clone、別姿勢、別厚さ、左右入替、別三角形対、別policy versionまたは証拠種別の付け替えは`indeterminate`へ倒す。

重大度の集約順はv1と同じである。

```text
penetrating > indeterminate > touching > separated
```

`indeterminate`は安全ではなくblockingであり、UIでは「交差の可能性・判定保留」として貫通と同等に目立たせる。

## 5. 現在のnative実姿勢接続

2026-07-19時点で、bit-exactな紙厚`+0.0`に限定したblocking専用の
ゼロ厚み面貫通・重なり証明を、公開静的衝突入口へ接続している。
三角形どうしの横断肯定には、同じissuer/poseへ束縛したcanonical exact Cayley姿勢
`E`と、保存binary64 affine係数をactual mmの有理数へ直接liftした姿勢`F`の双方で、
両面relative interiorの横断を厳密に証明する必要がある。旧zero-thickness集約だけで
このdual gateを迂回できない。

同じexact pose instance、canonical face registry、全unordered face pairおよび
全triangle-pairを認証済みのzero-thickness集約からは、180度の
`coplanar_area_overlap`と、少なくとも一方が非三角whole material faceである
`transversal_crossing`もblocking肯定する。共有頂点・共有辺上だけの点接触、
線接触または共有要素接触はどちらの肯定経路にも入らない。`-0.0`、正厚、
証拠不足および資源上限も肯定へ丸めない。

公開入口は旧zero-thickness解析の全pairとtriangle-pairを先に照合し、
`Penetrating` decisionだけを肯定証拠へ流用しない。上記2種類に限定したexact evidence
とdecisionの整合、whole material boundaryの頂点数およびpose/registry/pair完全性を
同時に再検証する。新旧二段およびCayley内部三段の累積workは一つの呼出側上限へ合算し、
one-shortでは部分的な肯定結果を返さない。
証明済みのゼロ厚み面貫通・共面正面積重なりはblocking errorにだけ変換し、
collision-free proofの発行条件を広げない。desktop wire/UIは
「ゼロ厚み面貫通・重なり」へ一般化し、旧横断専用DTOを受理しない。

これは4×11表の一部の実geometry issuerであり、表そのものを置き換えない。
正厚の一般的な共有ヒンジ・非三角面、連続経路certificateおよび場所別層順transportは
後続段階である。2三角形面・1共有ヒンジの限定production分類は次節のとおり接続済みで
ある。

## 6. 公開診断の6分類とproduction admission

純粋な4×11表のdecisionと、利用者へ返すpair dispositionは別の層である。公開静的診断は
全unordered face pairをcanonical順で保持し、次の6分類のいずれかを返す。

| disposition | 公開上の意味 |
| --- | --- |
| `separated` | 認証済み幾何が離間を確定した |
| `touching` | 非貫通の接触を確定した |
| `allowed` | 共有頂点、または限定された共有ヒンジmodelが接触を説明した |
| `penetrating` | production admission gateが面横断、共面正面積または正体積を肯定した |
| `indeterminate` | 交差の可能性を排除できない、または必要なmodelが未対応である |
| `candidate_excluded` | 候補生成段階の予約分類。全pair診断の現行runtime snapshotでは発行しない |

4×11表が`penetrating`を返しただけでは公開`penetrating`へ昇格させない。厚さ0の
canonical exact `E`とdirect-lift `F`によるstrict transversal dual gate、認証済み
whole-face共面正面積、または対応する正厚production gateのいずれも肯定しない場合は、
公開分類を`indeterminate`へ閉じる。逆に、独立したexact gateが肯定したpairは下位の
raw証拠が丸め誤差により`indeterminate`でも`penetrating`とし、公開pairの証拠を
`transversal_crossing`または`coplanar_area_overlap`へ正規化してproof provenanceを
必須保持する。proof markerと正規化済み証拠・policyが矛盾するwire payloadは拒否する。

紙厚がbit-exactな`+0.0`で、一直線の共有ヒンジを完全な境界辺として持つ2三角形は、
watertight exact poseで別途分類する。非平行な2平面の交線が共有辺だけである場合、
または共面で両対頂点が共有辺の反対側にある場合は、共有辺だけの接触として`allowed`に
する。共面で両対頂点が同じ側にある場合は共有辺近傍に正面積重なりが存在するため
`coplanar_area_overlap`・`penetrating`とする。binary64上の共有端点driftを許容半径で
吸収してはならない。対象pairだけを有限資源上限付きでexact theoremへ渡し、上限超過は
部分結果を返さずfail-closedとする。

有限な正厚では、初版の`centered_mid_surface_v1`に従い、2三角形面・1共有ヒンジだけを
complete `E/F` solid classifierへ接続している。完全交差集合が有限ヒンジmodelで説明
できた`boundary_area_contact`または`shared_feature_thickness_overlap`は`allowed`、
証明済み`positive_volume_overlap`は`penetrating`、層ずらし未再現または証拠不足は
`indeterminate`とする。`allowed`はこの限定pairの「許容積層」であり、multi-face全体の
collision-free proofではない。

UIは`penetrating`と`indeterminate`を同じ赤系の最上位警告として表示し、沈黙による
安全誤認を禁止する。集約件数に加えてcanonical pair行を表示し、表示上限を超える場合も
blocking pairを先に残して総件数・表示件数・省略件数を明記する。
nativeとrendererが受理する完全pair snapshotのhard capは同じ50,000件である。callerは
この上限を縮小できるが拡張できず、50,001件目はnative側でallocation・serialization前に
`ResourceLimitExceeded`へ閉じる。

wire reason `proven_positive_thickness_penetration`は正厚材料貫通の総称である。pair詳細の
`strictTransversalDualGateProven=true`と`transversal_crossing`は中央面横断を、
`sharedHingeSolidClassified=true`と`positive_volume_overlap`はcomplete solid
classifierによる正体積重なりを表す。総称reasonだけから証明経路を推測してはならない。

正厚2三角形共有ヒンジのexact 90度は、有限回廊のbinary64境界でcanonical vertex IDに
よる軸反転がdirect-lift `F`の最終bitを変え得るため、初版では常に`indeterminate`へ
固定する。ID、identity namespace、source格納順、root、hinge端点方向、山谷によって
`allowed`へ変化させてはならない。
