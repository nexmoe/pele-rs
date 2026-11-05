// Dev-only: draws the same charts with pele and with pele-rs, and compares the two pictures.
// The model of a chart is compared by tools/oracle/diff.mjs; this compares what is drawn, which
// is the layout port.
//
//   node tools/oracle/render-compare.mjs <pele checkout> <type>   (xychart, quadrantchart, git, architecture)
//
// The names of the classes and of the CSS variables are pele-rs's, not pele's, so they are
// normalized away before the two are compared. The checkout needs `npm ci`.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadPele } from './lib.mjs';

const [peleDir, type] = process.argv.slice(2);
if (!peleDir || !type) {
  console.error('usage: node tools/oracle/render-compare.mjs <pele checkout> <type>');
  process.exit(2);
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

/// The charts of each type, and the configuration each of them is drawn with.
const TYPES = {
  xychart: {
    modules: {
      'src/diagrams/xychart/db.ts': ['XyChartDb'],
      'src/diagrams/xychart/parser.ts': ['parseXyChart'],
      'src/diagrams/xychart/render.ts': ['renderXyChart'],
    },
    names: [
      ['pele-series pele-bars', 'pele-rs-xychart-series pele-rs-xychart-bars'],
      ['pele-series pele-line', 'pele-rs-xychart-series pele-rs-xychart-line'],
      ['pele-legend-item', 'pele-rs-xychart-legend-item'],
      ['pele-legend', 'pele-rs-xychart-legend'],
      ['pele-axis-labels', 'pele-rs-xychart-axis-labels'],
      ['pele-axis-titles', 'pele-rs-xychart-axis-titles'],
      ['pele-data-labels', 'pele-rs-xychart-data-labels'],
      ['pele-point-labels', 'pele-rs-xychart-point-labels'],
      ['pele-axis', 'pele-rs-xychart-axis'],
      ['pele-grid', 'pele-rs-xychart-grid'],
      ['pele-plot', 'pele-rs-xychart-plot'],
      ['pele-title', 'pele-rs-title'],
    ],
    cases: {
      full: [
        `xychart-beta
  title "An Example Chart"
  x-axis ["90d", "60d", "30d", "7d", "1d", "Current"]
  y-axis "Seconds" 0 --> 198.2
  line "avg" [48.1, 41.5, 45.7, 72.8, 67.7, 59.9]
  line "p50" [38.2, 36.8, 39.7, 54.5, 49.0, 38.4]
  line "p95" [112.2, 75.3, 103.0, 177.0, 180.2, 109.4]
`,
        {},
      ],
      bars: [
        `xychart
  x-axis [Q1, Q2, Q3, Q4]
  y-axis "Revenue" 0 --> 100
  bar [12, 2, 20, 25]
`,
        {},
      ],
      horizontal: [`xychart horizontal\nx-axis [a, b]\ny-axis "v" 0 --> 10\nline [1, 2]\n`, {}],
      values: [
        `xychart
  x-axis [Q1, Q2, Q3, Q4]
  y-axis "Revenue" 0 --> 100
  bar [12, 2, 20, 25]
`,
        { xyChart: { showDataLabel: true } },
      ],
      points: [
        `xychart
  x-axis [Q1, Q2, Q3]
  y-axis "v" 0 --> 10
  line [1 "first", 5 "second", 9]
`,
        {},
      ],
      mixed: [
        `xychart
  title "Mixed"
  x-axis [a, b, c, d]
  y-axis "v" 0 --> 40
  bar [10, 20, 30, 40]
  line [5, 15, 25, 35]
`,
        {},
      ],
      range: [`xychart\n  x-axis 0 --> 10\n  y-axis 0 --> 100\n  line [10, 50, 100]\n`, {}],
      narrow: [
        `xychart
  x-axis [a very long category name, another long one, and a third]
  y-axis "a long value axis title" 0 --> 1000
  line [10, 500, 1000]
`,
        {},
      ],
    },
  },
  git: {
    modules: {
      'src/diagrams/git/db.ts': ['buildGit'],
      'src/diagrams/git/parser.ts': ['parseGit'],
      'src/diagrams/git/render.ts': ['renderGit'],
    },
    names: [
      ['pele-branches', 'pele-rs-branches'],
      ['pele-branch', 'pele-rs-branch'],
      ['pele-lane', 'pele-rs-lane'],
      ['pele-edges', 'pele-rs-edges'],
      ['pele-edge-merge', 'pele-rs-edge-merge'],
      ['pele-edge-cherry-pick', 'pele-rs-edge-cherry-pick'],
      ['pele-edge', 'pele-rs-edge'],
      ['pele-commits', 'pele-rs-commits'],
      ['pele-commit-cherry-pick', 'pele-rs-commit-cherry-pick'],
      ['pele-commit-highlight', 'pele-rs-commit-highlight'],
      ['pele-commit-reverse', 'pele-rs-commit-reverse'],
      ['pele-commit-normal', 'pele-rs-commit-normal'],
      ['pele-commit-merge', 'pele-rs-commit-merge'],
      ['pele-commit-label', 'pele-rs-commit-label'],
      ['pele-commit', 'pele-rs-commit'],
      ['pele-tag', 'pele-rs-tag'],
      ['pele-label', 'pele-rs-label'],
      ['pele-title', 'pele-rs-title'],
    ],
    cases: {
      history: [
        `gitGraph
    commit
    commit
    commit
`,
        {},
      ],
      branches: [
        `gitGraph
    commit
    branch develop
    commit
    checkout main
    merge develop
    commit
`,
        {},
      ],
      tags: [
        `gitGraph
    commit id:"one" tag:"v1.0" msg:"first"
    commit id:"two" type:HIGHLIGHT tag:"v1.1"
    branch release
    commit id:"three" type:REVERSE
`,
        {},
      ],
      down: [`gitGraph TB:\n    commit\n    branch other\n    commit\n`, {}],
      up: [`gitGraph BT:\n    commit\n    commit\n`, {}],
      titled: [
        `gitGraph
    title The history of a project
    commit
    commit
`,
        {},
      ],
      plain: [
        `gitGraph
    commit
    branch feature
    commit
    checkout main
    merge feature
`,
        { gitGraph: { showBranches: false } },
      ],
      parallel: [
        `gitGraph
    commit
    branch a
    commit
    branch b
    commit
`,
        { gitGraph: { parallelCommits: true } },
      ],
    },
  },
  quadrantchart: {
    modules: {
      'src/diagrams/quadrant/db.ts': ['QuadrantDb'],
      'src/diagrams/quadrant/parser.ts': ['parseQuadrant'],
      'src/diagrams/quadrant/render.ts': ['renderQuadrant'],
    },
    names: [
      ['pele-quadrants', 'pele-rs-quadrants'],
      ['pele-quadrant', 'pele-rs-quadrant'],
      ['pele-axis-labels', 'pele-rs-axis-labels'],
      ['pele-point-labels', 'pele-rs-point-labels'],
      ['pele-points', 'pele-rs-points'],
      ['pele-point', 'pele-rs-point'],
      ['pele-label', 'pele-rs-label'],
      ['pele-title', 'pele-rs-title'],
    ],
    cases: {
      campaigns: [
        `quadrantChart
    title Reach and engagement of campaigns
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
    Campaign C: [0.57, 0.69]
    Campaign D: [0.78, 0.34]
    Campaign E: [0.40, 0.34]
    Campaign F: [0.35, 0.78]
`,
        {},
      ],
      boxes: [
        `quadrantChart
  x-axis Urgent --> Not Urgent
  y-axis Not Important --> "Important ❤"
  quadrant-1 Plan
  quadrant-2 Do
  quadrant-3 Delegate
  quadrant-4 Delete
`,
        {},
      ],
      styled: [
        `quadrantChart
  title Styled
  x-axis Low Reach --> High Reach
  y-axis Low Engagement --> High Engagement
  quadrant-1 We should expand
  quadrant-2 Need to promote
  quadrant-3 Re-evaluate
  quadrant-4 May be improved
  Campaign A: [0.9, 0.0] radius: 12
  Campaign B:::class1: [0.8, 0.1] color: #ff3300, radius: 10
  Campaign C: [0.7, 0.2] radius: 25, color: #00ff33, stroke-color: #10f0f0
  Campaign D: [0.6, 0.3] radius: 15, stroke-color: #00ff0f, stroke-width: 5px ,color: #ff33f0
  Campaign E:::class2: [0.5, 0.4]
  Campaign F:::class3: [0.4, 0.5] color: #0000ff
  classDef class1 color: #109060
  classDef class2 color: #908342, radius : 10, stroke-color: #310085, stroke-width: 10px
  classDef class3 color: #f00fff, radius : 10
`,
        {},
      ],
      axes: [
        `quadrantChart
  x-axis Low --> High
  y-axis Low --> High
  quadrant-1 A
  quadrant-2 B
  quadrant-3 C
  quadrant-4 D
`,
        { quadrantChart: { xAxisPosition: 'bottom', yAxisPosition: 'right' } },
      ],
      small: [
        `quadrantChart
  x-axis Low --> High
  y-axis Low --> High
  quadrant-1 A
  quadrant-2 B
`,
        {},
      ],
    },
  },
};

TYPES.architecture = {
  modules: {
    'src/diagrams/architecture/db.ts': ['ArchitectureDb', 'populate'],
    'src/diagrams/architecture/parser.ts': ['parseArchitecture'],
    'src/diagrams/architecture/render.ts': ['renderArchitecture'],
  },
  // Every class of pele's becomes pele-rs's.
  generic: true,
  names: [],
  cases: {
    services: [
      `architecture-beta
  service db(database)[Database]
  service server(server)[Server]
  service disk(disk)[Disk]
  service net(internet)[Internet]
  service cloud(cloud)[Cloud]
  db:R -- L:server
  server:R -- L:disk
  net:B -- T:cloud
`,
      {},
    ],
    groups: [
      `architecture-beta
  group api(cloud)[API]
  service db(database)[Database] in api
  service server(server)[Server] in api
  service disk(disk)[Disk]
  db:R -- L:server
  server:B -- T:disk
`,
      {},
    ],
    text: [
      `architecture-beta
  service a(server)[A very long title for the service that wraps]
  service b(cloud)[B]
  a:R --> L:b
`,
      {},
    ],
    junction: [
      `architecture-beta
  service a(server)[A]
  service b(server)[B]
  service c(server)[C]
  junction j
  a:R -- L:j
  j:R -- L:b
  j:B -- T:c
`,
      {},
    ],
    labels: [
      `architecture-beta
  title Edge labels
  service a(server)[A]
  service b(cloud)[B]
  a:R -[ships]- L:b
`,
      {},
    ],
    nested: [
      `architecture-beta
  group outer(cloud)[Outer]
  group inner(server)[Inner] in outer
  service a(database)[A] in inner
  service b(disk)[B] in outer
  a{group}:R -- L:b
`,
      {},
    ],
  },
};

const spec = TYPES[type];
if (!spec) {
  console.error(`unknown type: ${type}`);
  process.exit(2);
}

const { XyChartDb, parseXyChart, renderXyChart, QuadrantDb, parseQuadrant, renderQuadrant, buildGit, parseGit, renderGit, ArchitectureDb, populate, parseArchitecture, renderArchitecture } =
  await loadPele(resolve(peleDir), spec.modules);

function draw(src, config) {
  if (type === 'xychart') {
    const db = new XyChartDb();
    parseXyChart(src, db);
    return renderXyChart(db, config, {}).svg;
  }
  if (type === 'architecture') {
    const db = new ArchitectureDb();
    populate(parseArchitecture(src), db);
    return renderArchitecture(db, config, {}).svg;
  }
  if (type === 'git') {
    const model = buildGit(parseGit(src), config, undefined);
    return renderGit(model, config, {}).svg;
  }
  const db = new QuadrantDb();
  parseQuadrant(src, db);
  return renderQuadrant(db, config, {}).svg;
}

/// The configuration of a chart, as the front matter pele-rs reads it.
function frontMatter(config) {
  const sections = Object.entries(config);
  if (!sections.length) return '';
  const lines = [];
  for (const [section, values] of sections) {
    lines.push(`  ${section}:`);
    for (const [key, value] of Object.entries(values)) {
      lines.push(`    ${key}: ${value}`);
    }
  }
  return `---\nconfig:\n${lines.join('\n')}\n---\n`;
}

function normalize(svg) {
  let out = svg;
  for (const [a, b] of spec.names) {
    out = out
      .split(`"${a}"`)
      .join(`"${b}"`)
      .split(` ${a}"`)
      .join(` ${b}"`)
      .split(`${a} `)
      .join(`${b} `);
  }
  if (spec.generic) out = out.replace(/\bpele-(?!rs-)/g, 'pele-rs-').replace(/class="pele /g, 'class="pele-rs ');
  out = out.replace(/--pele-(?!rs-)/g, '--pele-rs-');
  out = out.replace(/var\(--pele-rs-series-\d+/g, 'var(--pele-rs-series-N');
  out = out.replace(/class="pele pele-([^"]+)"/g, 'class="pele-rs pele-rs-$1"');
  out = out.replace(/id="pele-(?!rs-)/g, 'id="pele-rs-');
  out = out.replace(/aria-(labelledby|describedby)="pele-(?!rs-)/g, 'aria-$1="pele-rs-');
  return out;
}

const work = mkdtempSync(join(tmpdir(), 'pele-rs-render-'));
let differ = 0;
for (const [name, [src, config]] of Object.entries(spec.cases)) {
  const pele = normalize(draw(src, config));

  const input = join(work, `${name}.mmd`);
  const output = join(work, `${name}.svg`);
  writeFileSync(input, frontMatter(config) + src);
  execFileSync('cargo', ['run', '-q', '-p', 'pele-rs-cli', '--', input, '-o', output], {
    cwd: root,
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  const rustSvg = normalize(readFileSync(output, 'utf8'));

  if (pele === rustSvg) {
    console.log(`${name}: the same picture`);
    continue;
  }
  differ++;
  let at = 0;
  while (at < pele.length && at < rustSvg.length && pele[at] === rustSvg[at]) at++;
  console.log(`${name}: different at ${at}`);
  console.log(`  pele:    ${pele.slice(Math.max(0, at - 60), at + 120)}`);
  console.log(`  pele-rs: ${rustSvg.slice(Math.max(0, at - 60), at + 120)}`);
}
process.exit(differ ? 1 : 0);
