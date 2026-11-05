// Dev-only helpers shared by the oracle scripts: a bundle of the parts of a pele checkout they
// use, and the model summaries the compat tests and the differential run compare.
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// The bundles already built, by the checkout and by what they export.
const bundles = new Map();

// Regeneration must use the recorded upstream, not whichever reference is beside the repo.
export function assertPeleReference(peleDir) {
  const pin = JSON.parse(readFileSync(new URL('../../upstream.json', import.meta.url), 'utf8')).pele;
  const version = JSON.parse(readFileSync(join(peleDir, 'package.json'), 'utf8')).version;
  const commit = execFileSync('git', ['-C', peleDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (version !== pin.version || commit !== pin.commit) {
    throw new Error(`Reference is Pele ${version} at ${commit}; expected ${pin.version} at ${pin.commit}. Update upstream.json deliberately before regenerating fixtures.`);
  }
  return pin;
}

// Bundles the named modules of a pele checkout (`npm ci` done, for esbuild) and imports them.
// `exports` maps a path in the checkout, or a package name, to the names it exports. Packages
// stay outside the bundle, which is written into the checkout's node_modules/.cache so that
// they resolve. The same bundle is built once, so a reference may ask for it for every input.
export async function loadPele(peleDir, exports) {
  const pele = resolve(peleDir);
  assertPeleReference(pele);
  const key = `${pele}|${JSON.stringify(exports)}`;
  const cached = bundles.get(key);
  if (cached) return cached;
  mkdirSync(join(pele, 'node_modules/.cache'), { recursive: true });
  const work = mkdtempSync(join(pele, 'node_modules/.cache/pele-rs-oracle-'));
  const entry = join(work, 'entry.ts');
  writeFileSync(
    entry,
    Object.entries(exports)
      .map(([path, names]) => {
        const from = path.startsWith('@') ? path : join(pele, path);
        return `export { ${names.join(', ')} } from ${JSON.stringify(from)};\n`;
      })
      .join('')
  );
  const bundle = join(work, 'oracle.mjs');
  execFileSync(join(pele, 'node_modules/.bin/esbuild'), [
    entry,
    '--bundle',
    '--format=esm',
    '--platform=node',
    '--packages=external',
    `--outfile=${bundle}`,
    '--log-level=error',
  ]);
  const mod = await import(pathToFileURL(bundle).href);
  bundles.set(key, mod);
  return mod;
}

export const q = (s) => (s === undefined || s === null ? '-' : JSON.stringify(String(s)));
export const list = (a) => (a && a.length ? JSON.stringify(a.join('\u0001')) : '-');
export function gitSummary(model) {
  const out = [`dir:${q(model.direction)}`];
  const commits = [...model.commits.values()].sort((a, b) => a.seq - b.seq);
  for (const c of commits) {
    // Mermaid ends the id of a commit it made up with seven random digits; pele-rs derives them,
    // so only the number of the commit is compared.
    const id = c.id.startsWith(`${c.seq}-`) ? `${c.seq}-*` : c.id;
    const parents = c.parents.map((p) => {
      const parent = model.commits.get(p);
      return parent ? String(parent.seq) : p === '' ? '-' : '?';
    });
    out.push(
      `commit:${c.seq}|${q(id)}|${q(c.message)}|${c.type}|${list(c.tags)}|p=${parents.join(',')}|b=${q(c.branch)}|ct=${c.customType ?? '-'}|ci=${c.customId === true}`
    );
  }
  for (const [name, head] of model.branches) {
    const commit = head ? model.commits.get(head) : undefined;
    out.push(`branch:${q(name)}|${commit ? commit.seq : '-'}`);
  }
  out.push(`lanes:${list(model.lanes)}`);
  out.push(`current:${q(model.currentBranch)}`);
  out.push(`head:${model.head ? model.head.seq : '-'}`);
  out.push(`title:${q(model.title)}|acc:${q(model.accTitle)}|${q(model.accDescr)}`);
  out.push(`warnings:${model.warnings.length}`);
  return out.join('\n');
}
// Jison copies only the own properties of `yy`, so a model's methods are bound onto a plain
// object.
export function asYy(db, extra) {
  const yy = { ...extra };
  // A model may hold a method as a field of its own, so those are bound as well.
  for (const name of Object.getOwnPropertyNames(db)) {
    if (name !== 'constructor' && typeof db[name] === 'function' && !(name in yy)) yy[name] = db[name].bind(db);
  }
  for (let proto = Object.getPrototypeOf(db); proto && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
    for (const name of Object.getOwnPropertyNames(proto)) {
      if (name !== 'constructor' && typeof db[name] === 'function' && !(name in yy)) yy[name] = db[name].bind(db);
    }
  }
  return yy;
}

export function flowSummary(db) {
  const out = [`dir:${q(db.direction)}`];
  for (const n of db.nodes.values()) {
    out.push(`node:${q(n.id)}|${q(n.text)}|${n.labelType}|${q(n.type)}|c=${list(n.classes)}|s=${list(n.styles)}|l=${q(n.link)}|t=${q(n.linkTarget)}`);
  }
  for (const e of db.edges) {
    out.push(
      `edge:${q(e.start)}|${q(e.end)}|${q(e.type)}|${q(e.stroke)}|${e.length ?? '-'}|${q(e.text)}|${e.labelType}|${q(e.id)}|i=${q(e.interpolate)}|s=${list(e.style)}|c=${list(e.classes)}`
    );
  }
  for (const s of db.subgraphs) out.push(`sub:${q(s.id)}|${q(s.title)}|${s.labelType}|n=${list(s.nodes)}|d=${q(s.dir)}|c=${list(s.classes)}`);
  for (const id of [...db.classes.keys()].sort()) {
    const c = db.classes.get(id);
    out.push(`class:${q(id)}|s=${list(c.styles)}|t=${list(c.textStyles)}`);
  }
  for (const id of [...db.tooltips.keys()].sort()) out.push(`tip:${q(id)}|${q(db.tooltips.get(id))}`);
  out.push(`default:s=${list(db.defaultEdgeStyle)}|i=${q(db.defaultInterpolate)}`);
  out.push(`acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

// JSON with the keys of every object sorted.
function canon(v) {
  if (v instanceof Map) v = Object.fromEntries(v);
  if (Array.isArray(v)) return `[${v.map(canon).join(',')}]`;
  if (v !== null && typeof v === 'object') {
    return `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canon(v[k])}`).join(',')}}`;
  }
  return JSON.stringify(v) ?? 'null';
}
const sorted = (map) => [...map.keys()].sort().map((k) => `${k}=${map.get(k)}`).join(',');
export function seqSummary(db) {
  const out = [];
  for (const [id, a] of db.actors) {
    const box = a.box ? db.boxes.indexOf(a.box) : '-';
    out.push(`actor:${q(id)}|${q(a.name)}|${q(a.description)}|${a.wrap}|${q(a.type)}|b=${box}|l=${canon(a.links)}|p=${canon(a.properties)}|d=${q(a.details)}`);
  }
  out.push(`created:${sorted(db.createdActors)}`);
  out.push(`destroyed:${sorted(db.destroyedActors)}`);
  for (const b of db.boxes) out.push(`box:${q(b.name)}|${b.wrap}|${q(b.fill)}|${list(b.actorKeys)}`);
  for (const m of db.messages) {
    const text = typeof m.message === 'string' ? q(m.message) : `n=${m.message.start ?? '-'},${m.message.step ?? '-'},${m.message.visible}`;
    out.push(`msg:${q(m.from)}|${q(m.to)}|${text}|${m.wrap}|${m.type ?? '-'}|${m.placement ?? '-'}|${m.centralConnection ?? 0}`);
  }
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}


// The fields of a Langium pie tree that pele-rs's parser keeps.
export function pieSummary(ast) {
  const out = [`show:${ast.showData === true}|t:${q(ast.title)}|at:${q(ast.accTitle)}|ad:${q(ast.accDescr)}`];
  for (const s of ast.sections ?? []) out.push(`section:${q(s.label)}|${s.value}`);
  return out.join('\n');
}

const opt = (v) => (v === undefined || v === null ? '-' : String(v));
export function stateSummary(db) {
  const out = [`dir:${q(db.direction)}`];
  for (const [id, st] of db.states) {
    const note = st.note ? `${q(st.note.position)}:${q(st.note.text)}` : '-';
    out.push(`state:${q(id)}|${list(st.descriptions)}|${q(st.type)}|doc=${st.doc ? 1 : 0}|n=${note}|c=${list(st.classes)}|s=${list(st.styles)}`);
  }
  for (const r of db.relations) out.push(`rel:${q(r.id1)}|${q(r.id2)}|${q(r.relationTitle)}`);
  for (const [id, c] of db.classes) out.push(`class:${q(id)}|s=${list(c.styles)}|t=${list(c.textStyles)}`);
  for (const [id, l] of db.links) out.push(`link:${q(id)}|${q(l.url)}|${q(l.tooltip)}`);
  for (const n of db.graph.nodes) {
    out.push(
      `node:${q(n.id)}|${q(n.shape)}|${q(n.label)}|${list(n.description)}|${q(n.cssClasses)}|cs=${list(n.cssCompiledStyles)}|` +
        `s=${list(n.cssStyles)}|${q(n.type)}|${n.isGroup}|p=${q(n.parentId)}|d=${q(n.dir)}|${q(n.position)}|${q(n.noteFor)}|${opt(n.colorIndex)}`
    );
  }
  for (const e of db.graph.edges) out.push(`edge:${q(e.id)}|${q(e.start)}|${q(e.end)}|${q(e.label)}|${e.pattern === 'dashed'}`);
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

export const memberSummary = (m) =>
  `${m.memberType}:${q(m.visibility)}|${q(m.id)}|${q(m.classifier)}|${q(m.parameters)}|${q(m.returnType)}|${q(m.getDisplayDetails().displayText)}`;
export function erSummary(db, buildErGraph) {
  const out = [`dir:${q(db.direction)}`];
  for (const [name, e] of db.entities) {
    out.push(`entity:${q(name)}|${q(e.id)}|${q(e.label)}|${q(e.alias)}|${q(e.cssClasses)}|s=${list(e.cssStyles)}`);
    for (const a of e.attributes) out.push(`  attr:${q(a.type)}|${q(a.name)}|${list(a.keys)}|${q(a.comment)}`);
  }
  for (const r of db.relationships) {
    out.push(`rel:${q(r.entityA)}|${q(r.roleA)}|${q(r.entityB)}|${r.relSpec.cardA}|${r.relSpec.relType}|${r.relSpec.cardB}`);
  }
  for (const [id, c] of db.classes) out.push(`class:${q(id)}|s=${list(c.styles)}|t=${list(c.textStyles)}`);
  for (const g of db.subgraphs) {
    out.push(`subgraph:${q(g.id)}|${list(g.nodes)}|${q(g.title)}|c=${list(g.classes)}|s=${list(g.cssStyles)}|${q(g.dir)}`);
  }
  const graph = buildErGraph(db);
  for (const n of graph.nodes) {
    out.push(
      `node:${q(n.id)}|${q(n.label)}|${q(n.alias)}|${n.isGroup}|p=${q(n.parentId)}|${list(n.cssCompiledStyles)}|${q(n.cssClasses)}|${q(n.dir)}`
    );
  }
  for (const e of graph.edges) {
    out.push(`edge:${q(e.id)}|${q(e.start)}|${q(e.end)}|${q(e.label)}|${e.arrowTypeStart}|${e.arrowTypeEnd}|${e.pattern}`);
  }
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

export function journeySummary(db) {
  const out = [];
  for (const s of db.sections) out.push(`section:${q(s)}`);
  for (const t of db.tasks) out.push(`task:${q(t.task)}|${t.score}|${list(t.people)}|${q(t.section)}`);
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

export function timelineSummary(db) {
  const out = [`dir:${q(db.direction)}`];
  for (const s of db.sections) out.push(`section:${q(s)}`);
  for (const p of db.periods) out.push(`period:${q(p.text)}|${p.sectionIndex}|${list(p.events)}`);
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

// A kanban board: each column followed by its cards.
export function kanbanSummary(db) {
  const node = (kind, n) =>
    `${kind}:${q(n.id)}|${n.level}|${q(n.label)}|${q(n.icon)}|${q(n.assigned)}|${q(n.ticket)}|${q(n.priority)}|${q(n.cssClasses)}`;
  const out = [];
  for (const s of db.sections) {
    out.push(node('section', s));
    for (const item of s.items) out.push(node('item', item));
  }
  out.push(`title:${q(db.title)}`);
  return out.join('\n');
}

// A gantt chart: its settings, then its tasks as scheduled at time 0 (run with TZ=UTC).
export function ganttSummary(db) {
  const t = (d) => (d && Number.isFinite(+d) ? String(+d) : '-');
  const out = [
    `title:${q(db.title)}|${q(db.accTitle)}|${q(db.accDescr)}`,
    `format:${q(db.dateFormat)}|${q(db.axisFormat)}|${q(db.tickInterval)}|${q(db.todayMarker)}|${db.weekday ?? '-'}|${db.weekend}`,
    `days:${q(db.includes.join(','))}|${q(db.excludes.join(','))}|${db.inclusiveEndDates}|${db.topAxis}`,
    `sections:${q(db.sections.join('|'))}`,
  ];
  for (const k of db.getTasks(0)) {
    const flags = ['active', 'done', 'crit', 'milestone', 'vert'].filter((f) => k[f]).join(',');
    out.push(`task:${q(k.id)}|${q(k.task)}|${q(k.section)}|${t(k.startTime)}|${t(k.endTime)}|${t(k.renderEndTime)}|${flags}|${k.order}|${q(k.classes.join(','))}`);
  }
  for (const [id, url] of db.links) out.push(`link:${q(id)}|${q(url)}`);
  return out.join('\n');
}

// A mind map: its nodes in source order, each with its parent.
export function mindmapSummary(db) {
  const parent = new Map();
  for (const n of db.nodes) for (const c of n.children) parent.set(c.id, n.id);
  const out = db.nodes.map(
    (n) =>
      `node:${n.id}|${q(n.nodeId)}|${n.level}|${q(n.descr)}|${n.type}|${parent.get(n.id) ?? '-'}|${n.section ?? '-'}|${q(n.class)}|${q(n.icon)}`
  );
  out.push(`title:${q(db.title)}`);
  return out.join('\n');
}

export function requirementSummary(db, buildRequirementGraph) {
  const out = [`dir:${q(db.direction)}`];
  for (const [name, r] of db.requirements) {
    out.push(
      `req:${q(name)}|${q(r.type)}|${q(r.requirementId)}|${q(r.text)}|${q(r.risk)}|${q(r.verifyMethod)}|s=${list(r.cssStyles)}|c=${list(r.classes)}`
    );
  }
  for (const [name, e] of db.elements) {
    out.push(`element:${q(name)}|${q(e.type)}|${q(e.docRef)}|s=${list(e.cssStyles)}|c=${list(e.classes)}`);
  }
  for (const r of db.relations) out.push(`rel:${q(r.type)}|${q(r.src)}|${q(r.dst)}`);
  for (const [id, c] of db.classes) out.push(`class:${q(id)}|s=${list(c.styles)}|t=${list(c.textStyles)}`);
  const graph = buildRequirementGraph(db);
  for (const n of graph.nodes) out.push(`node:${q(n.id)}|${n.requirement !== undefined}|${list(n.cssStyles)}|${q(n.cssClasses)}`);
  for (const e of graph.edges) out.push(`edge:${q(e.id)}|${q(e.start)}|${q(e.end)}|${q(e.type)}|${e.contains}`);
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

export function classSummary(db, buildClassGraph) {
  const out = [`dir:${q(db.direction)}`];
  for (const [id, c] of db.classes) {
    out.push(
      `class:${q(id)}|${q(c.type)}|${q(c.label)}|${q(c.cssClasses)}|a=${list(c.annotations)}|s=${list(c.styles)}|p=${q(c.parent)}|` +
        `l=${q(c.link)}|${q(c.linkTarget)}|cb=${c.haveCallback === true}|t=${q(c.tooltip)}`
    );
    for (const m of c.members) out.push(`  member:${memberSummary(m)}`);
    for (const m of c.methods) out.push(`  method:${memberSummary(m)}`);
  }
  for (const r of db.relations) {
    const { type1, type2, lineType } = r.relation;
    out.push(`rel:${q(r.id1)}|${q(r.id2)}|${q(r.relationTitle1)}|${q(r.relationTitle2)}|${q(r.title)}|${type1}|${type2}|${lineType}`);
  }
  for (const [id, n] of db.notes) out.push(`note:${q(id)}|${q(n.class)}|${q(n.text)}|${n.index}|p=${q(n.parent)}`);
  for (const [id, ns] of db.namespaces) {
    out.push(
      `ns:${q(id)}|${q(ns.label)}|c=${list([...ns.classes.keys()])}|n=${list([...ns.notes.keys()])}|k=${list([...ns.children.keys()])}|p=${q(ns.parent)}|${ns.explicit}`
    );
  }
  for (const i of db.interfaces) out.push(`iface:${q(i.id)}|${q(i.label)}|${q(i.classId)}`);
  for (const [id, sc] of db.styleClasses) out.push(`style:${q(id)}|s=${list(sc.styles)}|t=${list(sc.textStyles)}`);
  for (const hierarchical of [true, false]) {
    const graph = buildClassGraph(db, hierarchical);
    for (const n of graph.nodes) {
      out.push(`node:${hierarchical}|${q(n.id)}|${q(n.label)}|${n.shape}|p=${q(n.parentId)}|${n.parent}|${opt(n.colorIndex)}`);
    }
    for (const e of graph.edges) {
      out.push(
        `edge:${hierarchical}|${q(e.id)}|${e.source}|${e.target}|${q(e.label)}|${e.arrowTypeStart}|${e.arrowTypeEnd}|${q(e.startLabelRight)}|${q(e.endLabelLeft)}|${e.pattern}`
      );
    }
  }
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

// A sankey chart: its nodes in the order they were named, then its bands.
export function sankeySummary(db) {
  const out = db.nodes.map((n) => `node:${q(n.id)}|${n.index}`);
  for (const l of db.links) out.push(`link:${q(l.source.id)}|${q(l.target.id)}|${q(l.value)}`);
  out.push(`title:${q(db.title)}`);
  return out.join('\n');
}

// A quadrant chart: its four boxes, its axes, its points and its classes.
export function quadrantSummary(db) {
  const out = [];
  out.push(`quadrants:${db.quadrants.map((q) => (q ? q.text : '-')).join('|')}`);
  out.push(
    `axes:${q(db.xAxisLeft?.text)}|${q(db.xAxisRight?.text)}|${q(db.yAxisBottom?.text)}|${q(db.yAxisTop?.text)}`
  );
  for (const p of db.points) {
    out.push(`point:${q(p.text.text)}|${p.x}|${p.y}|${q(p.className)}|${styleSummary(p)}`);
  }
  for (const [name, style] of db.classes) {
    out.push(`class:${q(name)}|${styleSummary(style)}`);
  }
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}

// The style of a point or of a class, in the order the keys are read.
function styleSummary(style) {
  const out = [];
  if (style.radius !== undefined) out.push(`radius:${style.radius}`);
  if (style.color !== undefined) out.push(`color:${style.color}`);
  if (style.strokeColor !== undefined) out.push(`stroke-color:${style.strokeColor}`);
  if (style.strokeWidth !== undefined) out.push(`stroke-width:${style.strokeWidth}`);
  return out.join(',');
}

// An XY chart: its axes, then its series.
export function xychartSummary(db) {
  const out = [];
  const x = db.xAxis;
  if (x.type === 'band') {
    out.push(`xaxis:band|${q(x.title)}|${list(x.categories)}`);
  } else {
    out.push(`xaxis:linear|${q(x.title)}|${x.min}|${x.max}`);
  }
  out.push(`yaxis:linear|${q(db.yAxis.title)}|${db.yAxis.min}|${db.yAxis.max}`);
  out.push(`hasx:${db.hasSetXAxis}|hasy:${db.hasSetYAxis}`);
  out.push(`orientation:${q(db.orientation)}`);
  for (const p of db.plots) {
    const data = p.data.map(([k, v]) => `${q(k)}=${v === undefined ? '-' : v}`).join(',');
    out.push(`plot:${p.type}|${q(p.title)}|${data}|${p.pointLabels ? list(p.pointLabels) : '-'}`);
  }
  out.push(`title:${q(db.title)}|acc:${q(db.accTitle)}|${q(db.accDescr)}`);
  return out.join('\n');
}
