# Claude提案: 追加コストなしでCI時間を削減する（第2版・未着手項目に限定）

作成: 2026-08-02 / 作成者: Claude
対象: Codex
状態: **提案。コードは一切変更していない。**
観測基準: `origin/main` = `5e4a9b5c`（2026-08-02 20:16）
前提: **有料ランナー・外部サービス・新規課金を一切使わない案のみを扱う。**

---

## 0. 要旨

1. 前回提案（`claude-ci-runtime-reduction-proposal-2026-08-02.md`）のうち、**`CARGO_PROFILE_TEST_OPT_LEVEL=2` と nextest は既に採用済み**である（`ff66344e`、および`ori-collision`・`origami2-desktop`での`cargo nextest run`）。本書はそれ以外の**未着手かつ無料**の項目だけを扱う。
2. 現在 `rust` job は `timeout-minutes: 240` まで引き上げられ、CI内コメントは「**opt-level変更前の時点で両OSとも160分超**」と記録している。規模が閾値を超えている。
3. **最大の未着手項目は「依存クレートが依然 opt-level 0」である。**`profile.test`の設定は依存に及ばず、`num-bigint`・`ori-core`・`ori-numeric`という計算の実体が最適化されていない。
4. 次に大きいのは**デバッグ情報の全量生成**である。コンパイル時間とキャッシュ容量の両方を圧迫し、**GitHub Actionsのキャッシュ上限10 GiBを超えると毎回コールドビルドになる**。
5. **直近60コミットのうち13件（22%）がドキュメントのみの変更**であり、それらが4時間規模のRust CIを丸ごと起動している。
6. 本書の案はすべて設定変更のみで、テストの削除・`#[ignore]`・カバレッジ縮小・`debug-assertions`無効化を含まない。

---

## 1. 実測データ

### 1.1 規模の推移

```
#[test] 総数
  2026-07-27 17:28   2,743
  2026-07-30 03:13   3,223   （+17.5% / 2.4日）
  2026-08-02 20:16   3,914   （+21.4% / 3.7日）
                              6日間で +43%、直近は約 +200件/日

Rust行数
  2026-08-02 01:08   496,007
  2026-08-02 18:07   542,510   （1日で +46,500）
```

### 1.2 現在のCI設定（採用済みの最適化）

```yaml
rust:
  matrix: [windows-latest, macos-latest]
  timeout-minutes: 240
  env:
    CARGO_PROFILE_TEST_OPT_LEVEL: "2"        # 採用済み
    CARGO_PROFILE_TEST_DEBUG_ASSERTIONS: "true"
    CARGO_PROFILE_TEST_OVERFLOW_CHECKS: "true"
  # rust-cache: key=test-profile-opt2-v1, cache-on-failure=true  ← 適切
  # ori-collision / origami2-desktop は cargo nextest run に移行済み
```

**キャッシュのキー分離と`cache-on-failure: true`は適切な措置である。**タイムアウトしてもキャッシュが保存されるため、「毎回コールドビルド」の悪循環は回避されている。

### 1.3 未設定の項目

```
CARGO_PROFILE_DEV_OPT_LEVEL     未設定  →  依存は opt-level 0
CARGO_PROFILE_TEST_DEBUG        未設定  →  デバッグ情報を全量生成
CARGO_PROFILE_DEV_DEBUG         未設定  →  同上
on.push.paths / paths-ignore    未設定  →  docs のみの変更でも全CI起動
```

### 1.4 ドキュメント専用コミットの割合

```
直近60コミット中、docs/**.md および docs/**.json のみを変更したもの: 13件（22%）
```

`SIM-010の…証跡を同期する`、`進捗記録を…更新する`といった正本記録の更新がこれに当たる。**Rustコードを1行も変えずに4時間規模のジョブを2 OS分起動している。**

---

## 2. 案A: 依存クレートも最適化する 【最優先・env 1行・無料】

### 2.1 現状の問題

Cargoの仕様上、`profile.test`は**テストターゲット本体にのみ**適用され、依存は`profile.dev`（`opt-level = 0`）でビルドされる。

`ori-collision`のテストから見た依存は次のとおりである。

```toml
[dependencies]
num-bigint        # 多倍長整数（最も重い）
num-rational      # 厳密有理数
ori-numeric       # 決定論的超越関数・区間演算
ori-core          # 制約証明・層順
ori-foldability   # 層順SAT
ori-kinematics    # 姿勢・閉路
```

**証明計算の実体はすべて依存側にある。**現在の設定では、いちばん重い部分が最適化されていない。

### 2.2 提案

