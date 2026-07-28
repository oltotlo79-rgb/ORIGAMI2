# 実行環境間の証明再現性（`replayable_across_runtimes`）に関する提案

作成: 2026-07-27 / 作成者: Claude（読み取り専用監査）
対象: Codex
状態: **提案。コードは一切変更していない。**
計測基準: `21960f47`（2026-07-27 06:12、`origin/main` と一致）＋未コミット作業ツリー

---

## 0. 要旨

1. 本プロジェクト最大の差別化は「証明できたものだけを主張する」fail-closed 設計である。
2. しかしその証明は `replayable_across_runtimes() == false` により**そのプロセスの中でしか有効でない**。証明を外部価値へ変換できていない。
3. 原因は超越関数（`atan2` / `sin` / `cos` / `tan` / `hypot`）の最終ビットが libm 実装ごとに異なることで、**実測で本番48箇所・15ファイル**に限局している。想定より遥かに小さい。
4. **調査中に既存不具合を発見した。**プロジェクト読込時のビット完全比較が超越関数の結果を含むため、**環境をまたぐと保存ファイルが開けなくなる**。§2 に詳述する。これは将来課題ではなく現在の欠陥である。
5. 対処コストは**証明ファミリーが増えるほど単調に増加する**。現在15ファミリー・約35万行の時点で着手するのが最小コストである。
6. 推奨は「決定論的超越関数を `ori-numeric` へ実装し、**証明権威と永続データ生成の22箇所だけ**を差し替える」。数値ソルバの15箇所は権威でないため据え置き、性能を損なわない。

---

## 1. 現状の事実（実コード）

### 1.1 宣言

`crates/ori-core/src/constraint_exactification.rs:34-40`

```rust
/// The assignment is observation-only. It is not bound to a project or
/// revision, does not authorize mutation, and is not replayable across
/// runtimes whose transcendental operations may produce different last bits.
```

`replayable_across_runtimes()` は `constraint_exactification.rs:59` と `constraint_semantic_mus.rs:221` で恒常的に `false` を返し、frontend の strict parser も `record.replayable_across_runtimes !== false` を拒否条件としている（`geometricConstraintSemanticMus.ts:219, 309`）。

**この宣言は正しい。**問題は宣言ではなく、宣言せざるを得ない状態そのものである。

### 1.2 実測した超越関数の使用（本番コードのみ、`#[cfg(test)]` ブロックとテストファイルを除外）

| 関数 | 本番出現 | IEEE-754 が正確な丸めを義務付けるか |
|---|---:|---|
| `hypot` | 17 | **No** |
| `cos` | 9 | **No** |
| `sin` | 9 | **No** |
| `atan2` | 8 | **No** |
| `sin_cos` | 3 | **No** |
| `tan` | 2 | **No** |
| **非移植 小計** | **48** | |
| `sqrt` | 26 | Yes（移植可能） |
| `to_radians` | 12 | 単一乗算のため再現する |
| `to_degrees` | 4 | 単一乗算のため再現する |

**非移植な本番呼び出しは48箇所、15ファイル**である。

> 注: `sqrt` は IEEE-754 が正確な丸めを義務付けるため環境間で一致する。`to_radians` / `to_degrees` は Rust std において定数との単一乗算であり再現する。したがって対処対象から除外してよい。

---

## 2. 【最優先】既存不具合: 保存ファイルが環境をまたぐと開けない

### 2.1 発生箇所

`apps/desktop/src-tauri/src/lib.rs:4680` の
`fn validate_loaded_numeric_expression_bindings(document: &ProjectDocument) -> Result<(), String>`
は **プロジェクト読込時**に走り、内部で次のビット完全比較を行う（`lib.rs:4719-4728`、同型が `4783-4788` にも存在）。

```rust
let radians = angle_degrees.to_radians();
if length_mm.to_bits() != polar.adopted_length_mm.to_bits()
    || angle_degrees.to_bits() != polar.adopted_angle_degrees.to_bits()
    || (polar.adopted_start_x_mm + length_mm * radians.cos()).to_bits()
        != binding.adopted_x_mm.to_bits()
    || (polar.adopted_start_y_mm + length_mm * radians.sin()).to_bits()
        != binding.adopted_y_mm.to_bits()
{
    return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
}
```

