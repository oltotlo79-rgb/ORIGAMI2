# Claude提案: テストを一切緩めずCI実行時間を短縮する

作成: 2026-08-02 / 作成者: Claude
対象: Codex
状態: **提案。コードは一切変更していない。**
観測基準: `origin/main` = `6941b002`（2026-08-02 12:50）

---

## 0. 要旨

1. `rust` jobの`timeout-minutes`は当初45分から**180分**へ引き上げられている。テスト量の増加に対して時間側で対処している状態である。
2. **`Cargo.toml`に`[profile]`セクションが1つも存在しない。**そのため`ori-collision`・`ori-core`など証明本体のテストが**`opt-level = 0`のまま実行**されている。`num-bigint`による厳密有理数演算・網羅列挙・SAT探索はこの条件で最も損をする種類の計算である。
3. **提案の核は「テストを減らす」ではなく「同じテストを速く走らせる」である。**本書のどの案も、テストの削除・`#[ignore]`・パスフィルタ・`debug-assertions`無効化を含まない。
4. 効果順に4案を示す。**案1（profile設定）と案2（nextest）だけで大幅な短縮が見込め、変更量は数行**である。
5. 案2には副次的利点がある。2026-07-28の実測で、`cargo test`の並列実行時に**19件がテスト間干渉で失敗**し、`--test-threads=1`で全通過した。nextestはテストごとにプロセスを分けるため、この干渉が原理的に消える。**逐次実行へ退避せずに済む。**

---

## 1. 現状の実測

### 1.1 CI構成

```yaml
rust:
  strategy:
    matrix:
      os: [windows-latest, macos-latest]   # 全workspaceを2 OSで二重実行
  timeout-minutes: 180                      # 当初45分
```

### 1.2 テスト実行方法（`ci.yml` の`Rust tests`ステップ）

```bash
# パッケージごとに順次
cargo test -p "$package" --locked --all-targets --no-fail-fast

# デスクトップだけ release
cargo test -p origami2-desktop --release --locked --lib --no-fail-fast
cargo test -p origami2-desktop --locked --test event_schema_corpus --no-fail-fast
```

`origami2-desktop --lib`は`--release`化済みである（`954823df`、08-02 00:32）。**それ以外のパッケージはすべてdebugのまま**である。

### 1.3 プロファイル設定

```
$ grep -c '\[profile' Cargo.toml
0
```

**未設定。**したがって`profile.test`は既定値（`opt-level = 0`、`debug-assertions = true`、`overflow-checks = true`）で走る。

### 1.4 テスト規模と実測時間（2026-07-28、WSL2 aarch64、cargo 1.90.0、debug）

```
workspace合計                     2,742件
  ori-collision --lib      444件   317.45秒
  origami2-desktop --lib   734件   260.57秒
  ori-collision 統合テスト  1件    498.14秒   ← 単一テストで8分超
  同上                      33件   392.92秒
  他クレート                       各数十秒以下
```

**単一テストが498秒**という値は、計算量ではなく最適化の欠如を強く示唆する。この規模の厳密演算はdebugビルドで支配的に遅くなる。

### 1.5 依存

```toml
num-bigint = "=0.4.8"     # 厳密有理数（ori-numeric、constraint系）
rayon      = "=1.12.0"    # 並列スキャン
```

`num-bigint`の多倍長演算はインライン化と最適化の恩恵が特に大きい。

---

## 2. 案1: `[profile.test]` で最適化を効かせる 【最優先・変更は数行】

### 2.1 提案する設定

```toml
# Cargo.toml（workspace root）

[profile.test]
opt-level = 2
debug-assertions = true      # 維持（fail-closed契約の前提）
overflow-checks = true       # 維持（資源上限・桁溢れ検出の前提）

[profile.dev.package."*"]
opt-level = 3                # 依存クレートのみ最適化
```

`[profile.dev.package."*"]`は**依存だけ**を最適化する。自クレートのインクリメンタルビルドは速いままで、`num-bigint`のような重い依存だけが最適化される。開発時のビルド待ちを増やさずに実行速度を得る定石である。

### 2.2 なぜテストが緩まないか

