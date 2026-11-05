// The tree view: what Mermaid makes of a source, as a string.
//
// Mermaid reads a tree view with its own Langium parser, so the model is filled from Mermaid's own
// syntax tree. The tree of the model is built by pele's port of Mermaid's tree view database, and
// the box-drawing characters a tree may be drawn with are turned into an indentation by pele's
// port of Mermaid's preprocessor.
//
// This module is the shape every summary module of this directory has: the summary of the model,
// the modules of a pele checkout the reference needs, what a corpus entry of the type starts
// with, what the mutator writes into an input, and the reference itself.
import { loadPele, q } from '../lib.mjs';

export const treeviewSummary = (model) => {
  const out = [];
  out.push(`title:${q(model.title)}|accTitle:${q(model.accTitle)}|accDescr:${q(model.accDescr)}`);
  // The nodes are in the order they were read, which is the order a walk of the tree visits
  // them in.
  const walk = (node) => {
    out.push(
      `node:${node.id}|${node.level}|${q(node.name)}|${node.nodeType}|${q(node.cssClass)}|` +
        `${q(node.icon)}|${q(node.description)}`
    );
    for (const child of node.children) walk(child);
  };
  for (const child of model.root.children) walk(child);
  out.push(`count:${model.count}`);
  return out.join('\n');
};

export const treeviewModules = {
  '@mermaid-js/parser': ['parse as mermaidParse'],
  'src/diagrams/treeview/model.ts': ['buildTreeView'],
  'src/diagrams/treeview/boxDrawing.ts': ['preprocessBoxDrawing'],
  'src/preprocess.ts': ['encodeEntities'],
};

export const treeviewKeyword = /(^|\n)\s*treeView-beta\b/;

export const treeviewFragments = [
  'treeView-beta',
  'treeView-beta:',
  '    ',
  '\t',
  'src/',
  'index.js',
  'a',
  'A',
  '"',
  "'",
  ':::',
  ':::highlight',
  'icon(',
  'icon(logos:react)',
  'icon()',
  '##',
  '## a description',
  'title',
  'title A title',
  'accTitle:',
  'accDescr:',
  'accDescr {',
  '%%',
  '\n',
  ' ',
  'é',
  '0',
  '├──',
  '└──',
  '│',
  '─',
];

// The bundle of the checkout and Mermaid's own parser, built once.
let bundle;
async function reference(peleDir) {
  bundle ??= await loadPele(peleDir, treeviewModules);
  return bundle;
}

export const treeviewReference = async (peleDir, src) => {
  const b = await reference(peleDir);
  try {
    // Mermaid encodes the entities of the whole diagram before any parser sees it, and adds a
    // line break to the text it parses.
    const text = b.encodeEntities(src) + '\n';
    // A tree may be drawn with box-drawing characters, which Mermaid turns into an indentation
    // before its parser reads the text.
    const { text: drawn } = b.preprocessBoxDrawing(text);
    const ast = await b.mermaidParse('treeView', drawn);
    const model = b.buildTreeView(ast, undefined);
    return treeviewSummary(model);
  } catch (e) {
    // Mermaid's parser refuses the source; pele-rs refuses it as well.
    const err = new Error(String(e.message ?? e));
    err.code = 'syntax';
    throw err;
  }
};
