# コード監査第3回への対応記録

## 判定基準と再照合点

本書は `docs/plans/code-audit-round3-2026-07-23.md` の A-1〜A-8、B-1〜B-7、C-1〜C-5、D-1〜D-3 を、コード、テスト、文書、確定済みGit履歴へ再照合した結果である。

- `採用`: 指摘どおりの修正または振る舞い保存リファクタを確定済み。
- `修正採用`: 問題の根拠は認めるが、公開API互換性、fail-closed境界、CI gateなどを守る別の対応を採用。
- `不採用`: 根拠または提案手段が現行契約に適合しない。理由を明記する。
- 本書が実装根拠として再照合したcommitは、各表の「主要対応commit（非網羅）」欄に記載する。本書自身を確定するcommit、local/remoteの一時的な先行差、未commitの並行作業は、完了判定・CI完了・完成度更新の根拠に含めない。

完成度の正本は [`progress.md`](progress.md) の **79.32%（表示79.3%）**、MUST 87件の正本は [`requirements-status.md`](requirements-status.md) の **実装済み85 / 部分実装2 / 未着手0** である。[`progress-reassessment-pending-ci-2026-07-22.md`](progress-reassessment-pending-ci-2026-07-22.md) の81.96%（表示82.0%）は、同一remote headの必須CIと正本更新後CIがすべてgreenになるまで提案値のままとする。部分実装はEDT-009とSIM-010であり、本書の内部品質対応を理由に実装済みへ変更しない。

## A. 不具合

| 項目 | 判定 | 根拠と確定した対応 | 主要対応commit（非網羅） | 残課題 |
|---|---|---|---|---|
| A-1 法線角の日本語ラベル | 採用 | [`FoldPreview.tsx`](../apps/desktop/src/components/FoldPreview.tsx) の表示を計算内容と一致する「2面の法線角」へ訂正し、後続のカタログ化でも同じ文言を保持した。計測式とIPCは変更していない。 | `4ca5ef3`, `f98cfb8` | なし |
| A-2 state updater内の副作用 | 採用 | [`EffectiveCutDiagnosticPanel.tsx`](../apps/desktop/src/components/EffectiveCutDiagnosticPanel.tsx) で`setResult(null)`を`setSelected` updaterの外へ移した。選択結果は維持し、StrictModeでupdaterが再実行されても副作用を重複させない。 | `4ca5ef3` | なし |
| A-3 面0のplanar model | 採用 | [`instructionOnionSkin.ts`](../apps/desktop/src/lib/instructionOnionSkin.ts) は先頭面を取得して存在確認してからIDを読む。空面は既存の利用不可契約どおり`null`を返し、専用回帰を追加した。 | `4ca5ef3` | なし |
| A-4 `applied_pose`の`tree().expect()` | 採用 | production accessorを`Option`のまま扱い、Treeでない姿勢は固定エラーへfail-closedするよう統一した。現存する同文言の`expect`はtest fixture内だけである。 | `b5ab2be` | なし |
| A-5 `from_document`の復元panic | 採用 | [`lib.rs`](../apps/desktop/src-tauri/src/lib.rs) の初心者設計profile復元を`map_err(...)?`へ変更し、不正archiveを`PROJECT_ARCHIVE_INVALID_MESSAGE`で拒否する。 | `b5ab2be` | なし |
| A-6 共有ヒンジ推定とAABB先行判定 | 採用 | [`positive_thickness.rs`](../crates/ori-collision/src/cayley/positive_thickness.rs) で、対象2面、異なる2端点、hinge端点の一致を同時に要求する。回廊による許容はexact prism intersectionの`PositiveVolume`分類後だけに適用する。 | `83931a0` | なし |
| A-7 多面と2面の証明強度差 | 修正採用 | 監査時点に存在した、共有hinge pairのAABB `SharedHingeCorridorAllowed`をstrict E/F失敗時にもproof coverageへ昇格するfallbackを除去した。全pair exact prism走査後、各共有hingeのstrict classifierが`Allowed`を返した場合だけcoverageへ追加する。当該Tree E/F classifierの完全authorityは2面・1hinge限定のため、その経路の多面はsanitized `Indeterminate`から`PairEvidenceUnavailable`へ閉じ、public proofを発行しない。AABB corridor値は診断前分類には残るが、共有hinge proof authorityを持たない。CIで残存が判明した旧3面safe-certificate期待も、blockingかつ`EvidenceUnavailable`を要求する回帰へ同期した。 | `83931a0`, `1b0f2ff`, `3378c8c`, `31ba69f`, `dfbfb29` | 一般multi-face E/F classifierそのものは未完成。SIM-010の部分実装理由として維持する |
| A-8 MUS oracle上限 | 採用 | [`constraints.rs`](../crates/ori-core/src/constraints.rs) の上限を`(1 << MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1) - 1`から導出し、16件で65,535回になる境界を回帰した。 | `83931a0` | 直接矛盾oracleが扱わない制約種の一般unsat判定はEDT-009残課題 |

