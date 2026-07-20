# 展開プロジェクトフォルダー形式 V1 契約

## 1. 目的と初回実装の境界

IO-003の展開フォルダー形式は、`.ori2`と同じ`ProjectDocument V1`および
`EditorHistoryV1`を、通常ファイルへ展開して保存・読込するための形式である。
正本は常に`project.json`と、存在する場合の`editor-history.json`であり、画像は
正本から再生成できる派生物とする。

`ori-formats::project_folder`の初回実装は、通常ファイルのpathとbytesを受け取る
決定的かつ上限付きのin-memory admission boundaryに限定する。次はこの段階の
対象外であり、完了するまでIO-003を「実装済み」と扱わない。

- filesystemからの列挙、symlink、junction、reparse point、hard linkの拒否
- 一時directoryを用いた原子的な保存・置換・復旧
- desktopの保存先選択、読込、上書き確認、エラー表示
- Windows実機E2E

filesystem adapterはlinkを辿らず通常ファイルだけを収集し、このcoreへ渡すこと。
coreで認証が完了する前にSVGをWebView等へ表示してはならない。

## 2. V1の固定entry

物理entry順とmanifest内descriptor順は次で固定する。

| 順序 | path | role | 必須 | 正本 |
|---:|---|---|---|---|
| 1 | `manifest.json` | manifest | 必須 | 認証情報 |
| 2 | `project.json` | `project` | 必須 | はい |
| 3 | `editor-history.json` | `editor_history` | 任意 | はい |
| 4または3 | `preview/crease-pattern.svg` | `crease_pattern_preview` | 必須 | いいえ |

履歴がない場合は`editor-history.json`を省略する。既定128件かつUndo/Redoが空の
履歴も`.ori2`と同じく省略する。空でも履歴件数上限が既定値と異なる場合は、その
設定を失わないようentryを格納する。readerも既定の空履歴entryを非canonicalとして
拒否し、read後の無変更writeでentryが消える状態を受理しない。

V1は上記以外のentryを許可しない。将来任意entryを導入するときはcontainer
versionまたは明示的なoptional-role保持規則を先に追加し、旧readerによる
サイレント破棄を防ぐ。

## 3. manifest schema

`manifest.json`はUTF-8 JSONであり、次のstrict envelopeを使う。未知fieldを
拒否する。

folder manifestを`.ori2`のZIP container manifestそのものにはしない。
`project.json`と履歴本体のschemaは完全に共用する一方、folderにはZIPに存在しない
派生previewのrole、content type、size、hashとmandatory-role集合が必要だからである。
これらを`.ori2`へ意味の異なる任意fieldとして混在させず、folder固有container
envelopeで認証する。

```json
{
  "container": "ORIGAMI2_EXPANDED_FOLDER",
  "container_version": 1,
  "required_features": [],
  "required_roles": [
    "project",
    "crease_pattern_preview"
  ],
  "entries": [
    {
      "role": "project",
      "path": "project.json",
      "content_type": "application/json",
      "schema_version": 1,
      "uncompressed_size": 1234,
      "sha256": "64文字の小文字16進SHA-256"
    },
    {
      "role": "crease_pattern_preview",
      "path": "preview/crease-pattern.svg",
      "content_type": "image/svg+xml",
      "schema_version": 1,
      "uncompressed_size": 567,
      "sha256": "64文字の小文字16進SHA-256"
    }
  ]
}
```

- `required_roles`はdescriptorのroleと同じcanonical順にする。
- 未知のmandatory roleは明示的に拒否する。
- 未知のentry roleも、保持して再保存する規則がないV1では拒否する。
- `required_features`は`.ori2`と同じ語彙と順序を使う。
  `instruction_timeline_v1`、`numeric_expressions_v1`、
  `geometric_constraints_v1`、`layers_v1`、`editor_history_v1`の順である。
- 未知required feature、重複、順序違い、project内容との過不足を拒否する。
- descriptorのsizeは実byte数と一致しなければならない。
- descriptorと履歴envelopeのSHA-256は小文字16進64文字に固定する。

`manifest.json`自体を自己hash対象にはしない。writerは同一snapshotから
byte-for-byte同一のpretty JSONを生成する。

## 4. projectと履歴の正本

`project.json`は既存の`write_project_json` / `read_project_json_with_limits`を
そのまま使う。展開フォルダー独自のproject schemaや、読込時の暗黙補正を作らない。
頂点、辺、手順、layer等の`Vec`順は意味のある順序として保持する。

`editor-history.json`は次のstrict envelopeを使う。

```json
{
  "project_sha256": "project.jsonの小文字SHA-256",
  "history": {
    "schema_version": 1,
    "project_id": "project.jsonと同一のID"
  }
}
```

history本体は既存`EditorHistoryV1`である。readerは次を全て検証する。

1. descriptorのsizeとSHA-256
2. envelopeの`project_sha256`と実際の`project.json`のSHA-256
3. historyのproject IDと`project.json`のproject ID
4. Undo/Redo両stack、逆操作、現在documentとの意味的一致

これにより、履歴だけを別projectへ付け替えることを拒否する。

## 5. 安全な読み取り専用preview

