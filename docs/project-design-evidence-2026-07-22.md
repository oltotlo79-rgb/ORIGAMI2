# Project・保存・履歴／要件・基本設計 実装監査（2026-07-22）

## 監査方針

`docs/requirements-definition.md`のPRJ-001〜009、HIS-001〜006、IO-003と、versioned project schema、`.ori2`、expanded folder、recovery、Undo/Redo、autosave、provenanceの現行production codeを照合した。型が存在するだけでなく、strict reader、native mutation、保存・再読込、利用者UI、自動回帰が連続していることを進捗根拠とした。

本書は監査記録であり、CI全緑までは`docs/progress.md`正本を変更しない。GitHub Releaseの外部公開authorityや署名は配布領域、一般物理motionは3D・経路領域として、この2領域へ重複加算しない。

## Project・保存・履歴の実装証拠

| 要件・境界 | production evidence | 判定 |
|---|---|---|
| PRJ-001〜006 | 一枚紙identity、任意単純多角形boundary、cut policy、非接着、binary64厚みをdomain/editor/native commandで保持 | 実装済み |
| PRJ-007〜008 | 表裏色・個別texture asset、mm/cm/inch/ratio表示単位をproject保存・履歴・UIへ接続 | 実装済み |
| PRJ-009 | vertex/edge/faceのstable UUID、名前・色・memo・metadata文書をstrict保存、Canvas/UI、履歴へ接続 | 実装済み |
| HIS-001〜003 | 差分command Undo/Redo、1〜128件のproject/session上限、再起動後の認証済み両stack復元 | 実装済み |
| HIS-004〜006 | 30秒周期のprivate recovery、起動時の復元/破棄、通常保存と別slot、正常保存後の世代付きclear | 実装済み |
| IO-003 | `.ori2`とexpanded folderのstrict保存・読込、no-follow adapter、immutable phase journal、同一project置換、別project上書き拒否 | 部分実装（Windows実機folder E2Eを残す） |

## versioned authorityとstrict境界

- `.ori2`はversioned document、asset registry、editor history、history limit、numeric expression binding、layer、annotation、underlay、metadata、beginner profile/provenanceを一つの検証済みarchiveとして扱う。
- expanded folderはproject IDとmanifest、各phaseのcontent digest、immutable journalを検査し、途中失敗からnative private registryで回復する。filesystem pathはstrict IPC responseに含めない。
- serdeの`deny_unknown_fields`、canonical non-nil UUID、件数・byte数・文字数上限、重複ID・参照欠損・hash不一致をreader入口で拒否する。
- WebViewが与えたhashやraw asset bytesをauthorityにしない。texture、画像、GLB、consensusはnative registryのbytesから再解決する。
- mutationはinstance ID、project ID、expected revisionを要求し、成功時だけrevisionを進める。preview、cancel、stale、tamper、late responseは履歴にも文書にも変更を加えない。
- Undo/Redo stackのcommand payload、revision transition、history limitを保存時と読込時に再検証し、現在文書と適用不能な履歴を黙って採用しない。

## Autosave・recovery authority

`recovery.rs`はproject→recovery operation gateの固定lock順、世代・epoch fence、single writer、capture後のstale確認を持つ。古いautosaveが正常保存後にslotを復活させる競合、古いclearが新しい復旧を消す競合、復元中のproject ABAを拒否する。復旧候補はopaque `RecoveryId`で参照し、通常project pathやpayloadをrendererへ公開しない。

復元はprepareとcommitを分離し、commit時にcandidate identityとepochを再確認する。破損slotはinvalidとして明示し、通常ファイルを上書きしない。autosave healthはredactedな有限状態と単調transition IDだけをUIへ返す。

## Provenance・metadata tamper拒否