## B. 文書・契約の矛盾

| 項目 | 判定 | 根拠と確定した対応 | 主要対応commit（非網羅） | 残課題 |
|---|---|---|---|---|
| B-1 3D 99%表示 | 修正採用 | `progress.md`の表は79.32%確定時の凍結基準であり、現在能力99%の主張ではないことを冒頭で明記した。監査補正75%はpending再評価へ分離した。 | `64b5723` | CI gate通過後にだけ正本表を更新する |
| B-2 制約100%表示 | 修正採用 | 凍結表と現在能力を分離し、pending値を85%とした。限定直接矛盾と一般充足可能性を同一視せず、EDT-009は部分実装を維持する。 | `64b5723`, `f5b94aa` | 全制約種のsoundな一般矛盾原因と一般MUS |
| B-3 自動設計55%表示 | 修正採用 | 凍結表と現在能力を分離し、一般treeの未証明範囲を反映したpending値35%へ訂正した。過大値からの加点はしていない。 | `64b5723` | 一般画像・3D目標から平坦可解な一枚紙設計を生成する利用者経路 |
| B-4 矛盾variant数 | 採用 | 現行基準を`DirectConstraintConflictKindV1`の21 variant（17 fixed-pattern + 4 general-graph）へ統一し、過去時点の9種・13種・17種・18種・19種・20種等は履歴値と明記した。18番目は、同一役割順の回転対称2件がbit非同一角であり、実在する半径辺にconsistentな正の固定長がある場合だけ、collapse解を排除して直接矛盾とする。19番目は、同じcenterで`source/target`を逆にした2回転のbinary64加算結果が360度でなく、同じく正の固定半径がある場合だけ合成回転のcollapse矛盾を肯定する。exact和が非360でも加算結果が360度へ丸まる境界は判定保留にする。20番目は、鏡映対称の一方の頂点が同じ鏡映軸上にあるという明示制約と、鏡映頂点対を正の有限固定長で結ぶ実在辺が同時にある場合だけ、軸上点を不動点とする鏡映が頂点対の一致を要求するため直接矛盾とする。頂点対・軸辺・固定間隔辺はexact IDとexact endpointで照合し、初期座標の共線性や近似長を根拠にしない。固定長群はbinary64でbit一致する場合だけ再利用し、bit非同一群、異なる軸・頂点・辺、固定間隔の欠落は判定保留にする。鏡映軸自体が退化する経路は充足解として受理せず`NonConvergent`へfail-closedする。21番目は、180度ではない回転対称と、`source on line(center,target)`または`target on line(center,source)`を同一実在半径edgeで指定する`PointOnLine`の2制約を直接矛盾とする。正規化したPointOnLineはline edgeのcollapseを`NonConvergent`へ閉じ、非退化時は非零ベクトルの非180度回転像が共線にならない。したがって固定長は削除最小原因へ含めず、初期座標・epsilon・近似値も証拠にしない。solverとsolution verifierはsoundなdirect preflight conflictを数値許容誤差より先に拒否する。 | `64b5723`, `d5f9bb9`, `4df4837`, `d40165c`, `f4e42a2`, `fdb65ef`, `a4b5f47` | variant数は能力範囲そのものではないため、EDT-009完成根拠には使わない |
| B-5 総計・MUST集計・日付 | 修正採用 | 監査時点では [`requirements-status.md`](requirements-status.md) の更新日を2026-07-23、現在値を85/2/0へ統一した。後続の正本変更に合わせ、更新日は2026-07-26へ同期した。旧84.0%や86/1/0は監査履歴から削除せず「現在値ではない」と明示し、pending 82.0%と正本79.3%を分離した。 | `64b5723`, `d5f9bb9`, `f4e42a2` | pending値を正本へ反映するCI gate |
| B-6 多面proofのコメント | 採用 | 監査時点では [`static_collision.rs`](../crates/ori-collision/src/static_collision.rs) のコメントと到達可能な多面constructorが矛盾していた。A-7修正後は、当該Tree正厚枝のconstructorは全hingeのstrict `Allowed`を前提とし、2面・1hinge限定gateにより多面から到達不能である。zero-thickness fall-throughもproofを作らず、blocking penetrationまたは`PairEvidenceUnavailable`へ閉じる。現行コメントと3面のproduction回帰をこの実装境界へ同期した。 | `83931a0`, `3378c8c`, `31ba69f`, `dfbfb29` | A-7と同じ一般multi-face classifier |
| B-7 `ProvenTransversalPenetration`命名 | 修正採用（改名は不採用） | 指摘どおり意味はgeneral zero-thickness penetrationである。wire reason/DTOは既に`proven_zero_thickness_penetration`と中立名へ移行済み。一方、公開Rust variant/fieldの改名はAPI互換性を壊すため行わず、歴史的識別子であることと共面正面積・非三角whole-faceも含むことをdoc commentへ固定した。 | `8286dbf`, `83931a0` | 次の破壊的API版を設ける場合だけRust識別子を中立名へ移す |

