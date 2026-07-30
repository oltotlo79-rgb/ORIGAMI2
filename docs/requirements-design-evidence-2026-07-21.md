# MUST要件・基本設計・技術検証 証拠監査

## 2026-07-30 EDT-009 semantic MUS 現行正本訂正（v4・24/24）

2026-07-30 authoritative semantic inventory correction: `DirectConstraintConflictKindV1` の24 wire-compatible variantすべてに、限定形の各原因削除後を独立exact SAT witnessで再認証する24/24 sound semantic proof familyがある。現行model IDは `geometric_constraint_deterministic_binary64_semantic_mus_v4` である。本節が以下の22/24・v3記録を上書きする現行正本であり、過去の記録自体は履歴として保持する。

- 23番目の `NonParallelFixedAngleInParallelComponent` は、異なる3実在辺に対するexact 2-hopの `Parallel` 2件、両terminal間のbit-exact `FixedAngle(90.0)`、両terminalのbit-exact `FixedLength(1.0)` 2件から成るcanonical 5-ID causeだけを対象とする。semantic MUSへの昇格はsource topologyがcommon-center starで、5件すべての単独削除に専用finite production residual-only witnessがある場合に限る。非単位長、固定長欠落、90度の非exact値、1-hop・3-hop以上・任意長pathはsolver-required `Unknown`を維持し、nonstar topologyはdirect theoremを保持しても専用semantic constructorではfail closedとする。
- 24番目の `ParallelWithFixedNonParallelAngle` は、同じ2実在辺に対する `Parallel`、bit-exact `FixedAngle(45.0)` または `FixedAngle(135.0)`、どちらか片側のbit-exact `FixedLength(1.0)` 1件から成るcanonical 3-ID causeだけを対象とする。両側にunit lengthがある場合も最小 `ConstraintId` の1件だけをcanonicalに選び、semantic MUSへの昇格はcommon-center starと3件すべての単独削除witnessを要求する。45/135度以外または45/135度の非exact値、unit長の非exact値、one-sidedなgeneric angle、nonstar topologyはfail closedとし、generic angleの既存4-ID direct boundaryは両側unit lengthを要求したまま維持する。

2026-07-30 EDT-009 bounded constructive SAT現行追補: 現在配置のexact-zero肯定とは別に、全11種のsingleton制約をproduction exact certificateで個別に構成し、共有頂点の割当座標がbit一致する1..=16件だけをmergeして文書全体のproduction residualで再認証する。2件文書では、固定pair templateの候補を先に完全残差再認証し、非対応時だけsingleton mergeへfail-closedに戻る。native DTOは`current_assignment`と`detached_constructed_assignment`を別のclosed evidence kindとして返し、strict TypeScript parserは未知値をfail-closedする。日英UIは後者を現在配置の充足と表示せず、別の厳密配置を構成・再認証した非mutation証拠として表示する。候補座標はDTOへ公開せず、DirectConflict優先、17件以上、共有座標不一致、resource、取消、deadlineでは肯定しない。これは限定SAT存在証拠を増やすが、任意組合せの完全SAT/UNSAT、完全な一般原因、一般MUSは未完成である。

EDT-009は一般11制約種の完全SAT/UNSAT決定、完全な一般矛盾原因、一般MUS探索をまだ提供しないため部分実装のままである。MUST集計は実装済み85 / 部分実装2 / 未着手0から変更しない。次期反映headのCI発効までは数式・幾何制約85%、全体81.96%（表示82.0%）を維持し、発効条件成立後だけ数式・幾何制約86%、全体82.29%（表示82.3%）を採用する。

## 監査結果

`requirements-definition.md`のMUST 87件と`requirements-status.md`の正本表を照合した結果は、実装済み85件、部分実装2件、未着手0件である。部分実装はEDT-009とSIM-010であり、一般矛盾原因の特定、一般姿勢の複数層transport・正厚・多hinge連続経路とclosureという未完成境界を状態表に明記している。したがって初版MUST全体が完成したとは扱わない。

要件状態の「実装済み」はproduction実装、利用者経路、永続化または出力、fail-closed検証の証拠を持つ。状態表の説明を正本とし、過去の追記内にある時点集計や「未実装」は履歴であって現在値ではない。

