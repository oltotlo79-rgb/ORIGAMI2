# Normative Phase 2-C clarifications (2026-07-19)

This section overrides every ambiguous or conflicting statement below. The
literal-F baseline and the C2 affine-corridor experiment are private and
disconnected from production collision policy, DTOs, persistence, UI, safe
sets, and project mutation.

C2 may emit only
`ContainedUnadmitted(Box<DirectFAffineHingeCorridorDiagnosticV1>)`,
`Outside`, `LayerOffsetUnmodeled`, or `Unresolved`. The contained value is a
sealed geometry diagnostic, not a capability or certificate. C2 emits zero
`Certified` results. Neither a contained result nor a revalidated diagnostic
may be mapped to collision allowance.

Literal-F equality at the two hinge endpoints is not a sufficient zero-drift
fast path. Admission also requires all relevant boundary `F(v) = E(v)`
relations and material-normal/solid relations, followed by an approved
boundary-admission policy and the native exact topology margin. Those native
proofs and policy constants do not yet exist, so this phase must not invent a
threshold or issue admission authority.

A future production gate must borrow a separate, non-cloneable
`DirectFSharedHingeAdmissionCapabilityV1`. Its issuer must bind the
prerequisite, E/F boundary, exact-E proof, exact pose, bound model/pose
instance, thickness bits, face/hinge identities, all 24 face-transform and 12
hinge-parent binary64 coefficients, policy version, and sealed work. The
required order is policy availability, capacity preflight, full authority
revalidation, complete boundary point/normal/solid checks, approved
`B_raw/B_admit` checks, positive-thickness point-plus-normal checks, native
exact topology-margin checks, then sealing. Until those prerequisites are
approved and implemented, the future admission issuer returns no capability.

Within C2, the complete `P_F` vertex-by-10-halfspace containment scan precedes
corridor feasibility, recession, and Gram interpretation. Any outside vertex
is recorded only after the full containment scan. If all vertices are inside,
the positive nonempty `P_F` proves `C_F` nonempty; only then may the complete
120 plane-triple scan, 45 normal-pair by both-sign recession scan, and parent
Gram checks run. Phase 2-B work is inherited once, and literal-F prism rerun
work plus C2-local work are each added exactly once.

## Outside evidence is not authority

The current cloneable `DirectFFiniteHingeCorridorOutside` value is a numeric
diagnostic summary only. It is not a capability and the C2 diagnostic must not
consume it as authority. C2 reruns the complete literal-F scan rather than
trusting that earlier result. A future phase may instead
consume a future borrow-bound, non-cloneable sealed Outside capability. Such a
capability must bind the prerequisite, E/F boundary, exact-E token, literal-F
issuer token, model/pose/root, face and hinge identities, thickness bits, all
36 binary64 transform coefficients, the complete Phase 2-B work snapshot, and
the complete literal-F work snapshot. Revalidation must compare every binding
before any C2 work starts.

## Canonical source coordinates

The source half-prism must not use an arbitrary independent inward vector.
For authenticated hinge endpoints `p` and `q`, let `d = q - p`. Let `v` be the
authenticated opposite vertex of the triangular face and `w = v - p`. In the
exact source-paper metric define

```text
u_i = w - d * (w dot d) / (d dot d).
```

The prerequisite must prove `d dot d > 0`, `u_i dot u_i > 0`, and that positive
`u_i` points into the authenticated material half-plane. Coordinates are the
unique coefficients of

```text
x - p = lambda_i(x) * d + alpha_i(x) * u_i + beta_i(x) * e_y,
```

equivalently the three dual covectors obtained from the exact inverse of the
basis `[d, u_i, e_y]`. The source half-prism is therefore normatively

```text
0 <= lambda_i(x) <= 1,  alpha_i(x) >= 0,  -h <= beta_i(x) <= h.
```

Positive rescaling of `u_i` may rescale `alpha_i`, but must leave this
half-prism unchanged. Replacing `u_i` by `u_i + k*d` is forbidden because it
shears the endpoint planes and changes the physical set selected by
`0 <= lambda_i <= 1`. C2 has an explicit proof and regression obligation to
show invariance under positive scale and authenticated face/source ordering,
and to reject a sheared inward direction.

# Direct-F affine hinge corridor C2 設計メモ

## 状態

この文書は、正厚さ Phase 2-C の private C2 幾何診断
`DirectFAffineHingeCorridorDiagnosticV1`の設計と実装境界を記録する。
proof model ID、admission policy、閾値および production 接続は未承認である。
C2 は `ContainedUnadmitted` 診断を発行するが capability/certificate は発行しない。
既存の exact-E 回廊、literal direct-lift F 回廊、衝突分類、safe set、連続衝突、
DTO、UI、保存形式および mutation authority を変更しない。

