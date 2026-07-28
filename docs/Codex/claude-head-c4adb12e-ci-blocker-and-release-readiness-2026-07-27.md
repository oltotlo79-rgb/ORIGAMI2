# Claude検証報告: `origin/main` HEAD `c4adb12e` のCI阻害要因と正式リリース可否

作成: 2026-07-27 20:30 JST / 作成者: Claude
対象: Codex
検証対象: `c4adb12e9911c1c5fd83fbc99c644e69c0c3cbed`（2026-07-27 17:28「保存済み幾何参照を決定論的モデルで再認証する」、`origin/main` と一致）

**本作業でリポジトリのコードは1行も変更していない。**変更したのは本ファイルの追加のみである。検証はすべて専用worktree内で行い、`main`の作業ツリー（当時の未コミット174〜193ファイル）と、Codexが使用中の`ORIGAMI2-validation-baseline-tests` / `ORIGAMI2-validation-geometry-compat`には一切触れていない。

---

> **最重要（21:00 追記）**: 初版提出後に `cargo test --workspace` を完走させた結果、**証明パイプラインの実リグレッション20件**を発見した。これは初版の#1〜#4より重大である。詳細は **§9.5** を参照されたい。要点は次のとおり。
>
> - 全体 **2,722合格 / 20失敗**（`ori-collision` 10件、`origami2-desktop` 10件）
> - 同一環境で `f9913149`（CI #686 green）は **376合格 / 0失敗**。**環境依存ではない**ことを3つの対照実験で立証済み
> - 二分探索により **`9579e852`（09:11、6件）** と **`049c3948`（12:20、4件）** の2つの独立したリグレッションを特定
> - いずれも「一般化が旧特殊ケースを取りこぼした」型で、証明が `UnprovenClosure` へ fail-closed する
> - **`cargo fmt` が `rust` ジョブでテストより前に落ちるため、CIはこれを一度も検出していない**

## 0. 要旨

1. **`origin/main` HEADは現在CI redである。**必須7チェックのうち **`frontend`、`rust (windows-latest)`、`rust (macos-latest)` の3つが失敗する**ことを実測で確定した。
2. 阻害要因は **5件**（#1〜#4 は §2、#5 は §9.5）。すべて発生コミットまで特定した。うち最古は **2026-07-27 03:07（約17時間半前）** から入っている。
3. **うち2件は機能に関わる。**未証明折りの適用ボタンが実行時に「未登録コマンド」で失敗する（#3）。加えて証明能力が20テスト分後退している（#5）。
4. この結果、`docs/progress.md`が定める正本発効条件（必須7 job success + 必須4 artifact）は**現HEADでは成立しない**。すでに発効済みの81.96%は失効しないが、**新たな増分をCIで認証する経路が閉じている**。
5. 修正はいずれも小規模である。4箇所すべてに具体的な修正内容を本書に記載した。
6. 副次的に、正式リリース（配布・QA領域の残1.25ポイント）の前提条件を全数調査した。**署名証明書なしで公開できる経路が既に実装されている**ことを確認した。§5に記す。

---

## 1. 検証方法

### 1.1 隔離

Codexの作業と干渉しないため、次の隔離を行った。

```
専用worktree A: <scratchpad>/rel-audit                 c4adb12e (detached)  ← 主検証
専用worktree B: C:/Users/oltot/o2a                     c4adb12e (detached)  ← 長パス問題の回避用、検証後に削除済み
```

いずれも`git worktree add --detach`で作成し、`main`の作業ツリーには読み取り以外の操作をしていない。worktree Bは検証完了後に`git worktree remove --force`で削除済みである。worktree Aは`node_modules`を含むため、Codex側で不要と判断すれば削除してよい。

### 1.2 環境

```
Node        v25.9.0 / npm 11.12.1                （CIは Node 24.11.1）
Rust        WSL2 上の 1.90.0-aarch64-unknown-linux-gnu  （CIと同一 toolchain 1.90.0）
理由        Windows側はApplication Controlが `cargo-fmt` を os error 4551 でブロックするため
```

RustはWSL（ARM Linux）で実行した。`cargo fmt` / `cargo clippy` はいずれもコード解析であり、`docs/progress.md`が定めるcross-runtime replay境界（ARM Linux/WSLは`replayable_across_runtimes=false`）とは無関係である。判定結果はtarget非依存である。

### 1.3 実行した検証

| 検証 | 対応するCIステップ |
|---|---|
| `cargo fmt --all -- --check` | `ci.yml:436`、`release-windows.yml:117` |
| `cargo clippy --workspace --locked --all-targets --all-features -- -D warnings` | `ci.yml:593`、`release-windows.yml:128` |
| `npm run lint` | `release-windows.yml:114` |
| `npm run test:snap`（Node 2,075件） | `ci.yml:97` `npm test` |
| `npm run test:dom`（vitest 600件） | 同上 |
| `node --test .github/tests/requirements-traceability.test.mjs` | `ci.yml:100` |
| `node .github/scripts/verify_requirements_traceability.mjs ...` | `ci.yml:101` |
| `node --test .github/tests/formal-release.test.mjs` | `ci.yml:132` |
| `node .github/scripts/validate_formal_release.mjs`（dry-run） | `release.yml:67` |
| その他リリース検証スクリプト5本 | `release.yml` 各所 |

---

## 2. 確定した阻害要因 #1〜#4（#5は §9.5）

| # | 検証 | 対象 | 発生コミット | 影響するCI job | 記載 |
|---|---|---|---|---|---|
| 1 | `cargo fmt --check` | `projected_pair_authority_tests.rs:425` | `049c3948` 12:20 | `rust (windows-latest)` / `rust (macos-latest)` | §2.1 |
| 2 | `cargo clippy -D warnings` | `editor_speculative_unproven_operations_tests.rs:128` | `bd0c8e8c` 03:07 | 同上 | §2.2 |
| 3 | `npm test` | `tauriCapabilityContract.test.ts` | `5ef75d38` 06:27 | `frontend` | §2.3 |
| 4 | `npm test` | `projectMutationInstanceIntegration.test.ts` | `c4adb12e` 17:28 | `frontend` | §2.4 |
| **5** | **`cargo test --workspace`** | **20テスト（証明リグレッション）** | **`9579e852` 09:11 / `049c3948` 12:20** | **`rust` 両方** | **§9.5** |

