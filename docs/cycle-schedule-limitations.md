# 閉路 schedule の到達範囲

## 実装済みの安全境界

- `half-angle rational schedule v1` は hinge を canonical `EdgeId` 順で受け付ける。
- 有理係数、次数、hinge 数、exact arithmetic work、二進分割深さと leaf 数を固定上限で検査する。
- schedule の係数全体を SHA-256 指紋へ含め、別 schedule の区間閉路証明との差し替えを拒否する。
- material-local hinge transform を canonical spanning tree から world pose へ右合成し、全 spanning hinge と全 closure hinge を外向き区間で照合する。
- `u=0` と `u=1` は近傍 sample ではなく exact rational Horner 評価から外向き角度区間を作り、initial pose と requested pose をそれぞれ認証する。
- native preview は project instance、project、revision、fold-model fingerprint、pose generation、layer-order generation を再検証する。成功した token は一回だけ明示適用できる。
- schedule がない cycle、unknown field、非 canonical ID、係数上限超過、証明不能 leaf、旧 uniform endpoint 診断だけの入力は mutation token を得ない。

## 未完了の production fixture

現行の production cross-cycle fixture は、正方形へ二本の対角線を順に追加して作る。この fixture で確認済みの閉じた要求は flat pole の 180°だけである。`tan(angle / 2)` は 180°で有限な有理値を持たないため、pole-free half-angle rational v1 でこの fixture を表現してはならない。90°の uniform 要求は endpoint 自体が閉じず、既存試験が fail-closed を確認している。

spherical Kawasaki 型の非一様 4R fixture を完成させるには、次の両方が必要である。

1. sector angle が明示された production crease pattern。
2. その同じ geometry から導出した、pole-free な tangent-half-angle 関係の exact rational coefficients。

係数を推測した fixture や、endpoint だけを閉じた fixture は continuous closure の証拠として追加しない。したがって現時点では 4R の ready → apply → Undo/Redo production integration fixture は未完了である。実係数 fixture が追加されるまで、該当 cycle は `cycle_path_uncertified` で停止する。
