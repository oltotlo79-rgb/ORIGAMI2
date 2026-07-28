# Codex向け情報: repository フォルダ 260GB の内訳と安全な削減対象

作成日: 2026-07-26
対象 path: `C:\Users\oltot\Documents\git-projects\ORIGAMI2`
計測時刻: 2026-07-26 21:00〜21:45
作成者: Claude（読み取りのみ。file の削除・変更は一切していない）

この文書は実測値の報告と提案であり、削除の実施は行っていない。
すべての数値は当日の実測であり、推定値を含まない。

---

## 1. 結論

```text
実測合計                          260.4 GB
  target-*（56 ディレクトリ）      240.9 GB   ← 全体の 92.5%
  target（本体ビルド）              20.0 GB
  ソース・docs・.git・node_modules   0.28 GB
```

**source code 本体は 280 MB しかない。260 GB のほぼ全量が cargo のビルドキャッシュである。**

`target-*` のうち 53 個（約 224 GB）は確実に削除してよい。
削除後は 260 GB → 約 37 GB になる。

---

## 2. `target-*` が確実に不要である根拠

### 2.1 すべて cargo のビルドキャッシュである

各ディレクトリの直下は `CACHEDIR.TAG` / `debug` / `tmp` の 3 項目のみ。
`CACHEDIR.TAG` の中身は次のとおりで、cargo 自身がキャッシュであると宣言している。

```text
Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cargo.
# For information about cache directory tags see https://bford.info/cachedir/
```

source file、生成物の原本、report の類は含まれない。

### 2.2 CI と script から参照されていない

`.github/workflows/` と `scripts/` を全文検索した結果、`target-` の一致は 1 件のみで、
それはディレクトリ参照ではなかった。

```text
.github/workflows/ci.yml:420   "RUSTFLAGS=-C target-feature=+crt-static"
```

`target-feature` は compiler flag であり、build ディレクトリとは無関係である。

### 2.3 git 管理外であり、`.gitignore` にも入っていない

```text
.gitignore:3   target/
```

`target/` だけが指定されており、`target-*` は **ignore 対象ですらない**。
そのため `git status --short` に 56 行の `?? target-xxx/` が常時表示され、
実際の作業差分が読み取りにくくなっている。削除すれば status も正常化する。

### 2.4 大半が数日前の使い捨てである

最終更新日時と名前から、一時的な検証用と判別できる。
20 ディレクトリが 7/22〜7/23 のもので、3〜4 日間触られていない。

---

## 3. `target-*` 56 ディレクトリの実測一覧（最終更新の古い順）