- **`opt-level`は意味論を変えない。**`debug_assert!`、`assert!`、`panic!`、`Result`分岐はすべてそのまま残る。
- **`debug-assertions`と`overflow-checks`を明示的に`true`で維持**する。本プロジェクトは資源上限の検出（`checked_add`/`checked_mul`のほか、暗黙のオーバーフロー検査）に依存しているため、ここを落としてはならない。**本提案はこれらを一切無効化しない。**
- **決定論的超越関数のgolden bitsは影響を受けない。**`libm`は`default-features = false`かつ`libm/arch`拒否で固定されており、IEEE-754の`f64`演算はRustでは最適化により再結合されない（`-ffast-math`相当は既定で無効）。`opt-level`を変えても同一ビットが出る。

### 2.3 見込み

厳密有理数演算・網羅列挙が支配的なコードでは、debug→`opt-level=2`で**5〜20倍**が一般的である。`ori-collision --lib`の317秒、単一テストの498秒はいずれもこの帯域にある。

**ただしこれは一般的知見であり、本リポジトリでの実測値ではない。**§5に10分で検証できる手順を示す。

### 2.4 副作用として想定すべきこと

- **ビルド時間は増える。**特にクリーンビルド。ただし`Swatinem/rust-cache`が効いている限り、増分は限定的である。実測して割に合うか判断されたい。
- テストが速くなることで、これまで時間で隠れていた**タイムアウト依存のテスト**（`deadline`、`cancel`系）の挙動が変わる可能性がある。`b74fdaa0`で調整した実行時間上限との整合を確認されたい。

---

## 3. 案2: `cargo-nextest` へ移行 【2〜3倍・干渉問題も同時に解消】

### 3.1 変更

```yaml
- uses: taiki-e/install-action@nextest
- run: cargo nextest run --workspace --locked --no-fail-fast
```

### 3.2 本プロジェクト固有の利点

2026-07-28に当方が実測した事実である。

```
cargo test -p origami2-desktop --lib（既定の並列）  → 715 passed / 19 failed
同 --test-threads=1                                 → 734 passed /  0 failed
```

失敗した19件はすべて保存・読込系（`native_save_*`、`native_open_*`、`project_persistence::staged_payload_adapter_tests`）で、`single_flight_ownership_set`や`separate_process_crash_and_recovery`といった**プロセス全体で共有される状態**を奪い合っていた。

コード側にも同種の共有状態が実在する。

```rust
// apps/desktop/src-tauri/src/stacked_fold_blockwise_cycle_tests.rs:14-15
let original = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
STACKED_FOLD_READ_GENERATION.store(811, Ordering::Release);
```

**nextestはテストごとに別プロセスで実行するため、この種の干渉が原理的に発生しない。**「速くする」と「干渉を根治する」が同時に達成でき、`--test-threads=1`への退避（遅い上に問題を隠す）を避けられる。

なお`serial_test`等の逐次強制クレートは現在導入されていない。nextestを入れれば導入も不要になる。

### 3.3 移行前に確認すべき唯一のリスク

**同一プロセス内で状態を積み上げることを前提にしたテストがないか。**

具体的には、`pair_proof_cache`のヒット率を測るテスト（`docs/progress.md`が記録する「91 hit・14 cold」など）が、**複数の`#[test]`にまたがってプロセスローカルなキャッシュを共有している場合**、nextestでは各テストが空のキャッシュから始まるため成立しなくなる。

- 単一の`#[test]`関数内で完結しているなら**問題なし**
- 複数テストにまたがるなら、そのテストだけ1つの`#[test]`へ統合するか、nextestの`test-group`で同一プロセス実行を指定する

移行時にこの1点だけ確認されたい。

### 3.4 制約

- nextestはdoctestを実行しない。ただし現行CIは`--all-targets`であり**doctestは元々対象外**なので影響なし。必要なら`cargo test --doc`を別ステップで足す。
- テストごとのプロセス起動コストが乗る。2,742件×数十msで**Windowsでは1分前後の固定費**が増える。現在の実行時間に対しては誤差である。

---

## 4. 案3・案4

### 4.1 案3: `--partition` によるシャーディング（テストは1件も減らない）

```yaml
strategy:
  matrix:
    os: [windows-latest, macos-latest]
    shard: [1, 2, 3, 4]
run: cargo nextest run --workspace --locked --partition count:${{ matrix.shard }}/4
```

全シャードの和が従来の全テストと厳密に一致するため、**カバレッジは同一**である。wall-clockは理論上1/4。

**必ず併せて更新が必要な箇所**:

```js
// .github/scripts/verify_release_ci.mjs:190
const expectedNames = [
  'dependency-advisory-audit', 'frontend', 'macos-bundle',
  'rust (macos-latest)', 'rust (windows-latest)',   // ← シャード分の名前へ差し替え
  'slicer-acceptance', 'windows-bundle',
]
```

