import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SRC = path.join(ROOT, 'src', 'lib', 'i18n.ts');
const OUT = path.join(ROOT, 'src', 'lib', 'i18n-auto.ts');
const TL = 'iu';

function readEntries() {
  const text = fs.readFileSync(SRC, 'utf8');
  const start = text.indexOf('const EN: Table = {');
  const end = text.indexOf('\n};', start);
  const block = text.slice(start, end);
  const re = /(?:^|\n)\s*'((?:[^'\\]|\\.)*)'\s*:\s*'((?:[^'\\]|\\.)*)',?/g;
  const out = [];
  let m;
  while ((m = re.exec(block))) {
    const un = (s) => s.replace(/\\'/g, "'").replace(/\\\\/g, '\\').replace(/\\n/g, '\n');
    out.push([un(m[1]), un(m[2])]);
  }
  return out;
}

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);

function protect(s) {
  const vars = [];
  const text = s.replace(/\{[a-zA-Z0-9_]+\}/g, (v) => {
    const token = PUA(vars.length);
    vars.push([token, v]);
    return token;
  });
  return { text, vars };
}

const restore = (s, vars) => vars.reduce((acc, [t, v]) => acc.split(t).join(v), s);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function batch(list) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    TL +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let a = 0; a < 6; a++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2000 * (a + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      return null;
    } catch {
      await sleep(1000 * (a + 1));
    }
  }
  return null;
}

const entries = readEntries();
const CACHE = path.join(ROOT, 'src', 'lib', 'iu-cache.json');
const table = fs.existsSync(CACHE) ? JSON.parse(fs.readFileSync(CACHE, 'utf8')) : {};
const BATCH = 25;
for (let i = 0; i < entries.length; i += BATCH) {
  const chunk = entries.slice(i, i + BATCH).filter(([key]) => !table[key]);
  if (chunk.length === 0) continue;
  const prot = chunk.map(([, v]) => protect(v));
  const res = await batch(prot.map((p) => p.text));
  if (!res) {
    console.log('batch failed', i);
    await sleep(1500);
    continue;
  }
  chunk.forEach(([key], n) => {
    const val = restore(String(res[n] ?? ''), prot[n].vars);
    if (val.trim()) table[key] = val;
  });
  console.log(`${Math.min(100, Math.round(((i + chunk.length) / entries.length) * 100))}%`);
  fs.writeFileSync(CACHE, JSON.stringify(table, null, 1), 'utf8');
  await sleep(300);
}

const esc = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n');
let ts = fs.readFileSync(OUT, 'utf8');
if (/\n  iu: \{/.test(ts)) {
  ts = ts.replace(/\n  iu: \{[\s\S]*?\n  \},/, '\n  iu: {');
  ts = ts.replace(/\n  iu: \{/, '\n  iu: {');
}
const block = ['  iu: {'];
for (const [k, v] of Object.entries(table)) block.push(`    '${esc(k)}': '${esc(v)}',`);
block.push('  },');
const close = ts.lastIndexOf('};');
ts = ts.slice(0, close) + block.join('\n') + '\n' + ts.slice(close);
fs.writeFileSync(OUT, ts, 'utf8');
console.log(`iu: ${Object.keys(table).length}/${entries.length}`);
