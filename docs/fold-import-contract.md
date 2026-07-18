# FOLD取込契約

更新日: 2026-07-18

本書は、ORIGAMI2が現在取り込めるFOLD形式の範囲、利用者確認、失敗時の状態保証を定める。FOLD形式全体への対応を宣言するものではない。

## 1. 参照仕様

- [FOLD specification](https://github.com/edemaine/fold/blob/main/doc/spec.md)
- [FOLD specification history](https://github.com/edemaine/fold/blob/main/doc/history.md)
- [公式repositoryとexamples](https://github.com/edemaine/fold)

対応する`file_spec`は1、1.1、1.2である。未記載は警告して対応subsetとして解釈し、未知の版は安全側に拒否する。`C`（cut）と`J`（join）はFOLD 1.2で追加されたため、1.0/1.1での使用を拒否する。

## 2. 対応subset

- top-level objectにある単一の2D `creasePattern`だけを取り込む。
- `vertices_coords`、`edges_vertices`、`edges_assignment`を必須とし、parallel arrayの長さ、添字、有限値、退化辺、重複辺、交差を検証する。
- 頂点座標は`[x, y]`またはZが0の`[x, y, 0]`だけを許可する。非ゼロZ、`foldedForm`、`frame_attributes`の`3D`、複数frameは取り込まない。
- `B`辺は、各頂点の次数が2で自己交差・穴・分断のない単一の単純閉路でなければならない。穴、複数紙、複数境界は未対応である。
- 割当後の山折り、谷折り、切断線は、変換後も用紙内にあることを再検証する。

## 3. 単位と縮尺

`frame_unit`が`mm`、`cm`、`m`、`in`、`pt`、`um`、`nm`の場合は、仕様に従うmm換算値を初期値として表示する。単位なし、`unit`、独自単位の場合は自動決定せず、利用者が「1 FOLD単位あたり何mmか」を入力する。いずれの場合も確認画面で有限かつ正の任意倍率へ変更できる。

## 4. 線種割当

取り込む前に、実際に含まれるassignmentごとの本数と割当を表示する。

| FOLD | 意味 | 既定または選択可能な取込先 |
|---|---|---|
| `B` | boundary | 用紙境界に固定 |
| `M` | mountain | 山折りに固定 |
| `V` | valley | 谷折りに固定 |
| `C` | cut | 切断線を既定。切断線または無視だけを選択可能 |
| `F` | flat | 補助線または無視を利用者が選択 |
| `U` | unassigned | 山折り、谷折り、補助線、無視から利用者が選択 |
| `J` | join | 補助線または無視を利用者が選択 |

`F`、`U`、`J`はORIGAMI2に同義の線種がないため自動確定しない。未選択が1つでもあれば適用できない。`C`を切断線へ割り当てた場合は、取込projectの切断許可を有効にする。

## 5. プレビューと警告

- native側で全入力を検証してから、作品名候補、仕様、単位、頂点・辺・境界の件数、assignment別件数、SVG線プレビューを返す。
- プレビューは最大5,000辺で、境界を必ず先に含め、他のassignmentを決定論的に間引く。表示の間引きは取込本体を削減しない。
- 折り角度、面情報、frame情報など、対応形式に存在してもprojectへ保存しない既知情報を具体名で警告する。未知fieldは内容をWebViewへ渡さず、存在を要約して警告する。
- 警告が1件でもある場合は、利用者が情報損失を確認するまで適用できない。

## 6. 適用・取消の状態契約

1. nativeファイルダイアログで選択し、Rust側が最大16 MiBまで読み込む。
2. 検証済みbytesを1世代だけメモリ内へstageし、opaqueな取込IDと表示専用DTOをUIへ返す。ファイルpath、実ファイル名、raw JSONはWebViewへ渡さない。
3. UIで作品名、縮尺、線種割当、警告確認を完了する。
4. 適用直前に現在projectが未保存なら、破棄確認を行う。
5. Rust側で同じbytesを再解析・再変換し、stage時のproject instance、project ID、revisionと現在値を照合する。
6. 全検証成功時だけ、履歴・手順・保存先を持たないrevision 0の新規未保存projectとして原子的に置換する。失敗時は元projectとstageを維持し、修正または取消を可能にする。

取消は未適用stageだけを破棄し、projectを変更しない。同じ取込IDに対する重複取消は成功扱いにし、別世代のIDだけをstale取消として拒否する。project差替えとrevision変更のstale検出は適用時に行う。

## 7. 資源上限

| 対象 | 上限 |
|---|---:|
| 入力ファイル | 16 MiB |
| 頂点 | 10,000 |
| 辺 | 10,000 |
| 境界辺 | 1,414 |
| top-level field | 256 |
| metadata配列 | 各64項目 |
| 交差候補 | 1,000,000 |
| 変換後の内部包含判定 | 1,000,000 |
| UIプレビュー辺 | 5,000 |

配列は上限までstreaming deserializeし、超過後の値と未知JSONは保持せず読み捨ててから上限エラーとする。縮尺適用後の丸めで新たな交差候補が増える可能性があるため、変換後にも作業上限と幾何検証を行う。

## 8. 未対応

- child frame、複数frame、frame継承
- 3D `foldedForm`と非ゼロZ座標
- 穴、複数境界、複数枚の紙
- FOLD内の面、折り角度、層順、frame metadataの永続化

SVG取込も[SVG取込契約](svg-import-contract.md)に基づく利用者経路へ接続したため、FOLDとSVGの対応subsetを合わせて要件IO-004を「実装済み」とする。FOLDの複数frame、3D `foldedForm`、穴・複数紙は引き続き本契約の対象外である。

FOLD 1.2の2D展開図書き出しは[展開図書き出し契約](crease-pattern-export-contract.md)で別に定める。書き出したFOLDを本契約のimporterへ戻す場合、座標、外周、5線種、作品名は保持するが、ORIGAMI2 UUIDは再生成され、推奨metadataは非永続情報として警告される。