C2の前段として維持する旧literal-F baselineは、保存済み binary64 係数を
有理数へ直接 lift した affine map `F`について、従来の unit-normal 回廊式を
一切拡張せず厳密に判定する。このbaseline単独の歴史的な400 mm正方形
1ヒンジ・2三角形fixtureでは、Mountain/Valley、source/reordered、両 root、
紙厚 0.1/1/3 mm、角度0/10/90/135/179度の120件中、96件が `Contained`、
90度の24件がstrict `Outside`となった。これは現行C2の結果ではない。現行C2は
同じ120件すべてを診断専用`ContainedUnadmitted`として固定している。
旧baselineの最小fixture（Mountain/source、先頭root、0.1 mm、90度）の最初の
半径超過量

```text
c2*|(x-A)×d|² - h²*|d|²
= 44993417196633807545963059937356168830766334170625
  / 822752278660603021077484591278675252491367932816789931674304512
≈ 5.46864692612954e-14 mm^4
```

は正である。これは E/F component box で吸収したり、epsilon でゼロに丸めたり
してはならない。

## なぜ別の回廊が必要か

各 face の binary64 affine map を

```text
F_i(p) = A_i p + t_i
```

とする。非 cardinal 回転だけでなく、斜め軸の90度回転でも、`A_i` の係数を
BigRational へ直接 lift すると、列は一般に厳密な直交基底ではない。

```text
m_i = A_i e_y
m_i·m_i != 1
(A_i e_x)·m_i != 0 または (A_i e_z)·m_i != 0
```

したがって、unit かつ軸直交の co-oriented normal を前提に導出した

```text
c2 = (1 + n_L·n_R) / 2
c2*|(x-A)×d|² <= h²*|d|²
```

を literal `m_i`へそのまま適用すると、実際に構成した
`F_i(mid_surface) +/- h*m_i` と一致しない。係数誤差、E/F box、任意 tolerance
で右辺を拡張する方法は、相関を失い偽許容を作るため採用しない。

## 入力と authority

C2 は次の既存 private capability を同時に borrow し、利用時にも全て再認証する。

1. `AuthenticatedSingleTriangularHingePrerequisitesV1`
2. `AxisAlignedEfBoundaryCapabilityV1`
3. `ExactEFiniteHingeCorridorCapabilityV1`

同一 exact object pointer、native model/pose instance、左右 face index、
hinge index、紙厚 binary64 bits、全 face transform 24係数、hinge-parent
transform 12係数、および Phase 2-B の sealed work を再結合する。既存
literal-F の `Outside` 診断は authority として信用せず、同じ issuer 内で
literal-F prism の完全 exact scan を再実行する。E/F component box の数値は
authority の identity 以外には使用しない。

source geometry は認証済みの shared rest hinge endpoints、各 face の反対向き
boundary occurrence、hinge に対する内向き材料半平面、および canonical rest
座標だけから構成する。caller 提供の点、法線、軸、半径、matrix、box または
tolerance は受け取らない。

## Affine half-prism corridor の定義

各 face の source 面上で、shared hinge の始点を `p`、終点方向を `q-p`、
hinge から face 内部へ向かう独立な面内方向を `u_i` とする。face `i` の
局所的な正厚 hinge half-prism を次の集合とする。

```text
H_i = {
  F_i(p + lambda*(q-p) + alpha*u_i + beta*e_y)
  | 0 <= lambda <= 1, alpha >= 0, -h <= beta <= h
}
```

`F_i` は affine なので `H_i` は凸 polyhedron である。`A_i` が可逆なら、
source の5閉条件

```text
lambda >= 0
lambda <= 1
alpha >= 0
beta >= -h
beta <= h
```

を inverse-transpose で world の5閉 halfspace へ厳密に移せる。左右を別々に
変換し、C2 回廊を

```text
C_F = H_L ∩ H_R
```

と定義する。

この定義では parent と child の hinge endpoints を平均・置換・weld しない。
各々の `A_i p+t_i` と `A_i q+t_i` が、それぞれの5 halfspace にそのまま
寄与する。従って endpoint drift は `C_F` の形状へ相関を保ったまま入る。

実三角柱は各 source triangle の affine image と `beta in [-h,h]` から構成する。
Phase 1 の対向材料半平面が正しければ、shared hinge に起因して許容できる局所重なりは
`C_F` 内にある。一方、face 内部の別位置で発生する交差は `C_F` 外となり、
完全交差集合の包含検査で拒否できる。

