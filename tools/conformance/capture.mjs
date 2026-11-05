// Loaded only by the external Vitest capture config; never by the reference build.
import { appendFileSync } from 'node:fs';

const models = new WeakMap();
const seen = new Set();
export function parsed(run, source, options) {
  const model = run(source, options);
  models.set(model, { source, options: options ?? {} });
  return model;
}

export function captured(run, input, options = {}, registry) {
  const model = typeof input === 'string' ? { source: input, options: {} } : models.get(input);
  if (!model) return run(input, options);
  const plain = { ...options, now: options.now !== undefined && Number.isFinite(+options.now) ? +options.now : Date.now() };
  const widths = new Map();
  const icons = new Map();
  const math = new Map();
  const actual = { ...options, now: plain.now };
  if (options.measurer) actual.measurer = { width(text, size, style) {
    const width = options.measurer.width(text, size, style);
    widths.set(JSON.stringify([text, size, style]), width);
    return width;
  } };
  if (options.icons) actual.icons = name => {
    const icon = options.icons(name);
    icons.set(name, icon ?? null);
    return icon;
  };
  if (options.math) actual.math = (tex, size) => {
    const value = options.math(tex, size);
    math.set(JSON.stringify([tex, size]), value ?? null);
    return value;
  };
  delete plain.measurer;
  delete plain.icons;
  delete plain.math;
  let result, error;
  try { result = run(input, actual); }
  catch (caught) { error = caught; }
  const state = globalThis.expect.getState();
  const entry = {
    unregistered: error?.code === "unsupported-diagram" && registry?.length === 0 || undefined,
    foreignException: error && !error.code || undefined,
    file: state.testPath?.replace(/^.*\/pele\//, ''),
    test: state.currentTestName,
    source: model.source,
    parseOptions: typeof input === "string" ? undefined : model.options,
    options: plain,
    customMeasurer: !!options.measurer,
    widths: [...widths].map(([key, width]) => [...JSON.parse(key), width]),
    icons: options.icons ? [...icons] : undefined,
    math: options.math ? [...math].map(([key, value]) => [...JSON.parse(key), value]) : undefined,
    result,
    error: error ? { name: error.name, code: error.code, message: error.message, line: error.line, column: error.column, snippet: error.snippet } : undefined,
  };
  const serialized = JSON.stringify(entry, (_, value) => typeof value === "number" && !Number.isFinite(value) ? { $number: String(value) } : value);
  if (!seen.has(serialized)) {
    seen.add(serialized);
    appendFileSync(`${process.env.PELE_CAPTURE_DIR}/${process.pid}.jsonl`, serialized + '\n');
  }
  if (error) throw error;
  return result;
}

function portable(value) {
  if (ArrayBuffer.isView(value)) return Array.from(value);
  if (value instanceof Map) return [...value].map(([k,v]) => [portable(k),portable(v)]);
  if (Array.isArray(value)) return value.map(portable);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v]) => [k,portable(v)]));
  if (typeof value === 'number' && !Number.isFinite(value)) return { $number: String(value) };
  return value;
}
export function unit(run, name, args) {
  const state=globalThis.expect?.getState();
  const record=state && /\/tests\/(?:util\/|layout\/|render\/(?:ink|palette)\.test)/.test(state.testPath ?? '');
  const before=record ? portable(args) : undefined;
  let result, error;
  try {result=run(...args);} catch(caught) {error=caught;}
  if (record) {
    const entry={name, origins:[`${state.testPath.replace(/^.*\/pele\//,'')}: ${state.currentTestName}`], args:before, after:portable(args), result:portable(result), error:error ? {code:error.code,message:error.message} : undefined};
    const serialized=JSON.stringify(entry);
    if (!seen.has(serialized)) {seen.add(serialized);appendFileSync(`${process.env.PELE_CAPTURE_DIR}/${process.pid}.units.jsonl`,serialized+'\n');}
  }
  if(error) throw error;
  return result;
}