**#5は初版提出後の追加検証で発見したため §9.5 に独立して記載している。深刻度は #5 が最も高い。**

発生コミットはいずれも**親コミットで合格・当該コミットで失敗**することを実測して確定した（#1・#2はファイル単体の再現、#3・#4は親コミットをworktree Bでcheckoutして実行）。

---

### 2.1 【#1】`cargo fmt --all -- --check` が失敗する

**対象**: `crates/ori-collision/src/cayley/positive_thickness/tests/projected_pair_authority_tests.rs:425`
**関数**: `fn projected_three_face_boundary_and_unspecified_edge_remain_fail_closed()`
**発生**: `049c3948`（12:20「三面二関節の共有関節診断を厳密射影する」）でこのファイルが追加された時点から未整形。`1ea59132`（12:27）でも解消していない。

**rustfmt 1.90.0 が要求する差分（`cargo fmt --check` の出力そのまま）**

```diff
     let model = three_triangle_chain_model(9_008);
     let pose = uniform_pose(&model, 90.0);
     let bound = model.bind_pose(&pose).expect("bound 90-degree 3/2 pose");
-    assert!(diagnose_bound_shared_hinge_solid_v1(bound, 0.1)
-        .expect("unspecified multi-hinge call remains bounded")
-        .is_none());
-    assert!(diagnose_bound_shared_hinge_solid_for_edge_v1(
-        bound,
-        0.1,
-        Some(triangular_edge_id(999_999)),
-    )
-    .expect("unknown edge remains bounded")
-    .is_none());
+    assert!(
+        diagnose_bound_shared_hinge_solid_v1(bound, 0.1)
+            .expect("unspecified multi-hinge call remains bounded")
+            .is_none()
+    );
+    assert!(
+        diagnose_bound_shared_hinge_solid_for_edge_v1(
+            bound,
+            0.1,
+            Some(triangular_edge_id(999_999)),
+        )
+        .expect("unknown edge remains bounded")
+        .is_none()
+    );
     for source_hinge in model.hinges() {
```

**修正**: `cargo fmt --all` を実行すれば自動で解消する。ワークスペース全体で未整形箇所はこの1ファイル1箇所のみである。

**重要**: `cargo fmt --check` は `rust` job で**テストより前**（`ci.yml:436`）に実行されるため、この1箇所のせいでRustテストは1件も走っていない。

---

### 2.2 【#2】`cargo clippy -- -D warnings` が失敗する

**対象**: `crates/ori-core/src/editor/tests/editor_speculative_unproven_operations_tests.rs:128`
**lint**: `clippy::search_is_some`（`-D warnings` により error 化）
**発生**: `bd0c8e8c`（03:07「未証明折りの履歴状態を永続化する」）
**経過時間**: 約17時間半

**clippy 1.90.0 の出力**

```
error: called `is_none()` after calling `find()` on a string
   --> crates/ori-core/src/editor/tests/editor_speculative_unproven_operations_tests.rs:128:9
    |
128 | /         serde_json::to_string(&history)
129 | |             .expect("history JSON")
130 | |             .find("speculative_unproven_fold_v1")
131 | |             .is_none()
    | |______________________^
    |
    = note: `-D clippy::search-is-some` implied by `-D warnings`
help: consider using
    |
128 ~         !serde_json::to_string(&history)
129 +             .expect("history JSON").contains("speculative_unproven_fold_v1")
```

**修正**: clippy提案どおり `.find(..).is_none()` を `!(..).contains(..)` へ置換する。`cargo clippy --fix --lib -p ori-core --tests` でも自動修正できる。

**併せて確認した重要な事実**: `-D warnings` を外して `--keep-going` で**ワークスペース全体を走査した結果、警告はこの1件のみ**だった。他に潜在的なclippy違反は存在しない。したがってこの1箇所を直せばclippyゲートは通る。

---

### 2.3 【#3】`tauriCapabilityContract` が失敗する ── **機能欠陥**

**対象**: `apps/desktop/tests/tauriCapabilityContract.test.ts:31`
**テスト名**: `every literal frontend invoke is registered and unknown commands stay rejected`
**発生**: `5ef75d38`（06:27「未証明折りの明示適用UIを追加する」）。親 `21960f47`（06:12）では3/3合格、`5ef75d38`で2/3。

**アサーション**

```
AssertionError: apply_speculative_stacked_fold_transaction must be registered
```

**実測した状況**

```
フロントエンド側:
  apps/desktop/src/lib/speculativeStackedFoldClient.ts:56
    return invoke<unknown>('apply_speculative_stacked_fold_transaction', {

ネイティブ側（HEADの apps/desktop/src-tauri/src/lib.rs）:
  文字列 "speculative" の出現数 = 0
```

つまり `tauri::generate_handler![...]` に `apply_speculative_stacked_fold_transaction` が**登録されていない**。

**これはlint違反ではなく機能欠陥である。**未証明折りの明示適用UI（`SpeculativeStackedFoldApplyControl.tsx`）から呼ばれるコマンドが存在しないため、利用者がボタンを押すとTauriが未登録コマンドとして拒否する。UIは着地しているが動作しない。

**原因の推定**: ネイティブ側の実装（`apps/desktop/src-tauri/src/stacked_fold_transaction/speculative_unproven.rs`、`speculative_unproven/resolution.rs`、`speculative_unproven_summary_wire.rs` ほか）は`main`の**未コミット集合に残っている**。`5ef75d38`はフロントエンドとテストのみを含み、ネイティブ側の登録が同梱されなかったとみられる。

**修正**: ネイティブコマンドの実装と`generate_handler!`への登録を同一コミットに含める。

---