| ディレクトリ | GB | 最終更新 |
|---|---:|---|
| target-windows-check | 3.55 | 07-22 18:41 |
| target-fault-matrix | 0.03 | 07-22 18:59 |
| target-unix-fault | 7.02 | 07-22 19:35 |
| target-singleflight-check | 9.69 | 07-22 19:37 |
| target-windows-diagnose | 0.09 | 07-22 19:52 |
| target-petal-probe | 15.81 | 07-22 20:46 |
| target-canonical-block | 0.64 | 07-22 22:57 |
| target-focused-clean | 17.90 | 07-23 00:17 |
| target-unix-fallback | 6.37 | 07-23 00:20 |
| target-focused-a6 | 6.02 | 07-23 01:04 |
| target-focused-duplicate | 8.70 | 07-23 05:09 |
| target-wsl-multiblock | 20.54 | 07-23 05:12 |
| target-wsl-f4 | 6.94 | 07-23 08:35 |
| target-sim010 | 0.06 | 07-23 08:57 |
| target-exact-overlap | 0.94 | 07-23 10:04 |
| target-focused-relief-coverage | 0.05 | 07-23 11:25 |
| target-subdivision-plan | 3.66 | 07-23 12:23 |
| target-wsl-relief | 2.80 | 07-23 12:51 |
| target-cut-snapshot | 0.58 | 07-23 14:13 |
| target-effective-cut-ipc | 6.19 | 07-23 16:01 |
| **小計（7/23 以前 20 個）** | **約 104 GB** | |
| target-a68 | 3.30 | 07-26 04:45 |
| target-c3-doc | 2.18 | 07-26 04:56 |
| target-agent-a7 | 0.04 | 07-26 05:23 |
| target-a45 | 13.55 | 07-26 05:45 |
| target-c5-live | 3.20 | 07-26 05:55 |
| target-c5-d1-a7-verify | 2.97 | 07-26 05:56 |
| target-d1-work-sum | 1.61 | 07-26 06:00 |
| target-d1-work-sum-alloc-gcd | 1.61 | 07-26 06:04 |
| target-d1-exact-e-clamp | 1.61 | 07-26 06:08 |
| target-d1-exact-e-clamp-shared | 1.61 | 07-26 06:13 |
| target-d1-exact-prism-clamp | 1.61 | 07-26 06:17 |
| target-d1-resource-limit-check | 2.52 | 07-26 06:27 |
| target-a7-failclosed | 1.70 | 07-26 06:31 |
| target-d1-checked-sum-pt | 1.94 | 07-26 06:32 |
| target-d3-expected-command-root | 2.53 | 07-26 06:44 |
| target-d1-checked-sum-margin-solid | 1.61 | 07-26 06:49 |
| target-d3-cohort3 | 0.17 | 07-26 06:55 |
| target-d1-admission-clamp | 1.61 | 07-26 06:57 |
| target-d1-clamp-parent | 2.29 | 07-26 06:57 |
| target-d1-affine-parent-check | 1.84 | 07-26 07:02 |
| target-d1-topology-clamp | 2.25 | 07-26 07:09 |
| target-d1-solid-clamp | 2.25 | 07-26 07:16 |
| target-d3-cohort2 | 4.64 | 07-26 07:28 |
| target-d1-cayley-sums-verify | 0.26 | 07-26 07:37 |
| target-d1-cayley-sums | 4.95 | 07-26 07:45 |
| target-claude-p3 | 0.07 | 07-26 08:30 |
| **小計（7/26 朝 26 個）** | **約 64 GB** | |
| target-c5-audit | 0.35 | 07-26 14:58 |
| target-edt009-ratio-pair | 2.28 | 07-26 15:00 |
| target-focused-edt009 | 4.06 | 07-26 16:48 |
| target-wsl-sim010 | 3.74 | 07-26 16:55 |
| target-focused-general-tree | 7.79 | 07-26 16:55 |
| target-wsl-exact | 12.25 | 07-26 17:26 |
| target-wsl-edt009 | 11.81 | 07-26 17:44 |
| **小計（7/26 午後 7 個）** | **約 42 GB** | |
| target-wsl-general-tree | 12.58 | 07-26 20:32 |
| target-wsl-collision-audit | 2.36 | 07-26 20:42 |
| target-wsl-core-ratio-cycle | 2.17 | 07-26 21:36 |
| **小計（作業中の可能性 3 個）** | **約 17 GB** | |

---

## 4. 削除してはならないもの、および注意点

### 4.1 本体 `target`（20 GB）は残すことを強く推奨する

削除自体は安全（再ビルドで復元可能）だが、**実害の前例がある**。

2026-07-26 の 14〜15 時ごろ、本体 `target` が 225.7 GB から 10.5 GB へ縮小された。
その直後から、Windows 上の cargo が次で失敗するようになった。

```text
cargo check --locked -p origami2-desktop --lib --tests
  -> could not execute process
     `...\target\debug\build\windows_aarch64_msvc-...\build-script-build`
  Caused by: アプリケーション制御ポリシーによってこのファイルがブロックされました。 (os error 4551)
```

`target` が空になった結果 build script が新規生成され、
それを Windows Application Control が遮断している。
本報告の作成時点でも Windows 上での cargo 実行は不能であり、
native 検証は WSL 経由でしか行えていない。

**20 GB を回収するために作業環境を壊す取引は割に合わない。**

### 4.2 作業中の可能性がある 3 ディレクトリ（合計 17 GB）

```text
target-wsl-core-ratio-cycle   2.17 GB   07-26 21:36 更新（最新）
target-wsl-collision-audit    2.36 GB   07-26 20:42 更新
target-wsl-general-tree      12.58 GB   07-26 20:32 更新
```

いずれも直近 1 時間以内に書き込みがあり、進行中の WSL 検証で使用中と考えられる。
作業が一段落するまで残すのが無難である。

### 4.3 触れる必要がないもの

```text
apps/desktop/node_modules   195 MB
node_modules                 52 MB
.git                         23 MB
source / docs               約 10 MB
```

合計 280 MB であり、削減効果がない。`node_modules` を消しても
`npm install` の再実行コストに見合わない。

---

## 5. 推奨する削除手順

### 5.1 段階 1: 7/23 以前の 20 個（約 104 GB、最も安全）

3 日以上触られておらず、名前も一時検証用と判別できるもの。

