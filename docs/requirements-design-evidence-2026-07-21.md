# MUST要件・基本設計・技術検証 証拠監査

## 2026-07-30 EDT-009 semantic MUS 現行正本訂正（v4・24/24）

2026-07-30 authoritative semantic inventory correction: `DirectConstraintConflictKindV1` の24 wire-compatible variantすべてに、限定形の各原因削除後を独立exact SAT witnessで再認証する24/24 sound semantic proof familyがある。現行model IDは `geometric_constraint_deterministic_binary64_semantic_mus_v4` である。本節が以下の22/24・v3記録を上書きする現行正本であり、過去の記録自体は履歴として保持する。

- 23番目の `NonParallelFixedAngleInParallelComponent` は、異なる3実在辺に対するexact 2-hopの `Parallel` 2件、両terminal間のbit-exact `FixedAngle(90.0)`、両terminalのbit-exact `FixedLength(1.0)` 2件から成るcanonical 5-ID causeだけを対象とする。semantic MUSへの昇格はsource topologyがcommon-center starで、5件すべての単独削除に専用finite production residual-only witnessがある場合に限る。非単位長、固定長欠落、90度の非exact値、1-hop・3-hop以上・任意長pathはsolver-required `Unknown`を維持し、nonstar topologyはdirect theoremを保持しても専用semantic constructorではfail closedとする。
- 24番目の `ParallelWithFixedNonParallelAngle` は、同じ2実在辺に対する `Parallel`、bit-exact `FixedAngle(45.0)` または `FixedAngle(135.0)`、どちらか片側のbit-exact `FixedLength(1.0)` 1件から成るcanonical 3-ID causeだけを対象とする。両側にunit lengthがある場合も最小 `ConstraintId` の1件だけをcanonicalに選び、semantic MUSへの昇格はcommon-center starと3件すべての単独削除witnessを要求する。45/135度以外または45/135度の非exact値、unit長の非exact値、one-sidedなgeneric angle、nonstar topologyはfail closedとし、generic angleの既存4-ID direct boundaryは両側unit lengthを要求したまま維持する。

2026-08-02 EDT-009 bounded constructive SAT現行追補: 現在配置のexact-zero肯定とは別に、1件文書は既存の全11種singleton constructorを使い、2..=16件文書はproduction residualが参照する全頂点（明示頂点と参照辺の両端）の交差で連結成分へcanonicalに分解する。1件成分はsingleton constructor、2件成分はbit互換singleton mergeまたは既存の固定pair templateを使う。3..=16件ではbit互換singleton mergeを先行し、constructibleなordinary pair分解が一意で、残る1..=14件がpair参照集合と各exactly 1個のarticulationだけを共有し、leaf間にpair外の共有頂点がないone-core starへ4固定translation候補を追加する。実装commit `806747d9454ced96f9b77ec24fc0fea2742fed67`は4..=16件へ、record-disjointなordinary pair core 2組がcore間でexactly 1 articulationだけを共有し、残る0..=12件のsingleton leafがcore和集合と各exactly 1 articulationだけを共有するtwo-core starを追加した。全unordered record pair 120件、one-core leaf分類1,680件、unordered pair-core組7,140件、two-core leaf分類85,680件をそれぞれchecked hard ceilingとし、record重複、core間0/2以上の共有頂点、leafの複数core参照、leaf間のcore外共有、複数の完全分解、非有限translationはfail closedとする。それ以前の成分を反映したdetached候補上で段階構成し、各候補は成分外の座標bitと全topologyを変えず、成分全残差と最後の元raw文書全体のproduction residualを再認証する。2件文書の既存pair prepassも維持し、通常CreasePattern経路へresidual-only overlayを流用しない。N<=16の保守上限は138 composite preparation-or-verification passes・112 full-pattern clonesのままである。template非対応、固定候補枯渇、共有座標不一致、曖昧な分解、one/two-core star以外の非互換な連結成分は数学的にSATでも`None`となり得て、UNSATを意味しない。17件以上ではdetached構成だけを試さず、現在配置のexact certificateとDirectConflict判定は継続する。nativeは構成Some/None後に取消・deadlineを必ず再確認する。DTOは`current_assignment`と`detached_constructed_assignment`を分離し、strict TypeScript parserと日英UIは後者を現在配置の充足やproject mutation authorityとして扱わず、候補座標も公開しない。これは限定SAT存在証拠を増やすが、任意組合せの完全SAT/UNSAT、完全な一般原因、一般MUSは未完成である。

EDT-009は一般11制約種の完全SAT/UNSAT決定、完全な一般矛盾原因、一般MUS探索をまだ提供しないため部分実装のままである。MUST集計は実装済み85 / 部分実装2 / 未着手0から変更しない。次期反映headのCI発効までは数式・幾何制約85%、全体81.96%（表示82.0%）を維持し、発効条件成立後だけ数式・幾何制約86%、全体82.29%（表示82.3%）を採用する。

## 監査結果

`requirements-definition.md`のMUST 87件と`requirements-status.md`の正本表を照合した結果は、実装済み85件、部分実装2件、未着手0件である。部分実装はEDT-009とSIM-010であり、一般矛盾原因の特定、一般姿勢の複数層transport・正厚・多hinge連続経路とclosureという未完成境界を状態表に明記している。したがって初版MUST全体が完成したとは扱わない。

要件状態の「実装済み」はproduction実装、利用者経路、永続化または出力、fail-closed検証の証拠を持つ。状態表の説明を正本とし、過去の追記内にある時点集計や「未実装」は履歴であって現在値ではない。

