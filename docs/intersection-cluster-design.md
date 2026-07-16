# 輪郭交点・交点クラスタ接続設計

## 目的

既存のproper X交点と通常T字交点に続き、次を1回の操作と1回のUndo/Redoで接続する。

- 通常辺の既存端点が輪郭辺の厳密な内部点へ触れるT字
- 輪郭頂点へ通常辺が集まる交点
- 3本以上が同一点へ集まる新規交点または既存頂点交点
- 将来の輪郭辺と通常辺のproper交点

見た目だけ同じ位置へ頂点を追加せず、対象辺と`paper.boundary_vertices`を同じRust命令内で更新する。失敗時はpattern、paper、revision、dirty、Undo/Redo履歴を一切変えない。

## 段階方針

### 第1段階: sheet-aware T字

既存の`Command::ConnectTJunction`をsheet-awareに拡張する。最初の公開単位は次に限定する。

- 対象は2辺
- 輪郭辺は最大1本
- 輪郭辺がstrict-interior側
- 通常辺の一意な既存endpointを輪郭頂点として再利用
- `CreasePattern.vertices`には頂点を追加しない
- 輪郭辺と`paper.boundary_vertices`を同時に分割

既存Command fields、Tauri API、TypeScriptの`connect-t-junction`配置型を維持できるため、通常T字の回帰範囲を小さく保てる。`SplitBoundaryEdge`の逐次呼出しは、新規頂点作成と既存位置拒否を前提にし、Undoも2回になるため使用しない。

### 第2段階: 一般化クラスタ

3本以上へ進む前に、読み取り専用plannerと可変長の原子命令を追加する。既存の2辺用命令は安定するまで残し、新UIから段階的に移行する。

```rust
enum JunctionVertexIntent {
    Create { id: VertexId },
    Reuse { id: VertexId },
}

struct IntersectionEdgeTarget {
    edge: EdgeId,
    // strict interiorならSome、既にjunctionを端点に持つならNone
    new_edge: Option<EdgeId>,
}

Command::ConnectIntersectionCluster {
    junction: JunctionVertexIntent,
    targets: Vec<IntersectionEdgeTarget>,
}
```

## 第1段階のRust仕様

`ConnectTJunction`のfieldsは変更しない。

```rust
Command::ConnectTJunction {
    first_edge: EdgeId,
    second_edge: EdgeId,
    new_edge: EdgeId,
}
```

幾何判定からendpoint側の頂点とstrict-interior側の辺をRust側で一意に決める。全検証を完了してから配列を変更する。

1. 対象IDが異なり、各IDがpattern内で一意
2. `new_edge`が未使用
3. 全endpoint頂点レコードが存在し一意
4. endpoint座標が有限
5. strict T候補がちょうど1つ
6. junction位置を別IDの頂点が占有していない
7. 輪郭辺が0本または1本
8. 輪郭辺がstrict-interior側
9. 輪郭辺が`paper.boundary_vertices`の連続する無向頂点対へちょうど1回一致
10. junction IDが現在の`boundary_vertices`にない
11. junctionに別の輪郭辺が接続していない

グローバルなpattern/paper validを前提にしない。T字自体が修復対象のinvalid状態であり、無関係な未修復交点が残る文書も局所的に直せる必要がある。

### 配列更新

元の輪郭辺が`original.start -> original.end`、junctionが`J`の場合:

```text
edges[i]     = original.start -> J  // 元ID、kind、indexを維持
edges[i + 1] = J -> original.end    // new_edge、kind=Boundary
```

紙境界の一意な連続ペアが`A, B`なら次へ更新する。

```text
... A, B ... -> ... A, J, B ...
```

- 閉じ辺なら`J`を配列末尾へ追加する。
- patternの輪郭辺が紙順と逆向きでも、紙側は紙順、辺側は元の向きを維持する。
- pattern verticesは不変、edgesは1本増加、boundary verticesは1点増加する。

### Inverse

既存`RestoreTJunction`へ変更前の紙境界を追加する。

```rust
RestoreTJunction {
    original_edge_index: usize,
    original_edge: Edge,
    new_edge_index: usize,
    new_edge: Edge,
    boundary_vertices: Option<Vec<VertexId>>,
    changed_vertices: [VertexId; 4],
    changed_edges: [EdgeId; 3],
}
```

- 通常T字は`None`、輪郭T字は変更前ベクタ全体を`Some`で保持する。
- Undoは新辺除去、元辺復元、紙境界ベクタ復元を行う。
- Redoは同じforward commandとIDで完全再現する。
- `settings_changed`は輪郭T字だけ`true`にする。

### APIとUI

Tauri応答と配置型は既存のまま使う。

```ts
type Placement = {
  operation: 'connect-t-junction'
  firstEdgeId: string
  secondEdgeId: string
  junctionVertexId: string
}
```

- 成功応答の`vertex_id`は新辺のstartから既存junction IDを返す。
- 既存頂点クリック時のT字修復経路を維持する。
- proper boundary X、輪郭辺2本、輪郭辺がendpoint側のTは第1段階ではinvokeしない。
- ガイドは`輪郭T字`と表示し、成功後はjunction頂点を選択する。

## 一般化クラスタplanner

`plan_intersection_cluster`は変更前状態だけを読み、次を検証・計画する。

### 対象集合