2026-07-30 EDT-009現行証拠: legacy 21 wire variant、有界binary64 exact-zero closureの2 tag、二固定root比率domainの`InconsistentLengthRatioGraphBetweenFixedLengths`を合わせ、`DirectConstraintConflictKindV1`は24 wire-compatible variantとなった。二固定root familyは、consistentな正有限固定長と比率だけを使い、順方向をproduction binary64乗算、逆方向を除算なしの完全な非負有限binary64乗算preimageとして伝播し、同じexact edgeの保守domainが厳密に分離する場合だけ肯定する。原因は固定長2件と比率2〜254件の最大256 IDで、原因subgraphのproduction向き再生が非有限になる場合、domainが丸めaliasで重なる場合、resource・取消・deadlineが完了しない場合は肯定しない。exact 90/270度の`RotationalSymmetryWithCollinearRadius`は、center→sourceの実在する有向辺と、その同じ辺上へtargetを置く`PointOnLine`の厳密な2-ID原因だけを所有し、各原因削除後に独立exact SAT証人を再構成する。さらに`PerpendicularOrientationsInParallelComponent`のうち、異なる3実在辺`e0`・`e1`・`e2`に対する`Horizontal(e0)`、`Parallel(e0,e1)`、`Parallel(e1,e2)`、`Vertical(e2)`、bit-exactな`FixedLength(e2, 1.0)`から成るcanonical 5-IDのunit-terminal two-hop coreだけをhard semantic inventoryへ追加した。semantic MUSは3辺がsource topologyでcommon-center starを成し、5削除を専用の完全finite production residual-only overlayで独立exact SAT再認証できる場合に限定する。非単位終端、固定長欠落、3-hop以上または任意長のparallel pathはsolver-required `Unknown`を維持し、nonstar topologyはdirect theoremを保持しても専用semantic constructorでは肯定しない。hard semantic inventoryは22/24 familyで、model IDは`geometric_constraint_deterministic_binary64_semantic_mus_v3`である。残る`NonParallelFixedAngleInParallelComponent`と`ParallelWithFixedNonParallelAngle`の2 familyはcanonical unchecked IDを保持してblocking `Unknown`へfail-closedとし、project mutationを認可しない。全11種の一般充足可能性、完全な一般矛盾原因、一般semantic MUSは未完成であるため、EDT-009の部分実装、MUST集計85 / 2 / 0、数式・幾何制約85%、全体81.96%（表示82.0%）を変更しない。

2026-07-26 EDT-009履歴証拠: legacy 21 wire variantを維持したまま、有界binary64 exact-zero closureの`PositiveFixedLengthInBoundedZeroLengthClosure`と`ZeroLengthClosureReachesNondegenerateProvider`を追加し、`DirectConstraintConflictKindV1`は合計23 variant、実残差で肯定できるsound familyは9種、legacy fail-closedは14種となった。closureは10/11制約種を横断できるが256制約の独立上限内に限り、subset oracleは16制約以下の最小基数proof coreに限る。全11種の一般充足可能性、完全な一般矛盾原因、semantic MUSは未完成であるため、EDT-009の部分実装、MUST集計85 / 2 / 0、全体81.96%（表示82.0%）を変更しない。

## INS-007設計証拠の補完

自動記録は`apps/desktop/src/App.tsx`の明示toggleと安定姿勢観測から、通常のinstruction step追加commandへ接続する。分割・結合は`InstructionTimelinePanel.tsx`からnative IPCを経て、`ori-core`の`RewriteInstructionTimelineSplitMerge`を一度だけ実行する。

coreは次の不変条件を再認証する。

- 分割は厳密に一手順から隣接二手順、結合はその逆だけを許可する。
- 周辺timelineと先頭IDを維持し、追加IDは全timelineで一意とする。
- poseとmetadataは同一のまま、時間だけを正値として分配し、合計時間を維持する。
- 同じcommandを逆操作として使い、Undo/Redoを原子的にする。
- version固定のhistory codecで通常保存と復旧checkpointへ永続化し、改変・非有限・非対称な書換えを拒否する。

production証拠は`crates/ori-core/src/editor.rs`、`crates/ori-core/src/editor/history_persistence.rs`、`apps/desktop/src-tauri/src/lib.rs`、`apps/desktop/src/lib/coreClient.ts`、`apps/desktop/src/components/InstructionTimelinePanel.tsx`にある。`apps/desktop/tests/instructionRequirementsCoverage.test.ts`がINS-001〜010の縦断接続を、`projectMutationInstanceIntegration.test.ts`がsplit/mergeを含む全revision変更IPCのproject instance束縛を固定する。

## 技術検証境界

desktop Node統合試験1603件、`ori-core` unit試験292件とdoc test 6件、desktop Rust check、frontend production buildを通過した。これらはINS-007とINS-001〜010の回帰証拠であり、SIM-010の未証明範囲を完成へ昇格させる証拠には使用しない。
