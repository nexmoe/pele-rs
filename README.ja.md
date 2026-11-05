# pele-rs

[English](README.md) | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md) | **日本語**

[fermaid](https://github.com/tenyi/fermaid) から派生した独立プロジェクトで、ネイティブエディター向けに最適化しています。元の Git 履歴、著作者、ライセンスを保持しています。Obsidian の公式プロジェクトではありません。現在のバージョンは Git からインストールしてください。

```toml
[dependencies]
pele-rs = { git = "https://github.com/nexmoe/pele-rs" }
```

描画の参照先は **Pele 1.13.0** に固定しています。Rust テストで **178,887 件の公式描画呼び出し**と **19,308 件のユーティリティ／レイアウト呼び出し**を再現し、SVG の構造、スタイル、座標を比較します。SVG クラスと CSS 変数は公式の `pele-*` / `--pele-*` を使用します。許容誤差、除外項目、ブラウザー動作との境界、再現手順は[互換性検証](docs/CONFORMANCE.md)を参照してください。

全図種で再利用できる `parse` / `ParsedDiagram::render`、上限付き `CachingMeasurer`、元ソースへのエラー位置の対応、配色設定の互換性、数値検証を追加しました。オプションの `scene` 機能は usvg による `SvgScene` と resvg に一致する `FontMeasurer` を提供します。既定のビルドには実行時依存関係がありません。[ネイティブ統合ガイド](docs/NATIVE.md)を参照してください。crates.io にはまだ公開していません。


pele-rs は [Mermaid](https://mermaid.js.org) のダイアグラムを純粋な Rust で SVG に変換します。JavaScript ランタイム、ヘッドレスブラウザ、Node.js は不要です。小さく依存関係のない TypeScript 製レンダラー [pele](https://github.com/obsidianmd/pele) の移植で、Mermaid 12.2.0 の構文を対象とし、pele 独自のレイアウトとビジュアルスタイルを採用しています。

> **ステータス:** alpha。Mermaid 12.2.0 のすべてのダイアグラムに対応しています。下の表は、それぞれが読み取る記法をまとめたものです。[ロードマップ](docs/ROADMAP.md)を参照してください。

| ダイアグラムの種類 | 状況 |
|---|---|
| 円グラフ（`pie`） | 対応済み：`showData`、`title`、`accTitle`、`accDescr`、および `pie` の設定キー `textPosition`、`donutHole`、`legendPosition` |
| フローチャート（`flowchart`、`graph`） | 対応済み：すべてのノード形状（`@{ shape: … }` を含む）、あらゆる種類と長さのエッジ、エッジ ID、独自の向きを持てるサブグラフ、`classDef`、`class`、`style`、`linkStyle`、`click` のリンクとツールチップ、Markdown ラベル、および `flowchart` の設定キー `curve`、`nodeSpacing`、`rankSpacing`、`wrappingWidth` |
| シーケンス図（`sequenceDiagram`） | 対応済み：あらゆる種類の参加者とアクター、あらゆる種類の矢印、アクティベーション、ノート、`loop`、`alt`、`opt`、`par`、`critical`、`break`、`rect` ブロック、ボックス、`autonumber`、参加者の生成と破棄、および `sequence` の設定キー。参加者メニュー（`link`、`links`）はリンクとして描かれません |
| 状態遷移図（`stateDiagram`、`stateDiagram-v2`） | 対応済み：説明付きの状態、独自の方向を持つ複合状態、並行領域、開始と終了、choice、fork と join、ノート、`classDef`、`class`、`style`、`click` |
| クラス図（`classDiagram`） | 対応済み：メンバー、ジェネリック型、アノテーションを持つクラス、マーカーと多重度付きの関係、名前空間、ノート、ロリポップインターフェース、`classDef`、`cssClass`、`style`、`link`、`click`、`callback` |
| ER 図（`erDiagram`） | 対応済み：属性、キー、コメントを持つエンティティ、カラスの足記法の多重度付きリレーションシップ、エイリアス、サブグラフ、`classDef`、`class`、`style` |
| 要求図（`requirementDiagram`） | 対応済み：あらゆる種類の要求と要素およびそのフィールド、リレーションシップ、`classDef`、`class`、`style` |
| ガントチャート（`gantt`） | 対応済み：日付・期間・`after`・`until` を持つタスク、セクション、`done`・`active`・`crit`・`milestone`・`vert`、`excludes`・`includes`・`weekend`、`dateFormat`・`axisFormat`・`tickInterval`・`weekday`・`todayMarker`・`topAxis`、コンパクトモード、`click` リンク。日付はすべて UTC で、「今日」は `RenderOptions::now` で決まります |
| タイムライン（`timeline`） | 対応済み：タイトル、セクション、期間とそのイベント |
| ユーザージャーニー（`journey`） | 対応済み：タイトル、セクション、スコアとアクターを持つタスク |
| カンバン（`kanban`） | 対応済み：列と（形状付きの）カード、`@{ … }` メタデータ（`assigned`・`ticket`・`priority`・`label`）、クラス、`ticketBaseUrl` によるチケットリンク |
| マインドマップ（`mindmap`） | 対応済み：すべてのノード形状、アイコン、クラス、枝の色（枝ごと、または `colorBy` で階層ごと）。横向きに配置し、ホストが狭いときは縦向きにします |
| サンキー図（`sankey`、`sankey-beta`） | 対応済み：Mermaid の CSV 形式。1 行につき `source,target,value` を 1 つ、フィールドは引用符で囲めます。`sankey` の設定キー `width`、`height`、`nodeWidth`、`nodePadding`、`nodeAlignment`、`nodeColors`、`linkColor`、`showValues`、`prefix`、`suffix`、`labelStyle` に対応 |
| XY チャート（`xychart`、`xychart-beta`） | 対応済み：`title`、カテゴリまたは範囲を持つ `x-axis`、範囲を持つ `y-axis`、点ラベルを持つ `line` と `bar` の系列、`accTitle` と `accDescr`、`horizontal` と `vertical` の向き。`xyChart` の設定キー `width`、`height`、`chartOrientation`、`showTitle`、`showLegend`、`showDataLabel`、`xAxis.showTitle`、`xAxis.showTick`、`xAxis.showLabel`、`xAxis.showAxisLine`、および `yAxis` 側の同じ 4 つのキーに対応 |
| クアドラントチャート（`quadrantChart`） | 対応済み：`title`、`-->` を持つ `x-axis` と `y-axis`、`quadrant-1` から `quadrant-4`、`radius`、`color`、`stroke-color`、`stroke-width` を持つ点、`classDef` と点のクラス、`accTitle` と `accDescr`。`quadrantChart` の設定キー `chartWidth`、`chartHeight`、`titlePadding`、`xAxisLabelPadding`、`yAxisLabelPadding`、`xAxisPosition`、`yAxisPosition`、`quadrantPadding`、`quadrantTextTopPadding`、`pointRadius`、`pointTextPadding` に対応 |
| Git グラフ（`gitGraph`） | 対応済み：`title`、`accTitle` と `accDescr`、`id`、`msg`、`tag`、`NORMAL`、`REVERSE`、`HIGHLIGHT` を持つ `commit`、`order` を持つ `branch`、`checkout` と `switch`、`merge` と `parent` を持つ `cherry-pick`。`gitGraph` の設定キー `mainBranchName`、`mainBranchOrder`、`showCommitLabel`、`showBranches`、`rotateCommitLabel`、`parallelCommits` に対応 |
| ブロック図（`block`、`block-beta`） | 対応済み：`columns` でグリッドに並べるブロック、`block:` と `end` による複合ブロック、`space`、Mermaid のすべての形状、方向を持つブロック矢印、ラベルと矢印の終端を持つリンク、`classDef`、`class` と `style`。`block` の設定キー `padding` に対応 |
| アーキテクチャ図（`architecture`、`architecture-beta`） | 対応済み：グループ化されたサービス、サービスの間のエッジとそのラベル、ジャンクション、アイコンを持つ `service`、`group` と `junction`。`architecture` の設定キー `iconSize`、`padding`、`fontSize` に対応。サービスはアイコンの四角で、タイトルはその下に置く。`database`、`server`、`disk`、`internet`、`cloud` は組み込みで、ほかの名前は `icons` オプションに任せる |
| パケット図（`packet`、`packet-beta`） | 対応済み：`title`、`accTitle` と `accDescr`、ビット範囲を持つフィールドの行、`+bits` のフィールド。`packet` の設定キー `rowHeight`、`bitWidth`、`bitsPerRow`、`showBits`、`bitOrder`、`paddingX`、`paddingY` に対応 |
| レーダーチャート（`radar-beta`） | 対応済み：`title`、`accTitle` と `accDescr`、項目を持つ `axis` と `curve`、およびオプション `showLegend`、`ticks`、`min`、`max`、`graticule`。`radar` の設定キー `width`、`height`、`marginTop`、`marginRight`、`marginBottom`、`marginLeft`、`axisScaleFactor`、`axisLabelFactor`、`curveTension` に対応 |
| ツリーマップ（`treemap`） | 対応済み：セクションと葉から成る木とその値、`classDef` と `class` に対応 |
| C4 図（`C4Context`、`C4Container`、`C4Component`、`C4Dynamic`、`C4Deployment`） | 対応済み：`title`、`accTitle` と `accDescr`、人物、システム、コンテナ、コンポーネントと、その境界と関係、および `UpdateLayoutConfig` 文 |
| ユースケース図（`usecase-beta`） | 対応済み：`title`、`accTitle` と `accDescr`、アクターとユースケース、その関係。`usecase` の設定キー `nodeSpacing` と `rankSpacing` に対応 |
| インフォメーション図（`info`） | 対応済み：`title`、`accTitle` と `accDescr`、バージョンと本文の行 |
| ベン図（`venn-beta`） | 対応済み：集合、交差領域、ラベル、領域スタイル、内容に合わせたサイズ調整 |
| ツリービュー図（`treeView-beta`） | 対応済み：罫線文字で描かれる木、ラベルの前のアイコン、`classDef` と `class`。`treeView` の設定キー `rowIndent`、`paddingX`、`paddingY`、`lineThickness`、`showIcons`、`defaultIconPack`、`filenameIcons`、`extensionIcons` に対応 |
| Cynefin 図（`cynefin-beta:`） | 対応済み：`title`、`accTitle` と `accDescr`、四つの領域の項目と領域間の遷移。`cynefin` の設定キー `width`、`height`、`showDomainDescriptions`、`boundaryAmplitude`、`seed` に対応 |
| 石川図（`ishikawa`） | 対応済み：各段階の原因と背骨 |
| イベントモデリング図（`eventmodeling`） | 対応済み：レーン、フレーム、関係、注記 |
| Wardley マップ（`wardley-beta`） | 対応済み：`title`、`accTitle` と `accDescr`、各項目と座標、進化。`wardley` の設定キー `width`、`height`、`showGrid` に対応 |
| 鉄道図（`railroad-beta`、`railroad-ebnf-beta`、`railroad-abnf-beta`、`railroad-peg-beta`） | 対応済み：Mermaid の四つの記法と、その規則と終端。`railroad` の設定キー `padding`、`verticalSeparation`、`horizontalSeparation`、`arcRadius`、`fontSize`、`strokeWidth`、`showMarkers`、`markerRadius` に対応 |
| スイムレーン図（`swimlane-beta`） | 対応済み：フローチャートの記法と、最上位のサブグラフをレーンとして描く。設定は Mermaid の記述どおり `flowchart` の節から読む |
| エージェントフロー図（`agentflow-beta`） | 対応済み：`title`、`accTitle` と `accDescr`、エージェント、コンテナと間の辺、`classDef` と `class`。`agentflow` の設定キー `nodeSpacing`、`rankSpacing`、`wrappingWidth`、`minNodeWidth` に対応 |

## 目標

- 純粋な Rust、同期処理、I/O なし。既定のビルドには実行時依存関係がなく、オプションのシーンアダプターは usvg を使用します。
- Mermaid 構文との互換性を保ちつつ、独自のすっきりしたレイアウトとビジュアルスタイルを採用。
- 決定的な出力：同じ入力とオプションからはバイト単位で同一の SVG を生成。
- テーマ変更が可能：色とフォントは SVG 内の CSS 変数。
- 信頼できない入力にも安全：テキストのエスケープ、リンクとスタイルのフィルタリング、サイズとネストの深さの制限。

## ライブラリ

```rust
let options = pele_rs::RenderOptions {
    font_size: Some(14.0),
    ..Default::default()
};
match pele_rs::render("pie\n  \"Rust\": 70\n  \"Other\": 30", &options) {
    Ok(rendered) => println!("{}", rendered.svg),
    Err(e) => eprintln!("{}:{}: {}\n{}", e.line, e.column, e.message, e.snippet),
}
```

`render` は SVG とそのサイズ・リンクを返すか、エラーコード（`Unsupported`、`Syntax`、`Semantic`、`Limit`）、行番号と列番号、問題箇所を示すスニペットを含む `Error` を返します。`detect_type` はレンダリングせずにダイアグラムの種類を判定します。`max_width` にホストで使える幅を指定すると、幅を詰められるダイアグラムは狭く描かれます（円グラフは凡例をグラフの下に移します）。`click` 文のリンクはサニタイズされ、許可されたスキーム（`link_schemes`、デフォルトは `http`、`https`、`mailto`、`tel`）のものだけが残ります。横向きのフローチャートは `max_width` に収まらないときに縦向きで描かれます。ダイアグラムの種類ごとに Cargo feature（`flowchart`、`pie` など）があり、デフォルトの `all` feature にはすべての種類が含まれます。ガントチャートの「今日」は `now`（1970-01-01 UTC からのミリ秒）で決まります。デフォルトは 1970-01-01 で、出力が時計に左右されることはありません。

アイコンと数式はホストが提供します。pele-rs 自身はアイコンセットも組版エンジンも持たないためです。`RenderOptions::icons` は名前からアイコンの SVG マークアップを返し（ラベル内の `fa:fa-car` やノードの `icon`）、`RenderOptions::math` はフローチャート、スイムレーン、ステート、クラス、ER、要求、ブロック、シーケンス、ユースケース、かんばん、マインドマップのラベルにある `$$` の間の TeX を組版します。提供されない場合、アイコンは省かれ、TeX は書かれたままになります。ホストが返す内容はそのまま SVG に入るため、信頼できるものでなければなりません。

## コマンドライン

```sh
cargo install --git https://github.com/nexmoe/pele-rs pele-rs-cli
cargo install --path crates/pele-rs-cli  # チェックアウトからインストール
pele-rs diagram.mmd -o diagram.svg
cat diagram.mmd | pele-rs --font-size 14 --max-width 360 --id-prefix fig1 > diagram.svg
pele-rs plan.mmd --now 1700000000000 -o plan.svg   # 今日のマーカー付きのガントチャート
pele-rs page.mmd --config '{"theme": "dark"}' --no-links --no-images -o page.svg
pele-rs page.mmd --palette dark -o page.svg   # var() を読めないビューア（resvg など）向けに色を実値で出力
```

## テーマ

SVG の色とフォントは CSS カスタムプロパティから取得され、組み込みのフォールバック値があります。そのため、ページは再レンダリングせずにダイアグラムのテーマを変更できます。

```css
svg {
  --pele-bg: #1e1e1e;
  --pele-fg: #dcddde;
  --pele-line: #999;
  --pele-accent: #7f6df2;
  --pele-font: "Inter", sans-serif;
}
```

使用できる変数：`--pele-bg`、`--pele-fg`、`--pele-muted`、`--pele-line`、`--pele-surface`、`--pele-surface-alt`、`--pele-border`、`--pele-accent`、`--pele-radius`、`--pele-font`、`--pele-font-mono`、そして円グラフの扇形などデータ系列の色に使う `--pele-series-1` から `--pele-series-8`。

### CSS 変数を読めないビューア

resvg や librsvg などの SVG レンダラーは `var()` を読み取らないため、既定の SVG は黒一色で描画されます。`RenderOptions::palette` を設定すると、すべての色が実際の値で書き出され、SVG に `var()` は残りません（出力は決定的なままで、パレットを設定しなければ従来と完全に同じです）。

```rust
let options = pele_rs::RenderOptions {
    palette: Some(pele_rs::Palette::dark()), // または Palette::light()、独自の色
    ..Default::default()
};
```

`Palette` には上記の変数に対応する色（`bg`、`fg`、`muted`、`line`、`surface`、`surface_alt`、`border`、`accent`、`critical`、8 色の `series`）があります。色に使えるのは英数字と `# % . , ( ) -` のみで、それ以外はライト用の色に置き換えられます。このモードでは、汎用ファミリの後ろにサンセリフの CJK フォント（Noto Sans CJK、PingFang、Microsoft JhengHei など）も指定します。そうしないと resvg は CJK 文字を明朝系のセリフ体で描画するためです。文字幅は CJK 文字を 1 em として計算します。コマンドラインでは `--palette light|dark` を使います。

## 開発

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo +nightly fuzz run render    # cargo-fuzz が必要。ダイアグラムの種類ごとにターゲットがあります
cargo xtask compat pie --pele ../pele   # pele のチェックアウトから Mermaid の仕様テストケースを再生成
node tools/oracle/golden.mjs ../pele flowchart   # それらに対する pele の解析結果を再生成
node tools/oracle/diff.mjs ../pele   # Mermaid 自身のパーサーとの差分ファジング
```

サポートする最小の Rust バージョンは 1.85 です。

## クレジット

pele-rs のアルゴリズム、テキスト計測データ、制限は、Obsidian チームの kepano による [pele](https://github.com/obsidianmd/pele)（MIT）から移植しています。ダイアグラムの構文と、テストに使うパーサー仕様は [Mermaid](https://github.com/mermaid-js/mermaid)（MIT）に由来します。詳しくは [NOTICE](NOTICE) を参照してください。

## ライセンス

以下から選択して利用できます：
- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)
- [BSD-3-Clause](LICENSE-BSD3)
- [0BSD](LICENSE-0BSD)

詳細は [LICENSE](LICENSE) および [NOTICE](NOTICE) を参照してください。