- 2本以上、対象ID重複なし、各IDが一意
- endpoint頂点レコードが存在し一意、有限、長さ0でない
- API順ではなく元の`pattern.edges`順を記録
- 少なくとも1本はstrict-interior分割
- 全対象辺が同じjunctionを端点またはstrict interiorとして含む
- 共線重複、near miss、非有限、overflowは拒否

### 頂点

- junction位置を占有する頂点が0個なら`Create`だけ許可
- 1個なら同じIDの`Reuse`だけ許可
- 同座標別IDや同じIDの複数レコードは拒否
- 既存endpointまたは一意な孤立頂点がある場合は新規頂点を作らない
- `Create`では全対象辺が内部分割になる

### 完全性

Rustはクリック確定時に全辺を`O(E)`で再走査し、junctionを通る全有効辺が対象集合へ含まれることを確認する。1本でも省略されていれば部分接続せず`IncompleteIntersectionCluster`として拒否する。

フロントのカーソル問い合わせは局所BVHだけを使い、全辺走査は行わない。

## 輪郭トポロジー

- strict interiorで分割する輪郭辺は1本まで
- 輪郭辺IDとpaperの連続無向頂点対がちょうど1か所一致
- junction IDをpaper境界へ重複登録しない
- 元輪郭辺のID、向き、kind、indexを維持
- 後半輪郭辺を元辺直後へ挿入
- 輪郭頂点で接続する場合、隣接する2輪郭辺は端点対象として含めるがpaper配列は変更しない
- 輪郭辺2本のstrict interiorが同一点へ集まる自己交差は、同じIDをpaperへ2回置く必要があるため別の輪郭再構築操作へ分離する

## 配列順とUndo/Redo

- 新規頂点はvertices末尾
- 元辺は元位置とIDを維持
- 後半辺は各元辺直後
- 複数挿入は元index降順で適用
- 生成IDは対象edge IDと明示対応し、引数順に依存しない
- changed vertices/edgesは文書順で安定重複除去
- boundary変更時だけsettings changed

```rust
RestoreIntersectionCluster {
    original_boundary_vertices: Option<Vec<VertexId>>,
    original_edges: Vec<(usize, Edge)>,
    inserted_edges: Vec<(usize, Edge)>,
    created_vertex: Option<(usize, Vertex)>,
    junction_vertex: VertexId,
    changed_vertices: Vec<VertexId>,
    changed_edges: Vec<EdgeId>,
}
```

Undoは挿入辺を逆順に除去し、元辺、作成頂点、紙境界を完全復元する。Reuse頂点は削除しない。

## 切断線

- cutting allowed時はCutも対象にでき、分割後の両半分でkindを維持する。
- Cutを`paper.boundary_vertices`へ入れない。これは元の一枚紙の外周だけを表す。
- cutting disabledなのにCutが残る読込invalid状態をどう修復するかは、既存SplitEdge/proper/Tと同じ規則へ統一してからガードする。
- 新命令だけ異なる切断規則にしない。

## フロントエンドクラスタ型

```ts
type ClusterIntersectionTarget = {
  kind: 'intersection'
  classification: 'cluster'
  key: string
  point: SnapPoint
  distancePx: number
  sourceEdges: readonly {
    id: string
    fraction: number
    relation: 'interior' | 'endpoint'
    boundary: boolean
  }[]
  junctionVertexId?: string
}
```

- source edgesはID順でcanonical化する。
- keyは全edge IDから決定的に作る。
- 輪郭を含む場合、または3本以上ならcluster型にする。
- 最良seed pairを得た後、同一点を含む局所辺をBVHから集めてclusterへ拡張する。
- boundary-boundary pairはseedにしない。
- 上限到達時は部分候補を返さず、過密案内を表示する。

優先順位は次とする。

1. 既存頂点を使う未接続cluster
2. 通常頂点
3. 新規cluster/proper交点
4. 中点
5. 水平・垂直
6. 平行
7. 角度
8. 辺
9. グリッド

## 検証項目

### 第1段階

- 輪郭T字の引数順、輪郭辺の両向き、紙のCW/CCW、閉じ辺
- 対象辺の文書順が逆、途中に無関係辺がある場合の配列順
- vertices不変、edges +1、boundary vertices +1
- 実行・Undo・Redoでpattern/paper完全一致
- `settings_changed == true`
- 通常T字の既存全回帰
- 重複edge/vertex ID、同位置別ID、boundary内のjunction ID
- paper境界ペア欠落・複数、junctionへ別Boundary辺が接続
- 輪郭辺2本、輪郭辺endpoint側、proper boundary Xの安全な拒否
- 全拒否ケースでrevision・履歴を含む状態不変
- Tauri応答が既存junction IDを返す

### 一般化段階

- 輪郭proper、輪郭頂点、3本以上のproper/T混在、孤立頂点Reuse
- 山・谷・補助・切断のkind保持
- 引数全順列で同じ結果
- 対象省略・余分・誤分類・生成ID衝突
- 共線重複、異なる交点、near miss、MAX/overflow
- 1回Undo/Redoの完全再現
- 10,000本疎データで局所問い合わせを維持

## 実装順

1. 既存`ConnectTJunction`のsheet-aware輪郭T字
2. Tauri・Canvas経路の回帰と両OS CI
3. geometryの厳密point-on-segment分類
4. Rustのcluster planner、Command、Inverse
5. Tauri応答とcoreClient（UI未接続）
6. 輪郭proper交点
7. frontend indexのcluster展開と3本以上のUI
8. 既存2辺proper/Tをcluster命令へ移行
9. Cut規則統一、ファズ、10,000本性能、実Canvas確認