## C. 到達性・死蔵コード

| 項目 | 判定 | 根拠と確定した対応 | 主要対応commit（非網羅） | 残課題 |
|---|---|---|---|---|
| C-1 有界direct MUSの本番接続 | 採用 | [`analyze_geometric_constraint_document`](../apps/desktop/src-tauri/src/lib.rs) へ≤16件のsoundなdirect subset oracleを接続し、最小基数原因、呼出回数、未実行理由をstrict DTOと日英UIへ通した。`Unknown`をsatisfiableとは扱わない。 | `f5b94aa` | oracleの定理集合外は一般MUSではないため、EDT-009は部分実装 |
| C-2 `cycle_fold_transaction` | 修正採用 | test-only到達という指摘は妥当。公開low-level primitiveを削除・test gate化せず、open instance、runtime pose/layer、payloadとcertificateの結合を認証せずdesktop mutation authorityではないことをmodule/item契約へ固定した。 | `5ddabc6` | desktop実装との共通化は、payloadとtargetの完全なauthority結合を設計してから行う |
| C-3 `BlockComposedPathAuthorityV1` | 修正採用 | test-only callerという指摘は妥当。公開research wrapperは既発行のwhole-graph親proofを再結合するだけで、独立したclearance、layer transport、continuous motion、mutation authorityを持たないことを型の説明と回帰へ固定した。 | `a72db01` | productionで必要になるまではresearch境界のまま。一般multi-block authorityの完成根拠にしない |
| C-4 `UnsupportedConstraintKind` | 修正採用 | 公開V1 API互換の予約variantとして残す一方、production非emitを明記し、該当testは実際に返る`InvalidConstraintDocumentOrGeometry`だけを受理するよう厳格化した。 | `f127aa5`, `d5f9bb9` | 次の破壊的API版で予約variantを除去するか再評価する |
| C-5 `allow(dead_code)`の一括抑制 | 採用 | [`ori-collision/src/lib.rs`](../crates/ori-collision/src/lib.rs) のmodule全体allowを除去し、研究・test-only itemへ理由付きの狭いgate/allowを配置した。同時に共有counterへ安全に統合できる重複を整理した。 | `c68ee79` | 意図的なresearch APIは各itemの契約を維持して個別監査する |

