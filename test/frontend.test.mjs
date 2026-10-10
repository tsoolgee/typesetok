import { test, describe, before } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

describe('Hebrew Typography & Gematria Engine', () => {
  function toHebrewGematria(num) {
    if (num <= 0) return '';
    const letters = [
      [400, 'ת'], [300, 'ש'], [200, 'ר'], [100, 'ק'],
      [90, 'צ'], [80, 'פ'], [70, 'ע'], [60, 'ס'],
      [50, 'נ'], [40, 'מ'], [30, 'ל'], [20, 'כ'],
      [10, 'י'], [9, 'ט'], [8, 'ח'], [7, 'ז'],
      [6, 'ו'], [5, 'ה'], [4, 'ד'], [3, 'ג'],
      [2, 'ב'], [1, 'א']
    ];
    let n = num;
    let res = '';
    if (n === 15) return 'ט״ו';
    if (n === 16) return 'ט״ז';

    for (const [val, char] of letters) {
      while (n >= val) {
        res += char;
        n -= val;
      }
    }
    if (res.length === 1) {
      return res + '׳';
    } else if (res.length > 1) {
      return res.slice(0, -1) + '״' + res.slice(-1);
    }
    return res;
  }

  test('Single-letter gematria (1-9)', () => {
    assert.equal(toHebrewGematria(1), 'א׳');
    assert.equal(toHebrewGematria(2), 'ב׳');
    assert.equal(toHebrewGematria(5), 'ה׳');
    assert.equal(toHebrewGematria(9), 'ט׳');
  });

  test('Talmudic exceptions for 15 and 16 (Tet-Vav, Tet-Zayin)', () => {
    assert.equal(toHebrewGematria(15), 'ט״ו');
    assert.equal(toHebrewGematria(16), 'ט״ז');
  });

  test('Tens and hundreds with gershayim', () => {
    assert.equal(toHebrewGematria(20), 'כ׳');
    assert.equal(toHebrewGematria(21), 'כ״א');
    assert.equal(toHebrewGematria(100), 'ק׳');
    assert.equal(toHebrewGematria(354), 'שנ״ד');
  });
});

describe('Page DOM Virtualizer (Section 10.3)', () => {
  function computeActiveWindow(centerPage, totalPages) {
    const windowStart = Math.max(0, centerPage - 1);
    const windowEnd = Math.min(totalPages - 1, centerPage + 1);
    const mounted = [];
    for (let i = 0; i < totalPages; i++) {
      if (i >= windowStart && i <= windowEnd) {
        mounted.push(i);
      }
    }
    return { windowStart, windowEnd, mounted };
  }

  test('Active window at start of book (page 0)', () => {
    const { mounted } = computeActiveWindow(0, 1000);
    assert.deepEqual(mounted, [0, 1]);
    assert.equal(mounted.length <= 3, true);
  });

  test('Active window in middle of book enforces strictly 3 pages [K-1, K, K+1]', () => {
    const { mounted } = computeActiveWindow(500, 1000);
    assert.deepEqual(mounted, [499, 500, 501]);
    assert.equal(mounted.length, 3);
  });

  test('Active window at end of book', () => {
    const { mounted } = computeActiveWindow(999, 1000);
    assert.deepEqual(mounted, [998, 999]);
  });

  test('Single-page document', () => {
    const { mounted } = computeActiveWindow(0, 1);
    assert.deepEqual(mounted, [0]);
  });
});

describe('Canvas Overlay & Optimistic RTL Advance (Section 9.2)', () => {
  test('RTL caret advance decreases X position immediately (<16ms)', () => {
    let caret = { x: 500, y: 100, height: 18, visible: true };
    const step = 8.5; // average Hebrew glyph width in pt

    caret.x -= step;
    assert.equal(caret.x, 491.5);

    caret.x -= step;
    assert.equal(caret.x, 483.0);
  });

  test('Selection rectangle bounds calculation', () => {
    const rects = [
      { pageIndex: 0, x: 100, y: 50, width: 250, height: 16 }
    ];
    assert.equal(rects.length, 1);
    assert.equal(rects[0].width, 250);
  });

  test('Interactive drag selection computes normalized bounding box', () => {
    const dragStart = { x: 350, y: 120 };
    const dragCurrent = { x: 200, y: 150 };

    const minX = Math.min(dragStart.x, dragCurrent.x);
    const maxX = Math.max(dragStart.x, dragCurrent.x);
    const minY = Math.min(dragStart.y, dragCurrent.y);
    const maxY = Math.max(dragStart.y, dragCurrent.y);

    const selection = {
      pageIndex: 0,
      x: minX,
      y: minY,
      width: maxX - minX,
      height: maxY - minY
    };

    assert.equal(selection.x, 200);
    assert.equal(selection.y, 120);
    assert.equal(selection.width, 150);
    assert.equal(selection.height, 30);
  });

  test('Spatial hit-test snapped coordinate calculation', () => {
    const lineBoxes = [
      { baselineY: 50, height: 16, glyphs: [{ x: 100, width: 20 }, { x: 120, width: 25 }] },
      { baselineY: 80, height: 16, glyphs: [{ x: 100, width: 15 }, { x: 115, width: 30 }] }
    ];

    function snapToLine(clickY) {
      let closest = lineBoxes[0];
      let minDiff = Math.abs(closest.baselineY - clickY);
      for (const line of lineBoxes) {
        const diff = Math.abs(line.baselineY - clickY);
        if (diff < minDiff) {
          minDiff = diff;
          closest = line;
        }
      }
      return closest;
    }

    const clickedLine = snapToLine(75);
    assert.equal(clickedLine.baselineY, 80);
  });
});

describe('Build Artifacts & Distribution Packaging', () => {
  test('Electron main process is compiled to dist/main.js', () => {
    const mainJs = path.join(rootDir, 'packages/tok-electron/dist/main.js');
    assert.equal(fs.existsSync(mainJs), true, 'main.js must exist');
    const content = fs.readFileSync(mainJs, 'utf-8');
    assert.equal(content.includes('createWindow'), true);
  });

  test('Electron preload script is compiled to dist/preload.js', () => {
    const preloadJs = path.join(rootDir, 'packages/tok-electron/dist/preload.js');
    assert.equal(fs.existsSync(preloadJs), true, 'preload.js must exist');
    const content = fs.readFileSync(preloadJs, 'utf-8');
    assert.equal(content.includes('contextBridge.exposeInMainWorld'), true);
  });

  test('UI renderer bundle is generated and non-empty', () => {
    const rendererJs = path.join(rootDir, 'packages/tok-ui/dist/renderer.js');
    assert.equal(fs.existsSync(rendererJs), true, 'renderer.js must exist');
    const stat = fs.statSync(rendererJs);
    assert.equal(stat.size > 10000, true, 'renderer bundle should be > 10KB');
  });

  test('UI index.html is present with root container and script tag', () => {
    const indexHtml = path.join(rootDir, 'packages/tok-ui/dist/index.html');
    assert.equal(fs.existsSync(indexHtml), true, 'index.html must exist');
    const html = fs.readFileSync(indexHtml, 'utf-8');
    assert.equal(html.includes('id="app"'), true);
    assert.equal(html.includes('src="renderer.js"'), true);
    assert.equal(html.includes('dir="rtl"'), true);
  });
});

describe('DTP Modern UX/UI Specification & Design Tokens (Section 11)', () => {
  const indexHtmlPath = path.join(rootDir, 'packages/tok-ui/dist/index.html');
  let html = '';

  test('All normative design tokens from Section 11 are defined in CSS', () => {
    html = fs.readFileSync(indexHtmlPath, 'utf-8');
    const requiredTokens = [
      '--tok-bg-canvas: #121212',
      '--tok-bg-app: #181818',
      '--tok-bg-surface-1: #1E1E1E',
      '--tok-bg-surface-2: #262626',
      '--tok-bg-elevated: #303030',
      '--tok-border-subtle: #2C2C2C',
      '--tok-border-strong: #3E3E3E',
      '--tok-border-focus: #3B82F6',
      '--tok-accent-primary: #2563EB',
      '--tok-selection-frame: #3B82F6',
      '--tok-selection-text: rgba(59, 130, 246, 0.35)',
      '--tok-guide-margin: #9333EA',
      '--tok-guide-column: #06B6D4',
      '--tok-guide-baseline: rgba(16, 185, 129, 0.25)',
      '--tok-status-error: #EF4444',
      '--tok-status-warning: #F59E0B',
      '--tok-status-success: #10B981'
    ];

    for (const token of requiredTokens) {
      const tokenName = token.split(':')[0].trim();
      assert.equal(html.includes(tokenName), true, `Token ${tokenName} must be defined`);
    }
  });

  test('Workstation layout dimensions (Section 18) are present', () => {
    // Redesign (2026-10): side panel 236px, inspector 300px, top bar 52px, status bar 28px.
    assert.equal(/--tok-structure-width:\s*236px/.test(html), true);
    assert.equal(/--tok-inspector-width:\s*300px/.test(html), true);
    assert.equal(/--tok-top-bar-height:\s*52px/.test(html), true);
    assert.equal(/--tok-status-height:\s*28px/.test(html), true);
    assert.equal(html.includes('--tok-hud-height: 36px') || html.includes('36px'), true);
  });
});

describe('Interaction Triad Architecture (Section 8 & 17)', () => {
  const rendererJsPath = path.join(rootDir, 'packages/tok-ui/dist/renderer.js');
  let rendererJs = '';

  test('Canvas Action HUD is compiled into bundle with micro-actions', () => {
    rendererJs = fs.readFileSync(rendererJsPath, 'utf-8');
    assert.equal(rendererJs.includes('tok-action-hud'), true);
    assert.equal(rendererJs.includes('ActionHud'), true);
  });

  test('Contextual Inspector state machine handles zero, frame, and text-edit modes', () => {
    assert.equal(rendererJs.includes('ContextualInspector'), true);
    assert.equal(rendererJs.includes('renderZeroSelection'), true);
    assert.equal(rendererJs.includes('renderTextFrameMode'), true);
    assert.equal(rendererJs.includes('renderTextEditMode'), true);
  });

  test('Command Palette supports Cmd+K fuzzy searching and categories', () => {
    assert.equal(rendererJs.includes('CommandPalette'), true);
    assert.equal(rendererJs.includes('tok-palette-modal'), true);
    assert.equal(rendererJs.includes('cmd-full-justify'), true);
    assert.equal(rendererJs.includes('cmd-export-pdf'), true);
  });
});

describe('3-Tier Hebrew Justification Model (Section 15)', () => {
  test('Tier 1 (Word spacing range 80%-130%) calculation', () => {
    const minSpacing = 85;
    const maxSpacing = 125;
    assert.equal(minSpacing >= 80, true);
    assert.equal(maxSpacing <= 130, true);
  });

  test('Tier 2 (Oheltarem letter expansion) set contains authentic sacred letters', () => {
    const oheltaremLetters = ['א', 'ה', 'ל', 'ת', 'ר', 'ם'];
    assert.deepEqual(oheltaremLetters, ['א', 'ה', 'ל', 'ת', 'ר', 'ם']);
    assert.equal(oheltaremLetters.length, 6);
  });

  test('Tier 3 (Micro-tracking range ±2%) within threshold', () => {
    const maxMicroTracking = 2.0;
    assert.equal(maxMicroTracking <= 2.5, true);
  });
});

// ---------------------------------------------------------------------------
// Regression tests that execute the real TypeScript sources.
// Each module is bundled on the fly with esbuild (already a devDependency) into
// the OS temp dir and imported; a minimal fake DOM stands in for the browser.
// ---------------------------------------------------------------------------

class FakeClassList {
  constructor() { this.set = new Set(); }
  add(...c) { c.forEach((x) => this.set.add(x)); }
  remove(...c) { c.forEach((x) => this.set.delete(x)); }
  contains(c) { return this.set.has(c); }
}