```yaml
env:
  CARGO_PROFILE_TEST_OPT_LEVEL: "2"
  CARGO_PROFILE_TEST_DEBUG_ASSERTIONS: "true"
  CARGO_PROFILE_TEST_OVERFLOW_CHECKS: "true"
  # 追加
  CARGO_PROFILE_DEV_OPT_LEVEL: "2"
  CARGO_PROFILE_DEV_DEBUG_ASSERTIONS: "true"      # 明示して健全性を保持
  CARGO_PROFILE_DEV_OVERFLOW_CHECKS: "true"       # 同上
```

`dev`側にも`debug-assertions`と`overflow-checks`を**明示的に`true`で指定**するため、fail-closed契約と資源上限検出の前提は一切変わらない。

### 2.3 代償と注意

- **キャッシュキーを更新する必要がある**（例: `test-profile-opt2-v1` → `profile-opt2-dev2-v1`）。更新しないと旧プロファイルの成果物と混在する。
- **一度だけフルリビルドが発生する。**`cache-on-failure: true`があるので、途中で打ち切られてもキャッシュは残る。
- 効果が出るのは**3回目の実行から**である（1回目: test側の再ビルド、2回目: dev側の再ビルド、3回目以降: 温キャッシュ）。

---

## 3. 案B: デバッグ情報を line-tables-only にする 【効果大・env 2行・無料】

### 3.1 なぜ効くか

Rustの`dev`/`test`プロファイルは既定で**完全なデバッグ情報**（`debug = true`）を生成する。これは次の2つを同時に悪化させる。

1. **コンパイル時間** ── デバッグ情報の生成とリンクはビルド時間の相当部分を占める。`opt-level = 2`と組み合わさると特に重い。
2. **`target/`の容量** ── デバッグ情報は成果物サイズの大半を占める。

### 3.2 容量がキャッシュに与える影響（重要）

**GitHub Actionsのキャッシュは1リポジトリあたり10 GiBが上限**である。542,589行のRustを`opt-level = 2` × 完全デバッグ情報でビルドした`target/`は、これを容易に超える。

上限を超えると、
- `Swatinem/rust-cache`が保存に失敗する、または
- 古いキャッシュが追い出され、**毎回コールドビルドに戻る**

**「キャッシュが効いているはずなのに毎回遅い」場合、この容量超過が原因である可能性が高い。**まずキャッシュ保存ログのサイズを確認されたい。

### 3.3 提案

```yaml
env:
  CARGO_PROFILE_TEST_DEBUG: "line-tables-only"
  CARGO_PROFILE_DEV_DEBUG: "line-tables-only"
```

`line-tables-only`は**バックトレースのファイル名と行番号を保持**する。現在`RUST_TEST_THREADS`と同じstepに`RUST_BACKTRACE: "1"`が設定されているが、失敗時の診断能力は維持される。失われるのは変数の型情報など、デバッガ接続時にしか使わない情報である。

CIでデバッガを繋ぐことはないため、**診断能力を落とさずにビルド時間とキャッシュ容量を削減できる。**

### 3.4 期待効果

- ビルド時間: 削減（デバッグ情報生成・リンクの負荷が減る）
- `target/`容量: 大幅削減 → **キャッシュが10 GiB以内に収まる可能性**
- テスト実行時間: 変化なし
- 失敗時の診断: `file:line`は残る

---

## 4. 案C: ドキュメント専用コミットでRust CIを起動しない 【22%削減・無料】

### 4.1 現状

```
直近60コミット中 13件（22%）が docs/**.md・docs/**.json のみの変更
```

これらはRustコードを1行も変更しないため、Rustテストの結果は自明に不変である。にもかかわらず、両OSで4時間規模のジョブが走る。

### 4.2 提案

```yaml
on:
  push:
    branches: [main]
    paths-ignore:
      - 'docs/**'
      - '**/*.md'
  pull_request:
    paths-ignore:
      - 'docs/**'
      - '**/*.md'
```

### 4.3 **必ず併せて検討すべき副作用**

`docs/progress.md`の更新コミットでCIが走らなくなると、**そのcommitに対する必須7チェックが存在しなくなる**。`verify_release_ci.mjs`は反映headに対して7チェックすべてのsuccessを要求するため、**正本発効がそのheadで成立しなくなる**。

対処は次のいずれかである。

- **(a) 発効headをコード変更コミットに限定する** ── 運用規約として明文化する。progress.md更新は発効headの後に置く。
- **(b) skip時に成功を返すダミージョブを置く** ── `if: ${{ always() }}`の軽量ジョブで同名のチェックを作る。チェック名の存在は保たれるが、「successを騙る」ことになるため、本プロジェクトのfail-closed思想とは相性が悪い。
- **(c) `paths-ignore`を`pull_request`のみに適用する** ── mainへのpushでは常にフル実行し、PR段階のみ節約する。**最も安全である。**

**(c)を推奨する。**発効規則に一切触れず、日常の反復だけを削減できる。

### 4.4 除外してはならないパス

