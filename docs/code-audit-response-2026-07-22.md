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
| B-1 | 指摘の主旨は正しい・再計上要 | bounded template群と一般tree生成を「任意目標形状の自動設計」と同一視しない。完成率監査で実スコープへ限定する。 |
| B-2 | 正しい・再計上要 | `Indeterminate`を安全証明へ数えない。正厚・一般多面の未証明領域をSIM-010の完了率へ反映する。 |
| B-3 | 正しい・再計上要 | 直接矛盾7種とsolverによる一般充足可能性を区別し、EDT-009の表示を実証拠へ限定する。 |
| B-4 | 正しい・文書統合要 | `requirements-status.md`の複数集計値を単一の機械的集計へ統合する必要がある。 |
| B-5 | 正しい・利用者経路の証拠確認中 | layer-order版診断のcurrent-pose/stacked-fold read経路への配線をfocused UI/native testで確定する。 |
| B-6 | 既修正 | wire/DTO/UIは既に`proven_zero_thickness_penetration`と「ゼロ厚み面貫通・重なり」へ一般化済み。Rust variant名だけは公開API互換のため意図的に維持（coverage docsにも明記）。 |
| B-7 | 正しい・修正確認中 | `kinematics.reason`に基づくcycle/cut分離表示を要求し、固定cycle文言を認めない。 |

## 改善・機能欠落（C）

| ID | 判定 | 対応方針 |
|---|---|---|
| C-1 | 正しい改善 | App stateと機能panelを挙動不変で段階分割する。単一巨大変更は回帰範囲が広いため独立checkpoint化する。 |
| C-2 | 要追加証拠 | TS previewとRust authorityの責務は異なる。二重の「肯定authority」が存在する箇所だけを抽出し、preview近似自体は誤検知として区別する。 |
| C-3 | 正しい機能欠落 | 閉路Graphの対話poseは専用認証経路へ統合する必要がある。静的tree updaterへ流用しない。 |
| C-4 | 改善提案 | OCC fencingは安全境界なので削除対象ではない。引数newtype/constructor集約のみを検討する。 |
| C-5 | 正しい改善 | locale key catalogへの段階移行対象。機能不具合とは別checkpointにする。 |
| C-6 | 一部誤検知 | core moduleは回帰testと公開primitiveを持つため単純な死蔵ではない。desktop重複経路の有無だけを継続確認する。 |
| C-7 | 要追加証拠 | `#[allow(dead_code)]`だけでは死蔵と断定しない。production call graphと研究fixtureを分類する。 |
| C-8 | 複合指摘 | 未到達error、早期return、Windows registry ACLを別項目として個別検証する。 |
| C-9 | 正しい機能欠落 | 数値角度＋対象交点までの延長をnative atomic commandとして追加する必要がある。既存の長さ＋角度commandとは別機能。 |

## 検証ゲート

- 各修正はfocused test、関連crate全test、`rustfmt`、`git diff --check`を通す。
- 長時間matrixはtool timeoutをテスト失敗と混同せず、隔離`CARGO_TARGET_DIR`でterminal結果を取得する。
- B-1〜B-4を解消するまで、pending CIの90.35%案を権威ある完成度へ昇格しない。
- 未確定項目は「修正済み」と扱わず、コード経路またはfault-injection evidenceが揃うまで本表に残す。