- named-technique compilerはmodel/version/kind/segment/output SHA-256をtimeline stepへ束縛し、保存・復旧・PDF/SVG再出力前に再検証する。
- general treeとcustom targetはasset content、tree topology、generator version、source revision、physical-proof/apply authorityをversioned provenanceへ保持する。
- reference consensusは全bindingとpair digestをproject-private provenanceへ保存し、外部出力では検証後に安全な集約summaryだけを残す。
- mesh/crease/instruction export provenance readerはduplicate marker、unknown field、malformed hex/JSON、digest改変、resource超過をfail-closeにする。
- legacy文書はoptional fieldの既定値で読めるが、legacyを新しいauthorityや物理証明へ自動昇格しない。

## 要件・基本設計の証拠

要件定義はMUST 87件とFUTURE 14件をstable IDで管理し、`docs/requirements-status.md`が行単位の実装/部分/未着手を追跡する。紙厚、衝突分類、層順、fold model fingerprint、issuer-bound certificate、native/WebView trust boundary、resource limit、保存・回復、export lossをversion/model付き契約としてコードへ落としている。

特に近年の実装では、単なる設計文書ではなく次の設計原則がproductionと回帰へ固定された。

1. read/diagnose/previewとmutation authorityを分離する。
2. project instance・ID・revision・fingerprintでABAとstaleを拒否する。
3. proof issuer、consumer、exporterを分離し、必要証拠を別モデルで代用しない。
4. allocation前hard cap、cancel checkpoint、bounded progress、late response破棄を共通にする。
5. raw path、asset bytes、private certificateをrendererへ渡さず、安全な有限DTOだけを公開する。
6. unsupported/indeterminate/resource-limitを成功へ丸めず、利用者へ固定語彙で示す。

## テスト証拠

- `ori-formats`は`.ori2`、expanded folder、asset、history、provenanceのroundtrip、legacy、unknown/tamper、上限、独立readerを検査する。
- `ori-core`はcommand単位のrevision、Undo/Redo、history limit、保存後復元、invalid commandの無変更を検査する。
- desktop nativeはatomic/no-follow保存、folder journal recovery、autosave競合、startup restore/discard、stale instance/revisionを回帰する。
- frontend strict clientとDOMはsnapshot exact keys、recovery blocking、autosave health、history control、save/cancel/errorの利用者経路を検査する。
- 直近のfrontend snapshot 1,650件、DOM 328件、TypeScript build、WSL format/check、desktop cargo checkが成功している。Windows native executableがApplication Controlに遮断される場合はWSLとremote CIを併用する。

## 保守的進捗提案（正本未反映）

- 「プロジェクト・保存・履歴」: **78% → 94%** を提案する。PRJ/HISの利用者経路、strict通常保存、復旧、履歴authorityは実装済みだが、expanded folderのオーナーWindows実機E2Eと、異常終了・権限・セキュリティ製品を含む全環境matrixを残すため100%にはしない。
- 「要件・基本設計・技術検証」: **70% → 85%** を提案する。主要trust/proof/resource/persistence境界はversioned codeと回帰へ固定された。一方、一般正厚・一般cycle・一般物理motionの最終設計、正式release後の運用feedback、全FUTURE要件の受入基準確定を残す。

他領域を正本値のまま据え置く単独機械試算は、79.32% + 8% × (94%-78%) + 5% × (85%-70%) = **81.35%（表示81.4%）** となる。他の監査提案と合算する場合は同じ正本baselineから重複なく再計算すること。

## 未完・非加算事項

- expanded folderのWindows実機での作成、差替え、journal crash recovery、権限/no-follow E2E。
- filesystem/antivirus/同期softwareを含む長時間・障害注入matrixと、容量枯渇時の利用者回復経路。
- 将来schema migrationを複数世代にわたり保証する正式compatibility policy。
- 一般正厚、一般cycle、一般物理motionの仕様確定と受入（別領域の未完）。
- GitHub Releaseの外部公開、署名鍵、配布authority、実運用promotion（配布・QA領域で評価）。
- remote CI全jobが同一headでterminal greenになった後の`docs/progress.md`更新。
