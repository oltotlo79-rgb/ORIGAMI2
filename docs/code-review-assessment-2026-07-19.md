# 2026-07-19 コードレビュー第2回の評価

## 対象

Claudeから提供された`ORIGAMI2-code-review-2026-07-19.md`をUTF-8で全文確認し、現在の作業ツリーへ再照合した。原文は14,067 bytes、SHA-256は`0DA6320681AE092FC4BDDD34546F6ED6C58B7A2E3D49E6202FC1C27ACA964DA3`である。レビューはcommit `735eccf`を基準としており、その後のVAL-003、衝突判定、書き出し処理の変更は別途現行コードで確認した。

総評として、保存先、文字列error分類、同期BigInt処理、SVG ZIP、SVG style互換性の指摘は再現でき、妥当である。モノリスと重複機構の指摘も保守上妥当である。一方、FOLDの許容範囲、font subsetting、全commandのerror schema一括変更は、単独のbug修正ではなく、取込契約・対応文字集合・IPC移行を伴う設計変更として分離する。

## このcheckpointで採用した指摘

### 拡張子補正後の無確認上書き

展開図書き出し、折り図書き出し、`.ori2`保存に共通の保存先正規化を導入した。利用者が選んだ拡張子と期待拡張子が異なる場合、補正後の保存先にfile、directory、symlinkが存在すれば保存を中止する。事前確認後の競合も防ぐため、補正した保存先はtyped policyでatomic create-newへ固定し、Unix系では同一directoryの検証済みstageからhard linkを排他的に作成し、Windowsでは`FILE_RENAME_INFO.ReplaceIfExists = false`で確定する。確認と確定の間に別processが補正先を作った場合も既存bytesを変更せず、今回の一時fileだけを清掃してprojectまたは書き出しstageを維持する。利用者が正しい拡張子の既存pathを選び、OS dialogで上書きを確認した場合は従来どおり原子的置換を許可する。

エラーは作品pathやOS errorをIPCへ含めない。補正先の既存内容とproject stateが失敗時に変化しないこと、Windowsの大小文字違いでも既存先を検出すること、事前確認後に補正先を作る決定的競合で`.ori2`、展開図、折り図の3経路が既存bytesとstageを保つことを回帰する。

### 折り図errorの型付け

日本語messageの`contains`でcategoryを逆算する処理を削除した。折り図生成の内部経路は閉じた`InstructionExportErrorCategory`を直接返し、下位のproject、保存、形式libraryとの境界で発生源に応じたcategoryへ一度だけ写像する。既存17 categoryのwire値を固定し、path、glyph code point、実測上限値、raw OS errorを公開しない。

全47 commandを一度に同じschemaへ変える提案は採用しない。IPC互換性とfrontend catalogをfeature単位で移行し、文字列からcategoryを復元する実装を新たに作らない。

### 旧project検証のlock解放

`validate_project`を`capture → spawn_blocking → finish`へ変更した。project mutex下ではimmutable topology inputとidentityを取得するだけにし、geometry検証、紙検証、fingerprint、川崎・前川のBigInt計算をlock外で実行する。完了時はproject instance、project ID、revision、紙・展開図のbit-exact一致を再照合し、同revisionの内容差替えもstaleとして拒否する。worker panicと明示errorはproject lockを保持・poisonせず失敗へ閉じる。

### SVG ZIPの未使用font

折り図SVGは全glyphを`path` outlineへ変換し、`text`やfont URLを使わない。このためSVG ZIPから未参照の9.6 MB font fileと`@font-face`を削除した。archive構造とmanifest fieldが変わるためschema IDをv1のまま再利用せず`origami2.instruction-svg-pages.v2`へ更新した。manifestは`rendering: glyph_outlines`、outline生成に使ったfont source SHA-256、license pathとSHA-256を記録し、OFL全文を保持する。

application内部のfontはPDF/SVG outline生成とoffline再現性に必要なので、このcheckpointでは削除しない。静的instance化や文字subset化は、初版が受理するUnicode集合、生成手順、hash、golden outputを同時に固定できる場合だけ別profileとして行う。Git履歴を書き換えるLFS移行も通常のbug修正へ混ぜない。

### SVG style互換性

レビュー記載の、未対応propertyが33件以上、未対応値が121 scalar以上、または`!important`が一つあるだけで文書全体を拒否する挙動を再現した。対応する9 propertyだけに個別120 scalar上限を適用し、未対応propertyは名前を有界警告へ記録して値を保持・解釈せず無視する。style text全体の256 KiB上限、警告種類64件上限、外部resource禁止は維持する。

対応propertyの`!important`はstylesheet、presentation attribute、inline style、文書順の優先関係に従って解決する。未対応propertyの値を対応propertyへ流用せず、取込後の全group mappingと警告確認も省略しない。

## 条件付きで採用する指摘

### FOLDの任意fieldと用紙境界

FOLD仕様上、`edges_assignment`は任意であり、`B` assignmentの閉路を必須とする現行subsetは実在fileとの互換性を狭める、という指摘は妥当である。ただし現在のFOLD preview DTO、設定dialog、native validationには、SVG取込が持つ複数境界候補のID、強調表示、利用者選択、確認のauthorityがない。

parserだけを緩めて最大面積の閉路やconvex hullを自動採用すると、穴、複数紙、内側の閉路を用紙境界と誤認し得る。したがってこのcheckpointでは受理範囲を変更しない。将来対応は、候補列挙、利用者選択、mapping、再検証ID、apply直前の同一候補検証を一つの契約変更として実装する。`edges_assignment`欠落時も、全edgeを勝手にMountain/Valleyへ割り当てない。

## 構造提案の扱い

次の指摘は妥当であり、VAL-003 checkpoint確定後、SIM-010の折り重ねUIを既存モノリスへ追加する前の保守作業へ入れる。

1. `FoldPreview`からpose scheduling、衝突表示、pointer/keyboard coordinatorを分離する。
2. Tauri `lib.rs`からpersistence、import command、test moduleを段階分割する。
3. 取込・書き出しの世代管理を、型付きの共通pending slotへ寄せる。
4. modal focus、書き出しshell、frontend export flowを共通化する。
5. `ori-formats`の数値、XML、PDF、boundary helperを、挙動差をfixtureで固定してから統合する。
6. `Command::changes`は実利用を調査し、snapshot IPCを継続するなら未消費計算を削除する。

一括置換は行わない。各分割で公開型、stale検出、原子的commit、cancel、全回帰を保ち、機能完成率へは加算しない。

## 採用しない一括判断

- `catch`の文字列件数だけで全件を異常診断へ送らない。cancel、stale、入力拒否、資源上限、best-effort cleanupを除き、予期しない境界だけを既存のredacted diagnosticsへ接続する。
- 日本語文言849行を直ちに全面移行しない。新規文言はfeature別catalogへ寄せ、IPCではcategoryを正本にするが、既存schemaはfeature単位で互換移行する。
- FOLD/SVG互換性を理由に、外部resource、script、曖昧な境界、未知の山谷意味を推測して受理しない。
- test件数、リファクタリング、配布bytes削減だけを製品機能完成率へ加算しない。

## 検証方針

この評価で採用した変更は、対象testだけでなくRust workspace、frontend Node/DOM、production build、lint、Windows/macOS CIを通過してからcheckpointとする。衝突判定は別途、共有頂点A、180度重なりB、山山V字の厚さ`0 / 0.1 / 3 mm`×角度`90 / 135 / 179度`、厳密binary64横断交差、判定保留UIを回帰する。