## 完全性、boundedness、包含

左右合計10 halfspace の全 `C(10,3)=120` 平面三つ組を canonical 順に走査する。
非特異な組だけを Cramer 法で解き、全10閉 halfspace 内の候補を exact rational
で重複除去する。これは既存 exact prism kernel と同じ complete vertex
enumeration 原理を使うが、各 `H_i` は単独では半無限なので、次の独立した
recession-cone 証明を追加する。

各 halfspace の homogeneous normal を `a_j` とする。全 `C(10,2)=45` 組について
`r = a_j × a_k` の両符号を調べ、非ゼロ `r` が全ての
`a_l·r <= 0` を満たす場合は非有界とする。normal rank が3未満、または
非ゼロ recession direction を排除できない場合も capability を発行しない。
3次元 polyhedral recession cone が非自明なら extreme ray または lineality
direction がこの走査に現れることを、実装前に形式化して固定する。

既存 direct-F 三角柱どうしの完全交差 polytope を `P_F` とする。`C_F` が有界で
正しい closed halfspace 集合を持つ場合、`P_F` の全 canonical vertex を
`C_F` の全 halfspace に照合する。線形関数は convex polytope の頂点で最大となるため、
全頂点の合格は `P_F ⊆ C_F` を証明する。最初の outside を見つけても走査を
短絡せず、全頂点・全 halfspace を課金して確認する。

rank 3 の positive volume、または左右 prism の opposing support facet を持つ
rank 2 positive area だけを対象にする。empty、rank 0/1、support 未証明 rank 2、
不正 input、資源超過は許容 capability を発行しない。

## Gram metric による有限半径条件

`C_F` が有界でも、平坦折り付近では hinge 長に対して過大な局所重なりを許し得る。
parent affine map の線形部を `A_P` とし、`det(A_P) != 0` を exact に証明する。
world vector に対する parent-rest metric は

```text
G_P = (A_P^-1)^T * A_P^-1
```

である。parent path の world hinge direction を `d=A_P(q-p)`、
回廊点の parent start からの差を `r` とし、軸直交距離の二乗を

```text
Q_P(r) =
  r^T G_P r
  - (r^T G_P d)^2 / (d^T G_P d)
```

と定義する。平方根と法線正規化は不要で、全て BigRational で計算できる。
`G_P` の正定値性、`d^T G_P d > 0`、分母の canonical 性を再検証する。

`Q_P` は convex quadratic なので、有界な `C_F` 上の最大値は extreme point
のいずれかで達成される。全 corridor vertex を走査して

```text
R_F² = max Q_P(vertex - parent_start)
D_P² = d^T G_P d
```

を求め、閉境界 `R_F² <= D_P²` の場合だけ有限 hinge とする。strict
`R_F² > D_P²` は `LayerOffsetUnmodeled` とする。実交差 `P_F` に対する
軸方向条件は parent metric で

```text
0 <= r^T G_P d <= D_P²
```

とし、両端を閉境界として受理する。

## Rigid 極限との同値

全 `A_i` が厳密な回転、parent/child endpoints が一致し、左右 source
half-plane が hinge に対して反対側にあり、`e_y` が hinge と面に直交する極限では、
各 `H_i` の軸直交断面は幅 `2h` の半無限 strip になる。その交差
`C_F` は従来の centered-slab wedge overlap であり、最遠 extreme point の距離は

```text
R_F = h / cos(theta/2)
R_F² = h² / c2
c2 = (1 + n_L·n_R) / 2
```

となる。また `G_P=I` なので、`R_F² <= D_P²` は既存の
`h² <= D²*c2` と一致する。

C2 実装前に、上記の halfspace 変換、boundedness、および最遠点について代数的な
同値証明を test fixture だけでなく設計上の proof obligation としてレビューする。
rigid fixture では C2 と exact-E の interaction kind、corridor boundary、
`R=L`、0/10/90/135/179度および180度の結果が一致しなければならない。

## `LayerOffsetUnmodeled` と `Unresolved`

次は物理モデルの限界として `LayerOffsetUnmodeled` へ閉じる。

- `C_F` が非有界、または非有界性を排除できない。
- rigid 極限の `c2 <= f64::EPSILON` に対応する flat-fold 退化。
- `R_F² > D_P²`。
- 180度で有限の層ずらしなしには half-strip overlap を局所化できない。

次は証拠不成立として `Unresolved` へ閉じる。