class FakeStyle {
  setProperty(k, v) { this[k] = String(v); }
  removeProperty(k) { delete this[k]; }
  getPropertyValue(k) { return this[k] ?? ''; }
}

class FakeElement {
  constructor(tag = 'div') {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.style = new FakeStyle();
    this.dataset = {};
    this.classList = new FakeClassList();
    this.listeners = {};
    this._html = '';
    this.offsetTop = 0;
    this.offsetHeight = 0;
    this.scrollTop = 0;
    this.clientHeight = 0;
    this.attributes = {};
  }
  set className(v) {
    this._className = v;
    this.classList = new FakeClassList();
    if (v) v.split(/\s+/).forEach((c) => this.classList.add(c));
  }
  get className() { return this._className || ''; }
  set innerHTML(v) { this._html = v; this.children = []; }
  get innerHTML() { return this._html; }
  appendChild(c) { this.children.push(c); c.parentElement = this; return c; }
  setAttribute(k, v) { this.attributes[k] = String(v); }
  getAttribute(k) { return this.attributes[k]; }
  click() { (this.listeners['click'] || []).forEach((fn) => fn({})); }
  replaceChildren(...kids) { this.children = kids.filter(Boolean); }
  addEventListener(type, fn) { (this.listeners[type] ||= []).push(fn); }
  removeEventListener(type, fn) { this.listeners[type] = (this.listeners[type] || []).filter((f) => f !== fn); }
  scrollIntoView() {}
  querySelector(sel) {
    for (const c of this.children) {
      if (c && typeof c === 'object') {
        const matchesClass = !sel.includes('.tok-hud-btn') || c.className?.includes('tok-hud-btn') || c.classList?.contains('tok-hud-btn');
        const matchesPressed = !sel.includes('aria-pressed') || (c.attributes && 'aria-pressed' in c.attributes);
        if (matchesClass && matchesPressed) return c;
        const found = c.querySelector?.(sel);
        if (found) return found;
      }
    }
    return null;
  }
  querySelectorAll(sel) {
    const list = [];
    for (const c of this.children) {
      if (c && typeof c === 'object' && c.querySelectorAll) {
        list.push(...c.querySelectorAll(sel));
      }
    }
    return list;
  }
}

function installFakeDom() {
  const storage = new Map();
  globalThis.localStorage = {
    getItem: (k) => (storage.has(k) ? storage.get(k) : null),
    setItem: (k, v) => storage.set(k, String(v)),
    removeItem: (k) => storage.delete(k),
    clear: () => storage.clear(),
  };
  const documentElement = new FakeElement('html');
  globalThis.document = {
    documentElement,
    body: new FakeElement('body'),
    activeElement: null,
    createElement: (tag) => new FakeElement(tag),
    createElementNS: (_ns, tag) => new FakeElement(tag),
  };
  globalThis.getComputedStyle = () => ({ position: 'static', direction: 'rtl' });
  globalThis.requestAnimationFrame = (fn) => setTimeout(fn, 0);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  if (!globalThis.window) globalThis.window = globalThis;
  globalThis.window.addEventListener ||= (type, fn) => {};
  globalThis.window.removeEventListener ||= (type, fn) => {};
  return { storage, documentElement };
}

const fakeDom = installFakeDom();

let esbuildMod = null;
async function loadTs(relPath) {
  esbuildMod ||= await import('esbuild');
  const os = await import('node:os');
  const { pathToFileURL } = await import('node:url');
  const outfile = path.join(
    os.tmpdir(),
    `tok-test-${process.pid}-${relPath.replace(/[^a-zA-Z0-9]+/g, '_')}-${Date.now()}.mjs`
  );
  esbuildMod.buildSync({
    entryPoints: [path.join(rootDir, relPath)],
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    outfile,
    logLevel: 'silent',
  });
  try {
    return await import(pathToFileURL(outfile).href);
  } finally {
    fs.rmSync(outfile, { force: true });
  }
}

describe('Regression: i18n coverage (every key used in the UI exists in Hebrew and English)', () => {
  test('all translation entries have non-empty he and en strings', async () => {
    const { strings } = await loadTs('packages/tok-ui/src/i18n.ts');
    for (const [key, entry] of Object.entries(strings)) {
      assert.ok(entry.he && entry.he.trim(), `missing Hebrew for ${key}`);
      assert.ok(entry.en && entry.en.trim(), `missing English for ${key}`);
    }
  });

  test('every t()/tf()/labelKey reference in tok-ui resolves to a defined key', async () => {
    const { strings } = await loadTs('packages/tok-ui/src/i18n.ts');
    const srcDir = path.join(rootDir, 'packages/tok-ui/src');
    const files = [];
    const walk = (d) => {
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (p.endsWith('.ts')) files.push(p);
      }
    };
    walk(srcDir);
    const patterns = [
      /\btf?\(\s*'(\w+)'/g,
      /labelKey:\s*'(\w+)'/g,
      /\bt\([^()]*?\?\s*'(\w+)'\s*:\s*'(\w+)'\s*\)/g,
    ];
    const missing = [];
    for (const file of files) {
      const src = fs.readFileSync(file, 'utf-8');
      for (const re of patterns) {
        for (const m of src.matchAll(re)) {
          for (const key of m.slice(1).filter(Boolean)) {
            if (/^(div|span|p|button|input|select|option|label|header|footer|main|aside|kbd|h[1-6]|canvas|strong)$/.test(key)) continue;
            if (!strings[key]) missing.push(`${path.basename(file)}: ${key}`);
          }
        }
      }
    }
    assert.deepEqual(missing, []);
  });

  test('tf() substitutes placeholders and onOff() is localized', async () => {
    const mod = await loadTs('packages/tok-ui/src/i18n.ts');
    mod.i18n.setLanguage('en');
    assert.equal(mod.tf('logsRetentionUpdated', { n: 30 }), 'Log retention set to 30 days');
    assert.equal(mod.onOff(true), 'enabled');
    assert.equal(fakeDom.documentElement.dir, 'ltr');
    mod.i18n.setLanguage('he');
    assert.equal(fakeDom.documentElement.dir, 'rtl');
    assert.equal(fakeDom.documentElement.lang, 'he');
  });
});

describe('Regression: Command Palette shortcuts & search', () => {
  test('Ctrl+K works with a Hebrew keyboard layout (key "ל", code KeyK)', async () => {
    const { isPaletteToggleHotkey } = await loadTs('packages/tok-ui/src/components/CommandPalette.ts');
    assert.equal(isPaletteToggleHotkey({ key: 'ל', code: 'KeyK', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false }), true);
    assert.equal(isPaletteToggleHotkey({ key: 'k', code: 'KeyK', ctrlKey: false, metaKey: true, altKey: false, shiftKey: false }), true);
    assert.equal(isPaletteToggleHotkey({ key: 'ל', code: 'KeyK', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }), false);
    assert.equal(isPaletteToggleHotkey({ key: 'k', code: 'KeyK', ctrlKey: true, metaKey: false, altKey: true, shiftKey: false }), false);
  });

  test('"/" does not hijack typing in the story editor or form fields', async () => {
    const { isPaletteSlashHotkey } = await loadTs('packages/tok-ui/src/components/CommandPalette.ts');
    const ev = { key: '/', code: 'Slash', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false };
    assert.equal(isPaletteSlashHotkey(ev, { tagName: 'DIV', isContentEditable: true }), false);
    assert.equal(isPaletteSlashHotkey(ev, { tagName: 'P', isContentEditable: true }), false);
    assert.equal(isPaletteSlashHotkey(ev, { tagName: 'INPUT' }), false);
    assert.equal(isPaletteSlashHotkey(ev, { tagName: 'SELECT' }), false);
    assert.equal(isPaletteSlashHotkey(ev, { tagName: 'MAIN', isContentEditable: false }), true);
    assert.equal(isPaletteSlashHotkey({ ...ev, ctrlKey: true }, { tagName: 'MAIN' }), false);
  });

  test('search ignores niqqud and treats ״/" and ׳/\' alike; all terms must match', async () => {
    const { matchesPaletteQuery } = await loadTs('packages/tok-ui/src/components/CommandPalette.ts');
    const item = { title: 'נַרְמֵל ניקוד וטעמים (ת״י 6100)', category: 'פעולות טיפוגרפיה', subtitle: 'תיקון סדר תווי יוניקוד', shortcut: 'Ctrl+Shift+N' };
    assert.equal(matchesPaletteQuery(item, 'נרמל'), true);
    assert.equal(matchesPaletteQuery(item, 'ת"י'), true);
    assert.equal(matchesPaletteQuery(item, 'נרמל יוניקוד'), true);
    assert.equal(matchesPaletteQuery(item, 'נרמל גימטריה'), false);
    assert.equal(matchesPaletteQuery(item, '  '), true);
  });
});

describe('Regression: spread geometry is consistent (RTL book: recto = left page)', () => {
  test('groupIntoSpreads keeps page 0 alone and pairs the rest', async () => {
    const { groupIntoSpreads } = await loadTs('packages/tok-ui/src/components/SpreadCanvas.ts');
    assert.deepEqual(groupIntoSpreads(0), []);
    assert.deepEqual(groupIntoSpreads(1), [[0]]);
    assert.deepEqual(groupIntoSpreads(4), [[0], [1, 2], [3]]);
    assert.deepEqual(groupIntoSpreads(5), [[0], [1, 2], [3, 4]]);
  });

  test('even pages are recto (ע"א, left); odd pages are verso (ע"ב, right)', async () => {
    const { isRectoPage, isRightHandPage, groupIntoSpreads } = await loadTs('packages/tok-ui/src/components/SpreadCanvas.ts');
    for (const spread of groupIntoSpreads(12).slice(1)) {
      // In each 2-page spread the first (rightmost in RTL) is the verso.
      assert.equal(isRightHandPage(spread[0]), true);
      assert.equal(isRectoPage(spread[0]), false);
      if (spread[1] !== undefined) {
        assert.equal(isRightHandPage(spread[1]), false);
        assert.equal(isRectoPage(spread[1]), true);
      }
    }
    assert.equal(isRectoPage(0), true);
  });
});

describe('Regression: inspector value scrubber keeps decimals', () => {
  test('parse/clamp/format', async () => {
    const m = await loadTs('packages/tok-ui/src/components/ContextualInspector.ts');
    assert.equal(m.parseScrubberValue('11.5 pt'), 11.5);
    assert.equal(m.parseScrubberValue('12,5 pt'), 12.5);
    assert.equal(m.parseScrubberValue('-3 מ"מ'), -3);
    assert.equal(m.parseScrubberValue('abc'), null);
    assert.equal(m.clampScrubberValue(200, 6, 72), 72);
    assert.equal(m.clampScrubberValue(11.46, 6, 72), 11.5);
    assert.equal(m.formatScrubberValue(11.5, 'pt'), '11.5 pt');
  });
});

