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

2026-07-19時点で、三角形material faceとbit-exactな紙厚`+0.0`に限定した
blocking専用の横断証明を、公開静的衝突入口へ接続している。肯定には、同じ
issuer/poseへ束縛したcanonical exact Cayley姿勢と、保存binary64 affine係数を
actual mmの有理数へ直接liftした姿勢の双方で、両面relative interiorの横断を
厳密に証明する必要がある。共有頂点・共有辺上だけの点接触または線接触、
共面接触、非三角形、正厚、証拠不足、資源上限は、この横断肯定へ入らない。

公開入口は旧zero-thickness解析の全pairとtriangle-pairを先に照合し、その結果の
`Penetrating`表示を新しい肯定証拠へ流用しない。新旧二段およびCayley内部三段の
累積workは一つの呼出側上限へ合算し、one-shortでは部分的な肯定結果を返さない。
証明済み横断はblocking errorにだけ変換し、collision-free proofの発行条件を
広げない。

これは4×11表の一部の実geometry issuerであり、表そのものを置き換えない。
正厚の`boundary_area_contact`証拠、有限共有ヒンジcorridor、共面正面積の正式な
実姿勢issuer、連続経路certificateおよび場所別層順transportは後続段階である。
