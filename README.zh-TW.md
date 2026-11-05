# pele-rs

[English](README.md) | **繁體中文** | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

這是 [fermaid](https://github.com/tenyi/fermaid) 的獨立衍生專案，針對原生編輯器最佳化，保留原始 Git 歷史、署名和授權，並非 Obsidian 官方專案。目前版本請從 Git 安裝：

```toml
[dependencies]
pele-rs = { git = "https://github.com/nexmoe/pele-rs" }
```

新增所有圖種的 `parse` / `ParsedDiagram::render` 重用介面、有界 `CachingMeasurer`、原始碼錯誤定位、圖表調色盤相容與數值驗證。選用的 `scene` 功能透過 usvg 提供 `SvgScene` 向量樹及符合 resvg 的 `FontMeasurer`；預設建置仍無執行期相依套件。參見[原生整合說明](docs/NATIVE.md)。目前版本尚未發布至 crates.io。


pele-rs 以純 Rust 將 [Mermaid](https://mermaid.js.org) 圖表轉換為 SVG，不需要 JavaScript 執行環境、無頭瀏覽器或 Node.js。它移植自 [pele](https://github.com/obsidianmd/pele)（一個小巧、無相依套件的 TypeScript 渲染器），支援 Mermaid 12.2.0 語法，並採用 pele 自己的版面配置與視覺風格。

> **狀態：** alpha。已支援 Mermaid 12.1.0 的所有圖表類型，下表列出各類型可讀取的語法。請參閱[開發路線圖](docs/ROADMAP.md)。

| 圖表類型 | 狀態 |
|---|---|
| 圓餅圖（`pie`） | 已支援：`showData`、`title`、`accTitle`、`accDescr`，以及 `pie` 設定項 `textPosition`、`donutHole` 與 `legendPosition` |
| 流程圖（`flowchart`、`graph`） | 已支援：所有節點形狀（含 `@{ shape: … }`）、各種連線與長度、連線 ID、可自訂方向的子圖、`classDef`、`class`、`style`、`linkStyle`、`click` 連結與提示文字、Markdown 標籤，以及 `flowchart` 設定項 `curve`、`nodeSpacing`、`rankSpacing` 與 `wrappingWidth` |
| 循序圖（`sequenceDiagram`） | 已支援：各種參與者與角色、各種箭頭、啟動區段、註記、`loop`、`alt`、`opt`、`par`、`critical`、`break` 與 `rect` 區塊、分組框、`autonumber`、建立與銷毀參與者，以及 `sequence` 設定項。參與者選單（`link`、`links`）不會畫成連結 |
| 狀態圖（`stateDiagram`、`stateDiagram-v2`） | 已支援：帶描述的狀態、可自訂方向的複合狀態、並行區域、起點與終點、choice、fork 與 join、註記、`classDef`、`class`、`style` 與 `click` |
| 類別圖（`classDiagram`） | 已支援：含成員、泛型與註解的類別、帶標記與多重性的關係、命名空間、註記、棒棒糖介面、`classDef`、`cssClass`、`style`、`link`、`click` 與 `callback` |
| ER 圖（`erDiagram`） | 已支援：含屬性、鍵與註解的實體、鴉爪標記多重性的關係、別名、子圖、`classDef`、`class` 與 `style` |
| 需求圖（`requirementDiagram`） | 已支援：各種需求與元素及其欄位、關係、`classDef`、`class` 與 `style` |
| 甘特圖（`gantt`） | 已支援：含日期、工期、`after` 與 `until` 的任務，區段，`done`、`active`、`crit`、`milestone` 與 `vert`，`excludes`、`includes` 與 `weekend`，`dateFormat`、`axisFormat`、`tickInterval`、`weekday`、`todayMarker`、`topAxis`，緊湊模式與 `click` 連結。日期一律為 UTC，「今天」由 `RenderOptions::now` 決定 |
| 時間軸（`timeline`） | 已支援：標題、區段、時期及其事件 |
| 使用者旅程（`journey`） | 已支援：標題、區段、含分數與參與者的任務 |
| 看板（`kanban`） | 已支援：欄與卡片（含形狀）、`@{ … }` 中繼資料（`assigned`、`ticket`、`priority`、`label`）、class，以及透過 `ticketBaseUrl` 的票證連結 |
| 心智圖（`mindmap`） | 已支援：所有節點形狀、圖示、class 與分支顏色（依分支，或以 `colorBy` 依層級）；橫向排列，宿主較窄時改為縱向 |
| 桑基圖（`sankey`、`sankey-beta`） | 已支援：Mermaid 的 CSV 形式，每行一組 `source,target,value`，欄位可加引號；以及 `sankey` 的設定鍵 `width`、`height`、`nodeWidth`、`nodePadding`、`nodeAlignment`、`nodeColors`、`linkColor`、`showValues`、`prefix`、`suffix` 與 `labelStyle` |
| XY 圖（`xychart`、`xychart-beta`） | 已支援：`title`、以類別或範圍表示的 `x-axis`、以範圍表示的 `y-axis`、可帶點標籤的 `line` 與 `bar` 系列、`accTitle` 與 `accDescr`、`horizontal` 與 `vertical` 方向；以及 `xyChart` 的設定鍵 `width`、`height`、`chartOrientation`、`showTitle`、`showLegend`、`showDataLabel`、`xAxis.showTitle`、`xAxis.showTick`、`xAxis.showLabel`、`xAxis.showAxisLine`，以及 `yAxis` 底下同樣的四個 |
| 象限圖（`quadrantChart`） | 已支援：`title`、以 `-->` 連接的 `x-axis` 與 `y-axis`、`quadrant-1` 到 `quadrant-4`、可帶 `radius`、`color`、`stroke-color` 與 `stroke-width` 的點、`classDef` 與點的類別、`accTitle` 與 `accDescr`；以及 `quadrantChart` 的設定鍵 `chartWidth`、`chartHeight`、`titlePadding`、`xAxisLabelPadding`、`yAxisLabelPadding`、`xAxisPosition`、`yAxisPosition`、`quadrantPadding`、`quadrantTextTopPadding`、`pointRadius` 與 `pointTextPadding` |
| Git 圖（`gitGraph`） | 已支援：`title`、`accTitle` 與 `accDescr`、可帶 `id`、`msg`、`tag` 與 `NORMAL`、`REVERSE` 或 `HIGHLIGHT` 類型的 `commit`、可帶 `order` 的 `branch`、`checkout` 與 `switch`、`merge` 與可帶 `parent` 的 `cherry-pick`；以及 `gitGraph` 的設定鍵 `mainBranchName`、`mainBranchOrder`、`showCommitLabel`、`showBranches`、`rotateCommitLabel` 與 `parallelCommits` |
| 方塊圖（`block`、`block-beta`） | 已支援：以 `columns` 排成格線的方塊、以 `block:` 與 `end` 組成的複合方塊、`space`、Mermaid 的所有形狀、可指定方向的方塊箭頭、帶標籤與箭頭端點的連線、`classDef`、`class` 與 `style`；以及 `block` 的設定鍵 `padding` |
| 架構圖（`architecture`、`architecture-beta`） | 已支援：分組中的服務、服務之間的連線與其標籤、連接點、可帶圖示的 `service`、`group` 與 `junction`；以及 `architecture` 的設定鍵 `iconSize`、`padding` 與 `fontSize`。服務是圖示方塊，標題在其下方；內建 `database`、`server`、`disk`、`internet` 與 `cloud`，其他名稱交給 `icons` 選項 |
| 封包圖（`packet`、`packet-beta`） | 已支援：`title`、`accTitle` 與 `accDescr`、以位元範圍表示的欄位列、以 `+bits` 表示的欄位；以及 `packet` 的設定鍵 `rowHeight`、`bitWidth`、`bitsPerRow`、`showBits`、`bitOrder`、`paddingX` 與 `paddingY` |
| 雷達圖（`radar-beta`） | 已支援：`title`、`accTitle` 與 `accDescr`、可帶項目的 `axis` 與 `curve`，以及選項 `showLegend`、`ticks`、`min`、`max` 與 `graticule`；以及 `radar` 的設定鍵 `width`、`height`、`marginTop`、`marginRight`、`marginBottom`、`marginLeft`、`axisScaleFactor`、`axisLabelFactor` 與 `curveTension` |
| 矩形樹圖（`treemap`） | 已支援：由區段與葉節點組成的樹及其數值，以及 `classDef` 與 `class` |
| C4 圖（`C4Context`、`C4Container`、`C4Component`、`C4Dynamic`、`C4Deployment`） | 已支援：`title`、`accTitle` 與 `accDescr`、人員、系統、容器與元件及其邊界與關聯，以及 `UpdateLayoutConfig` 敘述 |
| 用例圖（`usecase-beta`） | 已支援：`title`、`accTitle` 與 `accDescr`、角色與用例及其間的關聯；以及 `usecase` 的設定鍵 `nodeSpacing` 與 `rankSpacing` |
| 資訊圖（`info`） | 已支援：`title`、`accTitle` 與 `accDescr`、版本與數行文字 |
| 韋恩圖（`venn-beta`） | 已支援：各圓與其標籤；以及 `venn` 的設定鍵 `width`、`height` 與 `padding` |
| 樹狀檢視圖（`treeView-beta`） | 已支援：以方塊繪製字符畫成的樹、標籤前的圖示、`classDef` 與 `class`；以及 `treeView` 的設定鍵 `rowIndent`、`paddingX`、`paddingY`、`lineThickness`、`showIcons`、`defaultIconPack`、`filenameIcons` 與 `extensionIcons` |
| Cynefin 圖（`cynefin-beta:`） | 已支援：`title`、`accTitle` 與 `accDescr`、四個區域中的項目與其間的轉換；以及 `cynefin` 的設定鍵 `width`、`height`、`showDomainDescriptions`、`boundaryAmplitude` 與 `seed` |
| 石川圖（`ishikawa`） | 已支援：各層級的原因與主脊 |
| 事件建模圖（`eventmodeling`） | 已支援：泳道、框架、關聯與註記 |
| Wardley 地圖（`wardley-beta`） | 已支援：`title`、`accTitle` 與 `accDescr`、各項目及其座標與演化；以及 `wardley` 的設定鍵 `width`、`height` 與 `showGrid` |
| 鐵路圖（`railroad-beta`、`railroad-ebnf-beta`、`railroad-abnf-beta`、`railroad-peg-beta`） | 已支援：Mermaid 的四種記法，含其規則與終端；以及 `railroad` 的設定鍵 `padding`、`verticalSeparation`、`horizontalSeparation`、`arcRadius`、`fontSize`、`strokeWidth`、`showMarkers` 與 `markerRadius` |
| 泳道圖（`swimlane-beta`） | 已支援：流程圖語法，最上層的子圖畫成泳道。設定取自 `flowchart` 一節，與 Mermaid 的說明相同 |
| 代理流程圖（`agentflow-beta`） | 已支援：`title`、`accTitle` 與 `accDescr`、代理、容器與其間的邊、`classDef` 與 `class`；以及 `agentflow` 的設定鍵 `nodeSpacing`、`rankSpacing`、`wrappingWidth` 與 `minNodeWidth` |

## 目標

- 純 Rust、同步執行、沒有執行期相依套件，函式庫本身不做 I/O。
- 相容 Mermaid 語法，並採用自己簡潔的版面配置與視覺風格。
- 輸出具確定性：相同的輸入與選項會產生逐位元組相同的 SVG。
- 可自訂主題：顏色與字型是 SVG 中的 CSS 變數。
- 能安全處理不受信任的輸入：跳脫文字、過濾連結與樣式、限制大小與巢狀深度。

## 函式庫

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

`render` 會回傳 SVG 及其尺寸與連結，或回傳一個 `Error`，其中包含錯誤代碼（`Unsupported`、`Syntax`、`Semantic`、`Limit`）、行號與欄號，以及指出問題位置的程式碼片段。`detect_type` 不需渲染即可判斷圖表類型。將 `max_width` 設為宿主可用的寬度，能畫得更窄的圖表就會縮窄（圓餅圖會把圖例移到圖的下方）。`click` 陳述式中的連結會經過清理，且只保留允許的 scheme（`link_schemes`，預設為 `http`、`https`、`mailto` 與 `tel`）；橫向的流程圖在放不進 `max_width` 時會改為縱向。每種圖表類型都是一個 Cargo feature（`flowchart`、`pie`、…）；預設的 `all` feature 包含所有類型。甘特圖的「今天」取自 `now`（自 1970-01-01 UTC 起的毫秒數）；預設為 1970-01-01，使輸出永不受時鐘影響。

圖示與數學式由宿主提供，因為 pele-rs 本身沒有圖示集，也沒有排版引擎。`RenderOptions::icons` 依名稱提供圖示的 SVG 標記（標籤裡的 `fa:fa-car`，或節點的 `icon`），`RenderOptions::math` 則排版流程圖、泳道圖、狀態圖、類別圖、ER 圖、需求圖、區塊圖、序列圖、使用案例圖、看板與心智圖標籤中 `$$` 之間的 TeX。沒有提供時，圖示會被省略，TeX 維持原文。宿主回傳的內容會原封不動放進 SVG，所以必須來自你信任的來源。

## 命令列

```sh
cargo install --git https://github.com/nexmoe/pele-rs pele-rs-cli
cargo install --path crates/pele-rs-cli  # 從原始碼安裝
pele-rs diagram.mmd -o diagram.svg
cat diagram.mmd | pele-rs --font-size 14 --max-width 360 --id-prefix fig1 > diagram.svg
pele-rs plan.mmd --now 1700000000000 -o plan.svg   # 畫出今天標記的甘特圖
pele-rs page.mmd --config '{"theme": "dark"}' --no-links --no-images -o page.svg
pele-rs page.mmd --palette dark -o page.svg   # 輸出純色彩，供 resvg 等不支援 var() 的檢視器使用
```

## 主題

SVG 的顏色與字型取自 CSS 自訂屬性，並內建預設值，因此網頁不必重新渲染就能更換圖表主題：

```css
svg {
  --pele-rs-bg: #1e1e1e;
  --pele-rs-fg: #dcddde;
  --pele-rs-line: #999;
  --pele-rs-accent: #7f6df2;
  --pele-rs-font: "Inter", sans-serif;
}
```

可用的變數：`--pele-rs-bg`、`--pele-rs-fg`、`--pele-rs-muted`、`--pele-rs-line`、`--pele-rs-surface`、`--pele-rs-surface-alt`、`--pele-rs-border`、`--pele-rs-accent`、`--pele-rs-radius`、`--pele-rs-font`、`--pele-rs-font-mono`，以及用於圓餅圖扇形等資料序列顏色的 `--pele-rs-series-1` 到 `--pele-rs-series-8`。

### 不支援 CSS 變數的檢視器

resvg、librsvg 等 SVG 渲染器不會讀取 `var()`，所以預設的 SVG 在其中會變成一片黑。設定 `RenderOptions::palette` 後，所有顏色都會直接寫成實際數值，SVG 中不再有 `var()`（輸出仍是決定性的；不設定調色盤時輸出與先前完全相同）：

```rust
let options = pele_rs::RenderOptions {
    palette: Some(pele_rs::Palette::dark()), // 或 Palette::light()，或自訂顏色
    ..Default::default()
};
```

`Palette` 含有上述變數對應的顏色（`bg`、`fg`、`muted`、`line`、`surface`、`surface_alt`、`border`、`accent`、`critical` 與八個 `series`）；顏色只能由字母、數字與 `# % . , ( ) -` 組成，其他內容會換成淺色調色盤的對應顏色。此模式也會在通用字型家族之後加上無襯線 CJK 字型（Noto Sans CJK、PingFang、Microsoft JhengHei 等），否則 resvg 會以襯線字型畫出中日韓文字；文字寬度以一個 CJK 字元等於一個 em 計算。命令列的對應旗標是 `--palette light|dark`。

## 開發

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo +nightly fuzz run render    # 需要 cargo-fuzz；每一種圖表類型都有自己的目標
cargo xtask compat pie --pele ../pele   # 從 pele 原始碼重新產生 Mermaid 規格測試案例
node tools/oracle/golden.mjs ../pele flowchart   # 重新產生 pele 對這些案例的解析結果
node tools/oracle/diff.mjs ../pele   # 與 Mermaid 自身的解析器做差異模糊測試
```

最低支援的 Rust 版本為 1.85。

## 致謝

pele-rs 的演算法、文字量測資料與限制移植自 Obsidian 團隊 kepano 的 [pele](https://github.com/obsidianmd/pele)（MIT）。圖表語法以及作為測試的解析器規格來自 [Mermaid](https://github.com/mermaid-js/mermaid)（MIT）。詳見 [NOTICE](NOTICE)。

## 授權

可任選以下任一授權協議使用：
- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)
- [BSD-3-Clause](LICENSE-BSD3)
- [0BSD](LICENSE-0BSD)

詳見 [LICENSE](LICENSE) 與 [NOTICE](NOTICE)。
