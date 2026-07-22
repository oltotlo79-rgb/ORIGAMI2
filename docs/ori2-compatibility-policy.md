# ORI2 schema compatibility policy

## 対応世代

`.ori2` containerと`project.json`はcurrent versionだけを書き出す。editor historyは次の3世代を読み込める。

| 入力世代 | history field | 読込後 |
|---|---|---|
| current | `history_entry_limit`、`undo_stack`、`redo_stack` | 値を保持 |
| current-1 | `history_entry_limit`、`undo_stack` | `redo_stack=[]`を補完 |
| current-2 | `undo_stack` | currentの既定上限と`redo_stack=[]`を補完 |

世代名は互換性policy上の相対名であり、wire上の`schema_version`を書き換えて旧versionを装わない。旧世代は当時存在しなかったoptional fieldの欠落だけで識別し、既知の意味を変更しない。

## 読込・保存規則

- unknown field、未知の未来schema version、未来container versionはfail-closedで拒否する。
- currentより古い`project.format_version`へのdowngrade書込みと、未来versionの書込みを禁止する。
- 旧historyを読み込んだ後の保存はcurrent canonical formだけを生成する。
- canonical archiveを再読込・再保存したbyte列は同一でなければならない。
- migrationはproject本文を変更せず、texture asset、参照GLB asset、Undo履歴、layer evidenceを保持する。
- history hash、project ID、manifest digest、required featureの検証をmigration前後で緩和しない。

## Version matrix E2E

`crates/ori-formats/src/ori2.rs`の`migrates_two_legacy_history_generations_and_resaves_canonically`は、current/current-1/current-2を同じ完全archive fixtureで検証する。各行で次を確認する。

1. project、texture asset、参照GLB asset、layer evidenceが一致する。
2. Undo履歴が保持され、欠落していたredoだけが空で補完される。
3. current-2だけhistory entry limitが既定値へ移行する。
4. current canonical formへの再保存後も意味が一致する。
5. 2回目のcanonical保存がbyte単位で冪等になる。

`ori2_version_policy_rejects_future_and_downgrade_writes`はversion 0とcurrent+1への書込みを`UnsupportedVersion`で拒否する。