```
docs/requirements-evidence.v1.json    ← verify_requirements_traceability が参照
docs/requirements-status.md            ← 同上
docs/requirements-ids.v1.txt           ← 同上
```

これらは`frontend`ジョブの要件トレーサビリティ検証の入力である。`docs/**`を一括除外すると検証が走らなくなるため、**`paths-ignore`は`docs/**/*.md`に限定し、上記JSONとtxtは除外しない**設計が安全である。ただし`requirements-status.md`は`.md`なので、個別に再包含する必要がある。

```yaml
paths-ignore:
  - 'docs/Codex/**'
  - 'docs/Claude/**'
  - 'docs/plans/**'
  - 'docs/progress.md'
```

**個別列挙のほうが安全である。**`docs/**`の一括除外は要件検証を壊す。

---

## 5. 案D: 重い証明テストを ubuntu へ集約する 【効果最大・ただし発効規則に影響】

### 5.1 現状

```yaml
matrix:
  os: [windows-latest, macos-latest]   # Linux が不在。同じ全テストを2回実行
```

**証明テストの大半はOS非依存である。**決定論的カーネルを`libm` 0.2.16固定＋`libm/arch`拒否で凍結済みであり、`ori_binary64_libm_0_2_16_no_arch_cardinal_v1`のgolden bit corpusがtarget間のビット一致を保証している。**そのgolden bitsテストは既に`dependency-advisory-audit`ジョブ（ubuntu）で実行されている。**

つまり、cross-runtime replayの保証は既にLinuxを含む形で成立しており、**証明ロジック本体を3 OSで重複実行する必要はない。**

### 5.2 提案

```yaml
rust-core:                          # 重い証明テスト（1回だけ）
  runs-on: ubuntu-latest
  run: 既存の全パッケージ実行

rust-os:                            # OS依存の実挙動のみ
  matrix: [windows-latest, macos-latest]
  run: |
    cargo test -p ori-numeric --lib deterministic_transcendental   # golden bits
    cargo nextest run -p origami2-desktop --lib project_persistence # パス・ファイルモード・ロック
    ./.github/tests/windows-installer-smoke.test.ps1               # Windowsのみ
```

### 5.3 効果と代償

- **効果**: 重い部分の実行が2回→1回。加えてLinuxランナーはWindows/macOSより高速である。**体感で半分以下**が見込める。
- **代償**: `verify_release_ci.mjs:190`の`expectedNames`を書き換える必要がある。

```js
const expectedNames = [
  'dependency-advisory-audit', 'frontend', 'macos-bundle',
  'rust (macos-latest)', 'rust (windows-latest)',   // ← 変更が必要
  'slicer-acceptance', 'windows-bundle',
]
```

**正本発効規則に直結するため、案A〜Cで不足する場合の次段として検討されたい。**

### 5.4 OS依存として必ず残すべきテスト

7/28の実測で、次はOS固有の挙動に依存することを確認している。

```
tests::native_save_overwrite_preserves_unix_file_mode
project_persistence::staged_payload_adapter_tests::unix_read_only_parent_redacts_errors_...
tests::unix_directory_sync_failure_is_only_reported_before_publish
Windows installer smoke（PowerShell 7）
```

これらを`rust-os`側へ確実に残すこと。

---

## 6. 案E: Windows の `crt-static` を再検討する 【1行削除・無料】

```yaml
- uses: Swatinem/rust-cache      # ← キャッシュ復元
  with:
    key: test-profile-opt2-v1
- name: Link the Windows Rust test harness to the static MSVC runtime
  if: runner.os == 'Windows'
  run: "RUSTFLAGS=-C target-feature=+crt-static" >> $GITHUB_ENV   # ← 直後に RUSTFLAGS 変更
```

**キャッシュを復元した直後に`RUSTFLAGS`を変更している。**`RUSTFLAGS`はビルドフィンガープリントに含まれるため、**復元したキャッシュがWindows側で丸ごと無効化されている可能性がある。**

Windowsだけ極端に遅い場合、これが主因である。

**確認方法**: Windows側のログで`Compiling`が全依存に対して出ているかを見る。温キャッシュなら依存の再コンパイルは出ないはずである。

対処の選択肢は次のとおり。

- `crt-static`が現在も必要かを確認し、不要なら削除する
- 必要なら、`RUSTFLAGS`を**rust-cacheの前**に設定してキーへ反映させる

なお本番バンドル（`windows-bundle`ジョブ）とは別ステップなので、テスト側から外しても配布物には影響しない。

---

## 7. 案F: 小さいが無料の項目

### 7.1 `CARGO_INCREMENTAL=0` の明示

`Swatinem/rust-cache`は既定でこれを設定するが、明示しておくと意図が明確になる。インクリメンタル成果物はCIでは再利用されず、`target/`容量だけを消費する。案Bのキャッシュ容量対策と併せて効く。