### 2.2 何が起きるか

`radians.cos()` / `radians.sin()` は環境依存である。したがって:

```
環境A（Windows / MSVC CRT）で極座標折線を含むプロジェクトを保存
        ↓  同一ファイル
環境B（Linux / glibc、macOS、あるいは別バージョンの CRT）で開く
        ↓
cos の最終1ビットが異なる → to_bits 比較が不一致
        ↓
PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE で読込拒否
```

**利用者から見ると「壊れていないファイルが壊れていると言われて開けない」。**復旧手段はない。

### 2.3 評価

- これは移植性の理論的懸念ではなく、**保存・読込という中核機能の環境間非互換**である。
- 現在は配布前かつ単一環境で開発しているため顕在化していないが、**CI を別 OS で回した時点、あるいは初めて他環境へ配布した時点で発火する**。
- 同種の危険は `pattern_edit_commands.rs:1106-1107` の
  `expected_x = start_position.x + length_mm * angle_radians.cos()` にもある。こちらも保存済み座標との照合に使われていないか要確認。

### 2.4 依頼（§6 の全体着手と独立に、単独で先行してよい）

以下のいずれかで閉じること。判断は Codex に委ねる。

- **(a) 決定論的 `sin`/`cos` を導入する**（§5 の案A）。根治。
- **(b) 保存側の値を正本にする。**読込時に再計算せず、保存された `adopted_x_mm` / `adopted_y_mm` をそのまま採用し、再計算値との一致は検証条件から外す。ただし保存値の改竄検出が弱まるため、別途 digest で保護すること。
- **(c) 角度を厳密表現へ移す。**`editor.rs:3471` が既に `angle_microdegrees` という整数表現を持っている。極座標の到達点を超越関数ではなく厳密有理数から導出できる角度に限定する。

**(b) は最小の変更で即座に不具合を止められるが、(a) を実施するなら不要になる。**両方やる必要はない。

---

## 3. 非移植48箇所の完全な分類

移植性への影響は箇所ごとに大きく異なる。**全部を直す必要はない。**

### 分類1: 証明権威（11箇所）— `replayable_across_runtimes` を直接決める

| ファイル:行 | 関数 | 役割 |
|---|---|---|
| `crates/ori-core/src/constraints.rs:3087` | `atan2` | `0.0_f64.atan2(dot)` — 固定角残差のゼロcross分岐 |
| `crates/ori-core/src/constraints.rs:3119` | `atan2` | `absolute_cross.atan2(dot)` — **固定角残差の本体** |
| `constraint_exactification/singleton_constructive.rs:241,242` | `sin`,`cos` | 単一制約 構成的SAT witness の候補生成 |
| `constraint_exactification/pair_constructive.rs:272,273` | `sin`,`cos` | 二制約 構成的SAT witness の候補生成 |
| `constraint_exactification/zero_closure_constructive.rs:303` | `atan2` | ゼロ長閉包 witness の角度分岐 |
| `constraint_exactification/zero_closure_constructive.rs:496` | `cos`,`sin` | 同 候補生成 |
| `constraint_exactification/zero_closure_constructive.rs:588` | `hypot` | 同 長さ評価 |

**`constraints.rs:3119` が最重要である。**`SameOrientationWithFixedNonParallelAngle` と `PerpendicularOrientationsWithFixedNonRightAngle` の全証明、および progress.md が繰り返し記す「production `abs(cross).atan2(dot)` が到達し得る全classをplatform演算で列挙」という手法そのものが、この1行に依存している。

### 分類2: 永続データ生成（11箇所）— 保存ファイル互換性を直接決める

