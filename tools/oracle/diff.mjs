// Dev-only differential fuzzing: parses inputs with Mermaid's own parsers and with pele-rs's,
// and reports every input on which the two disagree.
//
//   node tools/oracle/diff.mjs <pele checkout> [--count N] [--seed S] [pie] [flowchart] [sequence] [state] [class] [er] [requirement] [gantt] [timeline] [journey] [kanban] [mindmap] [url]
//
// The inputs are the type's corpus (Mermaid's spec and documentation examples, as pele collects
// them) and N mutated inputs (default 3,000) from a fixed seed (default 7). The reference is the
// parser Mermaid uses: `@mermaid-js/parser` for pie, and the parsers jison generates from
// Mermaid's flowchart, sequence, state, class, ER, gantt, timeline, journey, kanban and mind map
// grammars, which pele vendors, driving pele's port of Mermaid's model. `url` checks the link
// sanitizer against `@braintree/sanitize-url`, which Mermaid applies to click targets, on
// addresses mutated from a list below. pele-rs's side runs as the ignored `oracle_dump` test of
// each type. Both sides are reduced to the summaries in `lib.mjs`, and errors to just "error".
//
// Differences that are intended are listed in KNOWN.md, as a type and a JSON-quoted input on
// one line; they are counted but not reported. The exit code is 1 when anything else differs.
// A type that has a module in `summaries/` brings its own reference with it, so a new type
// needs no change here.
// The checkout needs `npm ci`.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { asYy, classSummary, erSummary, flowSummary, ganttSummary, gitSummary, loadPele, pieSummary, requirementSummary, timelineSummary, journeySummary, kanbanSummary, mindmapSummary, sankeySummary, seqSummary, stateSummary, xychartSummary, quadrantSummary } from './lib.mjs';
import { loadSummaries } from './summaries/index.mjs';

// Gantt dates are local time in pele and UTC in pele-rs, so the run pins the zone to UTC,
// as tools/oracle/golden.mjs does.
process.env.TZ = 'UTC';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  if (i < 0) return fallback;
  const value = Number(args[i + 1]);
  args.splice(i, 2);
  return value;
};
const count = option('--count', 3000);
const seed = option('--seed', 7);
const [peleDir, ...rest] = args;
// A type that has a module in `summaries/` brings its own reference with it.
const summaries = await loadSummaries();
const ALL = ['pie', 'flowchart', 'sequence', 'state', 'class', 'er', 'requirement', 'gantt', 'git', 'timeline', 'journey', 'kanban', 'mindmap', 'sankey', 'xychart', 'quadrantchart', 'url', ...summaries.keys()];
const types = rest.length ? rest : ALL;
if (!peleDir || types.some((t) => !ALL.includes(t)) || !Number.isFinite(count) || !Number.isFinite(seed)) {
  console.error('usage: node tools/oracle/diff.mjs <pele checkout> [--count N] [--seed S] [<type> ...]');
  process.exit(2);
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const pele = resolve(peleDir);
const m = await loadPele(pele, {
  'tests/support/jison.ts': ['buildParser'],
  'tests/support/corpus.ts': ['loadCorpus', 'mutator', 'random'],
  'tests/support/inert.ts': ['PAYLOADS'],
  'tests/support/sequence-constants.ts': ['LINETYPE', 'PLACEMENT'],
  'src/diagrams/flowchart/db.ts': ['FlowDb'],
  'src/diagrams/sequence/db.ts': ['SeqDb'],
  'src/diagrams/state/db.ts': ['StateDb'],
  'src/diagrams/class/db.ts': ['ClassDb'],
  'src/diagrams/class/graph.ts': ['buildClassGraph'],
  'src/diagrams/er/db.ts': ['ErDb'],
  'src/diagrams/er/graph.ts': ['buildErGraph'],
  'src/diagrams/requirement/db.ts': ['RequirementDb', 'RequirementType', 'RiskLevel', 'VerifyType', 'Relationships'],
  'src/diagrams/requirement/graph.ts': ['buildRequirementGraph'],
  'src/diagrams/gantt/db.ts': ['GanttDb'],
  'src/diagrams/timeline/db.ts': ['TimelineDb'],
  'src/diagrams/journey/db.ts': ['JourneyDb'],
  'src/diagrams/kanban/db.ts': ['KanbanDb'],
  'src/diagrams/mindmap/db.ts': ['MindmapDb'],
  'src/diagrams/sankey/db.ts': ['SankeyDb'],
  'src/diagrams/sankey/parser.ts': ['prepareTextForParsing'],
  'src/diagrams/git/db.ts': ['buildGit'],
  'src/diagrams/xychart/db.ts': ['XyChartDb'],
  'src/diagrams/quadrant/db.ts': ['QuadrantDb'],
  'src/diagrams/common/outline.ts': ['NODE_TYPE'],
  'src/preprocess.ts': ['encodeEntities'],
  '@mermaid-js/parser': ['parse as langiumParse'],
  '@braintree/sanitize-url': ['sanitizeUrl'],
});

const quiet = { trace() {}, debug() {}, info() {}, warn() {}, error() {} };

// A Langium tree without its bookkeeping.
function plain(node) {
  if (Array.isArray(node)) return node.map(plain);
  if (node === null || typeof node !== 'object') return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key.startsWith('$')) continue;
    if (value !== undefined) out[key] = plain(value);
  }
  return out;
}

