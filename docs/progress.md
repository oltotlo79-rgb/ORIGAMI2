# ORIGAMI2 開発進捗

## 完成率

**全体完成率: 約36.9%（2026-07-19、暫定の重み付き概算）**

完成率は画面数や実装行数ではなく、折り紙作家向けMUST 87件と、その後に作る初心者向け自動設計FUTURE 14件、品質検証、Windows正式版とmacOS自動ビルド・CI検証を合わせた全製品ビジョンの総工数に対する暫定概算である。各領域の進捗値は要件件数の単純比ではなく、利用者がUIから実行できる範囲を第三者監査とコードで見積もった概数である。UI未接続の解析基盤、テスト追加、内部品質改善は各節へ成果として記録するが、それだけでは機能完成率へ加算しない。MUST 87件の個別状態は`docs/requirements-status.md`で別に追跡する。

下表の「全体への寄与」は「全体比率 × 現在の領域進捗」である。直前の追跡値32.76%に対し、時間制限つき全体平坦折り3値判定、場所別層順序、native background worker、進捗・中止・終端状態のUI接続により、「折り可能性・経路探索」を12%から35%へ更新した差分4.14ポイント（全体比率18% × 23ポイント）を加えた36.90%を小数第1位へ丸めて表示している。専用の層順3D viewer、SIM-010の折り重ね、一般経路探索、FOLDの3D・複数frame、3D完成形出力は計上していない。入力値自体が概数なので、36.90%は追跡用の計算値であって測定誤差のない精密値ではない。

## 重み付け

| 領域 | 全体比率 | 現在の領域進捗 | 全体への寄与 | 状態 |
|---|---:|---:|---:|---|
| 要件・基本設計・技術検証 | 5% | 70% | 3.50% | 要件定義・設計文書・技術検証は充実。紙厚は中央面基準近似を初版仕様として確定。全体平坦折りと層順序の証明モデルをversion固定した |
| プロジェクト・保存・履歴 | 8% | 40% | 3.20% | 原子的編集、差分Undo/Redoの固定128件上限、`.ori2`保存は実装。利用者設定、履歴永続化、自動保存、クラッシュ復旧は未着手 |
| 2D展開図エディター | 15% | 50% | 7.50% | 基本編集と9種スナップは実装。面編集、数式作図、レイヤー、対称編集を残す |
| 数式・幾何制約 | 9% | 0% | 0.00% | EDT-004/005/008/009の数式入力、式保持、11種制約、矛盾特定は未着手。3D衝突用の数値計算はこの領域へ計上しない |
| 3D折り・紙厚・衝突 | 17% | 50% | 8.50% | 木構造1ヒンジの姿勢・紙厚・衝突・固定面・物理把持を実装。層順正本は得られたが、専用3D表示、折り重ね、閉路、切断由来を残す |
| 折り可能性・経路探索 | 18% | 35% | 6.30% | 1ヒンジCCD、補正候補の解析専用UI、川崎・前川局所条件に加え、凸面対象の全体平坦折り3値判定と場所別層順序を接続。候補3Dプレビュー・明示適用、局所十分性、一般経路探索を残す |
| 折り手順・PDF | 10% | 25% | 2.50% | 手動step登録、説明編集、並べ替え、Undo/Redo、`.ori2`保存・読込、実姿勢確認付き段階再生に加え、固定3D図付きのA4複数ページPDF・SVGページ画像ZIP書き出しを実装。連続動作、折る方向の矢印、手指guideを残す |
| 入出力・互換性 | 5% | 68% | 3.40% | `.ori2`、FOLD/SVG取込に加え、現在の一枚紙展開図をFOLD 1.2、静的SVG、実寸PDF 1.7、DXF AC1021へ、情報損失確認、revision固定stage、native原子的保存付きで書き出せる。FOLDの3D・複数frameと完成形3D形式を残す |
| 多言語・設定・配布・QA | 5% | 40% | 2.00% | frontend/Rustの自動回帰、Windows/macOS CI、環境・作品情報を含まないredacted diagnosticsの端末内保存・正確な内容確認・同一JSON手動保存を実装。i18n、設定、更新、GitHub Releases配布を残す |
| 初心者向け自動設計 | 8% | 0% | 0.00% | 将来要件のみ |
| **合計** | **100%** | — | **36.90%** | — |

## 完了

