# 設定・ロケール横断QA証跡（2026-07-22）

## 対象

Claude監査の補完として、テーマ設定の永続化・移行、日本語UIの固定英語文言、動的ARIAラベル、production build、lint、正式版配布契約を横断確認した。

## 設定スキーマ

- テーマ設定は `version: 1` と `preference` だけを許す厳密JSONへ移行した。
- 旧形式の `system` / `light` / `dark` は読み込み時に正規形式へ移行する。
- 未知フィールド、未知version、破損JSON、128 UTF-16 code unit超過を拒否する。
- 保存失敗時もセッション選択を維持し、初期化失敗時は安全な既定値へ戻す。
- テーマ設定DOMテスト: 4 / 4成功。テーマ単体テスト: 8 / 8成功。

## ロケールとアクセシビリティ

- surface範囲の割当・三角形番号、骨格バー入力の動的ARIAラベルを日英切替へ変更した。
- 突起目標の13入力ラベルと比較表7見出しを日英切替へ変更した。
- source scanで見つかる英語文字列は、次の2群を分離して判定した。
  - 利用者向けUI文字列: `text({ ja, en })` または `formattedText({ ja, en }, values)` へ移す。
  - protocol文字列: IPC field、enum wire value、command名、保存schema key、診断codeは互換性のため変更しない。
- 全DOMテスト: 49 files / 329 tests成功。
- 全source/snapshot契約テスト: 1,652 / 1,652成功。

## 配布・静的検証

- `npm run build`: 成功（TypeScript project buildとVite production build）。
- `npm run lint`: exit code 0。既存警告は検出されるが新規errorなし。
- `node --test .github/tests/formal-release.test.mjs`: 53 / 53成功。

## 判定

今回確認した設定移行と利用者向け固定英語文言は修正対象として対応した。wire互換性を構成するprotocol文字列はUI翻訳対象ではなく、変更すると保存データ・IPC・native側との契約を破るため維持した。

## 画像認識の最悪ケース上限

- 入力は最大4,000,000 pixel、連結成分は探索時64、採用時8までに制限される。
- 8成分のKruskal MST候補は `8 * 7 / 2 = 28` 比較、出力は成分内8本とbridge 7本の最大15本で決定的に制限される。
- 全採用pixelを複製していた一時 `Vec<usize>` を廃止し、boundsを既存component配列から直接集計する。4,000,000 pixel時に最大約32 MiBの一時複製を削減した（64-bit環境）。
- rotation 4種、mirror 4種、crop、polarity 3種は直積探索しない。利用者が選択した各1値だけを1回変換・1回解析し、変更は300 ms debounceする。
- 実行中の設定変更または明示キャンセルはgeneration IDを即時無効化し、遅延応答を表示・保存へ到達させず、busy表示も解除する。
- `ori-domain` 認識テスト7 / 7、frontend認識契約11 / 11、TypeScript buildが成功した。

## GLB node・mesh境界

- passive GLB parserは複数meshを個別に読み、最大20,000 vertex・40,000 triangle・16 MiBへ制限する。
- parserがnode transformを適用せずraw mesh座標を使うため、`matrix`、translation、rotation、scale（negative scaleを含む）、skin、morph weightsを持つnodeは取込み前にfail-closedとした。
- 同じmeshを複数nodeが参照するinstancingも、実体数を過少認識しないようfail-closedとした。identity nodeから異なる複数meshへの1対1参照は許可する。
- GLB単体回帰5 / 5が成功し、multi-meshを維持したままtransform・negative scale・instancingの誤認識経路を遮断した。
