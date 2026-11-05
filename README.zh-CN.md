# pele-rs

[English](README.md) | [繁體中文](README.zh-TW.md) | **简体中文** | [日本語](README.ja.md)

这是 [fermaid](https://github.com/tenyi/fermaid) 的独立衍生项目，面向原生编辑器优化，保留原始 Git 历史、署名和许可证，并非 Obsidian 官方项目。当前版本请从 Git 安装：

```toml
[dependencies]
pele-rs = { git = "https://github.com/nexmoe/pele-rs" }
```

新增所有图种的 `parse` / `ParsedDiagram::render` 复用接口、有界 `CachingMeasurer`、原始源码错误定位、图表调色板兼容和数值校验。可选 `scene` 功能通过 usvg 提供 `SvgScene` 矢量树及匹配 resvg 的 `FontMeasurer`，默认构建仍无运行时依赖。参见[原生集成说明](docs/NATIVE.md)。当前版本尚未发布到 crates.io。


pele-rs 以纯 Rust 将 [Mermaid](https://mermaid.js.org) 图表转换为 SVG，无需 JavaScript 运行时、无头浏览器或 Node.js。它移植自 [pele](https://github.com/obsidianmd/pele)（一个小巧、无依赖的 TypeScript 渲染器），支持 Mermaid 12.2.0 语法，并采用 pele 自己的布局与视觉风格。

> **状态：** alpha。已支持 Mermaid 12.2.0 的所有图表类型，下表列出各类型可读取的语法。请参阅[开发路线图](docs/ROADMAP.md)。

| 图表类型 | 状态 |
|---|---|
| 饼图（`pie`） | 已支持：`showData`、`title`、`accTitle`、`accDescr`，以及 `pie` 配置项 `textPosition`、`donutHole` 和 `legendPosition` |
| 流程图（`flowchart`、`graph`） | 已支持：所有节点形状（含 `@{ shape: … }`）、各种连线及长度、连线 ID、可自定义方向的子图、`classDef`、`class`、`style`、`linkStyle`、`click` 链接与提示文字、Markdown 标签，以及 `flowchart` 配置项 `curve`、`nodeSpacing`、`rankSpacing` 和 `wrappingWidth` |
| 时序图（`sequenceDiagram`） | 已支持：各种参与者和角色、各种箭头、激活条、注释、`loop`、`alt`、`opt`、`par`、`critical`、`break` 和 `rect` 块、分组框、`autonumber`、创建和销毁参与者，以及 `sequence` 配置项。参与者菜单（`link`、`links`）不会画成链接 |
| 状态图（`stateDiagram`、`stateDiagram-v2`） | 已支持：带描述的状态、可自定义方向的复合状态、并发区域、起点和终点、choice、fork 和 join、注释、`classDef`、`class`、`style` 和 `click` |
| 类图（`classDiagram`） | 已支持：含成员、泛型和注解的类、带标记和多重性的关系、命名空间、注释、棒棒糖接口、`classDef`、`cssClass`、`style`、`link`、`click` 和 `callback` |
| ER 图（`erDiagram`） | 已支持：含属性、键和注释的实体、鸦爪标记多重性的关系、别名、子图、`classDef`、`class` 和 `style` |
| 需求图（`requirementDiagram`） | 已支持：各种需求和元素及其字段、关系、`classDef`、`class` 和 `style` |
| 甘特图（`gantt`） | 已支持：带日期、工期、`after` 和 `until` 的任务，分区，`done`、`active`、`crit`、`milestone` 和 `vert`，`excludes`、`includes` 和 `weekend`，`dateFormat`、`axisFormat`、`tickInterval`、`weekday`、`todayMarker`、`topAxis`，紧凑模式和 `click` 链接。日期一律为 UTC，“今天”由 `RenderOptions::now` 决定 |
| 时间线（`timeline`） | 已支持：标题、分区、时期及其事件 |
| 用户旅程（`journey`） | 已支持：标题、分区、带分数和参与者的任务 |
| 看板（`kanban`） | 已支持：列和卡片（含形状）、`@{ … }` 元数据（`assigned`、`ticket`、`priority`、`label`）、class，以及通过 `ticketBaseUrl` 的工单链接 |
| 思维导图（`mindmap`） | 已支持：所有节点形状、图标、class 和分支颜色（按分支，或用 `colorBy` 按层级）；横向排列，宿主较窄时改为纵向 |
| 桑基图（`sankey`、`sankey-beta`） | 已支持：Mermaid 的 CSV 形式，每行一组 `source,target,value`，字段可加引号；以及 `sankey` 的配置键 `width`、`height`、`nodeWidth`、`nodePadding`、`nodeAlignment`、`nodeColors`、`linkColor`、`showValues`、`prefix`、`suffix` 和 `labelStyle` |
| XY 图（`xychart`、`xychart-beta`） | 已支持：`title`、以类别或范围表示的 `x-axis`、以范围表示的 `y-axis`、可带点标签的 `line` 与 `bar` 系列、`accTitle` 与 `accDescr`、`horizontal` 与 `vertical` 方向；以及 `xyChart` 的配置键 `width`、`height`、`chartOrientation`、`showTitle`、`showLegend`、`showDataLabel`、`xAxis.showTitle`、`xAxis.showTick`、`xAxis.showLabel`、`xAxis.showAxisLine`，以及 `yAxis` 下同样的四个 |
| 象限图（`quadrantChart`） | 已支持：`title`、以 `-->` 连接的 `x-axis` 与 `y-axis`、`quadrant-1` 到 `quadrant-4`、可带 `radius`、`color`、`stroke-color` 和 `stroke-width` 的点、`classDef` 与点的类别、`accTitle` 与 `accDescr`；以及 `quadrantChart` 的配置键 `chartWidth`、`chartHeight`、`titlePadding`、`xAxisLabelPadding`、`yAxisLabelPadding`、`xAxisPosition`、`yAxisPosition`、`quadrantPadding`、`quadrantTextTopPadding`、`pointRadius` 和 `pointTextPadding` |
| Git 图（`gitGraph`） | 已支持：`title`、`accTitle` 与 `accDescr`、可带 `id`、`msg`、`tag` 和 `NORMAL`、`REVERSE` 或 `HIGHLIGHT` 类型的 `commit`、可带 `order` 的 `branch`、`checkout` 与 `switch`、`merge` 和可带 `parent` 的 `cherry-pick`；以及 `gitGraph` 的配置键 `mainBranchName`、`mainBranchOrder`、`showCommitLabel`、`showBranches`、`rotateCommitLabel` 和 `parallelCommits` |
| 方块图（`block`、`block-beta`） | 已支持：用 `columns` 排成网格的方块、用 `block:` 与 `end` 组成的复合方块、`space`、Mermaid 的所有形状、可指定方向的方块箭头、带标签与箭头端点的连线、`classDef`、`class` 与 `style`；以及 `block` 的配置键 `padding` |
| 架构图（`architecture`、`architecture-beta`） | 已支持：分组中的服务、服务之间的连线及其标签、连接点、可带图标的 `service`、`group` 与 `junction`；以及 `architecture` 的配置键 `iconSize`、`padding` 与 `fontSize`。服务是图标方块，标题在其下方；内置 `database`、`server`、`disk`、`internet` 与 `cloud`，其他名称交给 `icons` 选项 |
| 封包图（`packet`、`packet-beta`） | 已支持：`title`、`accTitle` 与 `accDescr`、以位范围表示的字段行、以 `+bits` 表示的字段；以及 `packet` 的配置键 `rowHeight`、`bitWidth`、`bitsPerRow`、`showBits`、`bitOrder`、`paddingX` 与 `paddingY` |
| 雷达图（`radar-beta`） | 已支持：`title`、`accTitle` 与 `accDescr`、可带条目的 `axis` 与 `curve`，以及选项 `showLegend`、`ticks`、`min`、`max` 与 `graticule`；以及 `radar` 的配置键 `width`、`height`、`marginTop`、`marginRight`、`marginBottom`、`marginLeft`、`axisScaleFactor`、`axisLabelFactor` 与 `curveTension` |
| 矩形树图（`treemap`） | 已支持：由分区与叶节点组成的树及其数值，以及 `classDef` 与 `class` |
| C4 图（`C4Context`、`C4Container`、`C4Component`、`C4Dynamic`、`C4Deployment`） | 已支持：`title`、`accTitle` 与 `accDescr`、人员、系统、容器与组件及其边界与关系，以及 `UpdateLayoutConfig` 语句 |
| 用例图（`usecase-beta`） | 已支持：`title`、`accTitle` 与 `accDescr`、参与者与用例及其之间的关系；以及 `usecase` 的配置键 `nodeSpacing` 与 `rankSpacing` |
| 信息图（`info`） | 已支持：`title`、`accTitle` 与 `accDescr`、版本与若干行文字 |
| 韦恩图（`venn-beta`） | 已支持：各个圆及其标签；以及 `venn` 的配置键 `width`、`height` 与 `padding` |
| 树视图（`treeView-beta`） | 已支持：用方框绘制字符画成的树、标签前的图标、`classDef` 与 `class`；以及 `treeView` 的配置键 `rowIndent`、`paddingX`、`paddingY`、`lineThickness`、`showIcons`、`defaultIconPack`、`filenameIcons` 与 `extensionIcons` |
| Cynefin 图（`cynefin-beta:`） | 已支持：`title`、`accTitle` 与 `accDescr`、四个区域中的条目及其间的转换；以及 `cynefin` 的配置键 `width`、`height`、`showDomainDescriptions`、`boundaryAmplitude` 与 `seed` |
| 石川图（`ishikawa`） | 已支持：各层级的原因与主脊 |
| 事件建模图（`eventmodeling`） | 已支持：泳道、框架、关系与注记 |
| Wardley 地图（`wardley-beta`） | 已支持：`title`、`accTitle` 与 `accDescr`、各条目及其坐标与演化；以及 `wardley` 的配置键 `width`、`height` 与 `showGrid` |
| 铁路图（`railroad-beta`、`railroad-ebnf-beta`、`railroad-abnf-beta`、`railroad-peg-beta`） | 已支持：Mermaid 的四种记法，含其规则与终结符；以及 `railroad` 的配置键 `padding`、`verticalSeparation`、`horizontalSeparation`、`arcRadius`、`fontSize`、`strokeWidth`、`showMarkers` 与 `markerRadius` |
| 泳道图（`swimlane-beta`） | 已支持：流程图语法，最上层的子图画成泳道。设置取自 `flowchart` 一节，与 Mermaid 的说明相同 |
| 代理流程图（`agentflow-beta`） | 已支持：`title`、`accTitle` 与 `accDescr`、代理、容器与其间的边、`classDef` 与 `class`；以及 `agentflow` 的配置键 `nodeSpacing`、`rankSpacing`、`wrappingWidth` 与 `minNodeWidth` |

## 目标

- 纯 Rust、同步执行，库本身不做 I/O。默认构建没有运行时依赖，可选场景适配器使用 usvg。
- 兼容 Mermaid 语法，并采用自己简洁的布局与视觉风格。
- 输出具有确定性：相同的输入和选项会生成逐字节相同的 SVG。
- 可自定义主题：颜色和字体是 SVG 中的 CSS 变量。
- 能安全处理不受信任的输入：转义文本、过滤链接和样式、限制大小与嵌套深度。

## 库

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

`render` 返回 SVG 及其尺寸和链接，或返回一个 `Error`，其中包含错误代码（`Unsupported`、`Syntax`、`Semantic`、`Limit`）、行号和列号，以及指出问题位置的代码片段。`detect_type` 无需渲染即可判断图表类型。将 `max_width` 设为宿主可用的宽度，能画得更窄的图表就会变窄（饼图会把图例移到图的下方）。`click` 语句中的链接会经过清理，且只保留允许的 scheme（`link_schemes`，默认为 `http`、`https`、`mailto` 和 `tel`）；横向的流程图在放不进 `max_width` 时会改为纵向。每种图表类型都是一个 Cargo feature（`flowchart`、`pie`、…）；默认的 `all` feature 包含所有类型。甘特图的“今天”取自 `now`（自 1970-01-01 UTC 起的毫秒数）；默认为 1970-01-01，使输出永远不受时钟影响。

图标与数学公式由宿主提供，因为 pele-rs 本身没有图标集，也没有排版引擎。`RenderOptions::icons` 按名称提供图标的 SVG 标记（标签里的 `fa:fa-car`，或节点的 `icon`），`RenderOptions::math` 则排版流程图、泳道图、状态图、类图、ER 图、需求图、块图、序列图、用例图、看板和思维导图标签中 `$$` 之间的 TeX。未提供时，图标会被省略，TeX 保持原文。宿主返回的内容会原样放进 SVG，所以必须来自你信任的来源。

## 命令行

```sh
cargo install --git https://github.com/nexmoe/pele-rs pele-rs-cli
cargo install --path crates/pele-rs-cli  # 从源码安装
pele-rs diagram.mmd -o diagram.svg
cat diagram.mmd | pele-rs --font-size 14 --max-width 360 --id-prefix fig1 > diagram.svg
pele-rs plan.mmd --now 1700000000000 -o plan.svg   # 画出今天标记的甘特图
pele-rs page.mmd --config '{"theme": "dark"}' --no-links --no-images -o page.svg
pele-rs page.mmd --palette dark -o page.svg   # 输出纯颜色值，供 resvg 等不支持 var() 的查看器使用
```

## 主题

SVG 的颜色和字体取自 CSS 自定义属性，并内置默认值，因此网页无需重新渲染即可更换图表主题：

```css
svg {
  --pele-rs-bg: #1e1e1e;
  --pele-rs-fg: #dcddde;
  --pele-rs-line: #999;
  --pele-rs-accent: #7f6df2;
  --pele-rs-font: "Inter", sans-serif;
}
```

可用的变量：`--pele-rs-bg`、`--pele-rs-fg`、`--pele-rs-muted`、`--pele-rs-line`、`--pele-rs-surface`、`--pele-rs-surface-alt`、`--pele-rs-border`、`--pele-rs-accent`、`--pele-rs-radius`、`--pele-rs-font`、`--pele-rs-font-mono`，以及用于饼图扇区等数据系列颜色的 `--pele-rs-series-1` 到 `--pele-rs-series-8`。

### 不支持 CSS 变量的查看器

resvg、librsvg 等 SVG 渲染器不会读取 `var()`，所以默认的 SVG 在其中会变成一片黑。设置 `RenderOptions::palette` 后，所有颜色都会直接写成实际数值，SVG 中不再有 `var()`（输出仍是确定性的；不设置调色板时输出与之前完全相同）：

```rust
let options = pele_rs::RenderOptions {
    palette: Some(pele_rs::Palette::dark()), // 或 Palette::light()，或自定义颜色
    ..Default::default()
};
```

`Palette` 含有上述变量对应的颜色（`bg`、`fg`、`muted`、`line`、`surface`、`surface_alt`、`border`、`accent`、`critical` 与八个 `series`）；颜色只能由字母、数字与 `# % . , ( ) -` 组成，其他内容会换成浅色调色板的对应颜色。此模式还会在通用字体族之后加上无衬线 CJK 字体（Noto Sans CJK、PingFang、Microsoft JhengHei 等），否则 resvg 会以衬线字体画出中日韩文字；文字宽度按一个 CJK 字符等于一个 em 计算。命令行的对应参数是 `--palette light|dark`。

## 开发

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo +nightly fuzz run render    # 需要 cargo-fuzz；每种图表类型都有自己的目标
cargo xtask compat pie --pele ../pele   # 从 pele 源码重新生成 Mermaid 规范测试用例
node tools/oracle/golden.mjs ../pele flowchart   # 重新生成 pele 对这些用例的解析结果
node tools/oracle/diff.mjs ../pele   # 与 Mermaid 自身的解析器做差异模糊测试
```

最低支持的 Rust 版本为 1.85。

## 致谢

pele-rs 的算法、文本测量数据与限制移植自 Obsidian 团队 kepano 的 [pele](https://github.com/obsidianmd/pele)（MIT）。图表语法以及用作测试的解析器规范来自 [Mermaid](https://github.com/mermaid-js/mermaid)（MIT）。详见 [NOTICE](NOTICE)。

## 许可证

可任选以下任一许可证使用：
- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)
- [BSD-3-Clause](LICENSE-BSD3)
- [0BSD](LICENSE-0BSD)

详见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。