EDT-009については冒頭のv4・24/24節とbounded constructive SAT追補だけが現在の証拠境界である。以下の「EDT-009現行証拠」を含む日付付き段落は各checkpoint当時の履歴であり、冒頭の正本を上書きしない。

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

hard-32最終extensionは`be34d05ead8211129126a0a784414caf63ce2f30`で`CommonArticulationContinuousLayerPathExtension*V2`と`CompleteMultiBlockPositiveLayer*V2`の別型・別model APIとして確定した。actual=32/configured cap=32の封印済み実fixtureは探索node 0でcompact assignmentをlive再生し、complete/finalを発行して独立binding oracleと一致させ、同一live入力を再検証する。6資源のexact値と各one-short、replay/foreign/live/partition/source/target drift、cancel/deadlineをfail closedに固定する。当該Complete V2とfinal-extension V2の各6認可predicateはすべてfalseで、legacy V1 finalの既存3 true predicateを含む挙動、2..=10 path、desktop、Apply、viewerは不変である。これはactual-32最終下位証拠だがSIM-010完成や下流認可ではないため、MUST集計85 / 2 / 0と全体81.96%（表示82.0%）は不変である。

N=32算定は、9 face / 12 hingeの3×3 Miura blockを31個の共有material faceで連鎖するため、parent `F=257`、`H=384`となる。`C(257,2)=32,896`はparent pair上限、`81×C(32,2)=40,176`はclearance raw候補、そこから同一block内の`32×C(9,2)`を除いた`31,744`がcanonical cross-block registryである。`CommonArticulationResourceProfileV2`のfactor-8上限制式はpose `18,072 logical / 56,304 bytes`、clearance `9,154,601 logical / 2,347,648 bytes`（32-byte pair record）を与え、actual-32 final V2の実発行・再検証・exact/one-short試験で照合済みである。一般N compact V2 issuerの`N>=33`境界、動的whole-parent positive、Certified continuous motion、collision clearance、layer transport、project mutation、Apply、viewerの認可根拠にはしない。

`b7a4bc72`はglobal flat-foldabilityの論理transitivityをcell単位のcompact family、checkpoint付きstream、maintained transitive closure、rollback可能探索として実装した。当該commit単独では手動N=32 fixture-only受入だったが、`be34d05ead8211129126a0a784414caf63ce2f30`は32,896変数・4,112 byteのassignmentとregistry/assignment SHA-256を通常回帰fixtureへ封印し、actual-32 final V2から探索node 0でlive再生する。temporary extractorとdiagnostic harnessは削除済みである。この証拠を一般任意topology、動的正厚または下流認可へ流用しない。

`7e2e1247` / `bd3e9940`は、actual N=33およびN=34/configured cap=40のcanonical 3×3 Miura連鎖に対し、V2 profile、iterative decomposition、common pose、all-block closure、whole-parent dyadic closure、完全cross-block registryの発行・live再検証境界を追加した。既存clearance prerequisiteは現在も`Unpromoted`だが、`be34d05ead8211129126a0a784414caf63ce2f30`は別のprofile-bound stationary whole-parent positive outcomeを追加し、canonical N=33のexact live source/profile/transport/parent-admission/limits、bit-identical +0 pose、正有限紙厚、全pair evidenceが揃う場合だけ`Proven`を発行する。`PairEvidenceUnavailable`だけを`Unpromoted`へ写し、他のmalformed/resource/stop failureはerrorに保つ。当該stationary certificate/outcomeの各6認可predicateはすべてfalseなので、非静止continuous motion、一般任意topology、collision clearance、layer transport、project mutation、Apply、viewerやSIM-010完成の証拠には数えず、MUST集計85 / 2 / 0と全体81.96%（表示82.0%）を維持する。

一般N非静止閉包のPhase 2は、N=33 ordinary非静止0.5°→1.5°について全33 blockとparentのclosureを、exact parallel-cut認識とowned restricted-schedule/closure bundleで構成するcrate-private証跡である。restrictionの物理capacity・BigInt payload、bundle retained/issuance/revalidation peakをcheckedに計上し、exact/one-short、live instance再検証、cancel/deadlineを確認する。bundleは公開API・wire format・V1変換・永続化・下流認可を持たず、continuous motion、collision clearance、layer transport、project mutation、Apply、viewerはすべて未認可である。WSLローカルで`ori-kinematics` unit 340件、integration 4件と23件を含むall-target 367件が全成功した。remote CI・artifact・commit SHAの正本は引き続きremote CIであり、ローカル結果からは主張しないため、SIM-010完成、MUST集計、現行81.96%（表示82.0%）、82.29%（表示82.3%）候補の発効条件は変更しない。

続くPhase3Aでは、既存static/V1 APIを変えず、crate-private bundleを所有するadditive public `CommonArticulationDynamicClosureBridgeV2`を追加した。公開はbinding、actual N、retained/issuance/revalidation peakのscalarだけで、resource policyはsealed、revalidationは同一live inputの再発行・照合を行う。bridgeはcharged bundleと同じsize/alignmentの`#[repr(transparent)]` one-field wrapperであり、`Debug`のみでClone/serde/V1変換/Deref/leaf・schedule・closure・authority accessor/authorizationを持たない。N=33非静止、全live inputのforeign差し替え、全public policy field、resource exact/one-short、cancel/deadlineとstop precedenceを確認し、all-target 367件、doctest 32件、Clippy `-D warnings`、format、diff-checkは成功した。これはcollision clearance等の下流認可へ未接続であり、SIM-010、MUST集計、全体81.96%（表示82.0%）、82.29%（表示82.3%）候補のremote CI発効条件を変更しない。