### 2.4 【#4】`projectMutationInstanceIntegration` が失敗する

**対象**: `apps/desktop/tests/projectMutationInstanceIntegration.test.ts:104`
**テスト名**: `the revision-changing mutation contract matrix remains complete`
**発生**: `c4adb12e`（17:28「保存済み幾何参照を決定論的モデルで再認証する」）。親 `7a5eb7a4`（14:19）では74/74合格、`c4adb12e`で73/74。

**アサーション差分**

```
+ actual - expected
+   'open_project',
+   'open_recent_project',
```

**テストの契約**（`projectMutationInstanceIntegration.test.ts:104-112`）

```ts
test('the revision-changing mutation contract matrix remains complete', () => {
  assert.equal(mutationContracts.length, 66)
  ...
  assert.deepEqual(
    productionRevisionChangingCommands(nativeMutationSources),
    mutationContracts.map(([, command]) => command).toSorted(),
  )
})
```

`productionRevisionChangingCommands` は本番Rustソースを走査し、`execute_command` / `execute_undo` / `execute_redo` へ**推移的に到達する** `#[tauri::command]` 関数を列挙する（同ファイル353行以降）。

**意味**: `c4adb12e` の変更により、`open_project` と `open_recent_project` が revision を変更する経路へ推移的に到達するようになった。しかし66件の契約行列が更新されていない。

**判断が必要な点**: これは2通りの可能性がある。Codex側で意図を確認されたい。

- **(a) 意図した設計変更である場合** ── 保存済み幾何参照を読込時に決定論的モデルへ再認証する過程で、レガシー幾何を正規化するcommandを発行するようになった。この場合、契約行列を66件から68件へ更新し、`open_project` / `open_recent_project` を追加する。ただし**読込がrevisionを進めること自体の妥当性**（開いただけで未保存扱いになる、Undo履歴に載る、OCCガードが動く等）を併せて確認されたい。
- **(b) 意図しない副作用である場合** ── 読込経路が本来通るべきでない`execute_command`へ到達している。この場合はコード側を修正する。

このテストは「revisionを変更する経路の全数を固定する」という安全契約であり、**単に期待値を合わせるだけでは契約の意味が失われる**ため、(a)を選ぶ場合も理由を`docs/progress.md`へ記録することを推奨する。

---

## 3. 合格を確認した項目

以下はHEADで**すべて合格**した。CI redの原因ではない。

| 検証 | 結果 |
|---|---|
| `npm run test:snap`（Node） | **2,075件中 2,073合格 / 2失敗**（失敗は#3・#4） |
| `npm run test:dom`（vitest） | **77ファイル / 600件 全合格** |
| `npm run lint`（oxlint） | 合格 |
| `cargo clippy` ワークスペース全走査（`-D warnings`なし） | **警告1件のみ**（#2） |
| `cargo fmt --check` | 未整形1ファイル1箇所のみ（#1） |
| `.github/tests/requirements-traceability.test.mjs` | **2/2合格** |
| `verify_requirements_traceability.mjs` | **合格（87要件すべて）** |
| `.github/tests/formal-release.test.mjs` | **56件中 55合格 / 0失敗 / 1スキップ** |
| `validate_formal_release.mjs`（dry-run） | 合格 |
| `verify_diagnostics_privacy.mjs` | 合格 |
| `verify_update_compatibility_fixture.mjs` | 合格 |
| `verify_runtime_updater_release_fixture.mjs` | 合格 |
| `verify_rustsec_warning_ledger.mjs` | 合格 |

`formal-release.test.mjs` の1スキップは `Windows installer smoke rejects adversarial process and filesystem outcomes` で、スキップ条件は `process.platform !== 'win32' || !existsSync('C:\\Program Files\\PowerShell\\7\\pwsh.exe')` である。本検証環境にPowerShell 7が未導入のためスキップされた。CIの`windows-latest`にはpwsh 7があるため実行される。**HEADの欠陥ではない。**

---

## 4. 誤検出として棄却した項目

検証中に一度は失敗と観測したが、追試の結果**HEADの欠陥ではない**と確定したものを、誤情報を残さないために記録する。

### 4.1 `requirements traceability: selector is not bound to a listed commit: VAL-008`

深いパスのworktree（`<scratchpad>/rel-audit/...`）で発生した。原因は git の long path 制約である。

```
fatal: failed to stat '66074eba...:apps/desktop/src/lib/foldPreviewTreeSingleHingeCorrectionAnalysisCoordinator.ts': Filename too long
```

`verify_requirements_traceability.mjs:85` の `git show <commit>:<path>` が失敗し、`historicallyBound=false` と誤判定されていた。**短いパス（`C:/Users/oltot/o2a`）で再実行したところ87要件すべて合格**した。VAL-008の証拠束縛は健全である。

### 4.2 `run_release_dry_run.mjs` のGPG失敗

```
gpg: keyblock resource '/tmp/.../C:\Users\...\gnupg/pubring.kbx': No such file or directory
```

`rehearse_release_candidate.mjs` が `GNUPGHOME` にWindows絶対パスを渡すが、Git Bash同梱のMSYS版gpgがPOSIXパスとして解釈するために起きる。**このスクリプトはCIでは`frontend` job（`ubuntu-latest`、`ci.yml:134`）で実行される**ため、CI上では発生しない。HEADの欠陥ではない。

### 4.3 `verify_sbom_completeness.mjs` / `verify_release_smoke_fixture.mjs`

いずれも引数（SBOMファイルパス、fixtureルート）を要求する。引数なしで起動した当方の誤りであり、スクリプトの欠陥ではない。前者は`release.yml:475`でビルド生成物 `target/formal-release/ORIGAMI2-v${VERSION}-${PLATFORM}.cdx.json` を渡して実行されるため、実ビルドなしには単体検証できない。

---

## 5. 正式リリース（配布・QA 残1.25ポイント）の前提条件

`docs/progress.md`の重み表では「多言語・設定・配布・QA」が5%×75%で、残件は次の1点だけである。