| ファイル:行 | 関数 | 役割 |
|---|---|---|
| `crates/ori-core/src/editor.rs:3472` | `sin_cos` | 極座標レイからの折線作成 |
| `crates/ori-core/src/editor.rs:4016` | `sin_cos` | 同上（別経路） |
| `crates/ori-core/src/editor.rs:8125,8186` | `hypot` | 折線長・スナップ距離 |
| `apps/desktop/src-tauri/src/lib.rs:4722,4724,4783,4785` | `cos`,`sin` | **§2 の読込時ビット再検証** |
| `apps/desktop/src-tauri/src/pattern_edit_commands.rs:856` | `sin_cos` | 角度からの方向ベクトル |
| `apps/desktop/src-tauri/src/pattern_edit_commands.rs:1106,1107` | `cos`,`sin` | 極座標到達点の期待値 |

**この11箇所は証明以前の問題である。**パターンデータ自体が環境依存になるため、証明を移植可能にしても、**そもそも入力データが一致しない**。分類1より優先度が高い可能性がある。

### 分類3: 数値ソルバ（15箇所）— 権威ではない。**据え置き推奨**

`crates/ori-core/src/constraint_solver.rs` の 631, 635, 664(×2), 694, 729(×2), 730(×2), 743(×2), 745(×2), 746, 747。

progress.md が明記するとおり、ソルバの数値プレビューは権威として使われない。

> 有界solverの数値previewをその診断値・収束tolerance・rankを権威として使わず

したがって**ここは環境依存のままでよい**。むしろ内側ループで毎反復呼ばれるため、決定論的実装（後述のとおり10〜100倍遅い）へ置換すると性能が劣化する。

ただし1点確認を要する。progress.md は残差について「solver共有の乗算・減算順」と繰り返し述べている。`constraint_solver.rs:694` の `atan2` と `constraints.rs:3119` の `atan2` が**同一のヘルパを共有しているか、別実装か**を確認し、共有しているなら分岐させること。共有したまま片方だけ差し替えると、ソルバとverifierの一致という既存不変条件を壊す。

### 分類4: 入出力・表示・スケジュール（11箇所）— 権威ではない。**据え置き推奨**

`svg.rs:2604,2605,2616,2620`（SVG変換）、`fold_frames.rs:550`（FOLD書出の二面角）、`schedule.rs:777`、`stacked_fold_cycle_schedule.rs:230`（周期スケジュール角）、`continuous_path.rs:2930`（軸正規化）、`beginner_design_commands.rs:2125,2576`（設計ヒューリスティック）、`geometric_constraint_commands.rs:400,405`（DTO 表示値）。

いずれも証明権威でも永続データでもない。ただし `fold_frames.rs:550` は **FOLD書出結果が環境依存**になるため、書出ファイルの再現性を主張したい場合は分類2へ格上げすること。

### 分類まとめ

```
分類1 証明権威        11箇所  →  対処必須
分類2 永続データ生成  11箇所  →  対処必須（§2 の実バグを含む）
分類3 数値ソルバ      15箇所  →  据え置き（要: 共有ヘルパの分岐確認）
分類4 入出力・表示    11箇所  →  据え置き
                    ─────
                      48箇所      対処対象は 22箇所
```

---

## 4. なぜ今か（コストの時間依存性）

対処コストの主要部分は**既存回帰テストの再ベースライン**であり、これは証明ファミリー数に比例して増える。

現在の実測値:

```
sound direct family        15種
semantic MUS 認証済み      13/15 family
超越関数を参照するファイル  26（本番＋テスト）
テスト側の非移植呼び出し    34箇所
to_bits() によるビット比較  736箇所（本番＋テスト）
Rust 総行数                349,999（うちテスト 226,710 = 65%）
```

現行手法は「platform演算で到達クラスを全列挙して固定する」ものであるため、**回帰テストは現在のプラットフォームの `atan2` にピン留めされている**。決定論的実装へ移す際、この列挙を1回やり直す必要がある。

そして完成時のコード量は現在の実測トレンドから **約110万行**（現在の約2倍、直近の限界コスト31,000行/完成率1ポイントから外挿）と推定される。**同じ作業を完成時に行うと、再ベースライン対象は倍以上になる。**

> 補足: この外挿は完成率の定義が変わらない前提である。定義は 07-19→07-21 に一度再設定された実績があるため、推定には幅がある。ただし「今やる方が安い」という結論は定義変更に依存しない。

---

## 5. 選択肢

### 案A: 決定論的超越関数を `ori-numeric` に実装し、分類1+2 を差し替える 【推奨】

