# ORIGAMI2 開発進捗

## 完成率

**全体完成率: 約38.6%（2026-07-18）**

完成率は画面数ではなく、要件定義書のMUST 86件、FUTURE 14件、品質検証、両OS配布を含む総工数の概算である。研究要素の結果によって見積もりを更新する。

下表の「全体への寄与」は「全体比率 × 現在の領域進捗」であり、合計38.60%を小数第1位へ丸めて全体完成率としている。

## 重み付け

| 領域 | 全体比率 | 現在の領域進捗 | 全体への寄与 | 状態 |
|---|---:|---:|---:|---|
| 要件・基本設計・技術検証 | 5% | 70% | 3.50% | 固定面契約に加え、ヒンジ優先・面フォールバック・不正入力遮断を持つ3D picking境界を実装検証 |
| プロジェクト・保存・履歴 | 8% | 60% | 4.80% | 原子的編集とUndo/Redo、ネイティブ`.ori2`実ファイル操作、全OSの原子的上書きと失敗時保護を実装 |
| 2D展開図エディター | 15% | 52% | 7.80% | 基本編集、任意多角形用紙、9種のスナップに加え、2D・角度一覧・3Dヒンジの選択同期を実装 |
| 数式・幾何制約 | 9% | 59% | 5.31% | 共通並進を有限回転の二次調波最小二乗へ写像し、最大6件の未検証1ヒンジ角度seedを導出 |
| 3D折り・紙厚・衝突 | 17% | 52% | 8.84% | 合法な1ヒンジ補正姿勢を非隣接全走査と共有ヒンジ規則で静的再検証 |
| 折り可能性・経路探索 | 18% | 20% | 3.60% | 真正blocked終端から補正解析requestを再構成し、full/通常narrowのpair・witness走査を中断可能化 |
| 折り手順・PDF | 10% | 1% | 0.10% | タイムラインUI試作のみ |
| 入出力・互換性 | 5% | 16% | 0.80% | 安全制限付き`.ori2`、実パス読込・保存・再上書き・全OSの原子的置換を実装 |
| 多言語・設定・配布・QA | 5% | 77% | 3.85% | frontend 646件、独立監査3系統、Windows/macOS Rust、macOS `.app`をCI検証 |
| 初心者向け自動設計 | 8% | 0% | 0.00% | 将来要件のみ |
| **合計** | **100%** | — | **38.60%** | — |

## 完了

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
- exact terminal binding、blocked runner state、request evidenceを一度だけsnapshotして照合し、terminalのstart完全角度vectorからfresh motion contextを再生成する内部補正解析request。exact context・bindingはmodule-private authorityへ隔離し、公開tokenをdetached scalar/policyだけに限定。固定長をindex読取前に検証し、current lease・scene姿勢・適用権限を未結合に固定。frontend 628件と独立再監査C0/H0/M0で確認
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
- EdgeId、幾何学的左右面、共有辺両端のVertexId・座標、`centered_mid_surface_v1`を一対一で照合し、不完全・偽造・同向き境界を準備時に遮断する共有ヒンジ契約
- 共有辺に接する左右三角形が展開時の支持線を挟むことを検証し、有限軸区間と`R=(t/2)/cos(θ/2)`の中央面基準モデルにより、0度の境界接触と60・90度を含む通常角の厚さ由来重なりを許容分類
- 全triangle-pair走査、候補単位の走査件数照合、現在姿勢と実紙厚からの三角柱6頂点再構成により、早期終了・重複・偽造witnessを許容認定へ流さない接触ポリシー
- ゼロ厚、180度特異点、複数共有ヒンジ、姿勢不一致、許容境界、非ヒンジ三角形の不確定を明示し、許容領域外を証明できた貫通・接触だけを遮断候補へ昇格
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

## 進行中

- full/通常narrowの同期factory前処理・hinge policy・result finalizationをさらに増分化し、static候補生成と連続経路certificateをstale contextで取消しながら実行して、読み取り専用表示DTOを現在request lease内だけで提示するcoordinator
- 単一折りの紙面ドラッグをWindows/macOS実機のmouse・pen・touchで操作し、pointer capture、カメラ競合、表裏の掴みやすさを確認するネイティブE2E
- Windows/macOS実機での`.ori2`ダイアログ、キャンセル、上書き、破損入力、保存失敗時復旧のE2E確認
- macOSのDockメニュー終了・OS終了要求でも未保存データを保護する方式の検証
- Windows/macOS配布候補で1万辺の転送・初描画・30-frame FPSを採取し、基準PCと正式な合格値を決める作業

## 配布前の既知課題

- ファイルダイアログと読書きはRust側だけで実行し、WebViewには汎用ファイルシステム権限を付与していない。この境界を今後も維持する。
- 通常のウィンドウ終了とmacOSのCmd+Q／アプリメニュー終了にはdirtyガードがある。一方、macOSのDockメニュー終了やOS終了要求はTauriランタイムの経路によってガードを迂回する可能性があるため、macOS配布完了前に実機試験と自動復旧保存またはネイティブdelegateによる保護が必要である。

## 次の作業

1. OQ-002の物理的な厚さoffset・層ずれ規則を確定し、中央面基準の近似分類と選択可能にする
2. 補正解析requestをstatic候補の増分jobと現在request lease coordinatorへ接続し、stale・作業中・候補なしを明示する
3. 三角柱の局所形状再利用、広域・狭域の差分更新、worker分離により、大規模な面・ヒンジの判定を最適化する
4. 閉路拘束の診断と将来ソルバー境界を詳細化する
5. 3Dキーボード選択の実機AT確認、ネイティブE2E、終了時保護を進める
