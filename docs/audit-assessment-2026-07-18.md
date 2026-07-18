# 2026-07-18 第三者監査の評価

## 対象

Claude監査レポート`ORIGAMI2-audit-report-2026-07-18.md`（commit `013ba08`と当時の作業ツリー）を、現在のリポジトリへ再照合した。外部原文は23,580 bytes、SHA-256 `90064740B22FB0548024803E38BCA11B40B8E13E279A3281A6E2AE4BC6A5609E`で、共有元は`https://claude.ai/code/artifact/873b2c96-fddb-4630-8c75-61138340a951`である。監査本文の自己申告値37.6%、`013ba08`の進捗文書38.78%、後続作業時38.96%は並行作業中の異なる時点である。監査後に増えた補正解析はまだUI未接続であるため、利用者向け機能完成率は増えていない。

## 採用する指摘

- 旧完成率38.96%は、3D衝突向け数値研究、QA、UI未接続基盤を利用可能な機能へ厚く計上していた。監査の領域概算を既存重みへ入れ直し、数式・幾何制約をUI基準の0%へ補正した26.44%を現在の追跡値とする。この値はMUST 86件の件数比ではなく、初心者向けFUTURE 14件、品質検証、Windows/macOS配布まで含む全製品ビジョンに対する暫定の重み付き概算である。
- 数式・幾何制約はEDT-004/005/008/009を基準にするとほぼ未着手であり、3D衝突の数値計算をこの領域へ計上しない。
- 補正候補pipelineは新段階を増やす前にUIへ接続し、作業中・候補なし・判定不能・認定済みを利用者へ提示する。
- `FoldPreview.tsx`の約3,022行の主`useEffect`、9,225行の`ori-core/editor.rs`、4,711行のTauri `lib.rs`、文字列IPC error、手動列挙のfrontend test scriptは保守リスクである。大型機能の前に段階分割する。
- 予期しない例外をすべて無表示の`null`や`indeterminate`へ落とすだけでは診断不能になる。作品内容・パスを標準収集しないredacted diagnostics境界を追加し、空catchを段階的に減らす。
- Rustの`Editor::undo`/`redo`は履歴entryを`pop`した後で適用しており、適用失敗時にentryを失う経路がある。pattern、revision、両履歴stackを失敗時にも保つ原子的処理へ修正し、再試行可能性を回帰試験する。
- 折り手順、SVG/FOLD/PDF、履歴永続化・復旧、i18n、単位、レイヤーなど未着手MUSTを要件ID別に追跡し、UIから利用可能になった時点で機能完成率へ加算する。
- macOSは自動CIと`.app`生成だけを継続し、実機Macを必要とするE2E項目は所有者の指示どおり現在の作業範囲から除外する。

## 修正して採用する指摘

### VAL-004

一般の剛体折り可能性・経路問題にNP困難以上の結果があることは、全入力へ高速な汎用solverを約束できない根拠になる。ただしNP困難は「実装不能」や「厳密検証を全面中止」と同義ではない。現在の1ヒンジCCDは対応範囲内で安全区間を証明し、`clear / blocked / indeterminate`を返す限定的な厳密検証として維持する。

VAL-004を「反例検出だけ」へ正式変更すること、VAL-003を特定方式へ固定することは要件変更なので、所有者の明示決定なしには行わない。実装上は対応クラス、作業上限、判定不能を明示し、一般問題を解けたと表示しない。

### TypeScriptの信頼境界

自プロセス内の全DTOをhostile Proxyとして多段再検証する設計は過剰で、行数とテスト負担を増やしている。一方、Tauri IPCだけを唯一の境界としてprivate authority、revision、generation、request、現在scene poseの照合まで撤去すると、非同期stale結果や解析結果の誤適用を防げない。

外部入力は入口で一度検証し、内部の一般DTOでは敵対入力対策を増やさない。exact-object authorityはscene副作用、motion owner、committed terminal lease、certificate適用境界だけへ限定する。既存防御はUI接続・runtime分割と同時に共通化し、安全停止と原子的適用の回帰を保ったまま縮小する。

### `Command::changes`

監査どおり、現Tauri層は`CommandResult`を捨てて全`ProjectSnapshot`を返しているため、`Command::changes`/`Inverse::changes`の計算結果は製品UIで未使用である。ただし将来の差分更新に使う選択肢はある。即時削除ではなく、IPCを差分応答へ移すか全snapshotを継続するか決めたうえで、未使用のままなら削除する。

## そのまま採用しない数値・表現