// A Langium tree without its bookkeeping, but with the kind of every node.
function plainWithType(node) {
  if (Array.isArray(node)) return node.map(plainWithType);
  if (node === null || typeof node !== 'object') return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (key.startsWith('$') && key !== '$type') continue;
    if (value !== undefined) out[key] = plainWithType(value);
  }
  return out;
}

const flowParser = m.buildParser(join(pele, 'tests/compat/flowchart/upstream/flow.jison'));
const seqParser = m.buildParser(join(pele, 'tests/compat/sequence/upstream/sequenceDiagram.jison'));
const stateParser = m.buildParser(join(pele, 'tests/compat/state/upstream/stateDiagram.jison'));
const classParser = m.buildParser(join(pele, 'tests/compat/class/upstream/classDiagram.jison'));
const erParser = m.buildParser(join(pele, 'tests/compat/er/upstream/erDiagram.jison'));
const journeyParser = m.buildParser(join(pele, 'tests/compat/journey/upstream/journey.jison'));
const timelineParser = m.buildParser(join(pele, 'tests/compat/timeline/upstream/timeline.jison'));
const kanbanParser = m.buildParser(join(pele, 'tests/compat/kanban/upstream/kanban.jison'));
const mindmapParser = m.buildParser(join(pele, 'tests/compat/mindmap/upstream/mindmap.jison'));
const ganttParser = m.buildParser(join(pele, 'tests/compat/gantt/upstream/gantt.jison'));
const requirementParser = m.buildParser(join(pele, 'tests/compat/requirement/upstream/requirementDiagram.jison'));
const sankeyParser = m.buildParser(join(pele, 'tests/compat/sankey/upstream/sankey.jison'));
const xychartParser = m.buildParser(join(pele, 'tests/compat/xychart/upstream/xychart.jison'));
const quadrantParser = m.buildParser(join(pele, 'tests/compat/quadrant/upstream/quadrant.jison'));