`f64::atan2` / `sin` / `cos` / `sin_cos` / `hypot` を、環境非依存の実装へ差し替える。

**実装方針**（既存資産で完結する）

`ori-numeric` は既に `num-bigint` と `num-rational` に依存しており、`HighPrecisionValue`、`CertifiedF64Interval`、`rational_sqrt_bounds`、`rational_interval_to_f64_outward` を持つ。外部依存を追加せずに次を実装できる。

1. 高精度定数 π を用いた**厳密な引数簡約**（binary64 入力は厳密有理数なので簡約は誤差なく行える）。
2. 十分なガードビットでの級数評価。
3. **1回だけ丸める**。丸め境界に落ちた場合は精度を上げて再評価する（Ziv の戦略）。
4. `#[deny]` またはレビュー規約で、分類1+2 のモジュールから `f64` の超越メソッドを直接呼ばないよう固定する。

**利点**

- 既に書かれた証明が**一斉に**移植可能になる。数学の再導出が不要。
- §2 の保存ファイル不具合が同時に解消する。
- `replayable_across_runtimes()` を `true` にできる。
- 分類3・4 を据え置けるため、**性能への影響が実質ゼロ**（証明経路は既に 2,000,000 work 相当の予算を持ち、データ生成経路は利用者操作ごとに1回しか走らない）。

**代償**

- 分類1+2 に関わる回帰テストの期待値を1回だけ再生成する必要がある。
- 決定論的実装は platform libm より 10〜100倍程度遅い（証明経路では許容範囲、ソルバ内側ループでは不可 → だから分類3は据え置く）。
- 決定論的実装自体の正しさを検証する必要がある。これは有理数区間による包含チェックで自己検証できる（`ori-numeric` の既存 `CertifiedF64Interval` が使える）。

### 案B: 区間演算＋証明された誤差限界

ビット一致を求めず、残差の厳密な包含区間を計算し、libm の誤差上限（多くは ≤1 ULP と文書化）を証明へ組み込む。区間がゼロを含まなければ**任意の準拠環境で**矛盾が成立する。

- 利点: CGAL 等の exact predicates と同じ王道。`ori-numeric` の `CertifiedF64Interval` が既にある。
- 欠点: **判定が保守的になり `Unknown` が増える。**本プロジェクトの現在の最大の弱点（「できません」が多すぎる）を悪化させる方向であり、現状とは相性が悪い。
- 誤差上限を文書値に依存すると、それ自体が検証されていない前提になる。

### 案C: 代数的書き換えで超越関数を消す

角度制約を `atan2` を使わない同値式へ移す。例: 「u と v のなす角が θ」を `cross(u,v)·cosθ = dot(u,v)·sinθ` の形にし、θ を厳密表現可能な範囲に限定する。

- 利点: 最も強い。議論そのものが消える。
- 欠点: **任意角度（利用者が 37.5° と入力する）を厳密表現できない。**適用範囲が構造的に限られる。
- 参考: `702488cd` の「代数的SAT証拠」は既にこの発想の部分適用である。案A と併用して、可能な範囲を案C、残りを案A で受けるのが自然。

### 比較

| | 移植性 | `Unknown` 増加 | 性能影響 | 既存証明の再導出 | §2 の不具合 |
|---|---|---|---|---|---|
| **A** | 完全 | なし | 分類1+2 のみ、許容範囲 | 不要 | **解消** |
| B | 完全 | **増える** | 中 | 一部必要 | 部分的 |
| C | 完全 | なし | 改善 | **必要** | 解消 |

---

## 6. 推奨する実施手順

**案A を軸に、§2 を先行して閉じる。**

### 段階0（先行・独立）: §2 の保存ファイル不具合を止める
`lib.rs:4680` の読込時ビット再検証から超越関数の再計算を除去する（§2.4 の (b)）。案A 完了後に (a) へ移行するなら暫定措置として扱ってよい。**この段階だけで既存の環境間データ非互換が止まる。**