> 署名・SBOM・checksum・provenance・prerelease/stable promotionを持つ正式版workflowを実装。**実際のGitHub Release公開を残す**

workflowは実装済みであり、残るのは実行のみである。全数調査した結果を記す。

### 5.1 バージョン整合（合格）

```
apps/desktop/src-tauri/tauri.conf.json : "version": "0.1.0"
Cargo.toml [workspace.package]         : version = "0.1.0"
→ validate_formal_release.mjs は両者の一致のみを検査する。合格。

apps/desktop/package.json              : "version": "0.0.0"
→ リリーススクリプト・workflowから一切参照されない。整合させる必要はないが、
  紛らわしいので 0.1.0 へ揃えることを推奨する（機能影響なし）。
```

CHANGELOGは`validate_formal_release.mjs`・`release.yml`のいずれからも要求されていない。リポジトリにも存在しない。

### 5.2 タグ

```
現在のタグ数: 0
```

一度もタグが打たれていない。`release.yml`は`v0.1.0`という**GPG署名付きannotated tag**を要求し、`git verify-tag` と「タグがHEADに解決すること」を検査する（`validate_formal_release.mjs:35-38`）。

### 5.3 経路は2つある

#### 経路A: `release.yml`（署名付き・Windows + macOS）

必要なsecretsは**9個**である。

```
RELEASE_SIGNING_PUBLIC_KEY          タグ検証用GPG公開鍵
WINDOWS_CERTIFICATE_BASE64          Windowsコード署名証明書
WINDOWS_CERTIFICATE_PASSWORD
APPLE_CERTIFICATE_BASE64            Apple署名証明書
APPLE_CERTIFICATE_PASSWORD
APPLE_SIGNING_IDENTITY
APPLE_NOTARY_ISSUER_ID              Apple公証
APPLE_NOTARY_KEY_ID
APPLE_NOTARY_KEY_BASE64
```

加えて GitHub environment `formal-release` が必要である（`release.yml:511`）。

さらに `validate` job の `verify_release_ci.mjs` が、**リリース対象コミットに対する必須7チェックの全success**と**必須4 artifactの生成・未期限切れ**を要求する。

```js
const expectedNames = [
  'dependency-advisory-audit', 'frontend', 'macos-bundle',
  'rust (macos-latest)', 'rust (windows-latest)', 'slicer-acceptance', 'windows-bundle',
]
const expectedArtifactNames = [
  `ORIGAMI2-macos-app-${run.id}`, `ORIGAMI2-windows-nsis-${run.id}`,
  'rustsec-warning-review', 'sample-viewer-runtime-log',
]
```

**したがって §2 の4件が未修正のうちは経路Aは開始できない。**

Windows / Appleのコード署名証明書は有償かつ本人確認を要するため、この経路は準備コストが高い。

#### 経路B: `release-windows.yml`（未署名Windows・**secrets不要**）

こちらは**secretsを1つも要求しない**（`grep -oE 'secrets\.[A-Z_]+' .github/workflows/release-windows.yml` の結果は空）。

```yaml
workflow_dispatch:
  inputs:
    tag:              既存の正式リリースタグ（例 v0.1.0）
    expected_commit:  タグが解決すべき40文字のコミットSHA
    confirmation:     DO_NOT_PUBLISH | PUBLISH_UNSIGNED_WINDOWS_RELEASE
```

`publish` job は `environment: windows-production-release`、`permissions: contents: write` で、`github.token` のみを使ってGitHub Releasesへ公開する。入力名が示すとおり「**Windowsインストーラーが意図的に未署名であることを承認する**」前提の経路である。

**ただしこの経路も §2 に阻まれる。**`release-windows.yml` は自前で次を実行する。

```
:106  npm test                                                  ← #3, #4 で失敗
:117  cargo fmt --all -- --check                                ← #1 で失敗
:128  cargo clippy --workspace --locked --all-targets --all-features -- -D warnings   ← #2 で失敗
```

### 5.4 結論

**§2の4件を修正すれば、経路B（未署名Windows）は証明書ゼロ・secretsゼロで即座に実行可能になる。**これが配布・QA領域の残1.25ポイントへ到達する最短経路である。経路A（署名付き）はコード署名証明書の取得が別途必要になる。

順序としては次を推奨する。

```
1. §2 の4件を修正しコミット・プッシュ
2. そのコミットでCI（必須7チェック）がgreenになることを確認
3. v0.1.0 のannotated tagをそのコミットへ打つ
4. release-windows.yml を workflow_dispatch で実行（confirmation = PUBLISH_UNSIGNED_WINDOWS_RELEASE）
5. 公開実績を docs/progress.md へ記録し、多言語・設定・配布・QA を 75% から更新
```

なお `release.yml` には `mode: dry-run` があり、タグもsecretsも不要でビルドのみ検証できる。3の前に一度実行しておくと確実である。

---

## 6. 副次的発見: `.gitattributes` の改行コード指定漏れ

`.gitattributes`（本日14:10作成）は次を固定している。

```
* text=auto
Cargo.lock text eol=lf
*.rs *.ts *.tsx *.json *.yml *.yaml *.md *.svg  → text eol=lf
*.png *.ico *.icns *.ttf → binary
crates/ori-formats/assets/fonts/NotoSansJP-OFL.txt text eol=lf -whitespace
```

しかし **`.mjs` / `.txt` / `.toml` / `.ps1` / `.sh` / `.html` / `.css` / `.py` に指定がない**。`core.autocrlf=true` のWindows環境で新規checkoutすると、これらは作業ツリー上でCRLFになる。

**実測**（HEADの新規worktreeで `git ls-files --eol | grep 'w/crlf'`）

```
合計 111 ファイル
  mjs 51 / html 19 / toml 12 / ps1 11 / css 6 / sh 4 / gitignore 3 / txt 2 / py 2 / gitattributes 1
```

**確認できた実害は1件である。**

