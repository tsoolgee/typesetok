// Times one live-typeset round trip the way the desktop app does it:
// documentStateToDocumentRoot -> JSON file -> tok-cli typeset-document -> JSON.parse
// -> structured clone (IPC) -> layoutToPageDescriptors.
// Usage: node bench-live-typeset.mjs <versionRoot> <tokCli> <label> <paragraphs> [extra cli args...]
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
import esbuild from 'esbuild';

const [versionRoot, tokCli, label, paraArg, ...rest] = process.argv.slice(2);
const stringIpc = rest.includes('--ipc=string');
const extra = rest.filter((a) => a !== '--ipc=string');
const N = Number(paraArg);
const RUNS = 7;

async function loadTs(rel) {
  const outfile = path.join(os.tmpdir(), `bench-${process.pid}-${rel.replace(/\W+/g, '_')}.mjs`);
  esbuild.buildSync({
    entryPoints: [path.join(versionRoot, rel)],
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    alias: {
      'tok-viewer': path.join(versionRoot, 'packages/tok-viewer/src/index.ts'),
      'tok-canvas': path.join(versionRoot, 'packages/tok-canvas/src/index.ts'),
      'tok-story-editor': path.join(versionRoot, 'packages/tok-story-editor/src/index.ts'),
    },
    outfile,
    logLevel: 'silent',
  });
  return import(pathToFileURL(outfile).href);
}

const { documentStateToDocumentRoot } = await loadTs('packages/tok-ui/src/engine/documentBridge.ts');
const { layoutToPageDescriptors } = await loadTs('packages/tok-ui/src/engine/typesetBridge.ts');

const texts = [
  'וַיְהִי בִּימֵי אֲחַשְׁוֵרוֹשׁ הוּא אֲחַשְׁוֵרוֹשׁ הַמֹּלֵךְ מֵהֹדּוּ וְעַד כּוּשׁ שֶׁבַע וְעֶשְׂרִים וּמֵאָה מְדִינָה. ',
  'בַּיָּמִים הָהֵם כְּשֶׁבֶת הַמֶּלֶךְ אֲחַשְׁוֵרוֹשׁ עַל כִּסֵּא מַלְכוּתוֹ אֲשֶׁר בְּשׁוּשַׁן הַבִּירָה. ',
  'בִּשְׁנַת שָׁלוֹשׁ לְמָלְכוֹ עָשָׂה מִשְׁתֶּה לְכָל שָׂרָיו וַעֲבָדָיו חֵיל פָּרַס וּמָדַי הַפַּרְתְּמִים וְשָׂרֵי הַמְּדִינוֹת לְפָנָיו. ',
];
const paragraphs = [];
for (let i = 0; i < N; i++) {
  const text = (texts[i % 3] + texts[(i + 1) % 3] + texts[(i + 2) % 3]).trim();
  const p = { id: `p-${i}`, styleId: 'normal', text };
  if (i % 3 === 0) p.spans = [{ start: 10, end: 40, format: { italic: true, color: '#aa0000' } }, { start: 60, end: 90, format: { fontWeight: 700 } }];
  paragraphs.push(p);
}
const docState = { title: 'bench', templateType: 'prose', flows: { gemara: paragraphs, rashi: [], tosafot: [], notes: [] } };
const lookupSources = new Map(paragraphs.filter((p) => p.spans).map((p) => [p.id, { text: p.text, spans: p.spans }]));
const lookup = (id) => lookupSources.get(id);

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'bench-'));
const resolvedExtra = extra.map((a) => (a === '@PKG' ? path.join(tmp, 'doc.tok') : a));
if (extra.includes('@PKG')) {
  fs.writeFileSync(path.join(tmp, 'pkg-in.json'), JSON.stringify(documentStateToDocumentRoot(docState), null, 2));
  const r = spawnSync(tokCli, ['save-package', path.join(tmp, 'pkg-in.json'), path.join(tmp, 'doc.tok')]);
  if (r.status !== 0) throw new Error(`save-package failed: ${r.stderr}`);
}

const samples = [];
let outBytes = 0;
let pageCount = 0;
for (let run = 0; run < RUNS; run++) {
  const t = {};
  let s = performance.now();
  const root = documentStateToDocumentRoot(docState);
  const inFile = path.join(tmp, 'in.json');
  const outFile = path.join(tmp, 'out.json');
  fs.writeFileSync(inFile, JSON.stringify(root, null, 2));
  t.prepare = performance.now() - s;

  s = performance.now();
  const r = spawnSync(tokCli, ['typeset-document', inFile, outFile, ...resolvedExtra], { windowsHide: true });
  t.engine = performance.now() - s;
  if (r.status !== 0) throw new Error(`typeset failed: ${r.stderr || r.stdout}`);

  s = performance.now();
  const raw = fs.readFileSync(outFile, 'utf-8');
  outBytes = raw.length;
  let pages;
  let cloned;
  if (stringIpc) {
    const sent = structuredClone({ layoutJson: raw });
    t.ipc = performance.now() - s;
    s = performance.now();
    const parsed = JSON.parse(sent.layoutJson);
    pages = Array.isArray(parsed) ? parsed : parsed.pages;
    cloned = { pages };
    t.parse = performance.now() - s;
  } else {
    const parsed = JSON.parse(raw);
    pages = Array.isArray(parsed) ? parsed : parsed.pages;
    t.parse = performance.now() - s;
    s = performance.now();
    cloned = structuredClone(Array.isArray(parsed) ? { pages } : { pages, preflight: parsed.preflight });
    t.ipc = performance.now() - s;
  }
  pageCount = pages.length;

  s = performance.now();
  layoutToPageDescriptors(cloned.pages, lookup);
  t.render = performance.now() - s;

  t.total = t.prepare + t.engine + t.parse + t.ipc + t.render;
  samples.push(t);
}

const median = (k) => {
  const v = samples.map((x) => x[k]).sort((a, b) => a - b);
  return v[Math.floor(v.length / 2)];
};
const f = (k) => `${median(k).toFixed(0).padStart(6)}`;
console.log(
  `${label.padEnd(34)} paras=${String(N).padStart(5)} pages=${String(pageCount).padStart(4)} json=${(outBytes / 1e6).toFixed(1).padStart(6)}MB | ` +
    `total ${f('total')} ms = prep ${f('prepare')} + engine ${f('engine')} + parse ${f('parse')} + ipc ${f('ipc')} + render ${f('render')}`
);
fs.rmSync(tmp, { recursive: true, force: true });
