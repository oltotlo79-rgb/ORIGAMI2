# 初心者向けcustom target実装監査（2026-07-22）

## 監査範囲と判定方法

`docs/requirements-definition.md` の AUT-001〜007、AUT-101〜107を、2026-07-22以降に追加されたcustom target、画像・GLB参照、general tree、複数参照consensusのproduction code、利用者経路、保存・出力境界、回帰試験へ照合した。内部APIまたはfixtureだけの成果は利用者経路の完成として数えず、物理運動を証明しないread-only proposalは自動生成の完全実装として数えない。

本書は監査証拠であり完成率の正本ではない。`docs/progress.md` はCI全緑が確認されるまで変更しない。

## 要件との照合

| 要件 | 2026-07-22時点の証拠 | 保守的判定 |
|---|---|---|
| AUT-001〜003 | target category、部品、寸法付き棒状骨格、custom object名称・輪郭・一般木入力をprofile historyとUIへ保存 | 実装済み |
| AUT-004 | project underlay画像と複数GLBをnative asset registryから選択。consensusは2〜4件、WebViewからhash/bytes/pathを受け取らない | 実装済み |
| AUT-005 | marker/silhouette、閾値・極性・crop・回転・反転、複数輪郭、split/merge、部位割当、GLB成分・範囲・landmarkを利用者が修正可能。一般画像の意味認識は限定的 | 部分実装 |
| AUT-006〜007 | 突起属性、3D範囲、膨らみ方向・量、fold-model fingerprint bindingをprofile/historyへ保存 | 実装済み |
| AUT-101 | 動物・昆虫template、非対称landmark、custom general tree候補、展開図・説明timelineのread-only previewと確認applyを実装。任意形状からの一般物理motion生成は未完 | 部分実装 |
| AUT-102〜107 | 端末内処理、評価重み・preset・制約、推奨1件と追加候補、弾性非計算方針、global/local gateを実装 | 実装済み（AUT-101の一般性を除く） |

## 利用者経路

1. 画像下絵またはGLBをprojectへnative importする。raw bytesとfilesystem pathはWebViewへ渡さない。
2. 輪郭認識設定、部品、骨格、突起、3D範囲を修正し、同じ`BeginnerDesignProfileV1`履歴commandでUndo/Redo可能に保存する。
3. project内の画像・GLBから2〜4件をcheckboxで選ぶ。5件目はUIとnativeの双方で拒否し、nativeがUUID byte順へ正規化してasset bytesからSHA-256を再取得する（`bdbf698`）。
4. nativeが最大4 assetをdecodeし、最大6 pairについてcomponent数、正規化extent、branch数を比較する。1件の外れ値だけを利用者が明示除外でき、2 pair以上の不一致はapply不可とする（`9aaa9e4`、`d5e41a8`）。
5. 比較結果は最大6行のARIA tableとして安全な「Reference N」だけで表示し、行選択はread-only component highlightに限定する（`189e6dc`）。
6. 解析はgeneration-scoped single-flightで、各asset decode後と各pair後に取消checkpointを持つ。進捗はasset最大4、pair最大6に制限し、revision/profile/selection変更後のevent・応答を破棄する（`d807352`）。
7. confirmed candidate apply時にinstance/project/revision/profile、全asset ID/hash、pair結果をnativeで再検証する。不一致、取消、stale、tamperではproject mutationを行わない。

## strict境界と保存・出力

- Domain: `BeginnerReferenceConsensusV1`はschema version 1、binding 2〜4、asset ID一意、quality 0〜100、除外0〜1を強制する（`68e192a`）。
- IPC: TypeScriptはexact-key、canonical non-nil UUID、32-byte hash、score/count上限を検証する。選択commandの入力にhash、quality、path、bytesを含めない。
- Native: underlayに現存する画像またはproject GLBだけを解決し、lock下でrevisionとcontent hashを再確認する。canonical order、最大6 pair、取消generationをnative authorityとする。
- Save/recovery: profile、選択binding、明示除外、generation provenanceは通常`.ori2`、expanded folder、recovery、Undo/Redoで同じserde/domain validationを通る。
- Provenance: confirmed applyはsource revision、全binding、除外、pair digest、安全な集約summaryを保存する。一般木はtree topology hash、asset content binding、generator version、`authorizes_apply`/physical proof境界を保持する。
- Export: crease PDF/SVG/FOLD/DXFとinstruction PDF/SVG ZIPは保存済みprovenance全体を検証してから、version/model、source/excluded count、agreement、component/extent/branch subscoreだけを出力する。asset名・ID・hash・path・raw reasonは除去する（`b10bd5b`、`61806e1`）。legacy provenanceはsummaryなしで互換。

## テスト証拠

| 境界 | 主な回帰 |
|---|---|
| Domain/serde | consensus件数・重複・除外先・quality、provenance summary/model/subscore、legacy default |
| Native | asset liveness/hash再取得、canonical順、最大6 pair、複数不一致apply拒否、generation-scoped cancel、stale/no mutation |
| Strict client | exact response keys、UUID/hash/count/score/digest bounds、unknown/tampered response拒否 |
| UI/DOM | 2〜4 checkbox、5件目disabled、busy/progress/cancel、ARIA table/selection、snapshot変更時破棄、listen失敗fail-close |
| Save/output | ori2/folder/recovery/UndoRedo、PDF/SVG/FOLD/DXF provenance再読込、instruction PDF/SVG ZIP安全summary、legacy summaryなし |

直近の検証ではfrontend snapshot 1,650件、DOM 49 files / 328件、TypeScript build、WSL `cargo fmt --all -- --check`、desktop cargo checkが成功した。Windows native test executableは環境のApplication Controlで起動不能となる場合があるため、compile、WSL実行、remote CIを併用する。

## 進捗提案（正本未反映）

初心者向け自動設計領域は正本の55%から、保守的に **72%** への更新を提案する。理由は、custom targetの入力・修正、一般木候補、複数画像/GLB consensus、stale-safe apply、保存・安全な出力まで利用者経路が連続した一方、AUT-101の中心である任意目標からの一般的な物理運動付き展開図・折り手順生成は未完だからである。

全体比率8%の領域が55%から72%になる場合、他領域を据え置いた機械的試算は79.32% + 8% × 17% = **80.68%（表示80.7%）** となる。ただしこれはCI全緑後に正本所有者が採否を決める提案値であり、本書作成時点では全体完成度79.3%を変更しない。

## 未完事項

- 任意の一般画像・一般GLBから意味部位と目標surfaceを高信頼に認識すること。
- 任意の非対称・非tree・多成分目標から、一枚紙の展開図を一般生成すること。
- general tree proposalを実際の連続3D物理motion、衝突回避、層順、厚みまで証明して自動適用すること。
- consensusの一致が形状近似の補助証拠であり、折り可能性や物理実現性の証明ではない境界を解消すること。
- 一般曲面、弾性、塑性、紙厚、材料差を含む完成形誤差の計算（初期方針では弾性非計算）。
- CI全緑のterminal確認後に`docs/progress.md`正本と要件状態集計を更新すること。