const TYPES = {
  pie: {
    keyword: /(^|\n)\s*pie\b/,
    fragments: ['pie', 'showData', 'title ', 'accTitle: ', 'accDescr: ', 'accDescr {', '}', ':', '"', "'", '"a"', '1', '-1', '1.5', '.5', '1e3', '\n', ' ', '\t', '%%', '---\n', '\\', 'é', ';'],
    async reference(src) {
      return pieSummary(plain(await m.langiumParse('pie', src)));
    },
  },
  flowchart: {
    keyword: /(^|\n)\s*(graph|flowchart)\b/,
    fragments: ['-->', '---', '-.->', '==>', '--o', '--x', '<-->', '~~~', '[', ']', '(', ')', '{', '}', '((', '))', '>', '|', '"', '`', '\n', ';', '&', ':::', '@{', 'subgraph x\n', 'end\n', 'classDef c fill:red\n', 'class A c\n', 'style A fill:red\n', 'linkStyle 0 stroke:red\n', 'click A "https://x"\n', 'direction LR\n', 'A', 'B', 'é', '%%', ' '],
    async reference(src) {
      const db = new m.FlowDb({ maxEdges: 500 });
      let first = true;
      flowParser.yy = asYy(db, { lex: { firstGraph: () => first && !(first = false) } });
      // Mermaid's flowParser.ts applies this before handing the text to the generated parser.
      flowParser.parse(src.replace(/}\s*\n/g, '}\n'));
      return flowSummary(db);
    },
  },
  sequence: {
    keyword: /sequenceDiagram/i,
    fragments: ['->>', '-->>', '->', '-->', '<<->>', '-x', '--x', '-)', '--)', '-|\\', '-|/', '+', '-', '()', ':', ' as ', 'participant ', 'actor ', 'create ', 'destroy ', 'box ', 'end\n', 'loop ', 'rect ', 'opt ', 'alt ', 'else ', 'par ', 'and ', 'critical ', 'option ', 'break ', 'Note over A: ', 'Note left of ', 'links A: {"a": "b"}\n', 'activate ', 'deactivate ', 'autonumber', ' 10 2\n', 'title ', 'accTitle: ', 'accDescr { ', 'wrap: ', '@{ "type": "queue" }', '<br/>', 'A', 'B', 'é', '\n', ';', '#', '%%'],
    async reference(src) {
      const db = new m.SeqDb();
      seqParser.yy = asYy(db, { LINETYPE: m.LINETYPE, PLACEMENT: m.PLACEMENT });
      // Mermaid's Diagram.fromText encodes entities and adds a line break.
      seqParser.parse(m.encodeEntities(src) + '\n');
      return seqSummary(db);
    },
  },
  state: {
    keyword: /(^|\n)\s*stateDiagram/,
    fragments: ['-->', '[*]', 'state ', '{', '}', '--', '\n', 'note left of A : x', 'note right of A\nmore\nend note', '<<fork>>', '<<join>>', '<<choice>>', '[[fork]]', 'classDef c fill:red', 'class A c', 'style A fill:blue', ':::c', ': label', 'direction LR', 'click A "https://x" "tip"', 'click A href "https://y"', 'hide empty description', 'scale 100 width', ' as ', '"', '%%', 'A', 'é', ' '],
    async reference(src) {
      const db = new m.StateDb();
      stateParser.yy = asYy(db);
      stateParser.parse(src);
      return stateSummary(db);
    },
  },
  class: {
    keyword: /(^|\n)\s*classDiagram/,
    fragments: ['class ', 'namespace ', '{', '}', '<|--', '*--', 'o--', '-->', '..>', '..|>', '--', '..', '()--', '--()', ' : ', '~T~', '<<interface>>', 'note for A "x"', 'note "y"', '"1"', '"*"', '["label"]', 'classDef c fill:red', 'cssClass "A" c', ':::c', 'style A fill:blue', 'link A "https://x" "tip"', 'click A href "https://y" _self', 'callback A "f"', '+x()$', '-y*', '#z', '~w', '(', ')', '.', '`', 'direction LR', '%%', 'A', 'é', '\n', ' '],
    async reference(src) {
      const db = new m.ClassDb();
      classParser.yy = asYy(db, { relationType: db.relationType, lineType: db.lineType });
      classParser.parse(src);
      return classSummary(db, m.buildClassGraph);
    },
  },
  er: {
    keyword: /(^|\n)\s*erDiagram/,
    fragments: ['||--o{', '}|..|{', '|o--|{', 'u--o{', '}o', '|{', 'one or more', 'zero or one', 'only one', 'many(0)', 'to', 'optionally to', '1+', '0+', ' : ', '"a b"', '{', '}', 'string name PK, FK "comment"', 'int~T~ x', '`a b`', '?', '[alias]', ':::c', 'classDef c fill:red', 'class A c', 'style A fill:blue', 'subgraph x\n', 'subgraph y [t]\n', 'end\n', 'direction LR', 'accTitle: t', 'accDescr { d }', '%%', 'A', 'é', '1', '1.5', '\n', ' '],
    async reference(src) {
      const db = new m.ErDb();
      erParser.yy = asYy(db, {
        Cardinality: { ZERO_OR_ONE: 'ZERO_OR_ONE', ZERO_OR_MORE: 'ZERO_OR_MORE', ONE_OR_MORE: 'ONE_OR_MORE', ONLY_ONE: 'ONLY_ONE', MD_PARENT: 'MD_PARENT' },
        Identification: { NON_IDENTIFYING: 'NON_IDENTIFYING', IDENTIFYING: 'IDENTIFYING' },
      });
      erParser.parse(src);
      return erSummary(db, m.buildErGraph);
    },
  },
  requirement: {
    keyword: /(^|\n)\s*requirementDiagram/,
    fragments: ['requirement ', 'functionalRequirement ', 'designConstraint ', 'element ', '{', '}', 'id: 1', 'text: t', 'risk: high', 'verifymethod: test', 'type: x', 'docref: d', ' - contains -> ', ' - traces -> ', ' <- copies - ', '-', '->', '<-', ':::c', 'classDef c fill:red', 'class a c', 'style a fill:blue', '"q r"', ',', ':', 'direction LR', 'accTitle: t', 'accDescr { d }', '%%', 'a', 'é', '\n', ' '],
    async reference(src) {
      const db = new m.RequirementDb();
      requirementParser.yy = asYy(db, { RequirementType: m.RequirementType, RiskLevel: m.RiskLevel, VerifyType: m.VerifyType, Relationships: m.Relationships });
      requirementParser.parse(src);
      return requirementSummary(db, m.buildRequirementGraph);
    },
  },
  gantt: {
    keyword: /(^|\n)\s*gantt\b/,
    fragments: ['gantt', 'dateFormat YYYY-MM-DD', 'axisFormat %Y-%m-%d', 'tickInterval 1day', 'includes monday', 'excludes weekends', 'todayMarker stroke-width:5px', 'inclusiveEndDates', 'topAxis', 'weekday monday', 'weekend saturday', 'section S', 'title T', 'accTitle: t', 'accDescr: d', 'accDescr { d }', 'task', ':a1, 2014-01-01, 30d', 'done', 'active', 'crit', 'milestone', 'vert', 'after a1', 'until a2', 'd', 'w', 'h', 'm', 's', 'click a1 href "https://x"', '%%', '\n', ' ', 'é', ';', '#', ':', ','],
    async reference(src) {
      const db = new m.GanttDb();
      ganttParser.yy = asYy(db);
      ganttParser.parse(src);
      return ganttSummary(db);
    },
  },
  sankey: {
    keyword: /(^|\n)\s*sankey(-beta)?\b/,
    fragments: ['sankey', 'sankey-beta', 'a,b,1', '"a,b",c,1', '"a""b",c,1', ',', '"', '""', '1', '-1', '1.5', '0', '.5', '1e3', '\n', ' ', '%%', '---\n', 'é', 'A'],
    async reference(src) {
      const db = new m.SankeyDb();
      sankeyParser.yy = asYy(db);
      // Mermaid's diagram prepares the text before it hands it to the generated parser, and
      // Mermaid encodes the entities of the whole diagram before any parser sees it.
      sankeyParser.parse(m.prepareTextForParsing(m.encodeEntities(src) + '\n'));
      return sankeySummary(db);
    },
  },
  xychart: {
    keyword: /(^|\n)\s*xychart(-beta)?\b/,
    fragments: ['xychart', 'xychart-beta', 'horizontal', 'vertical', 'title ', 'x-axis ', 'y-axis ', '[', ']', '{', '}', '-->', ',', '"', '1', '-1', '1.5', '.5', '0', 'line ', 'bar ', 'accTitle: ', 'accDescr: ', 'accDescr {', '\n', ';', '%%', '---\n', 'é', 'A'],
    async reference(src) {
      const db = new m.XyChartDb();
      xychartParser.yy = asYy(db);
      // Mermaid's Diagram.fromText encodes entities and adds a line break.
      xychartParser.parse(m.encodeEntities(src) + '\n');
      return xychartSummary(db);
    },
  },
  quadrantchart: {
    // The corpus of the quadrant chart is under another name in pele.
    corpusName: 'quadrant',
    keyword: /(^|\n)\s*quadrant(Chart|-[1-4])\b/,
    fragments: ['quadrantChart', 'quadrant-1', 'quadrant-2', 'quadrant-3', 'quadrant-4', 'x-axis ', 'y-axis ', '-->', 'title ', 'classDef ', ':::', '[', ']', ',', ':', '"', '0', '1', '0.5', 'radius: 10', 'color: #ff0000', 'stroke-color: #00ff00', 'stroke-width: 5px', '\n', ';', '%%', '---\n', 'é', 'A', '❤'],
    async reference(src) {
      const db = new m.QuadrantDb();
      quadrantParser.yy = asYy(db);
      // Mermaid's Diagram.fromText encodes entities and adds a line break.
      quadrantParser.parse(m.encodeEntities(src) + '\n');
      return quadrantSummary(db);
    },
  },
  git: {
    keyword: /(^|\n)\s*gitGraph\b/,
    fragments: ['gitGraph', 'gitGraph:', 'LR', 'TB', 'BT', 'commit', 'branch', 'merge', 'checkout', 'switch', 'cherry-pick', 'id:', 'msg:', 'tag:', 'type:', 'order:', 'parent:', 'NORMAL', 'REVERSE', 'HIGHLIGHT', '"', "'", '\n', ' ', '%%', '---\n', 'é', 'A'],
    async reference(src) {
      // Mermaid's git graph is parsed with Langium; the model is filled by pele's port of
      // Mermaid's git graph database.
      const ast = plainWithType(await m.langiumParse('gitGraph', src));
      return gitSummary(m.buildGit(ast, {}, undefined));
    },
  },
  timeline: {
    keyword: /(^|\n)\s*timeline/,
    fragments: ['timeline', 'timeline TD', 'timeline LR', 'section ', 'title ', ' : ', ':', ': ', '\n', 'accTitle: t', 'accDescr: d', 'accDescr { d }', '<br>', '%%', '#', '2001', 'a b', 'é', ' '],
    async reference(src) {
      const db = new m.TimelineDb();
      timelineParser.yy = asYy(db);
      timelineParser.parse(src);
      return timelineSummary(db);
    },
  },
  journey: {
    keyword: /(^|\n)\s*journey/,
    fragments: ['journey', 'section ', 'title ', ': ', ':', '5', '1.5', '0x2', 'x', '\n', 'accTitle: t', 'accDescr: d', 'accDescr { d }', ',', '<br>', '%%', '#', ';', 'Me', 'é', ' '],
    async reference(src) {
      const db = new m.JourneyDb();
      journeyParser.yy = asYy(db);
      journeyParser.parse(src);
      return journeySummary(db);
    },
  },
  kanban: {
    keyword: /(^|\n)\s*kanban/,
    fragments: ['kanban', '\n', '  ', '    ', 'a', 'id1', '[', ']', '(', ')', '((', '))', '{{', '}}', '"', '`', '@{ ', '}', 'priority: high', 'ticket: X-1', "assigned: 'me'", 'label: L', 'shape: Rect', '\n  ', ':::c', '::icon(fa)', '%%', '<br>', '**', 'é', ' '],
    async reference(src) {
      const db = new m.KanbanDb();
      kanbanParser.yy = asYy(db, { getType: db.getType, getLogger: () => quiet });
      kanbanParser.parse(src);
      return kanbanSummary(db);
    },
  },
  mindmap: {
    keyword: /(^|\n)\s*mindmap/,
    fragments: ['mindmap', '\n', '  ', '    ', ' ', 'root', 'a', 'id1', '[', ']', '(', ')', '((', '))', '{{', '}}', ')', '(', '"', '`', ':::c', '::icon(fa)', '%%', '<br>', '**', '@{ shape: rect }', 'é'],
    async reference(src) {
      const db = new m.MindmapDb();
      mindmapParser.yy = asYy(db, { getType: db.getType, nodeType: m.NODE_TYPE, getLogger: () => quiet });
      mindmapParser.parse(src);
      return mindmapSummary(db);
    },
  },
  url: {
    corpus: [
      'https://example.com', 'HTTPS://Example.COM/a/b?c=d#e', 'http://x', 'http://a.b/c/../d/./e', 'https://user@host/p',
      'https://ex ample.com', 'https://日本.jp/パス?q=値#ä', 'https://0x7f.1/', 'http://192.168.0.1.', 'https://1.2.3.4.5/',
      'https://%41%42.com/%7e', 'https://a.com/a b"<>`{}', 'https://a.com/?a b\'"<>#f g', 'https://a.com/%2e%2E/x/.%2e',
      '//evil.com', '/local', './rel', '../up', 'page.html', '#anchor', 'mailto:a@b.c', 'tel:+1', 'ftp://x/y',
      'javascript:alert(1)', ' JaVaScRiPt:alert(1)', 'java&#x09;script:alert(1)', '&#106;avascript:alert(1)',
      '%6Aavascript:alert(1)', 'data:text/html,x', 'vbscript:x', 'https://a.com:8080/x', 'http:\\\\a.com\\b',
      'https://a.com/\u2028x', 'https:///a.com', 'https:', 'https://', 'https://a..b/', 'https://[::1]/', 'https://a%zz.com',
    ],
    fragments: ['/', '\\', ':', '@', '?', '#', '%', '%2e', '%25', '%0a', '&#', ';', '&colon;', '&newline;', '\\n', '.', '..', ' ', '\t', '<', '>', '"', "'", '`', '{', 'é', '日', 'xn--', '0x', '0', '255', 'javascript', 'https://', 'mailto:', 'A', '\u200b', '\ufeff'],
    async reference(src) {
      return m.sanitizeUrl(src);
    },
  },
};