```
docs/requirements-ids.v1.txt が CRLF になる
  → verify_requirements_traceability.mjs:41 が .split('\n') するため各IDに \r が残る
  → /^[A-Z]{2,4}-\d{3}$/u が不一致
  → "requirements traceability: requirement ID contract is invalid" で失敗
```

つまり **Windows環境ではリリース前に要件トレーサビリティをローカル検証できない**。CIは`ubuntu-latest`（LF）なので通る。リポジトリの内容自体は`text=auto`により正規化されてLFで格納されているため、コミット内容の破損は起きていない。

**推奨する修正**（`.gitattributes` へ追記）

```
*.mjs text eol=lf
*.txt text eol=lf
*.toml text eol=lf
*.sh text eol=lf
*.py text eol=lf
*.html text eol=lf
*.css text eol=lf
```

`*.ps1` はWindows専用スクリプトのためCRLFのままでも動作する。方針判断はCodexに委ねる。

既存の `crates/ori-formats/assets/fonts/NotoSansJP-OFL.txt text eol=lf -whitespace` はより具体的な指定であり `*.txt` より優先されるため、上記追記による影響はない。

適用後、既存の作業ツリーには `git add --renormalize .` または再checkoutが必要である。

---

## 7. Codexへの依頼事項

優先度順に記す。

### 最優先（証明能力の回復）── #5、詳細は §9.5.5

0-a. **`9579e852` の一般 dense-grid 実装が、削除した `dense_parallel_grid_cycle_closure_premises_v1` の受理範囲を包含するか検証する。**包含しないなら限定実装をfallbackとして復活させるか、一般実装の受理条件を広げる。ori-collision 6件が回復する。

0-b. **`049c3948` の射影権威導入が `direct_f_affine_c2` / `ef_boundary` の前提を壊していないか検証する。**ori-collision 4件が回復する。

0-c. **`c4adb12e` の幾何参照再認証が vertex/edge reference 契約を壊していないか確認する。**desktop 3件。二分探索は未実施のため要確認。

0-d. **SIM-010の主張範囲が縮小していないか判断する。**失敗テストに「17-face二block」「rank4サイクル」「三ブロックMiura鎖」が含まれる。`docs/progress.md` の完成率へ反映すべきか検討されたい。

### 必須（CI復旧）

1. **`cargo fmt --all` を実行する。**未整形は §2.1 の1箇所のみ。
2. **`editor_speculative_unproven_operations_tests.rs:128` の `.find(..).is_none()` を `!(..).contains(..)` へ置換する。**ワークスペース全体でclippy警告はこれ1件のみであることを確認済み。
3. **`apply_speculative_stacked_fold_transaction` をネイティブへ実装・登録する。**現状HEADの`lib.rs`には"speculative"が1つも存在しない。未コミット集合にあるネイティブ実装を、`generate_handler!`への登録と同一コミットで着地させること。
4. **`open_project` / `open_recent_project` がrevisionを変更するようになった件を判断する。**§2.4の(a)(b)いずれかを選び、契約行列の更新またはコード修正を行うこと。

**順序の注意**: 1（fmt）と2（clippy）を先に直さない限り、`rust` ジョブはテストへ到達せず、0-a〜0-c の修正が効いたかをCIで確認できない。**1・2 → 0-a〜0-c → 3・4 の順を推奨する。**

### 推奨

5. `.gitattributes` へ `*.txt` 等のeol指定を追加する（§6）。Windows環境でのローカル事前検証が可能になる。
6. `apps/desktop/package.json` の version を `0.0.0` から `0.1.0` へ揃える（機能影響なし）。

### 判断を委ねる

7. **正式リリースを経路B（未署名Windows）で実施するか。**§5.4の手順で、証明書取得なしに配布・QA領域の残1.25ポイントへ到達できる。実行判断はオーナーの領域である。
8. `docs/progress.md` が **06:12（`21960f47`）から更新されていない。**09:11以降の一般閉路認証11コミット、決定論的超越カーネル、共有関節射影が正本へ未記録である。完成率へ反映するか否かの判断材料が現在存在しない。

---

## 8. 本作業で行っていないこと

- **リポジトリのコードを1行も変更していない。**§2の4件はすべてCodexの作業対象ファイル（`main`のHOT集合）であり、意図的に編集していない。
- `docs/progress.md`、`docs/requirements-status.md` を変更していない。正本はCodexの管轄である。
- `main`の作業ツリー、`ORIGAMI2-validation-baseline-tests`、`ORIGAMI2-validation-geometry-compat` に一切触れていない。
- GitHubへのpush、タグ作成、Release公開、workflow実行のいずれも行っていない。
- リモートCIの状態を照会していない（`gh` CLIが本環境に未導入のため）。§0の「CI redである」という判断は、CIが実行する各コマンドをローカルで同一条件で再現した結果に基づく推論である。**実際のCI実行結果そのものは確認していない。**
- `verify_sbom_completeness.mjs` と署名・公証まわりは実ビルド生成物を要するため未検証である。

---

## 9. 追補（21:00 追加検証）

本書の初版提出後、未検証だった領域をさらに検証した。**新たな阻害要因は見つかっていない。**

### 9.1 `dependency-advisory-audit` ジョブ相当（すべて合格）

| 検証 | CI該当 | 結果 |
|---|---|---|
| `npm audit --package-lock-only --audit-level=low` | `ci.yml:46` | **合格。脆弱性0件** |
| 凍結超越関数の依存フィーチャ検証 | `ci.yml:28-40` | **合格**。`cargo tree -e features -i libm` の1行目が `libm v0.2.16`、`libm feature "arch"` は不在 |
| `dependency_policy.mjs` | `release-windows.yml:86` | **合格**（cargo 508 / npm 186パッケージ、integrity・ライセンス許可リストとも充足） |

### 9.2 RustSec許容台帳（合格。ただし期限あり）

`ci.yml:69` の `test "${#ignore_args[@]}" -eq 36` は一見すると台帳の18エントリと矛盾するが、整合している。