```powershell
$root = "C:\Users\oltot\Documents\git-projects\ORIGAMI2"
$old = @(
  'target-windows-check','target-fault-matrix','target-unix-fault',
  'target-singleflight-check','target-windows-diagnose','target-petal-probe',
  'target-canonical-block','target-focused-clean','target-unix-fallback',
  'target-focused-a6','target-focused-duplicate','target-wsl-multiblock',
  'target-wsl-f4','target-sim010','target-exact-overlap',
  'target-focused-relief-coverage','target-subdivision-plan','target-wsl-relief',
  'target-cut-snapshot','target-effective-cut-ipc'
)
foreach ($d in $old) { Remove-Item -Recurse -Force (Join-Path $root $d) -ErrorAction SilentlyContinue }
```

### 5.2 段階 2: 作業中 3 個を除く全 `target-*`（約 224 GB）

段階 1 を含む。作業が一段落してから実行する。

```powershell
$keep = @('target-wsl-core-ratio-cycle','target-wsl-general-tree','target-wsl-collision-audit')
Get-ChildItem "C:\Users\oltot\Documents\git-projects\ORIGAMI2" -Directory -Filter "target-*" |
  Where-Object { $_.Name -notin $keep } |
  Remove-Item -Recurse -Force
```

### 5.3 段階 3: 作業完了後に残り 3 個（17 GB）

WSL 検証が完了し、進行中の作業が commit された後に実行する。

```powershell
Get-ChildItem "C:\Users\oltot\Documents\git-projects\ORIGAMI2" -Directory -Filter "target-*" |
  Remove-Item -Recurse -Force
```

---

## 6. 再発防止の提案

### 6.1 `.gitignore` へ `target-*/` を追加する

現状 `.gitignore` は `target/` のみを指定しているため、
`target-*` は untracked file として `git status` に 56 行表示され続けている。

```diff
  target/
+ target-*/
```

これにより次の 2 点が改善する。

1. `git status --short` が実際の作業差分だけを表示するようになる。
2. `git add -A` などの誤操作でビルドキャッシュが index へ入る事故を防げる。

**注意**: この変更は repository file の編集にあたる。
Claude は `docs/Claude` の指示書に明示された file 以外を編集しない運用のため、
実施していない。必要であれば指示いただきたい。

### 6.2 検証用 target を repository 外へ置く運用

`CARGO_TARGET_DIR` を repository 外の path（例: WSL の `/tmp/...`）にすれば、
repository フォルダのサイズは増えない。

本日 Claude が行った native 検証では、WSL の `/tmp` 配下を使用しており、
repository 内に新しい `target-*` を作っていない。

```bash
CARGO_TARGET_DIR=/tmp/origami2-viewer-negative-matrix cargo test ...
```

ただし WSL の `/tmp` は VHD 上にあり、最終的には同じ物理ディスクを消費するため、
不要になった時点での削除は必要である。

---

## 7. 計測に使用した command

再現可能なよう記録する。

```powershell
# 全体内訳
Get-ChildItem -Force | ForEach-Object {
  $s=(Get-ChildItem $_.FullName -Recurse -File -ErrorAction SilentlyContinue |
      Measure-Object -Property Length -Sum).Sum
  [PSCustomObject]@{Name=$_.Name; GB=[math]::Round($s/1GB,2)}
} | Sort-Object GB -Descending

# target-* の個数と合計
Get-ChildItem -Directory -Force -Filter "target-*" | Measure-Object

# 最終更新日時付き一覧
Get-ChildItem -Directory -Force -Filter "target-*" | ForEach-Object {
  $last = (Get-ChildItem $_.FullName -Recurse -File -ErrorAction SilentlyContinue |
           Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime
  ...
}
```

```bash
# キャッシュ標識の確認
cat target-wsl-multiblock/CACHEDIR.TAG

# CI/script からの参照確認
grep -rn "target-" .github/workflows/ scripts/

# gitignore 状態
git check-ignore -v target-wsl-multiblock target
```

---

## 8. まとめ

| 判断 | 対象 | 回収量 |
|---|---|---:|
| **確実に不要** | `target-*` のうち 7/26 20:00 以前の 53 個 | **約 224 GB** |
| 作業完了後に削除 | `target-wsl-*` 直近 3 個 | 17 GB |
| **残すべき** | 本体 `target`（Application Control 再発防止） | 20 GB |
| 対象外 | `node_modules` 247 MB、`.git` 23 MB、source 約 10 MB | — |

段階 2 まで実施すれば **260 GB → 約 37 GB** になる。

Claude は本報告の作成にあたり file の削除・変更を一切していない。
実施の可否と時期は Codex または repository owner の判断に委ねる。