### 段階1: 共有ヘルパの調査
`constraint_solver.rs:694` の `atan2` と `constraints.rs:3119` の `atan2` が同一ヘルパを共有しているかを確定する。共有しているなら、ソルバ用（platform）と証明用（決定論的）へ分岐させ、既存の「solver/verifier 優先」不変条件が保たれることを回帰で固定する。

### 段階2: `ori-numeric` へ決定論的実装を追加
`atan2` / `sin` / `cos` / `sin_cos` / `hypot` の5関数。外部依存は追加しない。自己検証として、既存 `CertifiedF64Interval` による厳密有理数区間の包含チェックを全関数に付ける。

### 段階3: 分類1（証明権威 11箇所）を差し替え
`constraints.rs:3119` を最初に行う。ここが済めば固定角2ファミリーの証明が移植可能になる。到達クラス列挙の回帰は、決定論的実装に対して1回だけ再生成する。

### 段階4: 分類2（永続データ 11箇所）を差し替え
段階0 の暫定措置を撤去し、(a) に統一する。

### 段階5: `replayable_across_runtimes()` を `true` へ
差し替えが済んだ証拠型についてのみ `true` を返す。**未差し替えの型は `false` のまま据え置くこと**（fail-closed 原則を崩さない）。frontend の strict parser（`geometricConstraintSemanticMus.ts:219, 309` ほか）は現在 `!== false` を拒否条件としているため、同時に更新が必要である。

### 段階6: 検証
最低限、**2つの異なる libm** で同一入力に対しビット一致することを実測する。本環境では次が使える。

```
Windows (MSVC CRT)  ネイティブ
WSL     (glibc)     既存の WSL 検証経路をそのまま使える
```

WSL 経路は Windows Application Control 回避のため既に運用されているため、追加インフラは不要である。この2環境で `to_bits()` 一致を固定する回帰を1本入れれば、移植性の主張に実測の裏づけが付く。

---

## 7. なぜこれが製品価値に直結するか

現在できないこと（すべて `replayable_across_runtimes == false` に起因する）:

```
✗ 保存ファイルに証明を同梱する
✗ 「この作品は折れることを検証済み」と第三者へ示す
✗ CI で検証してから利用者へ配布する
✗ チーム・教室で検証結果を共有する
✗ Web版・クラウド検証（サーバとクライアントで結果が変わり得る）
✗ そもそも別 OS でプロジェクトファイルを開く（§2）
```

本プロジェクトは「証明できるものだけを主張する」ために膨大な投資（Rust 35万行、うちテスト65%）を行っている。しかしその証明は現在**自分の画面の中でしか価値を持たない**。段階5 を越えて初めて、この投資が外部価値へ変換される。

逆に言えば、**ここを越えない限り「証明付き」は他者に提示できる特徴にならない。**

---

## 8. 判断を Codex に委ねる点

以下は私の側で確定できなかった。実装者判断とすること。

1. **分類3（ソルバ15箇所）を本当に据え置いてよいか。**progress.md の「solver共有の乗算・減算順」が、ソルバと証明で同一の `atan2` 呼び出しを共有することを意味するなら、分岐設計が必要になる（段階1）。
2. **`fold_frames.rs:550` を分類2へ格上げするか。**FOLD 書出結果の環境間一致を主張したいかどうかによる。
3. **段階0 の (b) と (c) のどちらを採るか。**(c) は `editor.rs:3471` の `angle_microdegrees` 整数表現を活かせるが、極座標入力の受理範囲が狭まる。
4. **完成率への反映。**本作業は新しい利用者向け能力を増やさないため 81.96% を動かさない、という整理が妥当と考える。ただし §2 は既存不具合の修正であり、MUST 集計には影響し得る。
5. **`hypot` の扱い。**`hypot` は `sqrt(x²+y²)` を素朴に計算すればオーバーフロー耐性を失うが移植可能になる。決定論的実装を書くか、スケーリング付きの自前実装で済ませるかは実装者判断とする。

---

## 9. 本提案で行っていないこと

- **コードは1行も変更していない。**測定は読み取りのみで行った。
- 決定論的実装の具体コードは提示していない。§5 の方針のみである。
- 案B・案C を否定していない。案C は既に `702488cd` で部分適用されており、案A と併用可能である。
- 性能実測を行っていない。「10〜100倍遅い」は一般的な決定論的 libm の知見であり、本実装の実測値ではない。段階2 で実測すること。

