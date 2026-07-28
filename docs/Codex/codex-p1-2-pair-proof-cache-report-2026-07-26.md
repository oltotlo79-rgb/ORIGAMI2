# P1-2 証明済み pair 永続キャッシュ実装報告

日付: 2026-07-26
対象: `TwoHingePositiveThickness`（連続証明 model 4）の production 経路のみ

## 実装境界

- process-local の永続 pair 証明キャッシュを project instance、project、revision、geometry fingerprint、pose generation、正の paper thickness、face pair、certificate model、issuer context の完全 key へ束縛した。
- desktop は project → pose → cache の lock 順で capture / edit epoch を扱う。exact pose 準備、witness encoding、cold prism proof は cache mutex 外で行い、publish 直前に epoch と完全 binding を再検証する。
- vertex / edge / face の完全 edit impact を revision 間で集約し、current face footprint と exact pose の完全 snapshot で差分 rebind する。impact が不完全、取消、期限超過、上限超過、snapshot 不一致、stale epoch の場合は fail-closed とする。
- normal hit でも両 face の全 vertex / edge ID と exact witness の全 byte を比較する。snapshot canonicalization、cache lookup、pending rebind、hit 再検証は単一の 2,000,000 work envelope を共有し、長い比較には cooperative checkpoint を置いた。
- capacity 超過は証明済みに数えず、`capacity_unproven_pairs` として明示する。cold と hit は同じ pair work counter を最終診断へ計上する。

## 実測受入結果

- 実 model 4 fixture（6 face branched tree、30度、厚さ1.0、2 sampled poses）:
  - endpoint exact pair: 1
  - cold: `cold_proofs=1`, `cache_hits=0`
  - hit: `cold_proofs=0`, `cache_hits=1`
  - 最終 diagnostic は bit-exact 同一
  - additive 25 counter と maximum 10 counter は全項目同一
- 合法な15 face treeの unique leaf vertex 1点編集:
  - 全 unordered pair: 105
  - retained hit: 91
  - actual cold reproof: 14
  - reproof ratio: `14 / 105 = 13.33%`（20%未満）
- normal hit の全ID・全byte work、pending rebindを含む総work、desktop edit-impact sort workは、exact limit成功 / one-short拒否を回帰した。
- signed zero、snapshot違い、連続edit集約、capacity境界、cold中epoch更新、partial-cold取消、post-rebind取消後の不正retry拒否、deadlineを回帰した。

## 保守性

責務を次の専用moduleへ分離し、各新規fileを500行未満に抑えた。

- `continuous_path/pair_proof_cache.rs`: model 4 cache orchestration
- `proof_cache_runtime/edit_epoch.rs`: edit epoch / complete impact aggregation
- `proof_cache_runtime/snapshot_validation.rs`: snapshot準備 / hit再検証 / 総work
- `proof_cache_edit_impact.rs`: desktopのbit-exact edit impact導出

## 非主張

- 他の7 continuous certificate modelはこのcacheへ接続しておらず、hit / cold /完成扱いへ数えない。
- 本実装は model 4 の既存連続証明を高速化・堅牢化する内部品質対応であり、一般多hinge motion、一般三block以上のApply、未対応の一般ケースを完成させない。
- したがって SIM-010の部分実装、MUST集計、全体完成度 **81.96%（表示82.0%）** は変更しない。
