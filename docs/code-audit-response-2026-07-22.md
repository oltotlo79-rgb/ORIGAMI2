# Claudeコード監査 精査・対応表（2026-07-22）

対象: `docs/plans/code-audit-2026-07-22.md` の全26項目。分類は、監査文面だけでなく現行コード、履歴、focused testを照合した結果である。

## 不具合（A）

| ID | 判定 | 対応・根拠 |
|---|---|---|
| A-1 | 正しい・修正済み | `a0b4208`。閉路Graph姿勢を静的mesh sourceへ変換する前にfail-closed。欠落texture参照の`expect`も除去。panic後のmutex非poison回帰 1/1 green。 |
| A-2 | 正しい・修正済み | `2a72717`。FixedAngle/AngleBisectorの全辺を宣言vertexから外向きに評価。逆向き格納辺の90度回帰 1/1 green。 |
| A-3 | 正しい・修正済み | `4b6e348`。legacy shared-flat昇格を`CoplanarAreaOverlap`だけに限定し、`TransversalCrossing`を除外。focused 1/1 green。 |
| A-4 | 正しい・修正済み | `4b6e348`。`-0.0`を分類前に`InvalidPaperThickness`へ拒否。NaN/無限/負値を含む境界回帰 1/1 green。 |
| A-5 | 正しい・修正済み | `28e1717`。expanded-folderがlayer evidenceを表現できない間はwriter入口で専用errorにfail-closed。無音破棄回帰 1/1 green。 |
| A-6 | 正しい・修正済み | `ebd00ef`。親entry全走査とtransaction namespace探索を分離し、recovery prefixだけをbounded列挙。4098件の無関係entryがある実filesystem replacement回帰 1/1 green。 |
| A-7 | 正しい・修正済み | `9a5fb2f`。project JSON writer/reader共通で全crease vertex座標の有限性を検証。NaN/±Inf回帰 3/3 green。 |
| A-8 | 正しい・修正済み | `1a6c9fc`。各更新直後に残差を再評価し、最終許容反復の収束を成功として返す。focused 1/1 green。 |
| A-9 | 正しい・修正済み | `1a6c9fc`。全driver満足系はeffective solved rankを返し、UIのover-constrained誤分類を防止。focused 1/1 green。 |
| A-10 | 正しい・修正済み | `dbaf02f`。新規folder rename後のdirectory sync失敗を`RecoveryRequired`として伝播し、既公開targetを保持。fault-injection回帰 1/1 green。 |

`ori-core`はA-2/A-8/A-9適用後にlib全299/299 green。

## 矛盾・誤表示（B）

| ID | 判定 | 対応・根拠 |
|---|---|---|
| B-1 | 正しい・再計上済み | bounded template/custom treeを一般目標設計と同一視せず、pending案の初心者領域を60%へ限定。 |
| B-2 | 正しい・再計上済み | `Indeterminate`を安全証明へ数えず、一般正厚・一般多面の未証明を反映してpending案の3D領域を75%へ補正。 |
| B-3 | 正しい・再計上済み | 11種solverと限定的な直接矛盾certificateを区別し、pending案の制約領域を85%へ補正。 |
| B-4 | 正しい・修正済み | `requirements-status.md`の旧57/25/5説明を非表示の履歴へ移し、表示上の現在値を87行から機械集計した86/1/0へ一本化。 |
| B-5 | 正しい・修正済み | `d158110`。180度stacked endpointで同じtarget topology/model/poseへglobal layer orderをanchorし、layer-order版collision診断の結果を利用者DTOへ接続。desktop lib check green。 |
| B-6 | 既修正 | wire/DTO/UIは既に`proven_zero_thickness_penetration`と「ゼロ厚み面貫通・重なり」へ一般化済み。Rust variant名だけは公開API互換のため意図的に維持（coverage docsにも明記）。 |
| B-7 | 正しい・修正済み | `133606c`。`kinematics.reason`に基づきcut componentsとcycleをvisible note/accessibility descriptionの両方で分岐。source integration 1/1とTypeScript build green。 |

## 改善・機能欠落（C）

| ID | 判定 | 対応方針 |
|---|---|---|
| C-1 | 改善提案・不具合ではない | App stateとpanel分割は保守性改善だが、行数やhook数は到達可能な機能欠陥の証拠ではない。挙動不変refactorを完成率の必須条件にはしない。 |
| C-2 | 一部誤検知 | TS側は操作中preview、Rust側はapplyを許可するauthorityで責務が異なる。TS結果だけでproject変更を認可する経路はなく、二重の肯定authorityではない。相互整合fixtureは継続する。 |
| C-3 | 正しい・修正済み | 閉路Graphの認証preview/apply自体は`StackedFoldPanel`に既存。`157d198`で静的3D表示から同パネルへ日英・accessibility共通で明示案内し、無効dragを正規経路と誤認させない。integration 1/1とTypeScript build green。 |
| C-4 | 改善提案・現状維持 | OCC fencingとimmutable DTOはABA/改ざん防止の安全境界。重複量だけを根拠に削除すると保証を弱めるため、機能不具合としては不採用。 |
| C-5 | 改善提案・不具合ではない | locale key catalog化は翻訳運用の改善。現行`foldPreviewText`等はja/enを同時保持しlocale test対象で、未翻訳による到達可能な欠陥とは別。 |
| C-6 | 誤検知 | core moduleは公開`prepare/apply_cycle_fold_transaction_v1`とatomicity/ABA/retry回帰を持つ独立primitiveであり死蔵ではない。desktopが別の認証済みgraph transactionを持つことはcore APIを未到達にしない。 |
| C-7 | 正しい・修正済み | `ceff2e3`。production参照0、test参照のみを確認した`direct_f_affine_corridor` moduleを`#[cfg(test)]`化。production check green、関連10/10 green。 |
| C-8 | 複合指摘・根拠付き現状維持 | `UnsupportedConstraintKind`は公開fail-closed API互換、`execute_command`早期枝はstale/revision-exhausted時にeditor固有errorを保存するguard。Windows registryは秘密情報を格納せず、絶対path・hash・識別子も回復のためのlocal metadataである。削除/ACL変更を裏付ける実害証拠なし。 |
| C-9 | 正しい追加要望・要件外 | 現行EDT-003は仕様どおり「始点＋長さ＋角度」のatomic `add_connected_vertex`を実装済み。監査提案の「対象との交点まで」は別の新規UX要件であり、既存要件の未実装や不具合ではない。追加時はray/segment exact交差、target split、historyを単一core commandにする。 |

## 検証ゲート

- 各修正はfocused test、関連crate全test、`rustfmt`、`git diff --check`を通す。
- 長時間matrixはtool timeoutをテスト失敗と混同せず、隔離`CARGO_TARGET_DIR`でterminal結果を取得する。
- B-1〜B-3の過大計上を補正したpending案は83.96%（表示84.0%）。同一headの全必須CI greenまでは正本79.3%を変更しない。
- 未確定項目は「修正済み」と扱わず、コード経路またはfault-injection evidenceが揃うまで本表に残す。
