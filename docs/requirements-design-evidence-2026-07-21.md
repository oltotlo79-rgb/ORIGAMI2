# MUST要件・基本設計・技術検証 証拠監査

## 監査結果

`requirements-definition.md`のMUST 87件と`requirements-status.md`の正本表を照合した結果は、実装済み85件、部分実装2件、未着手0件である。部分実装はEDT-009とSIM-010であり、一般矛盾原因の特定、一般姿勢の複数層transport・正厚・多hinge連続経路とclosureという未完成境界を状態表に明記している。したがって初版MUST全体が完成したとは扱わない。

要件状態の「実装済み」はproduction実装、利用者経路、永続化または出力、fail-closed検証の証拠を持つ。状態表の説明を正本とし、過去の追記内にある時点集計や「未実装」は履歴であって現在値ではない。

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