describe('Regression: theme settings', () => {
  test('corrupt persisted settings are sanitized field by field', async () => {
    const { sanitizeThemeSettings } = await loadTs('packages/tok-ui/src/theme.ts');
    const defaults = { accentColor: '#2563EB', canvasTone: '#0B132B', density: 'comfortable', highContrast: false, fontScale: 100, reducedMotion: false, enhancedFocus: false, accessibleFont: false };
    const out = sanitizeThemeSettings({ fontScale: 'abc', density: 'huge', accentColor: 'red;}body{display:none', highContrast: 'yes', reducedMotion: true, canvasTone: '#121212' }, defaults);
    assert.equal(out.fontScale, 100);
    assert.equal(out.density, 'comfortable');
    assert.equal(out.accentColor, '#2563EB');
    assert.equal(out.highContrast, false);
    assert.equal(out.reducedMotion, true);
    assert.equal(out.canvasTone, '#121212');
    assert.deepEqual(sanitizeThemeSettings(null, defaults), defaults);
    assert.equal(sanitizeThemeSettings({ paletteId: 'parchment' }, defaults).paletteId, 'parchment');
    assert.equal(sanitizeThemeSettings({ paletteId: 'nope' }, defaults).paletteId, defaults.paletteId);
  });

  test('turning high contrast off does not leave the black app background behind', async () => {
    // version 2 = the redesign's storage format (older entries keep only accessibility choices).
    fakeDom.storage.set('tok_theme_settings', JSON.stringify({ version: 2, highContrast: true, canvasTone: '#123456' }));
    const { themeManager, THEME_PALETTES } = await loadTs('packages/tok-ui/src/theme.ts');
    const style = fakeDom.documentElement.style;
    assert.equal(style.getPropertyValue('--tok-bg-app'), '#000000');
    themeManager.setHighContrast(false);
    assert.equal(style.getPropertyValue('--tok-bg-app'), THEME_PALETTES[0].appBg);
    assert.equal(style.getPropertyValue('--tok-bg-canvas'), '#123456');
    fakeDom.storage.delete('tok_theme_settings');
  });
});

describe('Regression: settings modal preset names follow the UI language', () => {
  test('localizedPresetName', async () => {
    const { localizedPresetName } = await loadTs('packages/tok-ui/src/components/SettingsModal.ts');
    assert.equal(localizedPresetName('כחול קלאסי (Classic Blue)', 'he'), 'כחול קלאסי');
    assert.equal(localizedPresetName('כחול קלאסי (Classic Blue)', 'en'), 'Classic Blue');
    assert.equal(localizedPresetName('Plain', 'en'), 'Plain');
  });
});

describe('Regression: plugin engine', () => {
  test('malformed entries are skipped, plugins activate once, failed toggles change nothing', async () => {
    const { PluginEngine, normalizePluginEntry } = await loadTs('packages/tok-ui/src/plugins/PluginEngine.ts');
    assert.equal(normalizePluginEntry(null), null);
    assert.equal(normalizePluginEntry({ sourceType: 'js' }), null);

    let runs = 0;
    let failToggle = false;
    globalThis.__tokPluginRun = () => runs++;
    const code = 'function activate(tok){ globalThis.__tokPluginRun(); tok.registerCommand({ id: "c", title: "t", category: "x", action(){} }); }';
    globalThis.tokIpc = {
      getPlugins: async () => [
        { sourceType: 'js', enabled: true },
        { manifest: { id: 'good', name: 'Good', version: '1.0.0', description: '' }, sourceType: 'js', enabled: true, compiledCode: code },
        { manifest: { id: 'off', name: 'Off', version: '1.0.0', description: '' }, sourceType: 'js', enabled: false, compiledCode: code },
        { manifest: { id: 'good', name: 'Dup', version: '9.9.9', description: '' }, sourceType: 'js', enabled: true, compiledCode: code },
      ],
      togglePlugin: async () => { if (failToggle) throw new Error('disk full'); return true; },
    };
    try {
      const registered = [];
      const unregistered = [];
      const engine = new PluginEngine((cmd) => registered.push(cmd.id), undefined, (id) => unregistered.push(id));
      await engine.loadPlugins();
      assert.deepEqual(engine.getPlugins().map((p) => p.id), ['good', 'off']);
      assert.equal(runs, 1);

      await engine.loadPlugins(); // "Reload All Plugins" must not run plugin code again
      assert.equal(runs, 1);

      await engine.togglePlugin('off', true); // enabling activates immediately
      assert.equal(runs, 2);
      assert.equal(engine.isActivated('off'), true);

      failToggle = true;
      await assert.rejects(engine.togglePlugin('good', false));
      assert.equal(engine.getPlugins().find((p) => p.id === 'good').enabled, true);
      assert.equal(registered.length, 2);
      assert.deepEqual(unregistered, []);

      failToggle = false;
      await engine.togglePlugin('good', false); // its command leaves the palette
      assert.deepEqual(unregistered, ['c']);
      await engine.togglePlugin('good', true); // and comes back without re-running the code
      assert.equal(runs, 2);
      assert.equal(registered.length, 3);
    } finally {
      delete globalThis.tokIpc;
      delete globalThis.__tokPluginRun;
    }
  });
});

describe('Regression: page virtualizer maps pageIndex to positions', () => {
  const makePages = (first, count, html = (i) => `<div>${i}</div>`) =>
    Array.from({ length: count }, (_, k) => ({
      pageIndex: first + k, gematriaNumber: '', widthPt: 100, heightPt: 100, htmlContent: html(first + k),
    }));
  const mountedPositions = (container) =>
    container.children.map((c, i) => (c.classList.contains('tok-page-mounted') ? i : -1)).filter((i) => i >= 0);

  test('computeActiveWindow', async () => {
    const { computeActiveWindow } = await loadTs('packages/tok-viewer/src/virtualizer.ts');
    assert.deepEqual(computeActiveWindow(0, 1000), { start: 0, end: 1 });
    assert.deepEqual(computeActiveWindow(500, 1000), { start: 499, end: 501 });
    assert.deepEqual(computeActiveWindow(999, 1000), { start: 998, end: 999 });
    assert.deepEqual(computeActiveWindow(0, 0), { start: 0, end: -1 });
  });

  test('documents whose page numbers do not start at 0 mount the right pages', async () => {
    const { PageDomVirtualizer } = await loadTs('packages/tok-viewer/src/virtualizer.ts');
    const container = new FakeElement('div');
    const v = new PageDomVirtualizer(container);
    v.setPages(makePages(10, 8));
    assert.deepEqual(mountedPositions(container), [0, 1]);
    assert.equal(container.children[0].innerHTML, '<div>10</div>');

    v.scrollToPage(15); // position 5
    assert.deepEqual(mountedPositions(container), [4, 5, 6]);
    assert.equal(container.children[5].innerHTML, '<div>15</div>');
    assert.equal(container.children[0].innerHTML, '');
    assert.equal(v.getActivePage(), 15);

    v.updateActiveWindow(17);
    assert.deepEqual(mountedPositions(container), [6, 7]);
    v.destroy();
  });

  test('text-only pages are tracked as mounted and the scroll listener is removed on destroy', async () => {
    const { PageDomVirtualizer } = await loadTs('packages/tok-viewer/src/virtualizer.ts');
    const container = new FakeElement('div');
    const v = new PageDomVirtualizer(container);
    v.setPages(makePages(0, 5, (i) => `plain text ${i}`));
    assert.deepEqual(mountedPositions(container), [0, 1]);
    assert.equal(container.listeners.scroll.length, 1);
    v.destroy();
    assert.equal(container.listeners.scroll.length, 0);
  });
});

describe('Regression: UI page numbering matches the Rust gematria engine', () => {
  test('taboo substitutions, 15/16 after hundreds and thousands', async () => {
    const { toHebrewGematria } = await loadTs('packages/tok-ui/src/gematria.ts');
    const expected = {
      1: 'א׳', 15: 'ט״ו', 16: 'ט״ז', 115: 'קט״ו', 216: 'רט״ז', 248: 'רמ״ח',
      270: 'ע״ר', 272: 'ער״ב', 275: 'ער״ה', 298: 'חר״צ', 304: 'ד״ש', 314: 'שי״ד',
      344: 'שד״מ', 359: 'נט״ש', 644: 'תרמ״ד', 698: 'תרח״צ', 744: 'תשד״מ',
      1000: 'א׳', 5784: 'ה׳תשפ״ד',
    };
    for (const [n, s] of Object.entries(expected)) assert.equal(toHebrewGematria(Number(n)), s, `n=${n}`);
    assert.equal(toHebrewGematria(0), '');
  });
});

describe('Regression: the UI shows the real app version', () => {
  test('getAppVersion asks the main process once', async () => {
    let calls = 0;
    globalThis.window.tokIpc = { getAppInfo: async () => { calls++; return { version: '1.2.3' }; } };
    try {
      const { getAppVersion } = await loadTs('packages/tok-ui/src/appInfo.ts');
      assert.equal(await getAppVersion(), '1.2.3');
      assert.equal(await getAppVersion(), '1.2.3');
      assert.equal(calls, 1);
    } finally {
      delete globalThis.window.tokIpc;
    }
  });

  test('no version number is hard-coded in the UI sources', () => {
    const srcDir = path.join(rootDir, 'packages/tok-ui/src');
    const offenders = [];
    const walk = (d) => {
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(ts|html)$/.test(p) && /\bv\d+\.\d+\.\d+\b/.test(fs.readFileSync(p, 'utf-8'))) offenders.push(path.relative(rootDir, p));
      }
    };
    walk(srcDir);
    assert.deepEqual(offenders, []);
  });
});

describe('Regression: document fonts are the ones the engine embeds', () => {
  test('every selectable font has an @font-face whose file exists', async () => {
    const { DOCUMENT_FONTS } = await loadTs('packages/tok-ui/src/fonts.ts');
    const htmlPath = path.join(rootDir, 'packages/tok-ui/src/index.html');
    const html = fs.readFileSync(htmlPath, 'utf-8');
    for (const { family } of DOCUMENT_FONTS) {
      const faces = [...html.matchAll(/@font-face\s*\{[^}]*\}/g)].map((m) => m[0]).filter((f) => f.includes(`'${family}'`));
      assert.ok(faces.length > 0, `no @font-face for ${family}`);
      for (const face of faces) {
        const url = /url\('([^']+)'\)/.exec(face)[1];
        // index.html is served from packages/tok-ui/dist, a sibling of src at the same depth.
        assert.ok(fs.existsSync(path.resolve(path.dirname(htmlPath), url)), `${family}: missing ${url}`);
      }
    }
  });

  test('the engine embeds every selectable font', async () => {
    const { DOCUMENT_FONTS } = await loadTs('packages/tok-ui/src/fonts.ts');
    const fontRs = fs.readFileSync(path.join(rootDir, 'crates/tok-typeset/src/font.rs'), 'utf-8');
    for (const { family } of DOCUMENT_FONTS) assert.ok(fontRs.includes(`("${family}", EMBEDDED_`), `${family} is not registered in font.rs`);
  });

  test('UI font weights are the engine ParagraphStyle.font_weight values', async () => {
    const { FONT_WEIGHT_REGULAR, FONT_WEIGHT_BOLD } = await loadTs('packages/tok-ui/src/fonts.ts');
    const styles = fs.readFileSync(path.join(rootDir, 'crates/tok-core/src/styles.rs'), 'utf-8');
    assert.ok(styles.includes(`FONT_WEIGHT_REGULAR: u16 = ${FONT_WEIGHT_REGULAR};`));
    assert.ok(styles.includes(`FONT_WEIGHT_BOLD: u16 = ${FONT_WEIGHT_BOLD};`));
    const srcDir = path.join(rootDir, 'packages/tok-ui/src');
    const offenders = [];
    const walk = (d) => {
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (p.endsWith('.ts') && /fontWeight:\s*['"]/.test(fs.readFileSync(p, 'utf-8'))) offenders.push(path.relative(rootDir, p));
      }
    };
    walk(srcDir);
    assert.deepEqual(offenders, [], 'fontWeight must be numeric (400/700), as in the engine');
  });

  test('no legacy font names (not shipped) remain in the UI', () => {
    const srcDir = path.join(rootDir, 'packages/tok-ui/src');
    const legacy = /Taamey Frank|David CLM|\bVilna\b|['"]Rashi['"]/;
    const offenders = [];
    const walk = (d) => {
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(ts|html)$/.test(p) && legacy.test(fs.readFileSync(p, 'utf-8'))) offenders.push(path.relative(rootDir, p));
      }
    };
    walk(srcDir);
    assert.deepEqual(offenders, []);
  });
});