### 7.2 `ori-collision` を先頭で走らせる

現在は`packages=(...)`の順に実行している。最長パッケージを先頭に置くと、失敗時のフィードバックが早くなる（`break`で後続を打ち切る構造のため）。

### 7.3 残るパッケージも nextest へ

`ori-collision`と`origami2-desktop`は移行済みである。他のパッケージは`cargo test`のままだが、いずれも実行時間が短いため効果は限定的である。統一による運用の単純化が目的なら検討に値する。

---

## 8. 優先順位

| # | 施策 | 効果 | 変更量 | 発効規則への影響 | リスク |
|---|---|---|---|---|---|
| **A** | `CARGO_PROFILE_DEV_OPT_LEVEL=2` | **大**（計算の実体が最適化される） | env 3行 | なし | 低（要キャッシュキー更新） |
| **B** | `debug = line-tables-only` | **大**（ビルド時間＋キャッシュ容量） | env 2行 | なし | 低 |
| **C** | docs専用でCIを起動しない | 22%削減 | on: 数行 | **あり（§4.3の(c)推奨）** | 中 |
| **D** | 証明テストをubuntuへ集約 | **最大**（2回→1回） | job分割 | **あり** | 中 |
| **E** | `crt-static`見直し | Windowsのキャッシュ復活 | 1行 | なし | 要確認 |
| **F** | 細目 | 小 | 数行 | なし | 低 |

**推奨する順序: A + B を同時に投入 → 効果を測定 → 不足なら E → C(c) → D。**

AとBは同じキャッシュ無効化を1回で済ませられるため、**必ず同時に入れるべきである。**別々に入れるとフルリビルドが2回発生する。

---

## 9. 効果測定の方法

案を入れる前に、**ビルド時間とテスト実行時間の内訳**を把握されたい。GitHub Actionsのログはstep単位の所要時間を表示する。

```
Rust tests ステップ内で
  Compiling ... の連続       → ビルド時間
  Running / test result: ... → テスト実行時間
```

- **ビルドが支配的**なら → 案B（デバッグ情報）と案E（キャッシュ復活）が効く。案Aは逆効果になり得るため`opt-level=1`も検討する
- **テスト実行が支配的**なら → 案A（依存の最適化）が最も効く

キャッシュ容量も確認されたい。`Post Run Swatinem/rust-cache`のログにサイズが出る。**10 GiBに近い、または保存失敗のメッセージがあれば、案Bが最優先である。**

---

## 10. 採用してはならない選択肢

本プロジェクトの性質上、以下は短縮策として不適切である。

- **`--test-threads=1`への退避** ── 遅くなるうえ、7/28に実測したテスト間干渉を隠すだけである。nextestで既に根治している。
- **`#[ignore]`・テストの間引き** ── 本プロジェクトのテストは「production演算の到達クラスを全列挙して固定する」証明の一部である。
- **`debug-assertions = false` / `overflow-checks = false`** ── 高速化はするが、fail-closed契約と資源上限検出の前提が失われる。**本書の案A・Bはいずれもこれらを明示的に`true`で維持する。**
- **`docs/**`の一括 `paths-ignore`** ── 要件トレーサビリティ検証の入力（`requirements-evidence.v1.json`等）が除外され、検証が走らなくなる。§4.4のとおり個別列挙とすること。

---

## 11. 長期的な見通し（参考）

本書の案はすべて**一度きりの改善**である。テスト数が約200件/日で増加している現状では、
- A+B+E で得られる余裕は**数週間〜1ヶ月程度**
- D を加えても**2〜3ヶ月程度**

と見込まれる。

**テスト数の増加に追随して恒久的にスケールする手段は、シャーディング（`cargo nextest run --partition count:i/N`）しかない。**これはテストを1件も減らさず、ジョブ数に比例して実時間を短縮できる。ただし必須チェック名が増えるため、`verify_release_ci.mjs`と正本発効規則の改訂が必要である。

無料の範囲でも、GitHubホストランナーの同時実行枠内であればシャーディング自体は追加課金なしに構成できる。**A〜Eで頭打ちになった時点で、発効規則の改訂と併せて検討されたい。**

---

## 12. 本提案で行っていないこと

- **コードを1行も変更していない。**`Cargo.toml`・`.github/workflows/ci.yml`ともに未編集である。
- **効果の倍率を本リポジトリで実測していない。**§9の手順で必ず内訳を測ってから投入されたい。
- キャッシュの実サイズを確認していない（GitHub Actionsのログにアクセスできないため）。§3.2の「10 GiB超過の可能性」は推測であり、**ログで確認すべき事項**である。
- 有料ランナー・並列枠拡張・外部キャッシュサービスは、本書の対象外としている。