## D. 振る舞い保存リファクタ

| 項目 | 判定 | 根拠と確定した対応 | 主要対応commit（非網羅） | 残課題 |
|---|---|---|---|---|
| D-1 衝突metering/clamp重複 | 採用 | `clamp_to_hard!`、共有`checked_work_sum`、共通counter/limit判定へ段階移行した。stage、resource名、overflow時エラー、更新前検査、one-short境界を保持する。`direct_f_affine_corridor`のlocal counterは、監査が指摘した再初期化ガード差を変えないため意図的に残した。 | `c68ee79`, `30663da`, `c71ab1b`, `0fc2116`, `c5dcd0d`, `7ff9287`, `be751ea`, `8719838`, `37949c8`, `15dcae8`, `43dc702`, `baaa1bb`, `d009ce7`, `08da880`, `971de16` | 意味の異なるlocal guardは統合しない。新規meter追加時は共有helper境界を回帰する |
| D-2 frontend分割・OCC・i18n・重複 | 修正採用（継続中） | 3クラスタを専用hookへ移し、`ProjectOccGuard`/`matchesProjectOccGuard`をclient・App・関連panelへ展開した。2D/3Dのpair選択を`advanceMeasurementPair`へ一本化し、未分割交差件数を単一`useMemo`へ統合した。App本体と多数panelの文言をtyped catalogへ移行済みだが、全frontendのカタログ化は未完了である。 | `4ca5ef3`, `c2ddd59`, `a8d6585`, `a513e15`, `35bfdc2`, `3aa2067`, `bc58e3a`, `0c842bb`, `f37ccee`, `df77772`, `7779da7`, `9df9073`, `9dd9bf1`, `f98cfb8`, `fa8daee`, `1acffae`, `f5a8dfd`, `524bdfb`, `f60699e`, `07c48aa`, `1f9ccb0`, `4bf4eb1`, `5e13e89`, `f340b9b`, `144cb1d`, `21ae6f9`, `3ec2e7c`, `1b325e7`, `604e725`, `c1b536d`, `b35cbe7`, `adbdc23`, `9591c11`, `9aeec98`, `ff6ceb6`, `b8834f0`, `f4b92dd` | 残るinline日英文言と巨大componentを、1 panel単位のDOM/型/文字列同一性回帰付きで継続移行する |
| D-3 native expectation・lock・validator | 採用 | wireの3引数を変えずbody内で`ProjectExpectation`を構築し、`execute_expected_command`と`lock_and_expect`へ集約した。lockとverifyの間にgeneration等の検査が必要な経路は監査指示どおり直接呼出しを維持する。`.ori2` writer/readerは単一`validate_project_document`を共有し、source契約testでraw迂回を監視する。 | `3135154`, `f5b94aa`, `3d37a54`, `4d37fcd`, `35f4519`, `29e6d47`, `85b2b2d`, `eeabd44`, `235d102`, `f80c923`, `59e297e`, `22ffa33` | 新規mutation command追加時にtyped expectation経路とsource契約testへ含める |

## 完成度へ反映しない残件

- EDT-009は21種（17 fixed-pattern + 4 general-graph）の直接矛盾と≤16件のsoundなdirect MUSまでであり、全制約種の一般unsat oracle・一般最小不能部分集合は未実装。
- SIM-010は限定tree/dyadic/cycle/block証拠と一部production Applyまでであり、17-face・二block経路では適用済みtimelineへ保存した層順証拠の読み取り専用viewerも存在する。一方、現在適用中の一般non-flat 3D姿勢へ結合した層順viewer、任意姿勢・任意多hinge scheduleの一般正厚continuous collision、一般共有hinge admission、一般layer transportは未完成。
- したがって、A〜Dの監査対応、テスト追加、内部リファクタを単独で完成度へ加算せず、正本79.32%（表示79.3%）と85/2/0を維持する。
