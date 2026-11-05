// Run from any clean official checkout with PELE_CAPTURE_DIR set to a temporary directory.
import { fileURLToPath } from 'node:url';
process.env.TZ = "UTC";
const capture = fileURLToPath(new URL('./capture.mjs', import.meta.url));
export default {
  plugins: [{
    name: 'capture-official-core-calls',
    enforce: 'pre',
    transform(code, id) {
      const modules = {
        'svg/ink.ts': ['plainColor','ink','paints','inkFill'],
        'svg/theme.ts': ['rounded','resolveStyle','classNames','fontStyle','seriesColor'],
        'svg/palette.ts': ['palette'],
        'svg/builder.ts': ['esc','escText','num','spanStyle','textStyle'],
        'util/url.ts': ['sanitizeUrl','safeUrl','internalLink'],
        'util/yaml.ts': ['parseYaml'],
        'preprocess.ts': ['cleanupComments','normalizeLineBreaks','preprocess','encodeEntities'],
        'layout/layered.ts': ['layered'],
        'layout/compound.ts': ['compoundLayout'],
      };
      for (const [module,names] of Object.entries(modules)) {
        if (!id.endsWith('/src/'+module)) continue;
        const wrappers=[];
        for (const name of names) {
          const marker='export function '+name+'(';
          if (!code.includes(marker)) continue;
          code=code.replace(marker,'function reference_'+name+'(');
          wrappers.push(`export function ${name}(...args) { return captureUnit(reference_${name}, ${JSON.stringify(module+':'+name)}, args); }`);
        }
        return code+`\nimport { unit as captureUnit } from ${JSON.stringify(capture)};\n`+wrappers.join('\n');
      }
      if (!id.endsWith('/src/core.ts')) return;
      return code.replace('export function render(', 'function referenceRender(')
        .replace('export function parse(', 'function referenceParse(') + `
import { captured, parsed } from ${JSON.stringify(capture)};
import { defaultMeasurer as capturedDefaultMeasurer } from "./text/measurer.js";
export function render(source, options) { return captured(referenceRender, source, { ...options, measurer: options?.measurer ?? capturedDefaultMeasurer(options?.fontFamily, options?.fontFamilyMono) }, [...diagrams.keys()]); }
export function parse(source, options) { return parsed(referenceParse, source, options); }
`;
    },
  }],
  test: {
    globals: true,
    pool: 'forks',
    maxWorkers: 2,
    testTimeout: 60000,
    include: ['tests/**/*.{test,spec}.{ts,js}'],
    exclude: ['tests/website/**'],
  },
};