- `App.tsx`の直接`useState`は現在25呼出しであり、importを含む文字列出現は26件である。監査の構造上の指摘は妥当で、2,208行のファイル分割を進める。
- frontend testは監査時664件、現在673件である。これは品質件数であり、未接続機能の完成率には加算しない。
- 監査本文の行別判定を合計すると「実装済み20 / 部分実装23 / 未着手43」であり、本文集計の「21 / 23 / 42」と1件ずれる。`requirements-status.md`の86行を正本とする。
- 「研究的に不可能」という表現は強すぎる。最悪時計算量、実用入力、対応クラス、timeout/作業上限を分けて仕様化する。

## 反映順

1. 改訂済みのTypeScript信頼境界に従って補正解析authority chainの追補を完了する。以後は新しい汎用hostile DTO防御を増やさず、既存pipelineをUIへ接続する。
2. MUST 86件の実装済み・部分・未着手表をリポジトリ内で維持する。
3. UI接続後、`FoldPreview` runtime分割と同時に既存の重複防御を信頼境界へ寄せ、redacted diagnostics、test自動検出も小さなcheckpointへ分ける。
4. VAL-002の局所平坦折り条件と、折り手順・入出力・復旧・i18n等の未着手MUSTをbreadth-firstで進める。
5. VAL-003/004の正式な要件変更が必要な箇所は所有者判断を得る。

## 監査後の対応状況

この節は監査時点の判断を置き換えるものではない。上記の「UI未接続」と26.44%は監査再照合時点の歴史記録として残し、その後に実装・確認した差分だけを追記する。

- 単一ヒンジ補正候補の解析request、静的候補、候補別連続経路、切り離した表示DTOを複合jobへ接続し、RAF単位で進めるcoordinatorを`FoldPreview`へ統合した。UIは作業中、対応範囲内での候補なし、判定不能、認定済みを区別し、新request・姿勢・選択・固定面・紙厚の変更では旧結果をstaleとして無効化する。
- 認定済み表示も解析専用であり、`sceneApplied: false`・`autoApplicable: false`を維持する。候補3Dプレビュー、明示適用、一般の複数ヒンジ・閉路・切断由来経路は未実装で、`no_candidate`を作品の折り不可能性とは表示しない。
- terminalのstart角へ解析contextをrebaseする際、元の真正contextが持つexact model・tree・非選択角を保持するよう修正した。これにより、初回と同内容でも別model snapshotを発行してしまう2回目以降のrequestがterminal bindingのmodel provenanceを失う経路を閉じた。
- frontend testの手動列挙を引用符付きglobへ置換し、新規`*.test.ts`をNode 24 CIとWindowsで自動検出するよう変更した。さらに`FoldPreview`のscene・camera・renderer・照明・grid・紙/輪郭材質をReact非依存runtimeへ分離し、authorityを持つmotion・gesture・原子的scene適用はコンポーネント側へ残した。
- 監査の「空catchを全件`reportUnexpected(scope, error)`へ置換」は条件付き採用とした。catch件数は抽出方法と対象commitで変わり、現行の多くはキャンセル、stale、入力拒否、作業上限、判定不能、best-effort cleanupを明示的に処理している。これらを一括記録すると診断飽和と性能低下を招き、raw errorを受け取るAPIはパスや作品内容を混入させ得るためである。
- 代わりに`reportUnexpected(scope)`だけを持つ純粋なメモリ内境界を追加した。固定15 scope、65件飽和、6段階bucket、固定順・8 KiB以下のsnapshotに限定し、生の例外、任意context、作品情報、パス、ID、座標、時刻、環境情報、通信・保存機能を持たせていない。利用者影響のあるApp/FoldPreview上位境界とpayloadを無視するglobal handlerだけへ接続し、入力/編集拒否、権限不足、破損ファイル等の想定内失敗は数えない。専用10件で許可外scope、hostile object、秘密値非混入、接続scopeを固定した。
- 折り可能性・経路探索の領域進捗を8%から10%、全体への寄与を1.44%から1.80%へ更新した。監査再照合時の26.44%へ0.36ポイントを加え、現在の追跡値は26.80%（表示26.8%）である。MUST 86件の集計は実装済み20・部分実装23・未着手43のままで、VAL-008も部分実装を維持する。
- 現時点の自動回帰はfrontend 710件、Rust 304件である。件数と内部診断基盤は品質確認であり、それ自体を機能完成率へ加算していない。OPS-004〜006と全体完成率26.80%は変更しない。