`preview/crease-pattern.svg`は編集用データではなく、`project.json`から決定的に
生成する表示専用cacheである。readerはhash確認後にも、読込済みprojectからSVGを
再生成し、byte-for-byte一致しないSVGを拒否する。したがって、descriptorのhashを
攻撃者が更新しただけでは任意SVGを注入できない。

V1 previewは次の安全subsetだけをwriter自身が生成する。

- SVG 1.1の固定root、`rect`、`g`、`line`、`circle`、固定placeholder `path`
- project名、memo、任意attribute、CSS、script、event handlerを埋め込まない
- 外部URL、`href`、font、image、textureを埋め込まない
- 原座標のY方向を反転せず、表示原点への平行移動だけを行う
- 山、谷、補助、外周、切断は既存LIN-003共通表現を使う。Boundaryは実線、
  Mountainは`6 2 1 2`の一点鎖線、Valleyは`3 1.5`の破線、Auxiliaryは
  `0.5 1.5`かつround capの丸端点線、Cutは`8 2 1 2 1 2`の二点鎖線とし、
  色がなくても区別できる。色は補助情報に限る
- documentの頂点・辺順を保持する
- 座標は有限値のみを採用し、`-0`を`0`にcanonical化する

通常の展開図exportは製造・交換用であり、参照切れ、孤立頂点、未完成外周等を
strictに拒否できる。一方、このpreviewは保存を妨げてはならない。そのため次の
方針を固定する。

- 描画可能な一部だけがある場合は`data-preview-status="partial"`として安全な
  要素だけを描く。
- 有限geometryがない、数値範囲を安全に正規化できない、preview専用件数上限を
  超える場合は固定の`placeholder`を生成する。
- 完全previewがpreview byte上限を超える場合も固定placeholderへ縮退する。
- callerがplaceholderそのものより小さい上限を指定した場合だけ、明示的な
  size errorとする。通常上限では未完成projectを保存不能にしない。
- previewをprojectへ逆変換しない。
- `complete`は「保存済みの全頂点・辺をpreviewへ投影できた」という意味に限る。
  折り紙設計の完成、外周の妥当性、平坦折り可能性、通常export可能性を表さない。

## 6. pathと資源上限

V1 pathはportableなASCII相対pathに限定する。

- `/`始まり、`\`始まり、drive prefix、colonを拒否
- backslashを拒否し、separatorは`/`だけとする
- 空component、`.`、`..`、末尾`/`、control文字、非ASCIIを拒否
- component末尾の`.`と、拡張子の有無を問わないWindows予約device名
  (`CON`、`PRN`、`AUX`、`NUL`、`COM1`〜`COM9`、`LPT1`〜`LPT9`)を拒否
- 使用可能文字は英数字、`/`、`.`、`_`、`-`
- 完全重複とASCII case-insensitive衝突を別々に拒否
- 物理entryとmanifest descriptorの両方へ同じ検証を行う

hard ceilingは次である。callerは値を小さくできるが緩和できない。

| 対象 | hard ceiling |
|---|---:|
| entry件数 | 4 |
| path | 256 bytes |
| 任意1 entry | 128 MiB |
| `manifest.json` | 1 MiB |
| `project.json` | 128 MiB |
| `editor-history.json` | 64 MiB |
| preview SVG | 16 MiB |
| 全entry合計 | 256 MiB |
| preview生成対象頂点 | 100,000 |
| preview生成対象辺 | 100,000 |

件数、path、各entry size、加算overflow、合計sizeをJSON/SVG解析より前または可能な
限り早い段階で検証する。in-memory APIへ渡す前のfilesystem adapterにも、読込中の
allocationを防ぐ同じhard ceilingが必要である。

## 7. canonical writeとread admission順

writerは次の順で処理する。

1. projectと任意historyを既存validatorで検証
2. canonicalな`project.json`を生成
3. project hashへ束縛した任意history envelopeを生成
4. projectから安全previewを生成
5. size、hash、required feature/roleを持つmanifestを生成
6. 固定entry順のin-memory artifactを組み立てる
7. 自身の出力を同じreaderへ通し、検証済みartifactだけを返す

readerは次の順で処理する。

1. 件数、path、重複、case衝突、entry/合計size
2. strict manifest envelopeとcontainer version
3. 未知required role / feature
4. role、path、content type、schema version、canonical順
5. 全payloadのdeclared sizeとSHA-256
6. project schemaと意味検証
7. required featureとproject内容の一致
8. 任意historyのproject hash、project ID、履歴意味検証
9. previewの決定的再生成とbyte一致

読込後に変更せず再保存したartifact、およびその再読込結果は
byte-for-byteで安定しなければならない。

## 8. core回帰契約

最低限、次を自動テストする。

- 履歴なし/ありの決定的write、read、write-read-write
- 空または孤立頂点だけの未完成projectをplaceholder/partialで保存
- traversal、absolute相当、unsafe separator
- 完全重複path、case-insensitive path衝突
- entry件数、path、各role size、総量のcaller-tightened limit
- 未知mandatory role、未知required feature
- role/path/content type/schema version、物理順、descriptor順の不一致
- project/history/previewのsizeとhash改ざん
- historyのproject hashおよびproject ID付替え
- hashを更新した任意SVGでも、project由来previewと違えば拒否

filesystem adapterを追加するときは、これにsymlink/junction/reparse point、
列挙中の差替え、通常ファイル以外、原子的置換失敗、既存directory復旧の回帰を
追加する。