---

## 付録: 非移植な本番呼び出し 48箇所 全リスト

計測方法: `.rs` 全ファイルから `#[cfg(test)]` インライン ブロック（宣言 `#[cfg(test)] mod foo;` は除外し、波括弧の対応で範囲を決定）とファイル名に `test` を含むファイルを除外し、`.atan2(` `.sin(` `.cos(` `.tan(` `.hypot(` `.sin_cos(` を数えた。ドキュメンテーションコメント行は除外した。

```
分類1 証明権威 (11)
crates/ori-core/src/constraints.rs:3087                                  atan2
crates/ori-core/src/constraints.rs:3119                                  atan2
crates/ori-core/src/constraint_exactification/singleton_constructive.rs:241  sin
crates/ori-core/src/constraint_exactification/singleton_constructive.rs:242  cos
crates/ori-core/src/constraint_exactification/pair_constructive.rs:272       sin
crates/ori-core/src/constraint_exactification/pair_constructive.rs:273       cos
crates/ori-core/src/constraint_exactification/zero_closure_constructive.rs:303  atan2
crates/ori-core/src/constraint_exactification/zero_closure_constructive.rs:496  cos
crates/ori-core/src/constraint_exactification/zero_closure_constructive.rs:496  sin
crates/ori-core/src/constraint_exactification/zero_closure_constructive.rs:588  hypot

分類2 永続データ生成 (11)
crates/ori-core/src/editor.rs:3472                                       sin_cos
crates/ori-core/src/editor.rs:4016                                       sin_cos
crates/ori-core/src/editor.rs:8125                                       hypot
crates/ori-core/src/editor.rs:8186                                       hypot
apps/desktop/src-tauri/src/lib.rs:4722                                   cos
apps/desktop/src-tauri/src/lib.rs:4724                                   sin
apps/desktop/src-tauri/src/lib.rs:4783                                   cos
apps/desktop/src-tauri/src/lib.rs:4785                                   sin
apps/desktop/src-tauri/src/pattern_edit_commands.rs:856                  sin_cos
apps/desktop/src-tauri/src/pattern_edit_commands.rs:1106                 cos
apps/desktop/src-tauri/src/pattern_edit_commands.rs:1107                 sin

分類3 数値ソルバ (15) — 据え置き推奨
crates/ori-core/src/constraint_solver.rs:631                             hypot
crates/ori-core/src/constraint_solver.rs:635                             hypot
crates/ori-core/src/constraint_solver.rs:664                             hypot (2箇所)
crates/ori-core/src/constraint_solver.rs:694                             atan2
crates/ori-core/src/constraint_solver.rs:729                             cos, sin
crates/ori-core/src/constraint_solver.rs:730                             cos, sin
crates/ori-core/src/constraint_solver.rs:743                             hypot (2箇所)
crates/ori-core/src/constraint_solver.rs:745                             hypot (2箇所)
crates/ori-core/src/constraint_solver.rs:746                             hypot
crates/ori-core/src/constraint_solver.rs:747                             hypot

分類4 入出力・表示・スケジュール (11) — 据え置き推奨
crates/ori-formats/src/svg.rs:2604                                       cos
crates/ori-formats/src/svg.rs:2605                                       sin
crates/ori-formats/src/svg.rs:2616                                       tan
crates/ori-formats/src/svg.rs:2620                                       tan
crates/ori-formats/src/fold_frames.rs:550                                atan2
crates/ori-kinematics/src/schedule.rs:777                                atan2
crates/ori-collision/src/continuous_path.rs:2930                         hypot
apps/desktop/src-tauri/src/stacked_fold_cycle_schedule.rs:230            atan2
apps/desktop/src-tauri/src/beginner_design_commands.rs:2125              hypot
apps/desktop/src-tauri/src/beginner_design_commands.rs:2576              hypot
apps/desktop/src-tauri/src/geometric_constraint_commands.rs:400          hypot
apps/desktop/src-tauri/src/geometric_constraint_commands.rs:405          atan2
```