必須チェック名が`rust (windows-latest, 1)`のように変わるため、**リリース発効監査が壊れる**。正本発効規則に直結するので、案1・案2で不足する場合にのみ検討されたい。

### 4.2 案4: Windows の `crt-static` を再評価

```yaml
- name: Link the Windows Rust test harness to the static MSVC runtime
  run: "RUSTFLAGS=-C target-feature=+crt-static" >> $GITHUB_ENV
```

`RUSTFLAGS`の変更は**全依存の再ビルドを強制**し、`Swatinem/rust-cache`のヒット率を大きく下げる。テストハーネスのリンク目的で導入されたものと理解しているが、現在も必要かを確認する価値がある。不要であれば削除するだけでキャッシュ再利用が改善する。

なお本番バンドル（`windows-bundle` job）とは別ステップなので、テスト側から外しても配布物には影響しない。

### 4.3 補足（ほぼコスト0）

`packages=(...)`のループで、最長の`ori-collision`を**先頭**に移す。シャーディング導入時に末尾の詰まりが減る。

---

## 5. 検証手順（Codex側で約10分、ファイル変更なし）

`Cargo.toml`を書き換えずに、**環境変数だけでプロファイルを上書きして計測できる**。

```bash
# ベースライン（現状）
cargo test -p ori-collision --locked --lib --no-fail-fast

# opt-level=2 を試す（ファイル変更なし）
CARGO_PROFILE_TEST_OPT_LEVEL=2 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true \
  cargo test -p ori-collision --locked --lib --no-fail-fast
```

`finished in NNNs`の比較で効果が即座に分かる。**同時に、合格件数が完全に一致すること**（444/444）も確認されたい。一致しなければ案1は採用しない。

nextestの試行も同様に非破壊で行える。

```bash
cargo install cargo-nextest --locked
cargo nextest run -p origami2-desktop --lib
# → 7/28に並列で19件失敗したものが全通過するかを確認
```

---

## 6. 優先順位

| # | 施策 | 期待効果 | 変更量 | リスク | 発効監査への影響 |
|---|---|---|---|---|---|
| 1 | `[profile.test] opt-level=2` | 大（5〜20倍帯） | 数行 | 低 | なし |
| 2 | nextest移行 | 2〜3倍＋干渉解消 | CI数行 | 低（§3.3を要確認） | なし |
| 3 | `--partition`分割 | 台数分 | CI＋発効スクリプト | 中 | **あり（必須チェック名）** |
| 4 | `crt-static`見直し | キャッシュ改善 | 1行 | 要確認 | なし |

**案1→案2の順で、それぞれ効果を実測してから進めることを推奨する。**案3は発効規則に触るため、案1・2で不足する場合の最後の手段とされたい。

---

## 7. 採用してはならない選択肢

本プロジェクトの性質上、以下は短縮策として不適切である。参考までに明記する。

- **`--test-threads=1`への退避** ── 干渉を隠すだけで遅くなる。案2で根治すべきである。
- **`#[ignore]`・パスフィルタによる間引き** ── 本プロジェクトのテストは「production演算の到達クラスを全列挙して固定する」証明の一部である。間引きは健全性の穴になる。
- **`debug-assertions = false` / `overflow-checks = false`** ── 高速化はするが、fail-closed契約と資源上限検出の前提が失われる。**本提案は明示的にこれを`true`で維持する。**
- **`opt-level = 3` を`profile.test`へ** ── `2`と比べ体感差が小さくビルド時間だけ伸びやすい。依存側（`profile.dev.package."*"`）に限定するのが効率的である。

---

## 8. 本提案で行っていないこと

- **コードを1行も変更していない。**`Cargo.toml`・`.github/workflows/ci.yml`ともに未編集である。
- **`opt-level`変更の効果を本リポジトリで実測していない。**§2.3の倍率は一般的知見であり、§5の手順で必ず実測されたい。当方の環境ではworktree作成がパス深度制約で失敗し、またCodexの稼働中CPUと競合するため実測を見送った。
- CI実行時間の内訳（ビルド時間 vs テスト実行時間）を分離計測していない。案1はテスト実行時間を縮める代わりにビルド時間を増やすため、**この内訳が判断材料になる**。GitHub Actionsのログでステップ単位の所要時間を確認されたい。