- `A_i` または `A_P` が非可逆、`G_P` の正定値性を証明できない。
- source half-plane、hinge endpoint、face/hinge registry、transform bits、
  token、pose instance、root、紙厚または interaction kind の再認証失敗。
- canonical rational 不成立、halfspace/facet incidence 不成立、rank/support
  不成立。
- checked arithmetic failure。

資源上限到達は`Unresolved`値へ混ぜず、呼出側が再試行・上限不足を識別できる専用の
`ResourceLimitExceeded` errorとして返す。このerror、`LayerOffsetUnmodeled`、
`Unresolved`のいずれもproductionでは許容へ変換しない。

## 資源上限候補

値は未承認であり、実装・実測・one-short 後に owner が凍結する。

- authenticated faces 2、hinges 1
- face transform coefficient bindings 24、hinge-parent bindings 12
- affine half-prisms 2、halfspaces 10
- plane triples 120、membership tests 最大1,200
- recession normal pairs 45、signed ray tests 90、ray membership tests 最大900
- retained corridor vertices 最大120、dedup comparisons 最大7,140
- direct-F prism intersection vertices 最大120
- corridor containment tests 最大1,200
- Gram quadratic vertex tests 最大120、axial tests 最大120
- source coordinate lift、3x3 determinant/inverse、inverse-transpose、
  matrix multiply、dot/cross/division、paper thickness exact divisionを全て個別課金
- rational allocation 数、個別 allocation bits、累積 allocation bits、
  GCD fallback、shift、中間 bit 数を既存 `WorkMeter` へ単調加算

Phase 2-B の sealed cumulative work を一度だけ継承し、literal-F 再構築、
prism 再走査、C2-local 証明をそれぞれ独立 meter で hard-limit 内に計測した後、
各差分を一度ずつ原子的に merge する。前段 work や別段の未使用予算は借用せず、
二重加算もしない。caller 指定上限は hard cap との minimum へ射影する。全
structural counter、各 local exact counter、cumulative exact counterについて
exact-limit 成功、各 one-short 失敗、overflow、caller による hard cap 拡張拒否を
固定する。

## 必須回帰

- C2 の120件すべてが `ContainedUnadmitted` であり、180度24件が
  `LayerOffsetUnmodeled` であること。Mountain/Valley、source/reordered、
  両 root、紙厚、角度ごとの対称性を固定する。C2 の `Certified` 件数は0である。
- literal C で strict Outside の90度24件が、C2では数学的に正当な
  affine corridorへ `ContainedUnadmitted` となること。ただし epsilon や box
  による昇格でも、collision allowanceへの昇格でもないことを exact work と
  geometry snapshot で確認する。
- 旧baselineでContainedだった0/10/135/179度の96件、180度24件、`R=L`、
  軸端・回廊境界。
- parent/child endpoint driftを 0 ULP に weld しない。実測した角度別 mismatch
  合計 `[0,24,24,24,24]` を固定し、non-cardinal 全96件で drift が存在しても
  結果が診断専用 `ContainedUnadmitted` のままであることを確認する。
- affine shear、非直交、非有界half-strip。
- face/source順、root、山谷の入替え、巨大平行移動。
- token、ABA、foreign issuer、reroot、角度/紙厚/全36 transform係数の1 ULP。
- 完全120平面走査、完全recession走査、完全intersection vertex走査。

次はC2 checkpointの完成を示すものではなく、将来のproduction admission gateを
開く前に追加する回帰義務として追跡する。

- 列normの過大/過小、可逆限界、特異matrix、recession lineality。
- 共通linear affine frame changeに対する共変性。
- E/F component boxの全値を独立に変えた場合も、C2幾何がそのbox数値を
  toleranceとして流用しないこと。

## Production gate

C2 は次の全条件を満たすまで private のままとする。

1. owner が数学仕様、proof model ID、全定数、binary64 lift規約、資源上限を承認する。
2. 独立レビューで halfspace 変換、boundedness、Gram 最大値、rigid 極限同値を確認する。
3. exact-E と C2 が同じ interaction kind を返し、双方が contained である。
4. literal C の結果を上書き・再解釈せず、C2 は versioned diagnostic だけを
   発行する。future production gate は別の
   `DirectFSharedHingeAdmissionCapabilityV1`を borrow して初めて Certified を
   発行できる。
5. 全pair coverage、polygon/multi-hinge、layer order、continuous collision の
   後続gateと結合する。

この承認前に、正厚衝突分類、許容積層、折り重ね、safe set、UI表示または
project mutationへ接続してはならない。