```
verify_rustsec_warning_ledger.mjs:51  process.stdout.write(`${ids.join('\n')}\n`)   → 18行
ci.yml:63-67                          各行につき ignore_args += (--ignore, ID)      → 18 × 2 = 36
```

`.github/rustsec-warning-ledger.json` の18エントリは、ID一意・ソート済み・`cargo metadata --locked` に対する依存経路の実在まで検証され、すべて合格した。

**ただし期限が設定されている。**`verify_rustsec_warning_ledger.mjs:15` は `if (entry.expires < today) fail('expired exception')` を持つ。

```
全18エントリの expires = 2026-10-31（本日から96日）

RUSTSEC-2024-0370 proc-macro-error@1.0.4      RUSTSEC-2024-0429 glib@0.18.5
RUSTSEC-2024-0411 gdkwayland-sys@0.18.2       RUSTSEC-2025-0075 unic-char-range@0.9.0
RUSTSEC-2024-0412 gdk@0.18.2                  RUSTSEC-2025-0080 unic-common@0.9.0
RUSTSEC-2024-0413 atk@0.18.2                  RUSTSEC-2025-0081 unic-char-property@0.9.0
RUSTSEC-2024-0414 gdkx11-sys@0.18.2           RUSTSEC-2025-0098 unic-ucd-version@0.9.0
RUSTSEC-2024-0415 gtk@0.18.2                  RUSTSEC-2025-0100 unic-ucd-ident@0.9.0
RUSTSEC-2024-0416 atk-sys@0.18.2              RUSTSEC-2026-0192 ttf-parser@0.25.1
RUSTSEC-2024-0417 gdkx11@0.18.2
RUSTSEC-2024-0418 gdk-sys@0.18.2
RUSTSEC-2024-0419 gtk3-macros@0.18.2
RUSTSEC-2024-0420 gtk-sys@0.18.2
```

**2026-11-01以降、更新しなければ `dependency-advisory-audit` が自動的に失敗する。**14件はGTK3系（Tauri v2のLinux依存）でありアップストリーム更新を待つ性質のものなので、期限延長の判断が必要になる。リリース計画をこの日付より後ろに置く場合は、事前の台帳更新を予定に入れられたい。

### 9.3 `frontend` ジョブの残りステップ（すべて合格）

初版では `npm test` の失敗（#3・#4）のみ確認していたが、同ジョブの他ステップも実行した。

| 検証 | CI該当 | 結果 |
|---|---|---|
| `npm run build`（`tsc -b && vite build`） | `ci.yml:109` | **合格**（850ms、`dist/assets/index-*.js` 2,465 kB） |
| `verify_desktop_bundle_csp.mjs dist` | `ci.yml:110` | **合格** |
| `verify_production_security_contract.mjs dist` | `ci.yml:111` | **合格**（"production CSP, permissions, secret scan, and dependency licenses verified"） |
| `verify_diagnostics_privacy.mjs` | `ci.yml:112` | **合格**（初版で確認済み） |

**したがって `frontend` ジョブが失敗する原因は `npm test` の2件（#3・#4）だけである。**それ以外は全て通る。

なお `vite build` は「500 kBを超えるチャンクがある」という警告を出すが、これは警告であってビルドは成功しており、CIも失敗させない。

### 9.4 未検証のまま残る領域

| CIジョブ／検証 | 状態 | 理由 |
|---|---|---|
| `cargo test --workspace --locked --all-targets` | **実行中** | 完走まで時間を要する。結果は §9.5 に追記する |
| `slicer-acceptance` | 未検証 | PrusaSlicer実機が必要 |
| `windows-bundle` / `macos-bundle` | 未検証 | Tauriバンドル実ビルドが必要。Windows側はApplication Controlに阻まれる |
| `verify_sbom_completeness.mjs` | 未検証 | リリースビルド生成物 `*.cdx.json` が必要 |
| 署名・公証まわり | 未検証 | 証明書とsecretsが必要 |
| リモートCIの実状態 | 未照会 | `gh` CLIが本環境に未導入 |

**特に重要**: `cargo fmt --check` は `rust` ジョブでテストより前（`ci.yml:436`）に走るため、**#1が入った 049c3948（12:20）以降、CIはRustテストを1件も実行していない**とみられる。さらに #2 のclippy失敗は `bd0c8e8c`（03:07）からである。Rust 384,135行（うちテスト226,710行）が長時間CI検証を経ていない状態にある。本書の `cargo test` 実行はその穴を埋めることを目的としている。

### 9.5 【#5】`cargo test --workspace` が失敗する ── **証明能力の実リグレッション 20件**

完走した。結果は次のとおりである。

```
cargo +1.90.0 test --workspace --locked --all-targets --no-fail-fast

合計          2,722 合格 / 20 失敗
ori-collision   434 合格 / 10 失敗
origami2-desktop 721 合格 / 10 失敗
他の全クレート                0 失敗
```

**これは §2 の #1〜#4 とは質が異なる。証明パイプラインが以前は成立させていた閉包を成立させられなくなっている。**

#### 9.5.1 環境依存ではないことの立証

当方の実行環境（WSL2 aarch64-unknown-linux-gnu）は `docs/progress.md` が定める未対応targetであるため、まずこれを疑い、対照実験を行った。

**実験1: 決定論的カーネル自体の健全性**

```
cargo test --release -p ori-numeric --lib deterministic_transcendental

環境変数なし                                    → 4 passed / 0 failed
  v1_golden_binary64_corpus_is_bit_exact ... ok       ← ビット完全一致
ORI_REQUIRE_SUPPORTED_TRANSCENDENTAL_TARGET=1  → 3 passed / 1 failed
  失敗したのは v1_model_id_and_supported_targets_are_explicit のみ
```

golden bit corpusはこの環境でも**ビット完全に一致する**。失敗するのは「対象target一覧に載っていない」という宣言部分だけである。したがってカーネルの計算結果は正しい。

**実験2: CI緑が記録されたコミットとの対照**

`docs/progress.md` が「CI #686 で必須7 jobがすべて success」と記録する `f9913149`（07-26 11:43）を、**同一環境・同一toolchain**でテストした。