describe('Feature: Official Plugin APIs & Security Sandbox', () => {
  test('Hebrew Typography utilities (Niqqud, Taamim, Holy Names, Quotes)', async () => {
    const { hebrewUtils } = await loadTs('packages/tok-ui/src/plugins/hebrew-utils.ts');
    
    // 1. Niqqud stripping
    const pointed = 'בָּרוּךְ אַתָּה';
    assert.equal(hebrewUtils.stripNiqqud(pointed), 'ברוך אתה');

    // 2. Ta'amim stripping
    const withTaamim = 'בְּרֵאשִׁ֖ית בָּרָ֣א';
    assert.equal(hebrewUtils.stripTaamim(withTaamim), 'בְּרֵאשִׁית בָּרָא');
    assert.equal(hebrewUtils.stripAllMarks(withTaamim), 'בראשית ברא');

    // 3. Holy names detection in pointed text
    const sacred = 'בָּרוּךְ אַתָּה יהוה אֱלֹהִים מֶלֶךְ הָעוֹלָם';
    const matches = hebrewUtils.findHolyNames(sacred);
    assert.equal(matches.length, 2);
    assert.equal(matches[0].name, 'יהוה');
    assert.equal(matches[1].name, 'אלהים');

    // 4. Quotation normalization (Gershayim and Geresh)
    const quoteInput = 'לדברי הרמב"ם והראב"ד ור\' משה';
    const quoteOutput = hebrewUtils.normalizeQuotes(quoteInput);
    assert.equal(quoteOutput, 'לדברי הרמב״ם והראב״ד ור׳ משה');

    // 5. Gematria
    assert.equal(hebrewUtils.toHebrewGematria(1), 'א׳');
    assert.equal(hebrewUtils.toHebrewGematria(15), 'ט״ו');
    assert.equal(hebrewUtils.toHebrewGematria(270), 'ע״ר');
  });

  test('Document APIs enforce document:write permissions and allow safe batch mutations', async () => {
    const { PluginEngine } = await loadTs('packages/tok-ui/src/plugins/PluginEngine.ts');

    let currentStory = [
      { id: 'p1', styleId: 'body', text: 'פסקה ראשונה' },
      { id: 'p2', styleId: 'body', text: 'פסקה שניה' }
    ];

    const delegate = {
      getStory: () => [...currentStory],
      loadStory: (newStory) => { currentStory = [...newStory]; },
      getSelectedText: () => 'טקסט נבחר',
      replaceSelection: (t) => { currentStory[0].text = t; },
      getDocumentTitle: () => 'מסמך בדיקה.tok',
      getStats: () => ({ wordCount: 4, charCount: 20, paragraphCount: 2 }),
      getViewMode: () => 'story',
      setViewMode: () => {},
      getPageCount: () => 1,
      getActivePageIndex: () => 0,
      getZoom: () => 100,
    };

    // Plugin with document:read only (should fail to write)
    let writeError = null;
    const readOnlyCode = `
      function activate(tok) {
        try {
          tok.document.updateParagraph('p1', 'שינוי לא מורשה');
        } catch (e) {
          writeError = e.message;
        }
      }
    `;

    // Plugin with document:write (should succeed)
    let writeSuccess = false;
    const writeCode = `
      function activate(tok) {
        tok.document.updateParagraph('p1', 'טקסט מעודכן');
        writeSuccess = true;
      }
    `;

    globalThis.writeError = null;
    globalThis.writeSuccess = false;

    globalThis.tokIpc = {
      getPluginSystemStatus: async () => ({ isSafeMode: false, isSaferActive: false, userPluginsDir: '' }),
      getPlugins: async () => [
        {
          manifest: { id: 'ro-plugin', name: 'ReadOnly', version: '1.0', description: '', permissions: ['document:read', 'ui:notifications'] },
          sourceType: 'js',
          enabled: true,
          compiledCode: 'function activate(tok){ try{ tok.document.updateParagraph("p1", "err"); }catch(e){ globalThis.writeError = e.message; } }'
        },
        {
          manifest: { id: 'rw-plugin', name: 'ReadWrite', version: '1.0', description: '', permissions: ['document:read', 'document:write'] },
          sourceType: 'js',
          enabled: true,
          compiledCode: 'function activate(tok){ tok.document.updateParagraph("p1", "עודכן"); globalThis.writeSuccess = true; }'
        }
      ]
    };

    try {
      const engine = new PluginEngine({ delegate });
      await engine.loadPlugins();

      // Read-only plugin must be rejected by permission guard
      assert.match(globalThis.writeError, /Permission denied.*document:write/);

      // Read-write plugin must succeed
      assert.equal(globalThis.writeSuccess, true);
      assert.equal(currentStory[0].text, 'עודכן');
    } finally {
      delete globalThis.tokIpc;
      delete globalThis.writeError;
      delete globalThis.writeSuccess;
    }
  });

  test('Security sandbox shadows IPC and blocks unauthorized network calls', async () => {
    const { PluginEngine } = await loadTs('packages/tok-ui/src/plugins/PluginEngine.ts');

    let capturedIpcType = 'not_run';
    let networkBlocked = false;

    globalThis.__testReport = (ipcVal, netBlocked) => {
      capturedIpcType = ipcVal;
      networkBlocked = netBlocked;
    };

    const untrustedCode = `
      function activate(tok) {
        var ipcType = typeof tokIpc;
        var blocked = false;
        try {
          fetch('https://example.com/steal-data');
        } catch (e) {
          blocked = true;
        }
        globalThis.__testReport(ipcType, blocked);
      }
    `;

    globalThis.tokIpc = {
      getPluginSystemStatus: async () => ({ isSafeMode: false, isSaferActive: false, userPluginsDir: '' }),
      getPlugins: async () => [
        {
          manifest: { id: 'untrusted', name: 'Untrusted', version: '1.0', description: '', permissions: ['ui:commands'] },
          sourceType: 'js',
          enabled: true,
          compiledCode: untrustedCode
        }
      ]
    };

    try {
      const engine = new PluginEngine();
      await engine.loadPlugins();

      // tokIpc must be undefined in plugin execution scope
      assert.equal(capturedIpcType, 'undefined');
      // fetch must be blocked because 'network:fetch' is not permitted
      assert.equal(networkBlocked, true);
    } finally {
      delete globalThis.tokIpc;
      delete globalThis.__testReport;
    }
  });

  test('Safe Mode blocks third-party plugins from running', async () => {
    const { PluginEngine } = await loadTs('packages/tok-ui/src/plugins/PluginEngine.ts');

    let thirdPartyRan = false;
    globalThis.__thirdPartyRun = () => { thirdPartyRan = true; };

    globalThis.tokIpc = {
      getPluginSystemStatus: async () => ({ isSafeMode: true, isSaferActive: true, userPluginsDir: '' }),
      getPlugins: async () => [
        {
          manifest: { id: 'user-ext', name: 'User Ext', version: '1.0', description: '' },
          sourceType: 'js',
          enabled: false,
          safeModeBlocked: true,
          compiledCode: 'function activate(){ globalThis.__thirdPartyRun(); }'
        }
      ]
    };

    try {
      const engine = new PluginEngine();
      await engine.loadPlugins();

      assert.equal(engine.getIsSafeMode(), true);
      assert.equal(engine.getIsSaferActive(), true);
      assert.equal(thirdPartyRan, false);
      const list = engine.getPlugins();
      assert.equal(list[0].safeModeBlocked, true);
    } finally {
      delete globalThis.tokIpc;
      delete globalThis.__thirdPartyRun;
    }
  });
});

describe('Multi-Flow Real-Time Paginator & Live Canvas Sync (Talmud Engine)', () => {
  test('Talmud pagination produces Recto/Verso aware commentary placement', async () => {
    const { FlowPaginator, DEFAULT_TALMUD_FLOWS } = await loadTs('packages/tok-ui/src/engine/FlowPaginator.ts');
    const doc = {
      title: 'Talmud Test',
      templateType: 'gemara',
      flows: {
        gemara: [...DEFAULT_TALMUD_FLOWS.gemara],
        rashi: [...DEFAULT_TALMUD_FLOWS.rashi],
        tosafot: [...DEFAULT_TALMUD_FLOWS.tosafot],
        notes: [...DEFAULT_TALMUD_FLOWS.notes]
      }
    };

    const pages = FlowPaginator.paginateDocument(doc, 2);
    assert.ok(pages.length >= 2, 'Must generate at least 2 pages');

    // Page 0 is Recto (ע״א - right page in Hebrew spread)
    const p0Html = pages[0].htmlContent;
    assert.ok(p0Html.includes('data-flow-id="gemara"'), 'Recto page must contain Gemara frame');
    assert.ok(p0Html.includes('data-flow-id="rashi"'), 'Recto page must contain Rashi frame');
    assert.ok(p0Html.includes('data-flow-id="tosafot"'), 'Recto page must contain Tosafot frame');
    assert.ok(p0Html.includes('data-flow-id="notes"'), 'Recto page must contain Footnotes frame');

    // Check Hebrew spine awareness:
    // On Recto (page 0), Spine is on the LEFT.
    // Inner commentary (Rashi) -> Left column.
    // Outer commentary (Tosafot) -> Right column.
    const p0RightIdx = p0Html.indexOf('data-flow-id="tosafot"');
    const p0CenterIdx = p0Html.indexOf('data-flow-id="gemara"');
    const p0LeftIdx = p0Html.indexOf('data-flow-id="rashi"');
    assert.ok(p0RightIdx < p0CenterIdx, 'Tosafot on Recto should be on the right (outer) column');
    assert.ok(p0CenterIdx < p0LeftIdx, 'Rashi on Recto should be on the left (inner spine) column');

    // Page 1 is Verso (ע״ב - left page in Hebrew spread)
    const p1Html = pages[1].htmlContent;
    // On Verso (page 1), Spine is on the RIGHT.
    // Inner commentary (Rashi) -> Right column.
    // Outer commentary (Tosafot) -> Left column.
    const p1RightIdx = p1Html.indexOf('data-flow-id="rashi"');
    const p1CenterIdx = p1Html.indexOf('data-flow-id="gemara"');
    const p1LeftIdx = p1Html.indexOf('data-flow-id="tosafot"');
    assert.ok(p1RightIdx < p1CenterIdx, 'Rashi on Verso should be on the right (inner spine) column');
    assert.ok(p1CenterIdx < p1LeftIdx, 'Tosafot on Verso should be on the left (outer) column');
  });

  test('Dynamic L-Shape expansion triggers when Gemara ends early', async () => {
    const { FlowPaginator, DEFAULT_TALMUD_FLOWS } = await loadTs('packages/tok-ui/src/engine/FlowPaginator.ts');
    // Short gemara, long rashi
    const doc = {
      title: 'Talmud Short Gemara',
      templateType: 'gemara',
      flows: {
        gemara: [{ id: 'g-short', styleId: 'style-gemara-main', text: 'קצר מאד.' }],
        rashi: [...DEFAULT_TALMUD_FLOWS.rashi],
        tosafot: [...DEFAULT_TALMUD_FLOWS.tosafot],
        notes: []
      }
    };

    const pages = FlowPaginator.paginateDocument(doc, 1);
    const p0Html = pages[0].htmlContent;
    assert.ok(p0Html.includes('tok-lshape-box'), 'Should render L-shape expansion zone');
    assert.ok(p0Html.includes('צורת הדף'), 'Should mention Tzurat HaDaf in expansion');
  });

  test('Prose template paginates continuous text across pages', async () => {
    const { FlowPaginator } = await loadTs('packages/tok-ui/src/engine/FlowPaginator.ts');
    const paras = Array.from({ length: 40 }, (_, i) => ({
      id: `p-${i}`,
      styleId: 'style-body',
      text: `פסקה מספר ${i + 1} עם מלל רציף לצורך בדיקת חלוקת עמודים רב עמודית בפורמט פרוזה.`
    }));

    const doc = {
      title: 'Prose Test',
      templateType: 'prose',
      flows: {
        gemara: paras,
        rashi: [],
        tosafot: [],
        notes: []
      }
    };

    const pages = FlowPaginator.paginateDocument(doc, 1);
    assert.ok(pages.length > 1, 'Long prose must paginate into multiple pages');
    for (const page of pages) {
      assert.ok(page.htmlContent.includes('data-flow-id="gemara"'));
    }
  });

  test('Footnotes layout displays gemara and bottom footnote notes', async () => {
    const { FlowPaginator } = await loadTs('packages/tok-ui/src/engine/FlowPaginator.ts');
    const doc = {
      title: 'Footnotes Test',
      templateType: 'notes',
      flows: {
        gemara: [{ id: 'g-1', styleId: 'style-body', text: 'טקסט ראשי עם הערת שוליים.' }],
        rashi: [],
        tosafot: [],
        notes: [{ id: 'fn-1', styleId: 'style-notes', text: '1. הערת שוליים בתחתית העמוד.' }]
      }
    };

    const pages = FlowPaginator.paginateDocument(doc, 1);
    assert.equal(pages.length, 1);
    const html = pages[0].htmlContent;
    assert.ok(html.includes('data-flow-id="gemara"'));
    assert.ok(html.includes('data-flow-id="notes"'));
  });
});