- VAL-003のcurrent layer-orderをsnapshot cloneではなく、同一slot・certificate identity、project instance/ID/revision、bit-exact topology、fingerprint、proof/layer model、provenance、material registry、checked単調generationへ結合したprivate capabilityとして捕捉・再認証できるようにした。AppState→slotの固定lock順で両lockを保持するcommit closureによりcancelとのTOCTOUを閉じ、同内容再解析ABA、edit→Undo、reopen、別slot、deep clone、世代枯渇をdesktop 201件で回帰した
- `EditorState`のrevision上限をJavaScriptで正確に往復できる`2^53-1`へ固定し、execute・Undo・Redoは次revisionをmutation前に予約する。上限ではpattern、paper、timeline、revision、Undo/Redo履歴を完全不変のまま型付きエラーで拒否し、`face_lineage_v1`も同じ一段更新契約へ統一した。通常経路のrevisionはUndo/Redoでも単調増加する
- `LayerOrderSnapshot`のoverlap cellをimmutable geometryから全canonical supporting-line arrangement atom集合として再構築し、保存順と独立にkey・exact boundary・covering facesの完全一致を要求するcertificate再検証を完成した。人工分割、canonical lineをまたぐ結合、欠落、重複を拒否し、single-face coverage、partial overlap、3-ply、離隔cell、点・線接触ではorderなし、source保存順・edge方向不変、資源上限・deadline・cancelをcore 69件で固定した。元arrangementと検証用arrangementの二重live storageも同一budgetへ計上する。native current poseと原子的commandは増えていないため、完成率は36.9%のままとする
- `topology_contact_policy_v1`として共有関係4種×交差証拠10種の40セルを固定し、共有頂点だけのexact証明、共有ヒンジの有限corridor、正体積・共面正面積・横断交差、判定保留を認証済みruntime evidenceへ結合した。角起点山谷V字、辺中点山山V字、候補外の共有頂点・共有ヒンジ、共有ID欠損adjacency、候補内離間、ヒンジ制約欠落、厚さ0/0.1/1/3 mm、角度・面順・巨大平行移動を同期・分割・one-shot・full scanで回帰し、判定保留は貫通と同じ赤系blocking表示にした。frontend Node 1,012件、DOM 39件、本番build、lint、ブラウザMCP再読込とconsole error 0を確認した。内部品質の確定であり、折り重ねの利用者経路は増えていないため完成率には加算しない
- コミット`7f2e214`の[CI #221](https://github.com/oltotlo79-rgb/ORIGAMI2/actions/runs/29660148418)でfrontend、Windows/macOS Rust、Windows NSIS bundle、macOS `.app` bundleの全jobが成功した。macOSはオーナー決定どおり自動ビルド・CI検証までとし、実機検証には計上しない
- 紙厚入力は新規作成時の既定値を0.10 mmとし、専用の上下ボタンと上下矢印キーで現在値から正確に0.01 mmずつ増減する。0.075 mmのような細かい直接入力を0.01 mm格子へ丸めず、空欄・非有限値・負数を保存しない。Node、DOM、本番build、lintとブラウザMCPで入力欄、増減ボタン、accessible name、console error 0を確認した
- Claudeコードレビューの構造提案へ段階対応し、Tauri `lib.rs`から`.ori2`読込・検証・同一directory staging・原子的publishを`project_persistence`へ分離した。Windowsの相対pathを含むdesktop native 194件で、拡張子補正、既存先拒否、原子的置換、失敗時清掃、project state更新条件を維持した
- `FoldPreview`からkeyboard選択・camera操作のcoordinatorを分離し、scene generation、re-entry、dispose、hostile accessor、callback例外を境界ごとに失効させる専用22件を追加した。frontend Node 912件、DOM 35件、本番build、lintを通過し、次のSIM-010 UIを巨大effectへ直接追加しない責務境界を作った
- SIM-010の最初の内部基盤として`face_lineage_v1`を追加した。source/target topologyの再構築、凸sourceへのtarget全頂点の厳密包含、binary64由来のsource別面積保存、project/revision/fingerprint/material registry、決定順、資源上限を証明し、core 130件とcompile-fail doctestで固定した。これは一本の直線差分、層順authority、山谷割当て、衝突経路、timeline、原子的commitをまだ保証しないため、SIM-010の利用者経路と完成率には計上していない
- SIM-010を、現在のnative layer-order slot認証、3D直線の逆写像、層別山谷線、face lineage、連続衝突停止、target層順再証明、全timeline移行、pattern・姿勢・層順・timelineの一括commitまで成功して初めて一操作とする[原子的トランザクション設計](stacked-fold-design.md)へ固定した
- コミット`89ef51b`の[CI #216](https://github.com/oltotlo79-rgb/ORIGAMI2/actions/runs/29653109030)でfrontend、Windows/macOS Rust、Windows NSIS bundle、macOS `.app` bundleの全5 jobが8分33秒で成功し、検証済みWindows installer artifactを生成した
- 利用者報告の共有頂点A、180度重なりB、辺中点からの山山V字を実寸400 mm fixtureで再現し、厚さ`0 / 0.1 / 3 mm`と角度表を固定した。保存binary64座標のBigInt厳密横断証明で、退化三角柱・近平行SATから判定保留へ退避していた実在交差を貫通へ確定し、点・共有辺だけの接触は貫通へ昇格しない
- BigInt厳密横断証明は不変姿勢の1解析につき最大256回へ制限し、one-shot、同期drain、増分job、full scanの全経路でcandidate・stepをまたぐ同一budgetを共有する。超過pairはヒンジ許容へ落とさず「交差の可能性・判定保留」としてblocking表示し、attempt・省略数をdeep-frozen snapshotへ記録する
- 全体平坦折りsolverを128 MiBの決定論的な論理証明storage budgetへ統合し、`TupleConstraint`本体と内側buffer、固定代入、返却assignment、Union-Findと連続bufferによる連結成分、domain、明示stack、rollback trail、元問題と検証再生成問題の同時保持を会計した。対象確保はchecked算術と`try_reserve`の前に上限判定し、再確保時の旧・新buffer peak、1 byte不足、overflow、deadline・cancel優先を回帰する。この値をallocator metadataやBigInt objectを含む実heap/RSSのhard上限とは呼ばず、構造件数・bit長・演算回数の別上限と組み合わせる
- 2026-07-19のコードレビューを現行コードへ再照合し、拡張子補正先の無確認上書き防止、折り図errorの型付け、旧project検証中のmutex解放、SVG ZIP v2からの未使用font削除、SVG styleの未対応propertyと`!important`互換性を採用した。FOLD任意fieldは境界候補選択までを一体で設計する条件付き課題、モノリス分割はSIM-010前の保守作業として分離した
- `.ori2`・FOLD・SVGの読込、project保存前検証、topology構築、background worker失敗を原因別の固定文言へ閉じ、選択path、raw OS error、parser内部値、panic payload、作品名や座標をWebViewへ反射しない。失敗時に既存project・履歴・保存先・取込stageを変えないことをnative回帰する
- Windows上でdesktop native 193件、frontend Node 890件、DOM 35件、本番build、lint、全Rust workspaceのClippy `-D warnings`を通過した。外部parserは生成したPDF約1.04 MBとoutline-only SVG ZIP約245 KBを検証し、ブラウザMCPでは全体平坦折りpanelの時間制限・対象クラス・保証外事項を確認し、console error 0・warning 0を確認した
- 凸material faceを対象とする`convex_faces_facewise_v1`の全体平坦折り判定をUIへ接続し、証明できた可・不可と、対象外・時間切れ・資源上限・証明不足による不明を区別する。`possible`だけが場所別`facewise_layer_order_v1`をnative current slotへ採用する
- project identity、revision、version付きSHA-256 fold model fingerprint、proof modelをprovenanceへ束縛し、同revisionへ戻るABA、project再open、同一IDの別内容、旧job完了をstaleとして拒否する。source snapshotとactive/completion/current layer-orderは同一`Arc` bindingを共有し、pointer identityも照合する
- 1〜300秒の時間制限、単調phase、上限付き件数、利用者中止、panicを閉じた失敗へ変換するnative background workerとUI panelを接続した。source件数はfingerprint・topology構築前に上限確認し、超過時だけcurrent job/層順slotへ触れない未登録の不明結果を即時返す
- coreはimmutable geometryからtopologyと局所必要条件を独立に再生成して入力artifactと完全一致を確認し、exact平面配置・重なりcell・facewise制約・反復DFS・certificate再検証を行う。証明構築、検証、serializationを合わせたstorage budgetとchecked overflowを持ち、上限・deadline・cancel到達時は候補証明を公開しない
- VAL-003/005/006/007/009を実装済みへ更新した。MUST 87件の現在集計は実装済み32・部分実装27・未着手28。SIM-010と専用の層順3D viewerは未着手のままで、完成扱いしていない
- commit `48f4de6`の[CI #213](https://github.com/oltotlo79-rgb/ORIGAMI2/actions/runs/29643184224)でfrontend、Windows/macOS Rust、Windows NSIS bundle、macOS `.app` bundleの全jobが成功。Windows配布物では同梱Noto Sans JPとOFLのSHA-256、外向き通信を遮断した折り図生成、外部parserによるPDF・SVG ZIP監査を通し、折り図書き出しcheckpointを採用
- 折り手順timelineから、説明・注意事項・固定3D図を載せたA4複数ページPDFとページ別SVG画像ZIPを書き出す利用者経路を追加。INS-010を部分実装へ更新し、当該checkpoint時点のMUST 87件集計を実装済み27・部分実装31・未着手29とした
- 書き出し前の形式・ページ数・警告確認、revisionに固定した一度限りの生成、進捗表示、native保存ダイアログと原子的保存、取消後の再試行を接続。初版で省略する滑らかな連続動作、折る方向の矢印、手指guide、照明・影・透明表現を明示する
- 一枚紙展開図のPDF 1.7/DXF AC1021書き出しを既存の確認画面へ接続し、要件IO-006の列挙4形式をUIから利用可能にした。当該checkpoint時点のMUST 86件集計は実装済み26・部分実装30・未着手30
- PDFは全描画辺boundsへ四辺10 mm余白を加えた一ページを実寸1:1で生成し、`PrintScaling=None`、14,400 pt page上限、DeviceGray黒の5線種、UTF-16BE作品名、固定5 object/xref、64文字数値上限、未参照頂点拒否、edge向き・順序非依存の決定論的bytesを保証する
- DXFはBOMなしUTF-8・CRLFのAC1021 text-formとして、mm座標、固定header、5個の`ORIGAMI_*` layer、4個のcustom LTYPE、canonical LINE順、制御文字title拒否、scalar境界の999 comment分割、100,000 group pair・64文字数値上限、内部再parse照合を固定する
- PDFはpypdfとMuPDFで一ページ・実寸MediaBox・Unicode title・5描画batchを読込・renderし、DXFはezdxfでAC1021・mm・5 layer・8 LINEを読込後、audit error 0・fix 0を確認した。これら外部parserは製品依存へ追加していない
- 4形式すべてで形式/MIME/拡張子、完全なpreview allowlist、閉じたwire enum、exact-byte原子的保存、stage一度消費、project不変をnative回帰する。frontendは806件、`ori-formats`は114件、desktop nativeは126件を実行する
- コミット`67dee80`のCI Run `29638486053`全ジョブ完走（FOLD/SVG展開図書き出し、Windows/macOS Rust、macOS `.app` bundleを含む）
- FOLD 1.2と静的SVGの展開図書き出しをUIへ接続。現在projectのinstance・ID・revision・形式に固定したimmutable bytesをRust内の最新1世代だけへstageし、WebViewにはopaque ID、件数、サイズ、Cut有無、sanitize済み保存名候補、固定警告だけを返す
- FOLDはmm座標と`B/M/V/F/C`、SVGは1 unit = 1 mmとcanonical `data-origami-kind`で、外周・5線種・Cutを保持する。FOLD/SVGの既存importerへ戻すround-trip、決定性、悪意あるtitleのJSON/XML escape、負のviewBox、未参照SVG頂点、非有限値、件数・byte・交差候補上限を回帰する
- 書き出し確認画面で紙の見た目、ID・履歴、3D表示、camera、折り手順、線がない場合の切断許可を表示し、明示確認をnativeでも強制する。保存先はnative dialogだけで選び、同一directoryの一時fileへwrite・sync・同一handle再読込後に原子的置換する。取消・失敗・stale・旧tokenではprojectを変えず、dialog取消は同一stageを再試行できる
- 書き出し画面の形式切替、警告gate、件数表示、Tab/Shift+Tabと外部focus trap、IME中Escape、busy中の閉じる遮断、保存失敗後の同一stage再試行と再生成を実DOMで確認する。frontendは現在806件を実行する
- FOLD/SVG書き出し時点ではIO-006とIO-009を未着手から部分実装へ更新し、当時のMUST 86件集計を実装済み25・部分実装31・未着手30とした。後続のPDF/DXF実装による現在値は本節冒頭を正本とする
- SVG 1.1/2共通の静的直線subsetをUIへ接続。`line`、`polyline`、`polygon`、非角丸`rect`、直線path、nested affine transform、presentation/inline/class style、`currentColor`を読み、全source groupの線種、外周、縮尺、Cut許可、警告を確認して新規未保存projectへ適用できる
- SVGのDTD/entity宣言、外部resource、script、animationを実行・取得せず、曲線・text・画像等をflattenしない。16 MiB、XML depth 64、5万要素、1万source/final線、64 group/外周候補/警告、100万交差候補、5,000表示線の上限を固定し、X/T/外周接点だけをexactに分割する
- SVG bytesはRust側の最新1世代stageだけに保持し、path・実ファイル名・raw XMLをWebViewへ渡さない。scale・全mapping・外周から最終寸法と実際のCut有無を非破壊検証し、そのopaque検証IDを全入力とproject identityへ束縛する。適用時に同一bytesを再parse・再照合し、成功時だけ原子的に置換する
- SVG確認画面は実DOM試験を追加し、Tab/Shift+Tabと外部focusのtrap、IME中Escape、設定変更時の外周検証解除、失敗時dialog保持をcomponent eventで確認する
- IO-004とIO-005を実装済みへ更新
- FOLD 1/1.1/1.2のtop-level 2D `creasePattern`取込をUIへ接続。nativeファイル選択後に図形プレビュー、単位または任意mm倍率、B/M/V/C/F/U/Jの割当、捨てる情報の警告を確認し、全条件を満たした場合だけ新規未保存projectとして適用できる
- FOLD取込は単一の単純境界を持つ一枚紙へ限定し、3D `foldedForm`、非ゼロZ、穴・複数紙、未知版を安全側に拒否。16 MiB、頂点1万、辺1万、境界1,414辺、交差候補100万件、包含判定100万件、表示5,000辺の上限を固定し、未知JSONを保持せず読み捨てる
- FOLD bytesはRust側の1世代stageだけに保持し、path・実ファイル名・raw JSONをWebViewへ渡さない。project instance・ID・revisionを適用直前に再照合し、成功時だけ原子的に置換する。取消・失敗・stale操作では既存projectを変更しない。対応subsetと制限を`docs/fold-import-contract.md`へ固定
- FOLD取込の独立最終監査はC0/H0/M0。frontend 776件、FOLD adapterを含む`ori-formats` 50件、Tauri desktop 99件、production build、lint、format、全target/all-feature Clippyで回帰
- IO-004を未着手から部分実装へ更新。当該checkpoint時点のMUST 86件集計を実装済み23・部分実装30・未着手33とし、SVG未実装のためIO-004を実装済みには上げなかった
- 第三者監査を現在コードへ再照合し、UI未接続基盤・研究実装・QA件数を利用者向け機能完成率へ直接加算しない方式へ是正。数式・幾何制約をUI基準の0%へ補正した暫定概算26.44%へ更新し、採用・条件付き採用・不採用の根拠を`docs/audit-assessment-2026-07-18.md`へ記録
- 監査後、単一ヒンジ補正候補の静的解析と候補別連続経路解析を4段階の増分jobへ統合し、1 RAFにつき1 stepで進めるgeneration付きcoordinatorを`FoldPreview`へ接続。作業中・対応範囲内での候補なし・判定不能・認定済みを分け、request・姿勢・選択・固定面・紙厚の変更では旧結果をstaleとして先に無効化
- 補正解析UIへ渡すのは切り離した表示DTOだけとし、exact terminal lease、motion context、binding、完全角度vector、適用tokenをReact stateへ流出させない。認定結果も解析専用で、`sceneApplied: false`・`autoApplicable: false`を維持し、候補3Dプレビューやscene・設計dataへの適用は行わない
- MUST 86件をUI利用基準で実装済み23・部分実装23・未着手40へ更新し、根拠と不足を`docs/requirements-status.md`へ固定
- 補正解析authority 5系統のregistry・deep freezeをmodule初期化時のintrinsicへ固定し、後差替えによる可変な真正token生成を遮断。terminal full-scan bindingは包含するcanonical blocked terminalの公開確定後にだけ認証し、再入時は未公開bindingを認証せず取消
- Undo/Redoは履歴entryの適用成功後だけstack間を移動し、失敗時のpattern・paper・revision・両履歴と同revision再試行可能性を保持
- 73項目の要求確認
- 要件定義書
- 技術調査・基本設計
- GitHubリポジトリと継続プッシュ
- React/TypeScript/Three.jsの初期ワークスペース
- 2Dサンプル展開図のCanvas描画と線選択
- 3D二面モデルと折り角UIの試作
- 用紙の縦横比、表裏色、紙厚を反映し、裏返しを識別できる3D二面表示
- 折り角変更時にThree.jsシーンを再生成しない差分描画とGPU資源解放
- Rustワークスペース
- 不変ID、頂点、辺、線種の最小ドメインモデル
- orientation、線分交差、共線重複、退化線分の検証実装
- 展開図全体の交差、T字、重複頂点、長さ0、不正端点の検出
- 10,000本ベンチマークパターン生成器
- Rustから10,000辺・5,184頂点の実配列をUIへ渡し、通常プロジェクトを変更せず性能テスト表示へ切り替えるベンチマーク
- UTF-8 payload、生成+転送、UI変換、索引準備、Canvas初描画、30-frame FPS、p95描画時間のアプリ内計測
- 補助線・山折り・谷折り・輪郭・切断・選択の固定層による最大10 strokeのCanvas batch描画と、grid・頂点・選択haloのbatch化
- 頂点・中点・辺スナップとCanvasクリックで共有する決定論的BVH、10,000点・線の局所検索、30,000固定seedのindexed/unindexed不一致0
- Tauriコマンド境界
- revision競合を検出する編集コマンド
- 頂点・辺の追加、移動、削除とUndo/Redo
- 頂点・辺を削除してUndoしたときのベクター順序復元と保存済み状態へのdirty解除
- 山折り・谷折り・切断線の2点作図
- 選択した頂点の数値座標編集と、頂点・線の削除UI
- 接続中頂点の誤削除防止
- バージョン付きプロジェクトJSONと往復テスト
- 紙厚、切断可否、表裏色、境界頂点を含むプロジェクト永続化
- 紙厚、境界参照、閉路、境界線種、重複頂点などの紙モデル検証
- 有限かつ正の幅・高さから、順序付き4頂点と閉じた4境界線を持つ矩形一枚紙を生成するファクトリー
- 汎用辺コマンドによる境界線の追加・削除を拒否する境界トポロジー保護
- 新規セッションを400 mm角の検証済み一枚紙から開始する初期統合
- 作品名、矩形寸法、紙厚、表裏色、切断可否を指定する新規プロジェクト画面
- project IDとrevisionを照合し、失敗時に既存状態を維持する新規プロジェクト置換
- 用紙の縦横比・位置・表色に追従する2Dキャンバス、グリッド、ヒット判定、座標入力
- 紙厚、表裏色、切断可否を1操作で更新するプロパティフォーム
- Paperを編集履歴の単一状態源とした、紙設定のUndo/Redo・dirty・保存・検証統合
- 切断線が残る状態で切断禁止へ変更する操作の原子的な拒否
- 選択線のΔX、ΔY、長さ、角度と、Canvas上の計測ラベル表示
- 軸平行長方形の幅・高さ編集と、内外を含む全頂点の左上基準比例変換
- 長方形リサイズの完全な座標Undo/Redo、ID・辺・紙属性保持、極端値の原子的拒否
- 輪郭辺の中点分割と、新規頂点・辺・紙境界順を一操作で更新するUndo/Redo
- 逆向き・閉じ辺、極端座標、ID衝突、既存頂点との位置衝突、曖昧な辺IDを原子的に拒否する輪郭分割
- 紙境界順に従う任意多角形Canvas描画、グリッド切り抜き、輪郭頂点の範囲外移動
- 自己交差や面積0の無効な紙も実形状のまま表示し、修正操作を継続できるCanvasフォールバック
- 輪郭頂点の削除、隣接する輪郭辺の原子的統合、最小3頂点と接続関係の安全検証、完全なUndo/Redo
- グリッド・頂点・辺・中点スナップの個別切替、優先順位、距離閾値、スナップガイド表示
- 凹多角形の紙外への不可視スナップ、移動頂点自身と接続辺への自己スナップ、完全一致する重複頂点の防止
- 1万頂点の完全一致索引を構築してスナップ判定を約397msから平均約0.82msへ短縮
- 極小スケール、負座標、非有限値、候補除外、最大グリッド数を含むスナップ単体テスト16件
- 通常辺を厳密な内部点で原子的に分割し、元の辺ID・向き・線種・配列順を保つUndo/Redo
- 辺・中点スナップから通常辺または輪郭辺を分割し、孤立した重複頂点を作らない頂点配置
- 補助線の灰色短破線表示、2点作図、選択、削除、スナップ、通常辺分割
- proper X交点だけを局所検索するAABB/BVH索引と、10,000辺の索引構築＋250回問い合わせを約20msで完了する性能テスト
- 通常辺2本を同じ新規交点頂点で原子的に4辺へ分割し、1回のUndoで完全復元するRust/Tauri命令
- 頂点、交点、中点、辺、グリッドの優先順位、個別切替、凹形用紙の紙内候補選択を備えた交点スナップUI
- T字・端点・共線重複・輪郭辺・過密候補・ID曖昧性・非有限値・計算overflowを安全側に拒否する交点防御
- T字交点で既存端点を再利用し、相手側の通常辺だけを原子的に分割するRust/Tauri命令とCanvas操作
- 輪郭辺のstrict interiorへ触れる既存端点を輪郭頂点へ昇格し、PatternとPaperを1命令・1回のUndo/Redoで同時更新する輪郭T字接続
- 輪郭endpoint carrier、輪郭proper X、輪郭2辺、第三輪郭辺、曖昧な紙境界対応をUIとRustの両層で無変更拒否する防御
- T字修復、通常頂点、proper交点、中点、辺、グリッドの決定的な優先順位と、同座標別IDを含む曖昧候補の拒否
- 水平・垂直を個別切替できる明示アンカー方式の方向スナップ、追加・ドラッグ操作、アンカー参照ガイド
- 方向候補が一意な通常辺・輪郭辺の内部と一致する場合の原子的分割と、端点・複数辺・重複IDの安全な拒否
- 10,000個の無関係な頂点を保持しても10,000回の方向問い合わせを約10msで処理する`O(1)`性能回帰
- 明示した方向参照辺と選択アンカーから候補を作る平行スナップ、参照の固定・解除・破線表示、追加・ドラッグ操作
- 参照辺反転、MAX座標、巨大対角線、1 ULP差、別参照辺の丸め誤差を防御する決定的な平行候補と原子的辺分割
- 10,000要素での`O(1)`候補性能と、固定seed 100,000件で分割漏れ・線外誤分割0を確認する回帰
- 11.25°から90°のプリセットと任意角、水平・方向参照辺基準を選べる角度スナップ設定UI
- 時計回り・反時計回り候補、90°縮約、片側拒否時の再試行、全スナップ優先順位、正負射影へ接続する基準線・円弧ガイド
- raw座標からの同式再射影metadata検証、一意な通常辺・輪郭辺の原子的分割、端点・複数辺・重複IDの安全な拒否
- 10,000要素での角度候補`O(1)`性能と、固定seed 100,000件で候補欠落・分割漏れ・近接別線誤分割0を確認する回帰
- 3〜64本の通常辺を同じ新規頂点へ一括分割するCreateと、端点・孤立頂点を再利用するReuseを1命令・1回のUndo/Redoで完全復元する交点クラスタ接続
- 山折り・谷折り・補助線・切断線の混在、対象完全性、Boundary混入、ID衝突、共線重複、異なる交点、非有限値を変更前に検査するRust/Tauri防御
- 局所BVHから全クラスタ辺を収集し、3〜64本の関係をCanvasへ表示して原子的命令へ接続する交点クラスタUI
- 64本・2,016交点の展開結果再利用、65本以上・探索予算超過・重複ID・曖昧な同位置頂点を部分確定せず遮断する性能・安全上限
- 絶対座標epsilonを使わない行列式誤差境界、既存端点へのcanonical化、forward/reverse候補を含む10万件監査でfrontend/Rust候補不一致0を確認する数値回帰
- frontend単体・性能テスト515件と、X交点・通常/輪郭T字・最大64辺クラスタ編集・複数折り面解析を含むRustワークスペーステスト302件
- SHA-256、サイズ、形式バージョンを検証する`.ori2` ZIPコンテナ
- ZIP爆弾、巨大入力、重複名、暗号化、パストラバーサル、ZIP64の拒否
- `.ori2`の開く・保存・別名保存をRust側ネイティブダイアログへ接続
- 一時書込み、同期、生成コンテナ再検証、原子的置換による安全な保存
- project IDとrevisionによる非同期ファイル操作中の競合防止、内容比較によるdirty追跡、未保存終了ガード
- WebViewへ汎用ファイルシステム権限を渡さない最小権限構成
- 面、穴、切断岸、ヒンジ、安定ID、診断境界を定義した面抽出・平面トポロジー設計
- macOSのDock／OS終了要求、復旧世代、保存競合を扱う未保存データ保護設計
- 全エンティティIDのcanonical bytesと、project namespaceから導出する決定的UUIDv5 FaceId
- 誤差境界内を推測せず遮断するfiltered orientationと、開始点・向き・OSに依存しない厳密な多角形符号付き面積
- 境界のみの有効な紙から安定したFaceKey、FaceId、half-edge walk、辺所属を生成する`ori-topology`初期実装
- 1本の山折り・谷折りで弱凸紙面を二つの正面積Faceへ分割し、canonical方向基準の左右面、ヒンジ、隣接関係を生成する単一折り線トポロジー
- 山谷切替、辺方向、外周の開始点・向き、レコード順を変えてもFaceKey・FaceId・左右面が変わらない決定性と黄金値回帰
- 補助線の交差・重複・未解決端点を面構築から除外しつつ、重複ID、凹紙、非分離折り、独立内部ループ、紙外折り、切断線を構造化診断で安全側に遮断
- project ID、revision、入力内容を非同期処理の前後で照合し、古い解析結果やABA更新を3Dへ渡さないTauri面解析bridge
- Rustの面・辺所属・ヒンジ応答を再検証するfail-closed表示モデルと、実面の表・裏・側面を三角形化して実ヒンジ軸回りに0〜180度で折るThree.js表示
- BigInt厳密述語によるhalf-edge回転順、canonical walk列挙、紙外部walk固定、線分中点の紙内外判定
- 接続済み・切断なしのcellular fold graphから、平行2折りの3面2ヒンジ、X折りの4面4ヒンジを安定ID付きで抽出する公開解析経路
- 複数面・複数ヒンジのFaceKey順、左右向き、incidence、adjacency、元辺を全件照合し、不整合を描画前に遮断するfrontendモデル
- canonical FaceKeyを基準面に、接続済みhinge graphを木構造または閉路へ決定的に分類し、左右の走査方向に応じた山谷の回転符号を確定
- 木構造では`M_child = M_parent × T(p) × R(axis, angle) × T(-p)`を親から子へ伝播し、全材料面とヒンジを共通角度の3D運動へ接続
- 全ヒンジのEdgeIdが過不足なく1回ずつ現れることを検証する個別角度APIと、従来の一括角度操作との互換経路
- 木構造の各ヒンジを独立操作するスライダー・数値入力、projectId単位の角度状態隔離、シーン再構築なしの軽量`updatePose`
- 単一折りとfold graphからeffective EdgeIdを照合し、2Dの`selectedLineId`・個別角度一覧・3Dヒンジ選択を相互同期
- single・tree・cycleの各表示で選択専用materialを切り替え、WebGLシーンを再構築せずヒンジ強調だけを更新
- 性能ベンチマーク表示の選択状態を通常プロジェクトから隔離し、異なる入力間の選択漏洩を防止
- 任意の材料面から無向ヒンジ木を決定的に張り直し、逆向き走査だけ回転符号を反転する固定面reroot
- 非可換な複数回転でも新固定面の逆変換を左乗算した姿勢と一致し、個別角度のEdgeId対応を維持する数値回帰
- singleとtreeで「固定面」を選択し、projectId・FaceId照合によるstale状態排除と、左右の山谷符号を反映した3D再構築
- 選択ヒンジの子側部分木を固定面から決定的に収集し、固定面変更後は動く側を反転して返す従属面解析
- single・treeで固定面を青枠、選択ヒンジと従属面群を橙枠にし、角度変更中も姿勢行列と一緒に強調を追従
- ヒンジを面より優先し、空白・非有限座標・重複ID/対象を安全に扱うThree.js Raycaster選択契約
- 3Dヒンジクリックの選択・解除、面クリックの固定面変更、空白クリックの解除を既存2D・一覧状態へ接続
- 左ドラッグ回転、ホイール・中ドラッグ拡大縮小、右ドラッグ平行移動、1本指回転、2本指拡大縮小・平行移動を追加し、初期カメラ姿勢へ戻す操作を提供
- 6 CSS px以内の単一primary pointerだけを面・ヒンジ選択として受理し、ドラッグ、右・中ボタン、複数タッチ、cancel、範囲外解放をカメラ操作から選択へ誤接続しない入力境界
- 矢印キーの平行移動、Shift＋矢印の回転、`+`・`-`の拡大縮小、Home・`0`の視点リセットをJIS配列でも利用でき、高さ0や非有限速度ではカメラを変更しないキーボード境界
- フォーカス中の3D領域でH/Shift+Hによりヒンジを前後巡回し、F/Shift+Fで固定面を前後巡回し、Escapeでヒンジを解除する選択同期。入力順維持、特殊配列・OSキー再割当、stale model/callback、修飾キー・連打・IME、不正IDを安全側に扱い、常設2チャンネルのlive regionで同文再通知にも対応
- 3D領域と視点リセットボタンを独立したアクセシブル要素にし、重複読み上げ、見えないフォーカス枠、狭幅での衝突バッジ重なりを防ぐプレビュー構造
- 全多角形頂点の実紙厚プリズムを現在姿勢へ変換し、world-space AABBを作る衝突広域判定
- X軸sweep-and-prune、浮動小数margin、総面・頂点・隣接・候補上限により、1万面の疎な配置を候補0件で決定的に処理
- 共有ヒンジ面を除外せず`hinge_adjacent`として保持し、非隣接候補と分けて後段の狭域接触分類へ渡す安全な候補契約
- project・revision・固定面・紙厚・一括/個別角度を照合した姿勢スナップショットだけを3D診断へ表示し、古い候補件数を即座に`判定中`へ退避
- 3D左上に広域候補数と非隣接候補数を表示し、狭域判定前の候補を「衝突確定」と誤表示しない診断UI
- 描画用Float32頂点を判定へ流用せず、検証済みの倍精度多角形三角形indexだけを3D描画と衝突判定で共有する境界
- 各三角形を実紙厚の凸三角柱へ変換し、三角柱AABBと全face normal・edge cross軸のSATで広域候補の空振り、境界接触、体積貫通を分類
- 100万triangle-pair上限、剛体行列照合、ゼロ厚・近平行軸の`indeterminate`退避により、部分結果や数値推測を衝突なしとして返さない狭域契約
- authoritative SATが`touching`または`penetrating`へ確定した同一順序・同一座標の三角柱対だけから、法線、接触までの距離、許容margin内の微小gap、各4点以下のsupport、最大16点のsupport midpoint hullを導出する`triangle_prism_sat_witness_v1`
- authoritative classとの不一致、`indeterminate`、近平行軸、退化・非有限・cap不整合、過大supportを`null`へ退避し、結果をdeep freezeするwitness境界。局所分離hintは選択三角柱対だけを対象とする`autoApplicable: false`で、解析入力姿勢の位置候補やtranslationを安全停止姿勢へ自動適用しない
- authoritative狭域SATの非隣接interactionへface ID、triangle index、確定class、同じ6頂点から導出したwitnessを最大16件の`witnessSamples`として統合し、hinge、`indeterminate`、ゼロ厚を説明対象へ混入させない境界
- `penetrating`を`touching`より先に収録し、同一severity内の決定順を維持するbounded選択。eligible・attempted・unavailable・上限省略数と走査完了性を独立coverageで返し、witness導出不能や上限到達でもauthoritativeなinteraction分類を変更しない
- 木構造の連続経路で実際にblockedを返した同じ点判定から、危険検出時刻・選択角・完全な開始/目標/危険角度vector・blocker 2面の倍精度行列・全bounded witnessとcoverageを切り離してdeep freezeし、終端時刻とblockerが一致した場合だけ説明用snapshotへ固定
- 危険姿勢snapshotをproject・revision・固定面・選択ヒンジ・紙厚・context key・source pose key・runtime世代・request番号へ結合し、不一致や説明生成失敗では衝突停止自体を維持したまま説明だけを`null`へ退避。危険行列と局所分離hintは認定済み表示姿勢へ適用しない
- 停止詳細UIで危険解析角、対象三角形番号、位置候補数、走査範囲、局所法線・分離距離を表示し、内部IDを非表示のまま「選択した三角柱1組だけの非自動適用候補」であることを明示
- model・固定面・選択ヒンジ・紙厚・表示姿勢・外部要求・runtime stateを同一snapshotとして照合し、同期再入、新しい角度要求、描画姿勢不一致では旧い衝突証拠をfail-closedで破棄
- prepared狭域analyzerへ全非隣接triangle-pairを貫通後も決定順で走査するオンデマンドv2を追加し、通常の早期停止解析・連続運動中の作業量・v1 witnessを不変に維持
- 全走査coverage方程式、100万pair上限、16 witness上限、未確定・導出不能・上限省略を検証し、不完全時は件数と理由だけを返して部分witnessを非公開。状態依存Map・行列要素は公開境界で一度だけsnapshot
- request付きterminal blockの同一危険角でfull-scanを一度だけ再構成し、project・revision・request、開始/目標/危険角vector、危険姿勢key、固定/可動partition、v1 primary witnessへ結合する独立versionのnullable binding
- requestなし、10万pairのterminal上限超過、v2 unavailable、pose・blocker不一致ではfull-scanまたはbindingだけを省略し、v1 block・停止時刻・bracket・statsを不変に維持。same-body witnessは説明に保持しつつ二体並進solver適格性をfalseに固定
- 完全構築・deep freeze済みのterminal bindingだけを元model exact参照とともにprivate `WeakMap` provenanceへ登録し、exact発行object・modelの組だけをproperty非参照で受理。clone、spread、prototype wrapper、同値別model、hostile・revoked Proxy、primitive、非binding terminal要素を真正入力から除外
- exact terminal binding、blocked runner state、request evidenceを一度だけsnapshotして照合し、terminalのstart完全角度vectorへ選択角だけをrebaseする内部補正解析request。元の真正contextが持つexact model・tree・非選択角を保持し、2回目以降のrequestでもterminal model provenanceを失わない。exact context・bindingはmodule-private authorityへ隔離し、公開tokenをdetached scalar/policyだけに限定。固定長をindex読取前に検証し、current lease・scene姿勢・適用権限を未結合に固定
- terminal bindingのstart/sample pose key、全coverage式・class件数、partition、support・position generators・局所hintを再検証し、全witnessがcross-partitionの場合だけmoving subtreeの共通並進候補を導出
- 最大16制約のactive setを1〜3本、最大696組まで決定順に列挙し、KKT最小ノルムseedから認定用外向き候補を生成。必要量は上向き、各内積は下向きに囲い、射影下限がclearanceを厳密に超え、L1移動量上限が指定上限内の場合だけ返す
- 候補は非隣接pairの線形制約だけを満たす解析結果としてdeep freezeし、合法角度生成、全scene静的再判定、連続経路認定、全体constraint、共有ヒンジ、材料変形を未検証の`autoApplicable: false`に固定
- world軸・回転符号・危険角・最大角度差・共通並進・最大10万moving pointから、有限回転残差の端点と二次調波停留点を最大6件へ限定して未検証1ヒンジ角度seedを導出
- near-unit軸の正規化、単位軸の二次調波丸め境界、0.01度から0.000001度の安定な一次調波解析根、0/180度・符号反転・domain端点・overflow・hostile Proxyを回帰し、モデル束縛・合法姿勢・静的安全・連続安全・自動適用をすべてfalseに固定
- terminal bindingを一度だけsnapshotした二体並進候補を真正motion contextへ再結合し、source/blocking pose key、実reroot partition、worldヒンジ軸、moving material vertexを現在modelから再導出して照合
- 各有限回転seedを選択ヒンジだけ置換した完全角度vectorへ戻し、全非隣接triangle-pairのcomplete full scanと共有ヒンジ接触規則を再実行して、全scene静的安全が成立する候補だけをdeep freeze
- 最大6候補のfull scan・通常解析について開始前の保守上限と累積実績を100万triangle-pair以内に固定し、再ルート後の負回転符号、衝突残存、stale identity、partition・pose key偽造、hostile Proxyを含む回帰と独立監査3系統でC0/H0/M0を確認
- 静的候補の成功結果を生成元の真正motion context参照へprivate provenanceで結合し、構造clone、同値の別context、hostile・revoked Proxyをgetter非参照で拒否
- 最大6件の静的候補についてsource完全角度vectorから候補角への既存連続区間jobを順位順に実行し、最初に全経路がclearとなった候補だけを連続安全certificateへ昇格
- 1回の公開stepを現在候補1件に限定し、invalid budget、作業量・認定時刻の後退、inner例外をfail-closedに処理。旧request identityを再利用せずterminal full-scanを無効化し、cancel・再入・全件非認定・終端同一参照を不変状態機械で処理
- source/blocking/target pose key、完全角度vector、実partition、負回転符号、危険blocking開始との対照、tight cap、deep freezeをfrontend 606件と独立監査3系統C0/H0/M0で確認し、runtime request・現在scene・scene適用・自動適用はfalseを固定
- 連続経路certificateをexact contextとのprivate provenanceで照合してから、候補順位、source/target角、静的・連続検査集計だけを切り離す読み取り専用表示DTOへ投影。face ID、完全角度vector、pose key、scene/runtime命令を非公開
- badge単独で「解析上」「静的／連続経路確認済み」「現在姿勢未照合」を示し、現在有効とは限らず、この表示から3Dまたは設計dataへ適用できない制限を固定。clone、同値別context、hostile・revoked Proxy、権限情報漏洩をfrontend 612件と独立再監査C0/H0/M0で確認
- 全非隣接full-scanをcandidate・first triangle・second triangleのcursorへ分解し、AABB rejectを含むpair visitまたはwitness導出を1 work unitとして中断・再開。凍結work bounds、chunk非依存の同期互換、両phase cancel・再入・例外をfrontend 628件で回帰
- 通常narrow scanもSAT pair・witness cursorへ分解し、candidate内のpenetration早期停止、後続candidate継続、最終severity別のpenetrating優先witness順を同期`analyze()`と一致。potential 100万超の早期成功、100万ちょうどの完了、次pair未課金の上限停止を回帰
- 通常/full-scan両jobでbudget検証前から再入を遮断し、validation・SAT・hinge policy・witness中のcancel、再入、例外、cancel後throwを会計済み同一terminalへ固定。通常resultを切り離してdeep freezeし、同期factory・hinge policy・result finalizationがframe時間上限外であることをliteral flagsへ明示。frontend 646件、乱択one-shot差分250件、独立再監査2系統C0/H0/M0で確認
- 最大6件の静的補正seedをfull scan準備・走査、通常narrow準備・走査へ分割し、段階境界ごとに取消可能な`tree_single_hinge_static_correction_candidates_job_v1`へ統合。pairとwitnessの累積会計、multi-seed順序、full衝突時のnormal skip、chunk非依存、真正context/model/binding、budget検証・課金済みchild・result freeze中の再入を20件の専用回帰で確認し、成功公開後だけprovenanceを付与
- 静的候補の連続経路jobを現在候補だけの遅延生成へ変更し、`candidate_preparation`と`candidate_analysis`の明示的境界、候補切替時の同一call内非開始、候補数倍の累積上限、同期処理を示すliteral flagを公開。chunk非依存、複数候補順序、phase境界取消、budget検証・課金済みchild・候補切替・pending/certificate finalization中の再入、未公開certificateへのprovenance非付与、contextから連続certificate・requestまでのprivate registry method差し替え遮断を含むfrontend 670件で回帰
- 真正な補正解析requestだけから静的候補job、候補別連続経路job、切り離した表示DTOを順に進める複合jobと、RAF単位でそのjobを駆動するUI coordinatorを実装。全段階を完走しても候補経路が未確定なら`indeterminate`、認定可能な候補を全件否定できた場合だけ`no_candidate`とし、対応範囲外を「折り不可能」と断定しない。frontend 692件・Rust 304件で回帰
- `test:snap`の47ファイル手動列挙をNode test runnerの引用符付きglobへ置換し、新しい`*.test.ts`をNode 24 CIとWindowsのどちらでも自動検出する境界へ変更。既存47ファイル・frontend 692件の全実行を維持
- `FoldPreview`から背景・camera・renderer・照明・grid・紙3材質・輪郭6材質の構築、resize、冪等破棄をReact非依存scene runtimeへ分離。exact lease、原子的scene姿勢適用、OrbitControls、gesture、ヒンジ材質は元のauthority境界へ残し、grid・材質生成途中を含む自己rollback、cleanup例外継続、既存React子要素と所有外資源の非破棄を専用8件で固定。自動検出48ファイル・frontend 700件、Rust 304件で回帰
- 監査の「全catchを一括置換」は採用せず、キャンセル・stale・入力/編集拒否・ファイル権限/破損・判定不能・best-effort cleanupを除外して、グローバル例外、起動snapshot・topology・終了guard・検証・benchmark、3D初期化・姿勢適用・描画・姿勢予約・選択・camera・resizeの上位境界だけを計測。`reportUnexpected(scope)`は固定15コード以外を拒否し、生の例外・メッセージ・作品名・パス・ID・座標を引数として受け取らない。件数は65で飽和する6区分、固定順、8 KiB以下のメモリ内snapshotに限定し、通信・永続化・時刻・環境情報を持たない。専用10件を加え、自動検出50ファイル・frontend 710件、Rust 304件で回帰
- frontendの上位診断境界をTauri環境だけで動くscope-only runtimeへ接続し、Rust側でも同じ15 scope・同じ`{schema, unexpected}` v1 DTOを再検証する端末内保存を追加。アプリ専用log領域の固定ファイルだけを使い、環境情報・作品情報・時刻を保存しない。8 KiB上限、bucket遷移時だけの原子的置換、Unix user-only mode、古い一時ファイルの有界清掃、破損入力のfail-closed復旧、永続化失敗後のsession circuit、非同期gateとblocking poolによる単一I/O worker、scope別65回のfrontend/native二重上限を固定した。frontend 720件・Windows Rust 317件で回帰
- Tauri版のstatusbarから診断ダイアログを開き、固定schema・固定順・8 KiB以下のcanonical JSONを共有前に読取専用表示する利用者経路を接続。nativeで生成した最新1世代のexact bytesだけをcacheし、frontendからは世代番号だけを渡してnative保存ダイアログの選択先へ原子的に保存する。表示と保存の同一性、旧世代・改変・path付き応答の拒否、cancel時無書込み、固定error、stale無効化、focus trap・復帰、背景`inert`、狭幅表示を固定し、通信・自動送信・自動clipboard・任意path IPCを持たない。frontend 732件・Windows Rust 321件（診断17件を含む）、format・clippyで回帰。Windows Application Controlが遮断したのはtestを持たないdesktop binary targetの起動だけで、実test失敗は0件
- 各project頂点について、Mountain/Valleyだけを用いた川崎条件と前川条件を同じ検証操作で計算する局所平坦折り検証を追加。紙境界、Cut接続、折り線なし、構造遮断、厳密計算上限を、成立・不成立と混同しない固定状態で返す
- 川崎条件は保存binary64座標を共通`2^-1074`単位の`BigInt`へ変換してからrayを作り、反時計回りrotation上のbalanced complex productでepsilonなしに判定。前川条件は`|M-V|=2`を整数で判定し、次数256までは厳密計算、超過時も前川違反を優先して示す
- 検証結果は幾何検証の`is_valid/issues`と分離した同一project/revision応答とし、全頂点をcanonical ID順で返す。UIはIPC応答のmodel、固定field、全頂点集合、件数、次数、理由、両条件の整合を線形時間で再検証し、不成立を赤実線、判定不能を黄破線でCanvasへ最大2 batch表示する。問題一覧は20件へ制限し、選択頂点の両条件と理由を表示する
- 局所条件の成立を局所十分性、展開図全体の平坦折り可能性、厚さ付き紙の折りやすさ、実際の折り経路とは表示しない。frontend 746件・Windows Rust 340件、production build、lint、format、clippyで回帰
- 3Dへ実際に適用された平面・単一ヒンジ・木構造の完全姿勢を、固定面と全hinge角を含む手動stepとしてタイムラインへ登録。タイトル、説明、注意事項、表示時間の編集、削除、並べ替え、現在姿勢での更新をUIへ接続
- 折り手順を通常編集と同じrevision・Undo/Redo・dirty判定へ統合し、`.ori2`へ必須機能`instruction_timeline_v1`付きで保存。旧v1は空タイムラインとして読み、未対応の旧アプリが手順を無視して上書きしない互換境界を追加
- 展開図・紙厚・切断可否からRust側で正規化SHA-256指紋を生成し、古い展開図用のstepを編集可能なstale記録として保持しつつ再生を遮断。current poseは平面または接続木、固定面、全hinge完全一致を再検証し、非同期解析中の同一ID再読込ABAも非永続instance IDで遮断
- 段階再生は各stepの完全姿勢を3Dへ適用し、実描画姿勢の一致を確認してから次へ進む。project、revision、fold model、手動3D操作、性能表示、ファイル操作、非表示、適用失敗、30秒timeoutで停止し、連続経路安全を保証しないことをUIへ明示
- 折り手順checkpointを独立監査し、木構造再生が適用前の同値snapshot再発行を失敗と誤認する問題、停止・失敗理由が読み上げ専用で画面に見えない問題、同一document再読込中に遅延file dialog結果が別instanceへ届く問題を修正
- 最大10万角度recordのタイムライン全体を各Undoへ保持せず、追加ID、旧metadata、旧pose、削除step、旧indexだけを差分保存する履歴へ変更。全command共通で最新128件に制限し、最大timelineで144回metadata更新しても履歴へhinge vectorを複製しないことを回帰
- 折り手順の最終独立監査はC0/H0/M0。frontend 763件、Windows Rust 384件、production build、lint、format、check、clippyで回帰
- EdgeId、幾何学的左右面、共有辺両端のVertexId・座標、`centered_mid_surface_v1`を一対一で照合し、不完全・偽造・同向き境界を準備時に遮断する共有ヒンジ契約
- 共有辺に接する左右三角形が展開時の支持線を挟むことを検証し、有限軸区間と`R=(t/2)/cos(θ/2)`の中央面基準モデルにより、0度の境界接触と60・90度を含む通常角の厚さ由来重なりを許容分類。`R`が有限ヒンジ長を超える深角と正厚180度は無制限corridorへ広げず`layer_offset_unmodeled`で停止
- 全triangle-pair走査、候補単位の走査件数照合、現在姿勢と実紙厚からの三角柱6頂点再構成により、早期終了・重複・偽造witnessを許容認定へ流さない接触ポリシー
- 厚さ0を独立した面交差次元で分類し、一点・共有辺を接触、共面正面積と両面内部を横断する正長線分を貫通とする。通常の共有ヒンジ境界接触と厚さ0の180度平坦積層だけを明示許容し、非隣接面の平坦重なりは貫通を維持
- 0・±90・±180度の回転を診断・単一折り描画・木構造姿勢で同じcanonical Matrix4へ統一し、近傍角はsnapしない。400×400 mm、非原点の斜めV実寸fixtureそのものへA/Bと厚さ3種×角度4種×3表を固定し、Aは非隣接接触1・許容ヒンジ境界2・不確定0としてUIまで回帰
- sub-marginの正面積・正長交差、近平行面間隔、点併合、巨大座標は接触へ格下げせず判定不能へ退避し、同じrest座標の共有点・共有辺がworld位置でも一致して内部横断しない場合だけtopological contactを証明
- 3D診断を非隣接とヒンジ外の貫通・接触、モデル許容境界接触・領域内重なり、ヒンジ未解決、数値・方針不確定へ分離し、単一折りでは連続経路結果を別表示し、複数ヒンジでは連続運動未検証を明示
- 非隣接またはヒンジ外の貫通面を赤、接触面を紫、数値・方針不確定面を黄の3D輪郭で表示し、モデル許容接触は衝突色へ昇格しない
- 判定用の実入力紙厚と視認用に上下限調整した3D表示厚を分離し、欠損・非有限・負数・overflow時は表示既定値で衝突なしと推測せず判定不能へ退避
- 高頻度な一括/個別角度入力を次の描画フレームの最新1件へ統合し、姿勢伝播・狭域SAT・WebGL描画の同一フレーム内重複を防止
- フレーム待機中の値差替え、実行中の次フレーム予約、破棄時キャンセル、scheduler/実行例外境界を純粋ロジックで回帰
- 面座標・隣接・倍精度三角形分割をモデル単位でdeep snapshot化し、角度フレームでは姿勢・実紙厚・world AABB・SATだけを再計算するprepared衝突解析
- prepared準備を10万頂点で安全停止し、従来one-shot経路の候補なし・ゼロ厚に対する遅延三角形分割を維持する性能境界
- 単一折りの姿勢計算を履歴非依存の純粋関数へ集約し、検証済みヒンジ両端から回転軸を導出して描画・一点判定・連続判定を一致させる境界
- 任意の単一軸回転区間を覆う保守的swept AABBと、有限ヒンジ区間内・材料側にある三角形ペアだけを静的支持として証明する区間判定
- 点サンプルを安全証明に使わず、左側から時刻順に区間を細分化し、最後の証明済み時刻、最初の危険区間または未確定区間を返す中止・再開可能な連続運動ジョブ
- 単一折りの山折り・谷折り、左右固定面、0〜170度、180度特異点、凹形状のヒンジ外衝突、極薄紙の数値境界をfail-closedで扱う連続衝突アダプター
- 共有ヒンジの完全平面と表現可能な正負の微小角を法線外積で区別し、500種類の共有剛体変換を含む監査で誤った安全認定0を確認
- 単一折りの連続衝突ジョブを1描画フレームにつき1作業単位で進め、新しい指定では旧ジョブと予約フレームを破棄し、遅延callbackを世代照合で無効化するrunner
- 指定角と実表示角を分離し、`clear`・`blocked`・`indeterminate`を限定条件付き文言で表示して、危険または未確定時は未確認角へ進めない3D安全停止
- 現在姿勢の衝突診断と単一ヒンジの連続経路診断を別バッジ・別読み上げとして提示し、中央面基準・単一線形経路限定で実際の折り癖と層ずれが対象外であることを可視表示
- 停止・判定不能時に開始角、指定角、実表示角、経路順を保つ探索区間、対象面番号、相互作用分類、確認済み進捗を展開表示し、探索区間を衝突開始角と断定しない詳細UI
- blocked結果へ、実際に点判定が衝突を返した`blockingSampleTime`を必須保持し、常に`unsafeBracket[1]`と同じ値から生成。開始点0、中間探索点、目標点1、作業上限端点を網羅し、欠落・undefined・NaN・区間上端不一致をrunner・表示・詳細境界で安全側に拒否
- `[0,0]`の開始姿勢未確認と`[0,u]`の開始点のみ確認を区別し、不整合なreason・停止時刻・適用角・統計・非ゼロ点bracketを安全側に表示拒否する詳細契約
- blockerと詳細snapshotを不変化し、未知の面ID・内部reason・不正なヒンジ分類を画面へ生表示しない診断境界
- `vertical_parameter_v1`として、runnerの実表示角を始点に、ヒンジ線の上ドラッグで非負の折り角を増やし、下ドラッグで減らす非物理パラメータ操作
- ヒンジを画面へ投影し、可視深度、最小表示長、ポインター距離を再検証してからpointer captureし、capture phaseでOrbitControlsより先に操作を裁定
- ドラッグ移動中は`unverified_target`だけを表示し、3D姿勢やrunnerへ毎フレーム適用せず、pointerup時に目標角を1回だけ既存の連続検証経路へ渡す入力境界
- 複数pointer、横優勢gesture、cancel、capture喪失、blur、resize、範囲外、非有限入力、revision・固定面・紙厚の変更をfail-closedで取消し、カメラ状態を復元
- ドラッグ目標をrendererへ直接渡さず、連続運動runnerの`applyAngle(certifiedAngle)`だけが3D表示角を更新する安全境界
- `physical_grab_v2`として、単一折りの移動面上で掴んだ表裏の3D点をヒンジ軸回りの円軌道へ変換し、ポインター半直線から0〜180度の未確認目標角を逆算する物理把持契約
- Three.jsの面ヒットをworld座標とobject-local座標の不変snapshotへ分離し、移動面・表裏material・表示厚cap・履歴非依存の正規姿勢との一致を再検証して、固定面・側面・古い姿勢を開始点にしない境界
- 三角関数二次式の閉区間root列挙によりrayと回転軌道の距離停留点を解析的に求め、旧等間隔探索の最初の区間内に隠れる解、0/180度、side-onの複数解、接線、45度超branch jump、巨大座標を固定作業量で回帰
- mouse/pen 6 CSS px・touch 10 CSS pxの成立閾値、coalesced sampleの順次処理、拒否sampleでの古い目標消去、pointerup rayだけによる最終再計算を備え、move中の目標を完了値へ流用しない物理把持gesture
- 1 DOM eventあたりcurrentを含む最大32 pointer sampleへ作業量を固定し、重複currentを1回へ縮約して、上限超過時は角度要求を送らず全pointer終了まで抑止する入力境界
- 開始時のrunner state、camera行列・投影・注視点・viewport、project/revision/固定面/紙厚contextを全sampleで照合し、pointer capture、OrbitControls抑止、複数pointer、cancel、capture喪失、blur、resize、範囲外を安全側に調停
- 0/180度、side-on、clip外、画面上の把持半径8 CSS pxと1度移動量0.2 CSS pxの開始境界、およびcamera・注視点・viewport・指定角guardを独立表示契約として回帰
- 紙面ドラッグ中は物理目標角と認定済み表示角を分けて表示し、pointerup時に解けた最終目標だけを既存の連続検証runnerへ1回要求する3D UI統合
- 木構造を任意固定面へ再root化し、選択ヒンジ以外の完全角度vectorを保持したまま、子孫の任意面上の表裏capを選択軸回りの単一円軌道へ変換する物理把持準備
- 非可換な親子ヒンジ、選択角0/60/180度、山谷、固定面反転、現在world姿勢、表示hingeとkinematics hingeの形状・面接続・符号一致をfail-closedで検証するtree把持境界
- 180度などで複数の従属面が重なっても、全体最前面と同一深度の優先集合だけから決定的に面を選び、手前の固定面を透過しないsurface picker
- 物理ドラッグの副作用を純粋な順序付きcommandへ抽出し、stale guard、複数pointer抑止、malformed terminal、表示更新、cleanup、最終角度1要求の順序をDOM非依存で回帰
- 木構造の選択1ヒンジだけを動かし、他ヒンジの完全角度vectorを固定したまま、全材料面の点・区間衝突を時刻順に検証する連続衝突アダプター
- project、revision、固定面、選択ヒンジ、紙厚、完全角度vectorを不変contextへ束ね、direct更新・runner・terminal commitを世代とopaque tokenで調停するmotion owner
- 完了値の全fieldを1回だけsnapshot化し、pointer・hinge・context・solver結果の一致を再検証して、偽造完了から角度要求を生成しない物理把持coordinator
- 木構造の全face・hinge行列を登録して完全姿勢を事前検証し、欠損、余剰、alias、Proxy、不正行列では一切変更しない原子的3D姿勢適用
- 木構造の選択ヒンジから先にある従属面の表裏を3Dで掴み、未確認目標をpointerup時に1回だけ連続衝突runnerへ渡す物理ドラッグUI
- 他ヒンジの完全角度vectorを固定したまま認定済み姿勢だけを原子的に表示し、`clear`・`blocked`・`indeterminate`の終端安全角を対象ヒンジへ一度だけ確定
- 選択、外部角度、固定面、紙厚、callback権限の変更と古いRAFをbinding・世代・opaque tokenで遮断し、direct更新とrunner更新の競合を安全側に調停
- 閉路を含むhinge graphは拘束を推測せず、全材料面とヒンジの静的3D表示へ退避して角度操作を無効化
- 閉路または平面確認のみのプレビューでは個別角度UIを無効化し、未対応の運動を安全側に遮断
- 非可換な連続回転、親子ヒンジ端点の一致、逆向き走査の符号、履歴非依存、兄弟分岐の独立性をfrontend回帰で検証
- WebGL初期化・初回描画・リサイズ・角度更新の失敗時に、部分生成済みGPU資源を冪等に解放して3D欄だけを安全停止する例外境界
- `.ori2`の実パス読込・別名保存・再上書き・失敗時の状態維持・revision競合を検証するネイティブファイル試験
- 全OS共通のRAIIステージ清掃、macOS等のPOSIX mode維持と親ディレクトリ同期、Windowsの検証済みハンドル直接置換
- Tauri内のRustプロジェクト状態とUI接続
- Rust幾何検証ボタン、問題一覧、該当要素へのジャンプ
- Windows/macOS用の8-bit RGBA/ICO/ICNSアプリアイコン
- Windows/macOS Rust CIとmacOS `.app` bundle検証
- コミット`f252269`に対するCI #40の全ジョブ完走
- コミット`00eeb6b`に対するCI #42の全ジョブ完走
- コミット`9547b6b`に対するCI #46の全ジョブ完走
- コミット`308324a`に対するCI #48の全ジョブ完走
- コミット`daa7f6c`に対するCI #50の全ジョブ完走
- コミット`52df309`に対するCI #52の全ジョブ完走
- コミット`1e9b00a`に対するCI #53の全ジョブ完走
- コミット`bf3dc9e`に対するCI #55の全ジョブ完走
- コミット`6c6a93c`に対するCI #56の全ジョブ完走
- コミット`e93123a`に対するCI #57の全ジョブ完走
- コミット`20ec83a`に対するCI #58の全ジョブ完走
- コミット`19c3f8e`に対するCI #59の全ジョブ完走
- コミット`9ec528a`に対するCI #60の全ジョブ完走（T字接続、macOS `.app` bundleを含む）
- コミット`def7bba`に対するCI #61の全ジョブ完走
- コミット`bc9512d`に対するCI #62の全ジョブ完走（水平・垂直スナップ、macOS `.app` bundleを含む）
- コミット`fd949e3`を包含する`0b9be2a`のCI Run `29524716054`全ジョブ完走（平行スナップ、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`b773beb`のCI Run `29526140372`全ジョブ完走（角度スナップ、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`c1fe325`のCI Run `29527752631`全ジョブ完走（輪郭T字接続、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`5ee319f`のCI Run `29533981487`全ジョブ完走（3〜64辺の交点クラスタ、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`1f572c5`のCI Run `29535587215`全ジョブ完走（1万辺実データ転送、30-frame Canvas計測、batch描画、共有BVH、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`64c64cd`のCI Run `29539950477`全ジョブ完走（厳密面積、安定Face ID、境界面抽出、全OS原子的保存、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`45c4cf6`のCI Run `29540654513`全ジョブ完走（厳密orientation公開、1万辺面索引、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`498a0f9`のCI Run `29541363894`全ジョブ完走（単一折り線の二面・ヒンジ抽出、決定性回帰、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`ee4fddb`のCI Run `29541905981`全ジョブ完走（revision付き面解析bridge、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`5d63e64`のCI Run `29542900323`全ジョブ完走（実面・実ヒンジ3D表示、frontend 137件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`836495d`のCI Run `29547501733`全ジョブ完走（複数折りcellular face抽出基盤、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`220aa0d`のCI Run `29551761659`全ジョブ完走（1万面の広域衝突候補、frontend 168件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`d699752`のCI Run `29552530594`全ジョブ完走（倍精度三角柱SAT、frontend 182件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`5480133`のCI Run `29561917824`全ジョブ完走（単一折りの経路安全停止詳細、frontend 300件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`231be40`のCI Run `29564332302`全ジョブ完走（単一折りの安全な上下ドラッグ、複合pointer調停、frontend 328件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`33bcf3a`のCI Run `29567031733`全ジョブ完走（物理把持gesture、frontend 392件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`3332d9a`のCI Run `29567919388`全ジョブ完走（単一折りの紙面物理ドラッグ、frontend 401件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`5a3c389`のCI Run `29569214530`全ジョブ完走（物理把持の表示・入力境界強化、frontend 410件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`fd87178`のCI Run `29570027330`全ジョブ完走（木構造の選択ヒンジ物理把持準備、frontend 419件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`dd4b108`のCI Run `29570720185`全ジョブ完走（物理把持副作用coordinatorと重なり従属面picker、frontend 429件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`747ef88`のCI Run `29576382613`全ジョブ完走（木構造の連続解析、motion context・owner、原子的3D姿勢適用、frontend 485件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`82bab3f`のCI Run `29580248077`全ジョブ完走（木構造の選択1ヒンジ物理ドラッグUI、frontend 499件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`48a1627`のCI Run `29581596055`全ジョブ完走（3Dヒンジ・固定面キーボード巡回とアクセシブル通知、frontend 515件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`bd815d0`のCI Run `29582254012`全ジョブ完走（連続経路の危険検出時刻契約、frontend 515件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`c5b6921`のCI Run `29585500562`全ジョブ完走（bounded SAT collision witness seed、frontend 532件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`1ab15be`のCI Run `29587413353`全ジョブ完走（authoritative狭域SATへのbounded witness・coverage統合、frontend 542件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`6665f40`のCI Run `29589287905`全ジョブ完走（危険姿勢・request identity・bounded witnessの結合、frontend 546件、決定論的2,500ジョブ差分、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`01bfd3e`のCI Run `29605119396`全ジョブ完走（衝突根拠の停止詳細UI、stale・再入・表示姿勢guard、frontend 553件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`7de8897`のCI Run `29606086440`全ジョブ完走（全非隣接pairのfull-scan v2、hostile pose snapshot、frontend 561件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`59e81ef`のCI Run `29607122375`全ジョブ完走（terminal full-scan binding、same-body・unavailable・独立cap回帰、frontend 564件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`a5921b4`のCI Run `29609027499`全ジョブ完走（認定付き二体補正候補、有向丸め数値反例・偽造binding・hostile Proxy回帰、frontend 575件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`a2fe970`のCI Run `29610764873`全ジョブ完走（未検証1ヒンジ有限回転seed、near-unit軸・極小角・二次調波・hostile Proxy回帰、frontend 588件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`63a3846`のCI Run `29612236508`全ジョブ完走（モデル束縛済み1ヒンジ静的補正候補、負回転符号・stale identity・偽造partition・hostile Proxy回帰、frontend 597件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`ca74db2`のCI Run `29614029539`全ジョブ完走（静的候補への連続経路認定、真正provenance・危険開始対照・増分取消・作業上限回帰、frontend 606件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`a402f7d`のCI Run `29615389465`全ジョブ完走（連続経路certificateの真正性照合・読み取り専用表示DTO・非適用文言、frontend 612件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`227b649`のCI Run `29615862477`全ジョブ完走（terminal full-scan bindingのexact-object真正性guard、clone・wrapper・hostile Proxy拒否、frontend 612件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`d179c73`のCI Run `29617603356`全ジョブ完走（full-scanのpair/witness増分job、cancel・再入・例外・同期互換回帰、frontend 620件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`1bcc3d2`のCI Run `29617818861`全ジョブ完走（model-bound terminal provenance、private補正解析authority、固定長先行検証、frontend 628件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`22cb094`のCI Run `29619286102`全ジョブ完走（通常narrowのpair/witness増分job、早期停止・上限境界・cancel・再入・例外・同期互換回帰、frontend 646件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`013ba08`のCI Run `29623221663`全ジョブ完走（静的補正候補の段階別増分job、multi-seed・累積会計・cancel・再入・例外回帰、frontend 657件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`a2ac303`のCI Run `29623851234`全ジョブ完走（補正候補別連続経路の遅延生成、phase境界・累積会計・finalization再入回帰、frontend 665件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`3677d10`のCI Run `29625298125`全ジョブ完走（第三者監査の再照合、進捗是正、Undo/Redo失敗時の履歴保持、authority初期化強化、frontend 673件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`dc365ef`のCI Run `29626601401`全ジョブ完走（補正解析の複合job・UI coordinator・stale無効化・解析専用表示、frontend 692件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`da24cc3`のCI Run `29626823914`全ジョブ完走（frontend testの引用符付きglob自動検出、frontend 692件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`72ab520`のCI Run `29627636649`全ジョブ完走（`FoldPreview` scene runtime分離、frontend 700件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`905a2fd`のCI Run `29628436035`全ジョブ完走（privacy-safeなメモリ内redacted diagnostics境界、frontend 710件、Windows/macOS Rust、macOS `.app` bundleを含む）
- コミット`cef548e`のCI Run `29629616898`全ジョブ完走（redacted diagnosticsの端末内原子的保存、frontend 720件、Windows/macOS Rust、macOS `.app` bundleを含む）

## 進行中

- [native current applied pose設計](native-applied-pose-design.md)に従い、表示・投影から独立したnative kinematics、current pose capability、native静的・連続衝突、場所別cell-order transport、原子的commitを順に実装する作業。任意の現在3D状態で局所的に重なる層を安全に扱えるまで折り重ねUIへ着手しない
- `FoldPreview`のscene資源分離に続き、既存のexact lease・stale無効化・原子的scene更新を保ったまま残るcamera/入力runtimeを小さな責務へ分割する作業
- 単一折りの紙面ドラッグをWindows実機のmouse・pen・touchで操作し、pointer capture、カメラ競合、表裏の掴みやすさを確認するネイティブE2E
- Windows実機での`.ori2`ダイアログ、キャンセル、上書き、破損入力、保存失敗時復旧のE2E確認
- Windows実機で診断ダイアログの読取専用JSON、保存・キャンセル、保存中の重複操作防止、Escape・Tab循環・focus復帰、表示bytesとの一致を確認するネイティブE2E
- Windows配布候補で1万辺の転送・初描画・30-frame FPSを採取し、基準PCと正式な合格値を決める作業
- macOSはCI上のRust・frontend・`.app`生成検証を維持する。実機Macを必要とする操作・ダイアログ・Dock/Cmd+Q・性能E2Eは、利用可能な検証機がないため現在の作業範囲から除外する

## 配布前の既知課題

- ファイルダイアログと読書きはRust側だけで実行し、WebViewには汎用ファイルシステム権限を付与していない。この境界を今後も維持する。
- 通常のウィンドウ終了とmacOSのCmd+Q／アプリメニュー終了にはdirtyガードがある。一方、macOSのDockメニュー終了やOS終了要求はTauriランタイムの経路によってガードを迂回する可能性があるため、macOS配布完了前に実機試験と自動復旧保存またはネイティブdelegateによる保護が必要である。

## 次の作業

1. 表示・投影に依存しない決定論的tree kinematicsをnative共通crateへ抽出し、current applied poseを同一project・revision・topology・fingerprint・generationへ結合する
2. 衝突分類4×10表をnative static collisionへ移植し、続いてcurrent poseまでのcontinuous collisionと場所別cell-order transportを証明する。全180度flatは内部bootstrapに限定し、製品要件をflat限定へ縮小しない
3. 上記前提の完成後に`ApplyStackedFold`を展開図、3D姿勢、層順序、face lineage、timelineへ原子的に接続し、失敗時の全状態不変と段階再生を回帰する。UIはその後に接続する
4. MUST 87件のstatus表を各checkpointで維持し、履歴永続化・復旧、i18n、単位、レイヤーの未着手MUSTをbreadth-firstで進める
5. Windows正式版に向けて3Dキーボード選択の実機AT確認、ネイティブE2E、終了時保護を進める。macOSは自動ビルド・CI検証だけを継続する