// A type that has a module in `summaries/` brings its own reference, its own corpus filter and
// its own mutation fragments with it.
for (const [type, spec] of summaries) {
  TYPES[type] = {
    keyword: spec.keyword,
    fragments: spec.fragments,
    ...(spec.corpusName ? { corpusName: spec.corpusName } : {}),
    async reference(src) {
      return spec.reference(pele, src);
    },
  };
}

const known = new Set(
  readFileSync(join(root, 'tools/oracle/KNOWN.md'), 'utf8')
    .split('\n')
    .filter((line) => /^- \w+ "/.test(line))
    .map((line) => line.slice(2))
);

process.chdir(pele);
const work = mkdtempSync(join(tmpdir(), 'pele-rs-diff-'));
const runs = {};
for (const type of types) {
  const spec = TYPES[type];
  const corpus = spec.corpus ?? m.loadCorpus(spec.corpusName ?? type, spec.keyword);
  // Hostile payloads are a few of the fragments, so that most inputs stay close to valid.
  const next = m.mutator(corpus, [...m.PAYLOADS.slice(0, 8), ...spec.fragments], m.random(seed));
  const inputs = [...corpus];
  for (let i = 0; i < count; i++) inputs.push(next());
  // pele-rs reads the inputs as JSON into Rust strings, which cannot hold a lone surrogate.
  const kept = inputs.filter((s) => s.length <= 2000 && s.isWellFormed());
  const expected = [];
  for (const src of kept) {
    try {
      expected.push(await spec.reference(src));
    } catch {
      expected.push('error');
    }
  }
  writeFileSync(join(work, `${type}.json`), JSON.stringify(kept));
  runs[type] = { kept, expected, corpus: corpus.length };
}

execFileSync('cargo', ['test', '-q', '-p', 'pele-rs', '--lib', 'oracle_dump', '--', '--ignored'], {
  cwd: root,
  env: { ...process.env, PELE_RS_ORACLE: work },
  stdio: ['ignore', 'ignore', 'inherit'],
});

// Node's URL parser (ada) leaves `.` and `..` segments in a path that has a segment starting
// with a dot; the WHATWG standard, which browsers and pele-rs follow, resolves them. An address
// on which only that differs is a known difference.
function resolveDots(url) {
  const m = /^(https?:\/\/[^/?#]*)([^?#]*)(.*)$/s.exec(url);
  if (!m) return url;
  const out = [];
  const segments = m[2].split('/').slice(1);
  segments.forEach((segment, i) => {
    const last = i === segments.length - 1;
    if (segment === '..') {
      out.pop();
      if (last) out.push('');
    } else if (segment === '.') {
      if (last) out.push('');
    } else out.push(segment);
  });
  return m[1] + '/' + out.join('/') + m[3];
}

let unexplained = 0;
for (const type of types) {
  const { kept, expected, corpus } = runs[type];
  const got = JSON.parse(readFileSync(join(work, `${type}.out.json`), 'utf8'));
  let same = 0;
  let errors = 0;
  let listed = 0;
  const differ = [];
  kept.forEach((src, i) => {
    const want = expected[i];
    const have = got[i].startsWith('error') ? 'error' : got[i];
    if (want === have) {
      same++;
      if (want === 'error') errors++;
    } else if (known.has(`${type} ${JSON.stringify(src)}`)) listed++;
    else if (type === 'url' && resolveDots(want) === have) listed++;
    else differ.push({ src, want, have });
  });
  unexplained += differ.length;
  console.log(
    `${type}: ${kept.length} inputs (${corpus} from the corpus), ${same} agree (${errors} rejected by both), ` +
      `${listed} known differences, ${differ.length} unexplained`
  );
  for (const { src, want, have } of differ.slice(0, 10)) {
    console.log(`  ${JSON.stringify(src)}\n    mermaid: ${JSON.stringify(want)}\n    pele-rs: ${JSON.stringify(have)}`);
  }
}
process.exit(unexplained ? 1 : 0);