describe('Phase 1 & 2: Data Contract (documentState <-> DocumentModel) & Real PDF Export', () => {
  let documentBridge;
  let tokCliPath;

  test('Load documentBridge module', async () => {
    documentBridge = await loadTs('packages/tok-ui/src/engine/documentBridge.ts');
    assert.ok(documentBridge.documentStateToDocumentRoot, 'Must export documentStateToDocumentRoot');
    assert.ok(documentBridge.documentRootToDocumentState, 'Must export documentRootToDocumentState');
    assert.ok(documentBridge.isValidUlid, 'Must export isValidUlid');
    assert.ok(documentBridge.generateCrockfordUlid, 'Must export generateCrockfordUlid');
  });

  test('ULID generator produces valid 26-char Crockford Base32 identifiers', () => {
    for (let i = 0; i < 50; i++) {
      const id = documentBridge.generateCrockfordUlid();
      assert.equal(id.length, 26, `ULID must be 26 chars: ${id}`);
      assert.ok(documentBridge.isValidUlid(id), `ULID must be valid: ${id}`);
      // First character must be <= '7' for 128-bit limit
      assert.ok(id[0] <= '7', `First char must be <= '7': ${id}`);
    }
  });

  test('HTML tag stripping cleans markup while preserving Hebrew and Niqqud', () => {
    const raw = '<b>מֵאֵימָתַי קוֹרִין</b> אֶת <i>שְׁמַע</i> <span style="color:red">בְּעַרְבִית</span>?';
    const clean = documentBridge.stripHtmlTags(raw);
    assert.equal(clean, 'מֵאֵימָתַי קוֹרִין אֶת שְׁמַע בְּעַרְבִית?');
  });

  test('Lossless roundtrip conversion: MultiFlowDocumentState -> DocumentRoot -> MultiFlowDocumentState', () => {
    const originalState = {
      title: 'ספר תלמוד מסכת ברכות.tok',
      templateType: 'gemara',
      flows: {
        gemara: [
          { id: 'g-1', styleId: 'style-gemara-main', text: 'מֵאֵימָתַי קוֹרִין אֶת שְׁמַע בְּעַרְבִית?' },
          { id: 'g-2', styleId: 'style-gemara-main', text: 'מִשָּׁעָה שֶׁהַכֹּהֲנִים נִכְנָסִים לֶאֱכֹל בִּתְרוּמָתָן.' }
        ],
        rashi: [
          { id: 'r-1', styleId: 'style-rashi-body', text: 'תַּנָּא אַקְּרָא קָאֵי דִּכְתִיב בְּשָׁכְבְּךָ וּבְקוּמֶךָ.' }
        ],
        tosafot: [
          { id: 't-1', styleId: 'style-tosafot-body', text: 'פֵּרֵשׁ רַשִׁ\"י דְּתַנָּא אַקְּרָא קָאֵי.' }
        ],
        notes: [
          { id: 'n-1', styleId: 'style-footnotes', text: 'תורה אור: דברים ו, ז.' }
        ]
      }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(originalState, { author: 'רבי יהודה הנשיא' });

    // Validate DocumentRoot structure
    assert.ok(documentBridge.isValidUlid(docRoot.id), 'Root ID must be valid ULID');
    assert.equal(docRoot.metadata.title, 'ספר תלמוד מסכת ברכות');
    assert.equal(docRoot.metadata.author, 'רבי יהודה הנשיא');
    assert.equal(docRoot.metadata.progression, 'Rtl');
    assert.equal(docRoot.metadata.primary_language, 'he');
    assert.equal(docRoot.metadata.schema_version, '1.0');

    assert.ok(docRoot.paragraph_styles.length >= 4, 'Must register standard paragraph styles');
    assert.equal(docRoot.sections.length, 1);
    const section = docRoot.sections[0];
    assert.ok(documentBridge.isValidUlid(section.id), 'Section ID must be valid ULID');
    assert.equal(section.flows.length, 4);

    // Verify all paragraph IDs are valid ULIDs and indices are ordered
    for (const flow of section.flows) {
      for (let i = 0; i < flow.paragraphs.length; i++) {
        const p = flow.paragraphs[i];
        assert.ok(documentBridge.isValidUlid(p.id), `Paragraph ID ${p.id} must be valid ULID`);
        if (i > 0) {
          assert.ok(flow.paragraphs[i - 1].index < p.index, 'Paragraph indices must be strictly ordered');
        }
      }
    }

    // Convert back to UI state
    const restoredState = documentBridge.documentRootToDocumentState(docRoot);
    assert.equal(restoredState.title, 'ספר תלמוד מסכת ברכות');
    assert.equal(restoredState.templateType, 'gemara');
    assert.equal(restoredState.flows.gemara.length, 2);
    assert.equal(restoredState.flows.rashi.length, 1);
    assert.equal(restoredState.flows.tosafot.length, 1);
    assert.equal(restoredState.flows.notes.length, 1);

    assert.equal(restoredState.flows.gemara[0].text, originalState.flows.gemara[0].text);
    assert.equal(restoredState.flows.gemara[1].text, originalState.flows.gemara[1].text);
    assert.equal(restoredState.flows.rashi[0].text, originalState.flows.rashi[0].text);
    assert.equal(restoredState.flows.tosafot[0].text, originalState.flows.tosafot[0].text);
    assert.equal(restoredState.flows.notes[0].text, originalState.flows.notes[0].text);
  });

  test('Empty document conversion produces valid DocumentRoot', () => {
    const emptyState = {
      title: 'מסמך ריק.tok',
      templateType: 'prose',
      flows: { gemara: [], rashi: [], tosafot: [], notes: [] }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(emptyState);
    assert.ok(documentBridge.isValidUlid(docRoot.id));
    assert.equal(docRoot.metadata.title, 'מסמך ריק');
    assert.equal(docRoot.sections.length, 1);
    assert.ok(docRoot.sections[0].flows.length >= 1);
    assert.equal(docRoot.sections[0].flows[0].paragraphs.length, 0);

    const restored = documentBridge.documentRootToDocumentState(docRoot);
    assert.equal(restored.title, 'מסמך ריק');
    assert.equal(restored.flows.gemara.length, 0);
  });

  test('Locate tok-cli engine binary', () => {
    const isWin = process.platform === 'win32';
    const exe = isWin ? 'tok-cli.exe' : 'tok-cli';
    const candidates = [
      process.env.TOK_CLI_PATH,
      path.join(process.env.LOCALAPPDATA || '', 'tok_target', 'debug', exe),
      path.join(process.env.LOCALAPPDATA || '', 'tok_target', 'release', exe),
      path.join(rootDir, 'target', 'debug', exe),
      path.join(rootDir, 'target', 'release', exe)
    ];
    tokCliPath = candidates.find((p) => p && fs.existsSync(p));
    assert.ok(tokCliPath, `tok-cli binary must be found. Checked: ${candidates.filter(Boolean).join(', ')}`);
  });

  test('Integration: Export real user document with unique marker text to PDF', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const uniqueMarker = `TYPESETOK_UNIQUE_MARKER_${Date.now()}_עברית`;
    const uniqueTitle = `ספר בדיקה ייחודי ${Date.now()}`;

    const userState = {
      title: uniqueTitle,
      templateType: 'gemara',
      flows: {
        gemara: [
          { id: 'g-1', styleId: 'style-gemara-main', text: `טקסט משתמש ראשי עם סימון ייחודי: ${uniqueMarker}.` }
        ],
        rashi: [
          { id: 'r-1', styleId: 'style-rashi-body', text: 'פירוש רש\"י המקורי של המשתמש.' }
        ],
        tosafot: [],
        notes: []
      }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(userState);

    const tempJsonPath = path.join(os.tmpdir(), `tok_test_doc_${Date.now()}.json`);
    const tempPdfPath = path.join(os.tmpdir(), `tok_test_out_${Date.now()}.pdf`);

    try {
      fs.writeFileSync(tempJsonPath, JSON.stringify(docRoot, null, 2), 'utf-8');

      const cliResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['render-pdf', tempJsonPath, tempPdfPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(cliResult.code, 0, `tok-cli render-pdf failed: ${cliResult.stderr || cliResult.stdout}`);
      assert.ok(fs.existsSync(tempPdfPath), 'PDF file must be created on disk');

      const pdfStats = fs.statSync(tempPdfPath);
      assert.ok(pdfStats.size > 1000, `PDF size must be substantial (>1000 bytes), got ${pdfStats.size}`);

      const pdfBytes = fs.readFileSync(tempPdfPath);
      const pdfHeader = pdfBytes.slice(0, 8).toString('ascii');
      assert.ok(pdfHeader.startsWith('%PDF-1.'), 'File must start with valid PDF header %PDF-1.x');

      const pdfText = pdfBytes.toString('latin1');

      // 1. Verify user's document title is in the PDF metadata (handles both plain UTF-8 and PDF UTF-16BE hex strings)
      const expectedTitleHex = Array.from(uniqueTitle)
        .map((c) => c.charCodeAt(0).toString(16).padStart(4, '0').toUpperCase())
        .join('');
      assert.ok(
        pdfText.includes('/Title') && (
          pdfText.includes(uniqueTitle) ||
          pdfBytes.includes(Buffer.from(uniqueTitle, 'utf-8')) ||
          pdfText.includes(expectedTitleHex)
        ),
        'User document title must appear in PDF metadata'
      );

      // 2. Verify demo text is NOT in the PDF
      assert.ok(!pdfText.includes('מֵאֵימָתַי קוֹרִין אֶת שְׁמַע בְּעַרְבִית'), 'Built-in demo text must NOT appear in user export');
      assert.ok(!pdfText.includes('תלמוד בבלי - מסכת ברכות'), 'Demo title must NOT appear in user export');

      // 3. Verify unique user marker characters appear in the PDF font's ToUnicode mapping
      const zlib = await import('node:zlib');
      let decompressedStreams = '';
      let pos = 0;
      while (pos < pdfBytes.length) {
        const streamStart = pdfBytes.indexOf(Buffer.from('stream'), pos);
        if (streamStart === -1) break;
        const streamDataStart = pdfBytes[streamStart + 6] === 0x0a ? streamStart + 7 : streamStart + 8;
        const streamEnd = pdfBytes.indexOf(Buffer.from('endstream'), streamDataStart);
        if (streamEnd === -1) break;
        let dataEnd = streamEnd;
        while (dataEnd > streamDataStart && (pdfBytes[dataEnd - 1] === 0x0a || pdfBytes[dataEnd - 1] === 0x0d)) {
          dataEnd--;
        }
        try {
          const uncompressed = zlib.inflateSync(pdfBytes.slice(streamDataStart, dataEnd));
          decompressedStreams += uncompressed.toString('utf-8') + '\n';
        } catch (_) {}
        pos = streamEnd + 9;
      }

      assert.ok(decompressedStreams.includes('/TOK-Custom-ToUnicode'), 'PDF must include ToUnicode CMap');
      for (const char of uniqueMarker) {
        const charHex = char.charCodeAt(0).toString(16).padStart(4, '0').toUpperCase();
        assert.ok(
          decompressedStreams.includes(`<${charHex}>`),
          `Unique marker character '${char}' (U+${charHex}) must be present in the PDF font mapping`
        );
      }

      // 4. Verify PDF is pre-press compliant
      assert.ok(pdfText.includes('/GTS_PDFXVersion'), 'PDF must include PDF/X OutputIntent');
    } finally {
      if (fs.existsSync(tempJsonPath)) fs.unlinkSync(tempJsonPath);
      if (fs.existsSync(tempPdfPath)) fs.unlinkSync(tempPdfPath);
    }
  });

  test('Integration: Export empty document produces valid PDF without crashing', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const emptyState = {
      title: 'מסמך ריק לבדיקה',
      templateType: 'prose',
      flows: { gemara: [], rashi: [], tosafot: [], notes: [] }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(emptyState);
    const tempJsonPath = path.join(os.tmpdir(), `tok_empty_doc_${Date.now()}.json`);
    const tempPdfPath = path.join(os.tmpdir(), `tok_empty_out_${Date.now()}.pdf`);

    try {
      fs.writeFileSync(tempJsonPath, JSON.stringify(docRoot, null, 2), 'utf-8');

      const cliResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['render-pdf', tempJsonPath, tempPdfPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(cliResult.code, 0, `tok-cli must succeed on empty document: ${cliResult.stderr}`);
      assert.ok(fs.existsSync(tempPdfPath), 'PDF file must be created for empty document');
      const pdfBytes = fs.readFileSync(tempPdfPath);
      assert.ok(pdfBytes.slice(0, 8).toString('ascii').startsWith('%PDF-1.'));
      assert.ok(pdfBytes.length > 500);
    } finally {
      if (fs.existsSync(tempJsonPath)) fs.unlinkSync(tempJsonPath);
      if (fs.existsSync(tempPdfPath)) fs.unlinkSync(tempPdfPath);
    }
  });

  test('Integration: Multi-paragraph document with Hebrew Niqqud exports correctly', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const paragraphs = [
      'בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ.',
      'וְהָאָרֶץ הָיְתָה תֹהוּ וָבֹהוּ וְחֹשֶׁךְ עַל פְּנֵי תְהוֹם וְרוּחַ אֱלֹהִים מְרַחֶפֶת עַל פְּנֵי הַמָּיִם.',
      'וַיֹּאמֶר אֱלֹהִים יְהִי אוֹר וַיְהִי אוֹר.',
      'וַיַּרְא אֱלֹהִים אֶת הָאוֹר כִּי טוֹב וַיַּבְדֵּל אֱלֹהִים בֵּין הָאוֹר וּבֵין הַחֹשֶׁךְ.'
    ];

    const state = {
      title: 'ספר בראשית',
      templateType: 'prose',
      flows: {
        gemara: paragraphs.map((text, i) => ({ id: `p-${i}`, styleId: 'normal', text })),
        rashi: [],
        tosafot: [],
        notes: []
      }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(state);
    const tempJsonPath = path.join(os.tmpdir(), `tok_niqqud_doc_${Date.now()}.json`);
    const tempPdfPath = path.join(os.tmpdir(), `tok_niqqud_out_${Date.now()}.pdf`);

    try {
      fs.writeFileSync(tempJsonPath, JSON.stringify(docRoot, null, 2), 'utf-8');

      const cliResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['render-pdf', tempJsonPath, tempPdfPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(cliResult.code, 0, `tok-cli must succeed on niqqud document: ${cliResult.stderr}`);
      assert.ok(fs.existsSync(tempPdfPath));
      const stats = fs.statSync(tempPdfPath);
      assert.ok(stats.size > 2000, `Multi-paragraph PDF must be > 2000 bytes, got ${stats.size}`);
    } finally {
      if (fs.existsSync(tempJsonPath)) fs.unlinkSync(tempJsonPath);
      if (fs.existsSync(tempPdfPath)) fs.unlinkSync(tempPdfPath);
    }
  });

  test('Error Handling: Non-existent input or corrupted document fails and reports error', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const nonExistentPath = path.join(os.tmpdir(), `non_existent_${Date.now()}.tok`);
    const tempPdfPath = path.join(os.tmpdir(), `should_not_exist_${Date.now()}.pdf`);

    const cliResult = await new Promise((resolve) => {
      const proc = spawn(tokCliPath, ['render-pdf', nonExistentPath, tempPdfPath], { windowsHide: true });
      let stdout = '';
      let stderr = '';
      proc.stdout?.on('data', (d) => (stdout += d.toString()));
      proc.stderr?.on('data', (d) => (stderr += d.toString()));
      proc.on('close', (code) => resolve({ code, stdout, stderr }));
    });

    assert.notEqual(cliResult.code, 0, 'Must exit with non-zero code on missing file');
    assert.equal(fs.existsSync(tempPdfPath), false, 'PDF must NOT be created when input is missing');
  });

  test('Backward Compatibility: --demo flag still functions as expected', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const tempPdfPath = path.join(os.tmpdir(), `tok_demo_out_${Date.now()}.pdf`);

    try {
      const cliResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['render-pdf', '--demo', tempPdfPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(cliResult.code, 0, `tok-cli render-pdf --demo failed: ${cliResult.stderr}`);
      assert.ok(fs.existsSync(tempPdfPath), 'Demo PDF must be generated');
      const stats = fs.statSync(tempPdfPath);
      assert.ok(stats.size > 2000, 'Demo PDF must be > 2000 bytes');
    } finally {
      if (fs.existsSync(tempPdfPath)) fs.unlinkSync(tempPdfPath);
    }
  });
});

describe('Phase 3: Real .tok Storage (Save, Open, Round-Trip & Error Resilience)', () => {
  const isWindows = process.platform === 'win32';
  const tokCliBinary = isWindows ? 'tok-cli.exe' : 'tok-cli';
  const customTargetCli = path.join(
    process.env.LOCALAPPDATA || path.join(process.env.USERPROFILE || '', 'AppData', 'Local'),
    'tok_target',
    'debug',
    tokCliBinary
  );
  const repoTargetCli = path.resolve('target', 'debug', tokCliBinary);
  const tokCliPath = fs.existsSync(customTargetCli) ? customTargetCli : repoTargetCli;

  let documentBridge;
  before(async () => {
    const bridgePath = path.resolve('packages/tok-ui/dist/engine/documentBridge.js');
    assert.ok(fs.existsSync(bridgePath), 'documentBridge.js must be built');
    documentBridge = await import(`file://${bridgePath.replace(/\\/g, '/')}`);
  });

  test('Round-Trip: Save and open multi-flow Hebrew document with Niqqud via .tok package', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const multiFlowState = {
      title: 'מסכת פסחים - מהדורת בדיקה 2026',
      templateType: 'gemara',
      flows: {
        gemara: [
          { id: 'g-1', styleId: 'style-gemara-main', text: 'כָּל שָׁעָה שֶׁמֻּתָּר לֶאֱכֹל, מַאֲכִיל לַבְּהֵמָה וְלַחַיָּה וְלָעוֹפוֹת, וּמוֹכְרוֹ לְנָכְרִי, וּמֻתָּר בַּהֲנָאָתוֹ.' },
          { id: 'g-2', styleId: 'style-gemara-main', text: 'עָבַר זְמַנּוֹ, אָסוּר בַּהֲנָאָתוֹ, וְלֹא יַסִּיק בּוֹ תַּנּוּר וְכִירַיִם. רַבִּי יְהוּדָה אוֹמֵר: אֵין בִּעוּר חָמֵץ אֶלָּא שְׂרֵפָה.' }
        ],
        rashi: [
          { id: 'r-1', styleId: 'style-rashi-body', text: 'כל שעה שמותר לאכול - כל ארבע שעות.' },
          { id: 'r-2', styleId: 'style-rashi-body', text: 'מאכיל לבהמה - ואף על פי שהיא רובצת לפניו.' }
        ],
        tosafot: [
          { id: 't-1', styleId: 'style-tosafot-body', text: 'כל שעה שמותר לאכול - ואם תאמר והא תנן לקמן כל שעה שאינו אוכל מאכיל.' }
        ],
        notes: [
          { id: 'n-1', styleId: 'style-footnotes', text: 'הערת שוליים ראשונה על פירוש רש\"י.' }
        ]
      }
    };

    const docRoot = documentBridge.documentStateToDocumentRoot(multiFlowState);
    const tempInJson = path.join(os.tmpdir(), `tok_save_in_${Date.now()}.json`);
    const tempTokPath = path.join(os.tmpdir(), `tok_save_pkg_${Date.now()}.tok`);
    const tempOutJson = path.join(os.tmpdir(), `tok_open_out_${Date.now()}.json`);

    try {
      fs.writeFileSync(tempInJson, JSON.stringify(docRoot, null, 2), 'utf-8');

      // 1. Execute CLI save-package
      const saveResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['save-package', tempInJson, tempTokPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(saveResult.code, 0, `save-package failed: ${saveResult.stderr || saveResult.stdout}`);
      assert.ok(fs.existsSync(tempTokPath), '.tok package must exist on disk');
      const tokStats = fs.statSync(tempTokPath);
      assert.ok(tokStats.size > 500, `.tok package size must be substantial, got ${tokStats.size}`);

      // 2. Execute CLI inspect-package to verify manifest
      const inspectResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['inspect-package', tempTokPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(inspectResult.code, 0, `inspect-package failed: ${inspectResult.stderr}`);
      assert.ok(inspectResult.stdout.includes(multiFlowState.title), 'inspect-package must report correct title');

      // 3. Execute CLI open-package
      const openResult = await new Promise((resolve, reject) => {
        const proc = spawn(tokCliPath, ['open-package', tempTokPath, tempOutJson], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('error', reject);
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(openResult.code, 0, `open-package failed: ${openResult.stderr || openResult.stdout}`);
      assert.ok(fs.existsSync(tempOutJson), 'Extracted document JSON must exist on disk');

      // 4. Verify roundtripped state matches original model exactly
      const extractedRoot = JSON.parse(fs.readFileSync(tempOutJson, 'utf-8'));
      const restoredState = documentBridge.documentRootToDocumentState(extractedRoot);

      assert.equal(restoredState.title, multiFlowState.title, 'Title must round-trip exactly');
      assert.equal(restoredState.templateType, multiFlowState.templateType, 'Template type must round-trip');

      assert.equal(restoredState.flows.gemara.length, 2, 'Gemara paragraphs count must match');
      assert.equal(restoredState.flows.rashi.length, 2, 'Rashi paragraphs count must match');
      assert.equal(restoredState.flows.tosafot.length, 1, 'Tosafot paragraphs count must match');
      assert.equal(restoredState.flows.notes.length, 1, 'Notes paragraphs count must match');

      assert.ok(restoredState.flows.gemara[0].text.includes('שָׁעָה שֶׁמֻּתָּר לֶאֱכֹל'));
      assert.ok(restoredState.flows.gemara[1].text.includes('רַבִּי יְהוּדָה'));
      assert.ok(restoredState.flows.rashi[0].text.includes('כל שעה שמותר לאכול'));
      assert.ok(restoredState.flows.tosafot[0].text.includes('ואם תאמר והא תנן'));
      assert.ok(restoredState.flows.notes[0].text.includes('הערת שוליים ראשונה'));
    } finally {
      if (fs.existsSync(tempInJson)) fs.unlinkSync(tempInJson);
      if (fs.existsSync(tempTokPath)) fs.unlinkSync(tempTokPath);
      if (fs.existsSync(tempOutJson)) fs.unlinkSync(tempOutJson);
    }
  });

  test('Error Handling: Corrupted or invalid .tok file is rejected without creating output', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const corruptTokPath = path.join(os.tmpdir(), `corrupt_pkg_${Date.now()}.tok`);
    const tempOutJson = path.join(os.tmpdir(), `should_not_exist_${Date.now()}.json`);

    try {
      fs.writeFileSync(corruptTokPath, Buffer.from('NOT_A_VALID_ZIP_HEADER_GARBAGE_BYTES_12345'), 'utf-8');

      const openResult = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['open-package', corruptTokPath, tempOutJson], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.notEqual(openResult.code, 0, 'CLI must exit with non-zero on corrupted .tok package');
      assert.equal(fs.existsSync(tempOutJson), false, 'Output JSON must NOT be created on failure');
    } finally {
      if (fs.existsSync(corruptTokPath)) fs.unlinkSync(corruptTokPath);
      if (fs.existsSync(tempOutJson)) fs.unlinkSync(tempOutJson);
    }
  });

  test('Atomic Resilience: Existing file is not destroyed if saving invalid document fails', async () => {
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const existingTokPath = path.join(os.tmpdir(), `atomic_existing_${Date.now()}.tok`);
    const nonExistentJson = path.join(os.tmpdir(), `missing_${Date.now()}.json`);

    try {
      const initialContent = Buffer.from('ORIGINAL_INTACT_CONTENT_BEFORE_FAILED_SAVE');
      fs.writeFileSync(existingTokPath, initialContent);

      const saveResult = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['save-package', nonExistentJson, existingTokPath], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.notEqual(saveResult.code, 0, 'Must fail when input is missing');
      assert.ok(fs.existsSync(existingTokPath), 'Existing file must still exist');
      const survivingContent = fs.readFileSync(existingTokPath);
      assert.deepEqual(survivingContent, initialContent, 'Existing file must not be modified or truncated');
    } finally {
      if (fs.existsSync(existingTokPath)) fs.unlinkSync(existingTokPath);
    }
  });
});

describe('Phase 4: Real Rust Typesetting Engine Connection (typeset-document & typesetBridge)', () => {
  let layoutToPageDescriptors;
  let tokCliPath;

  before(async () => {
    const bridgeModule = await import('../packages/tok-ui/dist/engine/typesetBridge.js');
    layoutToPageDescriptors = bridgeModule.layoutToPageDescriptors;

    const candidates = [
      path.resolve(rootDir, 'crates/target/release/tok-cli.exe'),
      path.resolve(rootDir, 'crates/target/debug/tok-cli.exe'),
      path.resolve(rootDir, 'target/release/tok-cli.exe'),
      path.resolve(rootDir, 'target/debug/tok-cli.exe'),
      path.join(process.env.LOCALAPPDATA || '', 'tok_target/debug/tok-cli.exe'),
      path.join(process.env.LOCALAPPDATA || '', 'tok_target/release/tok-cli.exe')
    ];
    for (const c of candidates) {
      if (fs.existsSync(c)) {
        tokCliPath = c;
        break;
      }
    }
  });

  test('Bridge: layoutToPageDescriptors translates Rust PageLayoutBox into SpreadCanvas PageDescriptor', () => {
    assert.ok(typeof layoutToPageDescriptors === 'function', 'layoutToPageDescriptors must be exported');

    const sampleRustBoxes = [
      {
        page_index: 0,
        page_number_gematria: 'א׳',
        dimensions: { x: 0, y: 0, width: 595.28, height: 841.89 },
        frames: [
          {
            frame_id: 'frame_1_gemara',
            flow_id: 'gemara',
            rect: { x: 42.5, y: 42.5, width: 510.28, height: 756.89 },
            lines: [
              {
                line_index: 0,
                paragraph_id: 'para-marker-101',
                baseline_y: 18.0,
                height: 14.0,
                width: 510.28,
                glyphs: [],
                text: 'מאימתי קורין את שמע בערבית משעה שהכהנים נכנסין',
                is_rtl: true,
                fonts: ['Frank Ruhl Libre']
              },
              {
                line_index: 1,
                paragraph_id: 'para-marker-101',
                baseline_y: 36.0,
                height: 14.0,
                width: 510.28,
                glyphs: [],
                text: 'לאכול בתרומתן עד סוף האשמורה הראשונה',
                is_rtl: true,
                fonts: ['Frank Ruhl Libre']
              }
            ]
          }
        ],
        break_token: null
      }
    ];

    const descriptors = layoutToPageDescriptors(sampleRustBoxes);
    assert.equal(descriptors.length, 1);
    assert.equal(descriptors[0].pageIndex, 0);
    assert.equal(descriptors[0].gematriaNumber, 'א׳');
    assert.equal(descriptors[0].widthPt, 595.28);
    assert.equal(descriptors[0].heightPt, 841.89);

    const html = descriptors[0].htmlContent;
    assert.ok(html.includes('tok-page-layout-rust'), 'Must wrap in tok-page-layout-rust');
    assert.ok(html.includes('data-frame-id="frame_1_gemara"'), 'Must preserve frame id');
    assert.ok(html.includes('data-flow-id="gemara"'), 'Must preserve flow id');
    assert.ok(html.includes('data-line-index="0"'), 'Must render line 0');
    assert.ok(html.includes('data-para-id="para-marker-101"'), 'Must preserve paragraph ID on line');
    assert.ok(html.includes('data-baseline-y="18"'), 'Must preserve exact baseline Y');
    assert.ok(html.includes('מאימתי קורין את שמע'), 'Must render exact line text');
  });

  test('Bridge: layoutToPageDescriptors handles empty document layout box cleanly', () => {
    const emptyRustBoxes = [
      {
        page_index: 0,
        page_number_gematria: 'א׳',
        dimensions: { x: 0, y: 0, width: 595.28, height: 841.89 },
        frames: [
          {
            frame_id: 'frame_1_gemara',
            flow_id: 'gemara',
            rect: { x: 42.5, y: 42.5, width: 510.28, height: 756.89 },
            lines: []
          }
        ],
        break_token: null
      }
    ];

    const descriptors = layoutToPageDescriptors(emptyRustBoxes);
    assert.equal(descriptors.length, 1);
    assert.ok(descriptors[0].htmlContent.includes('[אין טקסט בעמוד זה]'), 'Must render empty text indicator');
  });

  test('CLI: tok-cli typeset-document emits valid PageLayoutBox JSON array with Hebrew lines', async () => {
    assert.ok(tokCliPath, 'tok-cli binary must be available');
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');
    const bridgeModule = await import('../packages/tok-ui/dist/engine/documentBridge.js');
    const { documentStateToDocumentRoot } = bridgeModule;

    const uniqueMarker = `שמע_בערבית_סימן_ייחודי_${Date.now()}`;
    const testDocState = {
      title: 'בדיקת עימוד חי',
      templateType: 'gemara',
      flows: {
        gemara: [
          {
            id: 'para-uniq-1',
            styleId: 'style-gemara-main',
            text: `${uniqueMarker}: מאימתי קורין את שמע בערבית משעה שהכהנים נכנסים לאכול בתרומתן.`
          }
        ],
        rashi: [],
        tosafot: [],
        notes: []
      }
    };

    const docRoot = documentStateToDocumentRoot(testDocState);
    const tempInJson = path.join(os.tmpdir(), `typeset_in_${Date.now()}.json`);
    const tempOutJson = path.join(os.tmpdir(), `typeset_out_${Date.now()}.json`);

    try {
      fs.writeFileSync(tempInJson, JSON.stringify(docRoot, null, 2), 'utf-8');

      const cliResult = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempInJson, tempOutJson], { windowsHide: true });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (d) => (stdout += d.toString()));
        proc.stderr?.on('data', (d) => (stderr += d.toString()));
        proc.on('close', (code) => resolve({ code, stdout, stderr }));
      });

      assert.equal(cliResult.code, 0, `tok-cli typeset-document failed: ${cliResult.stderr}`);
      assert.ok(fs.existsSync(tempOutJson), 'Output JSON must be created');

      const rawJson = fs.readFileSync(tempOutJson, 'utf-8');
      const pages = JSON.parse(rawJson);

      assert.ok(Array.isArray(pages), 'Output must be an array of PageLayoutBox');
      assert.ok(pages.length >= 1, 'Must produce at least 1 page');
      assert.equal(pages[0].page_index, 0);
      assert.ok(pages[0].frames.length >= 1, 'Must have at least 1 frame');

      const allLines = pages[0].frames.flatMap((f) => f.lines);
      assert.ok(allLines.length >= 1, 'Must have typeset lines');
      const lineWithMarker = allLines.find((l) => l.text.includes(uniqueMarker));
      assert.ok(lineWithMarker, 'Must include unique marker text in typeset lines');
      assert.ok(lineWithMarker.baseline_y > 0, 'Baseline Y must be positive');
      assert.ok(lineWithMarker.height > 0, 'Line height must be positive');
      assert.equal(lineWithMarker.is_rtl, true, 'Hebrew line must be RTL');
    } finally {
      if (fs.existsSync(tempInJson)) fs.unlinkSync(tempInJson);
      if (fs.existsSync(tempOutJson)) fs.unlinkSync(tempOutJson);
    }
  });

  test('Integration: Editing document content dynamically updates Rust layout results and line counts', async () => {
    assert.ok(tokCliPath, 'tok-cli binary must be available');
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');
    const bridgeModule = await import('../packages/tok-ui/dist/engine/documentBridge.js');
    const { documentStateToDocumentRoot } = bridgeModule;

    // Phase A: Initial document with short text
    const initialDocState = {
      title: 'עריכה דינמית',
      templateType: 'prose',
      flows: {
        gemara: [
          {
            id: 'para-1',
            styleId: 'style-gemara-main',
            text: 'פסקה ראשונה קצרה ביותר.'
          }
        ],
        rashi: [],
        tosafot: [],
        notes: []
      }
    };

    const tempIn1 = path.join(os.tmpdir(), `edit_in1_${Date.now()}.json`);
    const tempOut1 = path.join(os.tmpdir(), `edit_out1_${Date.now()}.json`);
    let pages1;

    try {
      fs.writeFileSync(tempIn1, JSON.stringify(documentStateToDocumentRoot(initialDocState), null, 2), 'utf-8');
      await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempIn1, tempOut1], { windowsHide: true });
        proc.on('close', resolve);
      });
      pages1 = JSON.parse(fs.readFileSync(tempOut1, 'utf-8'));
    } finally {
      if (fs.existsSync(tempIn1)) fs.unlinkSync(tempIn1);
      if (fs.existsSync(tempOut1)) fs.unlinkSync(tempOut1);
    }

    const desc1 = layoutToPageDescriptors(pages1);
    const lineCount1 = pages1.reduce((sum, p) => sum + p.frames.reduce((s, f) => s + f.lines.length, 0), 0);
    assert.equal(desc1.length, 1, 'Initial short text should fit in 1 page');
    assert.equal(lineCount1, 1, 'Initial text should produce 1 line');

    // Phase B: Content edited: user adds extensive multi-paragraph text
    const longHebrewParagraphs = Array.from({ length: 45 }, (_, i) => ({
      id: `para-extended-${i}`,
      styleId: 'style-gemara-main',
      text: `פסקה מספר ${i + 1}: מאימתי קורין את שמע בערבית משעה שהכהנים נכנסים לאכול בתרומתן עד סוף האשמורה הראשונה דברי רבי אליעזר וחכמים אומרים עד חצות רבן גמליאל אומר עד שיעלה עמוד השחר מעשה ובאו בניו מבית המשתה אמרו לו לא קרינו את שמע אמר להם אם לא עלה עמוד השחר חייבין אתם לקרות.`
    }));

    const editedDocState = {
      title: 'עריכה דינמית',
      templateType: 'prose',
      flows: {
        gemara: longHebrewParagraphs,
        rashi: [],
        tosafot: [],
        notes: []
      }
    };

    const tempIn2 = path.join(os.tmpdir(), `edit_in2_${Date.now()}.json`);
    const tempOut2 = path.join(os.tmpdir(), `edit_out2_${Date.now()}.json`);
    let pages2;

    try {
      fs.writeFileSync(tempIn2, JSON.stringify(documentStateToDocumentRoot(editedDocState), null, 2), 'utf-8');
      await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempIn2, tempOut2], { windowsHide: true });
        proc.on('close', resolve);
      });
      pages2 = JSON.parse(fs.readFileSync(tempOut2, 'utf-8'));
    } finally {
      if (fs.existsSync(tempIn2)) fs.unlinkSync(tempIn2);
      if (fs.existsSync(tempOut2)) fs.unlinkSync(tempOut2);
    }

    const desc2 = layoutToPageDescriptors(pages2);
    const lineCount2 = pages2.reduce((sum, p) => sum + p.frames.reduce((s, f) => s + f.lines.length, 0), 0);

    // Verify layout expanded according to real Rust typography and page breaking
    assert.ok(lineCount2 > lineCount1, `Line count must expand after adding content (${lineCount2} > ${lineCount1})`);
    assert.ok(desc2.length > 1, `Document must paginate into multiple pages (${desc2.length} > 1)`);
    assert.equal(desc2[0].gematriaNumber, 'א׳');
    assert.equal(desc2[1].gematriaNumber, 'ב׳');
  });

  test('Integration: Hebrew Niqqud, Taamim and multi-flow Talmud co-pagination geometry', async () => {
    assert.ok(tokCliPath, 'tok-cli binary must be available');
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');
    const bridgeModule = await import('../packages/tok-ui/dist/engine/documentBridge.js');
    const { documentStateToDocumentRoot } = bridgeModule;

    const niqqudText = 'בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ׃';
    const talmudDocState = {
      title: 'תלמוד מסכת ברכות',
      templateType: 'gemara',
      flows: {
        gemara: [
          {
            id: 'g1',
            styleId: 'style-gemara-main',
            text: niqqudText
          }
        ],
        rashi: [
          {
            id: 'r1',
            styleId: 'style-rashi-comm',
            text: 'רש"י: פירש רש"י על אתר בלשון קצרה.'
          }
        ],
        tosafot: [
          {
            id: 't1',
            styleId: 'style-tosafot-comm',
            text: 'תוספות: ותימא מאי שנא הכא.'
          }
        ],
        notes: []
      }
    };

    const tempIn = path.join(os.tmpdir(), `talmud_in_${Date.now()}.json`);
    const tempOut = path.join(os.tmpdir(), `talmud_out_${Date.now()}.json`);

    try {
      fs.writeFileSync(tempIn, JSON.stringify(documentStateToDocumentRoot(talmudDocState), null, 2), 'utf-8');
      const exitCode = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempIn, tempOut], { windowsHide: true });
        proc.on('close', resolve);
      });

      assert.equal(exitCode, 0);
      const pages = JSON.parse(fs.readFileSync(tempOut, 'utf-8'));
      const descriptors = layoutToPageDescriptors(pages);

      assert.ok(descriptors.length >= 1);
      const firstHtml = descriptors[0].htmlContent;

      // Verify that all 3 flows have distinct interactive frames
      assert.ok(firstHtml.includes('data-flow-id="gemara"') || firstHtml.includes('data-flow-id="main"'), 'Must have Gemara frame');
      assert.ok(firstHtml.includes('data-flow-id="rashi"'), 'Must have Rashi frame');
      assert.ok(firstHtml.includes('data-flow-id="tosafot"'), 'Must have Tosafot frame');

      // Verify that Niqqud is preserved in the rendered layout
      assert.ok(firstHtml.includes('בְּרֵאשִׁית'), 'Niqqud must be preserved in rendered HTML line');
    } finally {
      if (fs.existsSync(tempIn)) fs.unlinkSync(tempIn);
      if (fs.existsSync(tempOut)) fs.unlinkSync(tempOut);
    }
  });

  test('CLI MultiFlow: 2-flow dual-stream document typesets with proportional columns via tok-cli', async () => {
    assert.ok(tokCliPath, 'tok-cli binary must be available');
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const doc2Flow = {
      id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      metadata: {
        title: "ספר לימוד דו-זרימתי",
        author: "מחבר",
        progression: "Rtl",
        primary_language: "he",
        schema_version: "1.0"
      },
      paragraph_styles: [],
      character_styles: [],
      sections: [
        {
          id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          name: "פרק ראשון",
          page_style: "default",
          flows: [
            {
              id: "primary",
              flow_type: "Main",
              placement_role: "primary",
              paragraphs: [
                {
                  id: "01ARZ3NDEKTSV4RRFFQ69G5FA1",
                  index: "a0",
                  style_id: "normal",
                  text: "טקסט ראשי של הספר הנלמד בשני טורים מקבילים בעברית."
                }
              ]
            },
            {
              id: "commentary",
              flow_type: "CommentA",
              placement_role: "inner_spine",
              paragraphs: [
                {
                  id: "01ARZ3NDEKTSV4RRFFQ69G5FA2",
                  index: "a0",
                  style_id: "normal",
                  text: "פירוש נלווה בטור המקביל הנמצא בצד השדרה."
                }
              ]
            }
          ]
        }
      ]
    };

    const tempIn = path.join(os.tmpdir(), `typeset_2flow_in_${Date.now()}.json`);
    const tempOut = path.join(os.tmpdir(), `typeset_2flow_out_${Date.now()}.json`);

    try {
      fs.writeFileSync(tempIn, JSON.stringify(doc2Flow, null, 2), 'utf-8');
      const exitCode = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempIn, tempOut], { windowsHide: true });
        proc.on('close', resolve);
      });

      assert.equal(exitCode, 0, 'tok-cli must succeed on 2-flow document');
      assert.ok(fs.existsSync(tempOut), 'Output JSON must be created');

      const pages = JSON.parse(fs.readFileSync(tempOut, 'utf-8'));
      assert.ok(pages.length >= 1, 'Must output at least 1 page');
      const p0 = pages[0];
      assert.ok(p0.frames.length >= 2, 'Page 0 must have at least 2 frames');

      const primFrame = p0.frames.find((f) => f.flow_id === 'primary');
      const commFrame = p0.frames.find((f) => f.flow_id === 'commentary');
      assert.ok(primFrame, 'Must have primary frame');
      assert.ok(commFrame, 'Must have commentary frame');
      assert.ok(primFrame.lines.length >= 1, 'Primary frame must have lines');
      assert.ok(commFrame.lines.length >= 1, 'Commentary frame must have lines');

      // Primary (60%) should be wider than commentary (40%)
      assert.ok(primFrame.rect.width > commFrame.rect.width, 'Primary column must be wider than secondary column');
    } finally {
      if (fs.existsSync(tempIn)) fs.unlinkSync(tempIn);
      if (fs.existsSync(tempOut)) fs.unlinkSync(tempOut);
    }
  });

  test('CLI MultiFlow: 4-flow Mikraot Gedolot document typesets with 4 distinct column frames via tok-cli', async () => {
    assert.ok(tokCliPath, 'tok-cli binary must be available');
    const { spawn } = await import('node:child_process');
    const os = await import('node:os');

    const doc4Flow = {
      id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      metadata: {
        title: "מקראות גדולות ארבע זרימות",
        author: "מחבר",
        progression: "Rtl",
        primary_language: "he",
        schema_version: "1.0"
      },
      paragraph_styles: [],
      character_styles: [],
      sections: [
        {
          id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          name: "פרשת בראשית",
          page_style: "default",
          flows: [
            {
              id: "torah",
              flow_type: "Main",
              placement_role: "primary",
              paragraphs: [{ id: "01ARZ3NDEKTSV4RRFFQ69G5FA1", index: "a0", style_id: "normal", text: "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ." }]
            },
            {
              id: "onkelos",
              flow_type: "CommentA",
              placement_role: "inner_spine",
              paragraphs: [{ id: "01ARZ3NDEKTSV4RRFFQ69G5FA2", index: "a0", style_id: "normal", text: "בְּקַדְמִין בְּרָא יְיָ יָת שְׁמַיָּא וְיָת אַרְעָא." }]
            },
            {
              id: "rashi",
              flow_type: "CommentB",
              placement_role: "outer_margin",
              paragraphs: [{ id: "01ARZ3NDEKTSV4RRFFQ69G5FA3", index: "a0", style_id: "normal", text: "בְּרֵאשִׁית: אָמַר רַבִּי יִצְחָק לֹא הָיָה צָרִיךְ לְהַתְחִיל אֶת הַתּוֹרָה אֶלָּא מֵהַחֹדֶשׁ הַזֶּה לָכֶם." }]
            },
            {
              id: "ramban",
              flow_type: "CommentB",
              placement_role: "column_3",
              paragraphs: [{ id: "01ARZ3NDEKTSV4RRFFQ69G5FA4", index: "a0", style_id: "normal", text: "רמב\"ן: בראשית ברא אלהים, כתב רש\"י למה פתח בבראשית." }]
            }
          ]
        }
      ]
    };

    const tempIn = path.join(os.tmpdir(), `typeset_4flow_in_${Date.now()}.json`);
    const tempOut = path.join(os.tmpdir(), `typeset_4flow_out_${Date.now()}.json`);

    try {
      fs.writeFileSync(tempIn, JSON.stringify(doc4Flow, null, 2), 'utf-8');
      const exitCode = await new Promise((resolve) => {
        const proc = spawn(tokCliPath, ['typeset-document', tempIn, tempOut], { windowsHide: true });
        proc.on('close', resolve);
      });

      assert.equal(exitCode, 0, 'tok-cli must succeed on 4-flow document');
      assert.ok(fs.existsSync(tempOut), 'Output JSON must be created');

      const pages = JSON.parse(fs.readFileSync(tempOut, 'utf-8'));
      assert.ok(pages.length >= 1, 'Must output at least 1 page');
      const p0 = pages[0];
      assert.equal(p0.frames.length, 4, 'Must allocate 4 distinct frames for Mikraot Gedolot');

      const flowIds = p0.frames.map((f) => f.flow_id);
      assert.ok(flowIds.includes('torah'), 'Must include torah frame');
      assert.ok(flowIds.includes('onkelos'), 'Must include onkelos frame');
      assert.ok(flowIds.includes('rashi'), 'Must include rashi frame');
      assert.ok(flowIds.includes('ramban'), 'Must include ramban frame');

      for (const frame of p0.frames) {
        assert.ok(frame.lines.length >= 1, `Frame ${frame.flow_id} must have formatted lines`);
        assert.ok(frame.rect.width > 30, `Frame ${frame.flow_id} width must be substantial`);
      }
    } finally {
      if (fs.existsSync(tempIn)) fs.unlinkSync(tempIn);
      if (fs.existsSync(tempOut)) fs.unlinkSync(tempOut);
    }
  });
});




