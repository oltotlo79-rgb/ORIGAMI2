# 数式入力の最初の利用者向け縦切り

更新日: 2026-07-19

## 初版スコープ

最初の接続対象は、新規プロジェクト作成時の紙幅と紙高である。既存紙のサイズ変更へ
式を関連付けると、式メタデータも幾何と同じUndo/Redo履歴へ入れる必要があるため、
この縦切りには含めない。保存する式は現在寸法を拘束する式ではなく、明示的に
「作成時サイズ式」として扱う。

入力は `ori-numeric` と native IPC で192 bit評価する。幅と高さの両方が成功するまで
既存プロジェクトは変更しない。WebViewから評価済みの `number` を新規作成commandへ
渡さず、native側で式を再評価してから紙を作る。評価開始時の
`project_instance_id`、`project_id`、revisionをnative側で採用直前にまとめて照合し、
同じproject IDとrevisionの文書を開き直すABAも拒否する。

## f64採用規則

保証区間の下端と上端が次の条件をすべて満たす場合だけmm値を採用する。

- 両端が有限で、保証区間全体が0より大きい。
- 両端が同一のbinary64値、または正のbinary64上で隣接する1 ULPの値である。
- 隣接時は正の下端値を採用する。中点演算は行わず、Rust/TypeScript間のunderflowや
  丸めmode差を持ち込まない。

それ以外は `result_out_of_range` とし、幾何を変更しない。この規則はfrontendの事前確認と
nativeの正式採用で同じである。正式なauthorityはnative側にある。

## project保存schema

`ProjectDocument.numeric_expressions.rectangular_paper_creation` は任意fieldであり、
fieldがない従来projectは空のbindingへmigrationする。

```json
{
  "numeric_expressions": {
    "rectangular_paper_creation": {
      "schema_version": 1,
      "width_source": "200 * sqrt(2)",
      "height_source": "400 / 3",
      "adopted_width_mm": 282.842712474619,
      "adopted_height_mm": 133.33333333333334
    }
  }
}
```

sourceは空でない1行、UTF-8で4,096 byte以下に制限する。WebViewでは入力欄にも
4,096 code unitの上限を設定し、それを超える値はUTF-8変換より前に拒否する。nativeの
UTF-8 byte上限が最終境界である。評価値は有限かつ正でなければならない。
.ori2 manifestは `numeric_expressions_v1` をrequired featureとして宣言し、式を理解しない
旧実装による黙った破棄を防ぐ。

open時にはnativeが両方のsourceを再評価する。再採用したbinary64値と保存済み評価値を
bit単位で照合し、一致しないprojectは現在のprojectを置き換える前に拒否する。評価workerが
使用中の場合は破損扱いにせず、「評価中のため待って再試行」と固定文言で知らせる。

## UIの安全動作

- blurまたはIME変換中でないEnterで評価する。
- 入力変更ごとに世代を更新し、古いresponseは画面状態へ適用しない。
- nativeの1-worker制約に合わせ、frontendは実行中1件と最新の待機1件だけを保持する。
  待機中に新しい入力が来た場合、置き換えられた要求はnativeへ送らず
  `stale_response` で完了させる。
- Escapeは最後に成功した式へ戻す。成功履歴がなければ初期式へ戻す。
- 式と評価値を切り替えて表示できる。保存後も「作成時サイズ」として表示する。
- browser開発表示では `native_unavailable` を明示し、幾何を変更しない。
- 失敗category以外のnative error内容や入力式をerror表示へ混ぜない。
