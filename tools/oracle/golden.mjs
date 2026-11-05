// Dev-only: writes what the reference makes of each generated Mermaid spec case, as golden data
// for the compat tests. pele passes Mermaid's own specs (less what its SKIPPED.md lists), so a
// case on which pele-rs's model matches the reference's passes the spec too.
//
//   node tools/oracle/golden.mjs <pele checkout> <type>
//
// The checkout needs `npm ci` for esbuild. Writes crates/pele-rs/tests/compat/<type>/expected.rs.
// A type that has a module in `summaries/` is read with its own reference; the others are read
// with the reference written here.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { asYy, classSummary, erSummary, flowSummary, ganttSummary, gitSummary, memberSummary, loadPele, journeySummary, kanbanSummary, mindmapSummary, requirementSummary, timelineSummary, seqSummary, stateSummary, sankeySummary, xychartSummary, quadrantSummary } from './lib.mjs';
import { loadSummaries } from './summaries/index.mjs';

// Gantt dates are local time in pele and UTC in pele-rs.
process.env.TZ = 'UTC';
const [peleDir, type] = process.argv.slice(2);
const summaries = await loadSummaries();
// The types whose reference is written here, rather than in a module of `summaries/`.
const LEGACY = ['class', 'er', 'flowchart', 'gantt', 'git', 'journey', 'kanban', 'mindmap', 'requirement', 'sankey', 'sequence', 'state', 'timeline', 'xychart', 'quadrantchart'];
if (!peleDir || (!LEGACY.includes(type) && !summaries.has(type))) {
  console.error('usage: node tools/oracle/golden.mjs <pele checkout> <type>');
  process.exit(2);
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const { buildGit, parseGit, XyChartDb, parseXyChart, QuadrantDb, parseQuadrant, ClassDb, ClassMember, parseClassDiagram, buildClassGraph, ErDb, parseEr, buildErGraph, FlowDb, parseFlowchart, GanttDb, parseGantt, RequirementDb, parseRequirement, buildRequirementGraph, TimelineDb, parseTimeline, JourneyDb, parseJourney, KanbanDb, parseKanban, MindmapDb, parseMindmap, SeqDb, parseSequence, StateDb, parseState, SankeyDb, parseSankey, prepareTextForParsing, encodeEntities } = LEGACY.includes(type)
  ? await loadPele(peleDir, {
  'src/diagrams/class/db.ts': ['ClassDb'],
  'src/diagrams/class/parser.ts': ['parseClassDiagram'],
  'src/diagrams/class/graph.ts': ['buildClassGraph'],
  'src/diagrams/class/members.ts': ['ClassMember'],
  'src/diagrams/er/db.ts': ['ErDb'],
  'src/diagrams/er/parser.ts': ['parseEr'],
  'src/diagrams/er/graph.ts': ['buildErGraph'],
  'src/diagrams/flowchart/db.ts': ['FlowDb'],
  'src/diagrams/flowchart/parser.ts': ['parseFlowchart'],
  'src/diagrams/requirement/db.ts': ['RequirementDb'],
  'src/diagrams/requirement/parser.ts': ['parseRequirement'],
  'src/diagrams/requirement/graph.ts': ['buildRequirementGraph'],
  'src/diagrams/timeline/db.ts': ['TimelineDb'],
  'src/diagrams/gantt/db.ts': ['GanttDb'],
  'src/diagrams/gantt/parser.ts': ['parseGantt'],
  'src/diagrams/journey/db.ts': ['JourneyDb'],
  'src/diagrams/journey/parser.ts': ['parseJourney'],
  'src/diagrams/kanban/db.ts': ['KanbanDb'],
  'src/diagrams/kanban/parser.ts': ['parseKanban'],
  'src/diagrams/mindmap/db.ts': ['MindmapDb'],
  'src/diagrams/mindmap/parser.ts': ['parseMindmap'],
  'src/diagrams/timeline/parser.ts': ['parseTimeline'],
  'src/diagrams/sequence/db.ts': ['SeqDb'],
  'src/diagrams/sequence/parser.ts': ['parseSequence'],
  'src/diagrams/state/db.ts': ['StateDb'],
  'src/diagrams/state/parser.ts': ['parseState'],
  'src/diagrams/sankey/db.ts': ['SankeyDb'],
  'src/diagrams/sankey/parser.ts': ['parseSankey', 'prepareTextForParsing'],
  'src/diagrams/git/db.ts': ['buildGit'],
  'src/diagrams/git/parser.ts': ['parseGit'],
  'src/diagrams/xychart/db.ts': ['XyChartDb'],
  'src/diagrams/xychart/parser.ts': ['parseXyChart'],
  'src/diagrams/quadrant/db.ts': ['QuadrantDb'],
  'src/diagrams/quadrant/parser.ts': ['parseQuadrant'],
  'src/preprocess.ts': ['encodeEntities'],
})
  : {};

// Reads the Rust string literals of cases.rs, as `{:?}` writes them.
function rustString(lit) {
  let out = '';
  for (let i = 1; i < lit.length - 1; i++) {
    const c = lit[i];
    if (c !== '\\') {
      out += c;
      continue;
    }
    const n = lit[++i];
    if (n === 'n') out += '\n';
    else if (n === 'r') out += '\r';
    else if (n === 't') out += '\t';
    else if (n === '0') out += '\0';
    else if (n === 'u') {
      const end = lit.indexOf('}', i);
      out += String.fromCodePoint(parseInt(lit.slice(i + 2, end), 16));
      i = end;
    } else out += n;
  }
  return out;
}

const casesRs = readFileSync(join(root, `crates/pele-rs/tests/compat/${type}/cases.rs`), 'utf8');
const cases = [...casesRs.matchAll(/name: ("(?:[^"\\]|\\.)*"),\n\s*src: ("(?:[^"\\]|\\.)*"),/g)].map((m) => ({
  name: rustString(m[1]),
  src: rustString(m[2]),
}));

let body = '';
for (const { name, src } of cases) {
  let result;
  if (src.includes('${')) result = 'template';
  else {
    try {
      const spec = summaries.get(type);
      if (spec) {
        result = await spec.reference(peleDir, src);
      } else if (type === 'flowchart') {
        const db = new FlowDb({ maxEdges: 500 });
        parseFlowchart(src, db);
        result = flowSummary(db);
      } else if (type === 'class') {
        // Many class specs test members on their own, so each source is also read as one.
        const members = `${memberSummary(new ClassMember(src, 'method'))}\n${memberSummary(new ClassMember(src, 'attribute'))}`;
        try {
          const db = new ClassDb();
          parseClassDiagram(src, db);
          result = `${classSummary(db, buildClassGraph)}\n${members}`;
        } catch (e) {
          result = `error:${e.code ?? 'internal'}\n${members}`;
        }
      } else if (type === 'er') {
        const db = new ErDb();
        parseEr(src, db);
        result = erSummary(db, buildErGraph);
      } else if (type === 'requirement') {
        const db = new RequirementDb();
        parseRequirement(src, db);
        result = requirementSummary(db, buildRequirementGraph);
      } else if (type === 'journey') {
        const db = new JourneyDb();
        parseJourney(src, db);
        result = journeySummary(db);
      } else if (type === 'gantt') {
        const db = new GanttDb();
        parseGantt(src, db);
        result = ganttSummary(db);
      } else if (type === 'kanban') {
        const db = new KanbanDb();
        parseKanban(src, db);
        result = kanbanSummary(db);
      } else if (type === 'mindmap') {
        const db = new MindmapDb();
        parseMindmap(src, db);
        result = mindmapSummary(db);
      } else if (type === 'timeline') {
        const db = new TimelineDb();
        parseTimeline(src, db);
        result = timelineSummary(db);
      } else if (type === 'state') {
        const db = new StateDb();
        parseState(src, db);
        result = stateSummary(db);
      } else if (type === 'sankey') {
        // Mermaid's diagram parses the text as Mermaid prepares it first, and Mermaid encodes
        // the entities of the whole diagram before any parser sees it.
        const db = new SankeyDb();
        parseSankey(prepareTextForParsing(encodeEntities(src) + '\n'), db);
        result = sankeySummary(db);
      } else if (type === 'quadrantchart') {
        // Mermaid's diagram parses the text as Mermaid prepares it first, and Mermaid encodes
        // the entities of the whole diagram before any parser sees it.
        const db = new QuadrantDb();
        parseQuadrant(encodeEntities(src) + '\n', db);
        result = quadrantSummary(db);
      } else if (type === 'git') {
        // Mermaid's diagram parses the text as Mermaid prepares it first, and Mermaid encodes
        // the entities of the whole diagram before any parser sees it.
        const ast = parseGit(encodeEntities(src) + '\n');
        result = gitSummary(buildGit(ast, {}, undefined));
      } else if (type === 'xychart') {
        // Mermaid's diagram parses the text as Mermaid prepares it first, and Mermaid encodes
        // the entities of the whole diagram before any parser sees it.
        const db = new XyChartDb();
        parseXyChart(encodeEntities(src) + '\n', db);
        result = xychartSummary(db);
      } else {
        // As pele's spec adapter does: the text as given, with entities encoded.
        const db = new SeqDb();
        parseSequence(encodeEntities(src) + '\n', db);
        result = seqSummary(db);
      }
    } catch (e) {
      result = `error:${e.code ?? 'internal'}`;
    }
  }
  body += `    (${JSON.stringify(name)}, ${JSON.stringify(result)}),\n`;
}
const target = join(root, `crates/pele-rs/tests/compat/${type}/expected.rs`);
if (cases.length === 0) throw new Error(`No ${type} cases found; refusing to replace the golden file.`);
const reference = JSON.parse(readFileSync(join(root, 'upstream.json'), 'utf8')).pele;
writeFileSync(
  target,
  `// What pele 1.9.0 makes of each case in cases.rs, as summarized by tools/oracle/golden.mjs.\n` +
    `// Reference: ${reference.repository} at ${reference.commit}.\n` +
    `// Generated; do not edit.\n\nconst EXPECTED: &[(&str, &str)] = &[\n${body}];\n`
);
console.log(`wrote ${cases.length} results to ${target}`);