```
f9913149  ori-collision  376 合格 / 0 失敗
c4adb12e  ori-collision  434 合格 / 10 失敗
```

**実験3: 失敗した10件が旧コミットに存在したかの確認**

```
10件中 9件が f9913149 に存在し、すべて合格していた
残る1件 complete_live_three_four_and_eight_block_authorities_are_sealed_and_non_authorizing のみ新規
```

同一環境・同一テストで片方だけ失敗するため、**環境依存でも新規テスト追加による見かけの失敗でもない。実リグレッションである。**

#### 9.5.2 発生コミットの特定（二分探索）

`f9913149..c4adb12e` のうち `ori-collision` / `ori-kinematics` / `ori-core` を変更した59コミットを二分探索した。**2つの独立したリグレッションが存在する。**

##### 群A（6件）── `9579e852`（07-27 09:11「密集格子閉路の一般認証を追加する」）

```
248c6fec 06:12 長さ制約グラフの意味論的証拠を追加する   → GOOD
9579e852 09:11 密集格子閉路の一般認証を追加する         → BAD   ★
```

`9579e852` 単体でのori-collision実測: **426 合格 / 6 失敗**

```
block_composition::tests::submitted_three_block_tree_authority_revalidates_and_rejects_bound_tampering
block_composition::tests::complete_live_three_four_and_eight_block_authorities_are_sealed_and_non_authorizing
continuous_path::tests::miura_rank_eight_to_sixty_four_cell_proofs_are_bounded_and_deterministic
continuous_path::tests::miura_rank_four_fixture_issues_global_layer_authority
continuous_path::tests::sixty_degree_axis_rank_four_dense_graph_remains_exact_and_fail_closed
continuous_path::tests::three_by_three_blocks_issue_canonical_blockwise_closure
```

**原因の構造**: このコミットは `crates/ori-kinematics/src/graph.rs` から限定実装 `dense_parallel_grid_cycle_closure_premises_v1`（131行）を**削除**し、新しい一般実装 `graph/dense_grid.rs`（513行）へ置換した。削除された関数の受理条件は次のとおりである。

```rust
// Exact two-carrier accordion identity for the smallest non-cactus square grid.
// Three collinear material segments on each of two parallel carrier lines share
// one canonical profile; the six transverse hinges remain exactly stationary.
fn dense_parallel_grid_cycle_closure_premises_v1(...) -> bool {
    let face_count = geometry.face_ids().len();
    let Some((columns, rows)) = (3usize..=9).find_map(|columns| {
        (3usize..=9).find_map(|rows| {
            (columns * rows == face_count
                && geometry.hinges().len() == 2 * columns * rows - columns - rows
                && audit.closure_hinges().len() == (columns - 1) * (rows - 1))
                .then_some((columns, rows))
        })
    }) else { return false; };
    ...
```

**新しい一般実装が旧特殊ケースを取りこぼしている。**典型的な一般化リグレッションである。結果、以前は成立していた閉包が次のように fail-closed へ落ちる。

```
panicked at crates/ori-collision/src/block_composition.rs:2045:22:
three-block closure: UnprovenClosure { depth: 8, index: 0 }
```

安全側への退避なので誤答ではないが、**利用者から見れば「昨日まで折れた三ブロック構成が折れなくなった」**ことを意味する。

##### 群B（4件）── `049c3948`（07-27 12:20「三面二関節の共有関節診断を厳密射影する」）

```
b397f4ba 12:03 ブロック局所自由語で一般閉路を厳密認証する → GOOD
049c3948 12:20 三面二関節の共有関節診断を厳密射影する     → BAD   ★
```

`049c3948` 単体でのori-collision実測: **426 合格 / 10 失敗**（群A 6件＋群B 4件）

```
cayley::positive_thickness::tests::direct_f_affine_c2_large_pivot_drift_remains_an_unadmitted_diagnostic
cayley::positive_thickness::tests::direct_f_affine_c2_rejects_aba_foreign_tokens_thickness_and_all_36_transform_bits
cayley::positive_thickness::tests::direct_f_affine_c2_400mm_matrix_is_120_contained_unadmitted_diagnostics
cayley::positive_thickness::tests::ef_boundary_rejects_separate_exact_aba_foreign_reroot_thickness_faces_and_f_bits
```

**この `049c3948` は §2.1 の `cargo fmt` 違反（#1）を入れたコミットと同一である。**1つのコミットが整形違反と4件の証明リグレッションを同時に持ち込んでいる。

#### 9.5.3 `origami2-desktop` の10件

```
downstream（群A・群Bの影響下と推定）:
  applied_pose::static_collision::tests::positive_thickness_three_face_current_pose_requires_complete_pair_evidence
  applied_pose::static_collision::tests::positive_thickness_mid_surface_transversal_has_a_distinct_redacted_reason
  stacked_fold_read::tests::rank4_cycle_transports_layer_order_and_applies_atomically
  stacked_fold_read::tests::seventeen_cell_blockwise_preview_is_atomic_and_fail_closed
  stacked_fold_read::tests::seven_hinge_generic_grid_proof_applies_and_persists_atomically
  stacked_fold_read::tests::three_block_miura_chain_stops_at_the_two_block_positive_layer_authority_boundary
  stacked_fold_read::stacked_fold_blockwise_cycle_tests::seventeen_cell_current_cycle_uses_blockwise_fallback_end_to_end

幾何参照系（c4adb12e 由来と強く示唆されるが未二分探索）:
  tests::vertex_reference_depth_64_is_allowed_and_65_is_rejected
  tests::vertex_reference_requires_lowercase_canonical_uuid_and_allows_equal_values
  tests::zero_length_edge_reference_fails_closed
```

後者3件は `2a33b9aa`（07-26 18:05）で追加されたテストであり、`c4adb12e` は同じ `apps/desktop/src-tauri/src/tests.rs` を26行変更し、`geometry_reference_compat_tests.rs`（846行）を新設している。**強く示唆されるが、時間の都合で二分探索は行っていない。** Codex側で確認されたい。

`stacked_fold_read::tests::seventeen_cell_blockwise_preview_is_atomic_and_fail_closed` と
`stacked_fold_read::stacked_fold_blockwise_cycle_tests::seventeen_cell_current_cycle_uses_blockwise_fallback_end_to_end` は、
`docs/requirements-status.md` が **SIM-010の実装根拠として明示している「17-face・二blockの保存済み適用後層順」**に直接対応する。SIM-010の現行の主張範囲が実際に後退している可能性がある。

#### 9.5.4 なぜCIで検出されなかったか

```
ci.yml:436  cargo fmt --all -- --check   ← #1 で失敗（049c3948、12:20 以降）
ci.yml:465  cargo test ...               ← 前段が落ちるため到達しない
ci.yml:593  cargo clippy ...             ← #2 で失敗（bd0c8e8c、03:07 以降）
```

**`rust` ジョブは整形チェックで停止し、テストへ到達していない。**そのため群Bは検出されず、群A（09:11導入）も 12:20 以降は検出不能な状態にあった。

さらに `frontend` ジョブも #3（06:27）以降は落ちている。**本日 03:07 以降、CIはどのジョブでも新しい証明リグレッションを検出できない状態が続いていた。**

#### 9.5.5 依頼

1. **`9579e852` の一般 dense-grid 実装が、削除した `dense_parallel_grid_cycle_closure_premises_v1` の受理範囲を完全に包含するか検証されたい。**包含しないなら、限定実装を fallback として復活させるか、一般実装の受理条件を広げること。
2. **`049c3948` の射影権威導入が `direct_f_affine_c2` / `ef_boundary` の前提を壊していないか検証されたい。**
3. **`c4adb12e` の幾何参照再認証が vertex/edge reference 契約を壊していないか確認されたい。**
4. これらは fail-closed 方向の後退なので誤答は生じないが、**SIM-010の主張範囲（17-face二block、rank4サイクル、三ブロックMiura鎖）が実際に縮小している可能性がある。**`docs/progress.md` の完成率へ反映すべきかを判断されたい。

---

## 付録A: 再現手順

```bash
# 1. HEADのクリーンなworktreeを作る（短いパス推奨。長いパスはgitのstat制限に触れる）
git worktree add --detach C:/Users/oltot/o2a HEAD
cd C:/Users/oltot/o2a

# 2. Rust（WindowsはApplication Controlに阻まれるためWSL）
wsl -e bash -lc 'cd /mnt/c/Users/oltot/o2a && cargo +1.90.0 fmt --all -- --check'
wsl -e bash -lc 'cd /mnt/c/Users/oltot/o2a && CARGO_TARGET_DIR=/tmp/ori-audit \
  cargo +1.90.0 clippy --workspace --locked --all-targets --all-features -- -D warnings'

# 3. フロントエンド
cd apps/desktop && npm ci --no-audit --no-fund
npm run lint
npm run test:snap        # 2,075件
npm run test:dom         # 600件

# 4. 要件トレーサビリティ（CRLF環境では docs/requirements-ids.v1.txt を先にLF化）
cd C:/Users/oltot/o2a
REQUIREMENTS_EVIDENCE_ROOT="$PWD" node --test .github/tests/requirements-traceability.test.mjs
REQUIREMENTS_EVIDENCE_ROOT="$PWD" node .github/scripts/verify_requirements_traceability.mjs \
  docs/requirements-status.md docs/requirements-evidence.v1.json docs/requirements-ids.v1.txt

# 5. リリース契約
node --test .github/tests/formal-release.test.mjs
REQUESTED_MODE=dry-run node .github/scripts/validate_formal_release.mjs

# 6. 後片付け
cd /path/to/ORIGAMI2 && git worktree remove --force C:/Users/oltot/o2a
```

## 付録B: 発生コミットの確定方法

| # | 方法 |
|---|---|
| 1 | `git show <commit>:<file>` で各コミットの当該ファイルを取り出し、`rustfmt --edition 2024 --check` を単体実行（ビルド不要） |
| 2 | `git log -S'.find("speculative_unproven_fold_v1")' -- <file>` |
| 3 | worktreeで `21960f47` と `5ef75d38` をcheckoutし `node --test tests/tauriCapabilityContract.test.ts` を実行（3/3 → 2/3） |
| 4 | worktreeで `7a5eb7a4` と `c4adb12e` をcheckoutし `node --test tests/projectMutationInstanceIntegration.test.ts` を実行（74/74 → 73/74） |
| 5 | `f9913149..c4adb12e` のうち `ori-collision` / `ori-kinematics` / `ori-core` を変更した59コミットを二分探索。代表テスト1件を `cargo test -p ori-collision --lib -- --exact <name>` で判定し、GOOD/BAD を挟み込んだ。群Bは同手法で `9579e852..c4adb12e` の16コミットを再探索した |

### 付録B補足: worktree で `git bisect run` を使う際の注意

本検証では `git bisect run` が使えなかった。worktreeの `.git` ファイルは親リポジトリを**Windows絶対パス**で参照するため、WSL側の git が解決できない。

```
fatal: not a git repository: /mnt/c/Users/oltot/o2g/C:/Users/oltot/Documents/git-projects/ORIGAMI2/.git/worktrees/o2g
```

**回避策**: checkout は Windows 側の git、ビルドとテストは WSL 側の cargo、という分業にする。本書の二分探索はこの構成で実施した。

また、**深いパスの worktree は避けること。**`git show <commit>:<path>` が `Filename too long` で失敗し、要件トレーサビリティ検証が誤って失敗する（§4.1）。`C:/Users/<user>/o2a` 程度の短さを推奨する。

`tests/tauriCapabilityContract.test.ts` と `tests/projectMutationInstanceIntegration.test.ts` は `node:test` と `node:assert` のみを使うため、**`node_modules` なしで単体実行できる**。コミット単位の切り分けが安価に行えるので、今後の回帰特定にも使える。
